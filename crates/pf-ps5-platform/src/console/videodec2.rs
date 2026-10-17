//! The Videodec2 lane: H.264 and HEVC decoded by the console into direct memory, copied on the
//! GPU into device-local planes the presenter samples with the two-plane CSC.
//!
//! Plan §3.3 step 2. The decoder's frame slot is imported as a `VkBuffer` through
//! `VK_EXT_external_memory_host` (no repository has imported it as an image), and one buffer to
//! image copy per plane lands the picture. Where the import is refused the slot is copied into a
//! staging buffer first (step 3). The probe's `vd2-import` line says which step a console takes.

use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use ash::vk;
use pf_client_video::video_color::ColorDesc;
use pf_ps5_app::core::media::{FeedFlags, MediaClock, VideoSink, VideoSinkCaps};
use pf_ps5_app::platform::StreamSpec;
use pf_ps5_sys::kernel::*;
use pf_ps5_sys::videodec2::*;
use pf_ps5_vk::{GpuDevice, Picture, Planes, VideoSlot};
use punktfunk_core::quic;

/// Frame slots the decoder writes into; a slot is free again once its copy has finished.
const FRAME_SLOTS: usize = 4;
/// Device-local plane sets the presenter samples; one is reused once nothing holds it. The
/// presenter holds one per frame in flight plus one waiting in the slot, and the lane writes one.
const PLANE_SETS: usize = pf_ps5_vk::ui::FRAMES + 2;

/// One direct-memory mapping, released on drop.
struct Direct {
    addr: *mut c_void,
    len: usize,
    phys: i64,
}

// SAFETY: the mapping is process-wide memory this struct owns; access is serialized by the
// decoder's mutex.
unsafe impl Send for Direct {}

impl Direct {
    fn new(len: usize, prot: i32) -> Result<Self> {
        let len = page_align(len.max(1));
        let mut phys = 0i64;
        let mut addr = std::ptr::null_mut();
        // SAFETY: out-parameters are locals; the search range is the whole direct pool.
        unsafe {
            let limit = sceKernelGetDirectMemorySize();
            let rc = sceKernelAllocateDirectMemory(0, limit, len, PAGE, MEMORY_TYPE, &mut phys);
            if rc != 0 {
                bail!("sceKernelAllocateDirectMemory({len}): {}", pf_ps5_sys::sce_error(rc));
            }
            let rc = sceKernelMapDirectMemory(&mut addr, len, prot, 0, phys, PAGE);
            if rc != 0 {
                sceKernelReleaseDirectMemory(phys, len);
                bail!("sceKernelMapDirectMemory({len}): {}", pf_ps5_sys::sce_error(rc));
            }
        }
        Ok(Self { addr, len, phys })
    }
}

impl Drop for Direct {
    fn drop(&mut self) {
        // SAFETY: unmapping and releasing what `new` mapped; nothing uses it any more (the decoder
        // and the GPU copies that read it finished before their owners dropped this).
        unsafe {
            sceKernelMunmap(self.addr, self.len);
            sceKernelReleaseDirectMemory(self.phys, self.len);
        }
    }
}

/// The flexible workspace Videodec2 asks for on the CPU side.
struct Flexible {
    addr: *mut c_void,
    len: usize,
}

// SAFETY: as `Direct`.
unsafe impl Send for Flexible {}

impl Drop for Flexible {
    fn drop(&mut self) {
        // SAFETY: releasing what was mapped for the decoder, after the decoder is deleted.
        unsafe {
            sceKernelReleaseFlexibleMemory(self.addr, self.len);
            sceKernelMunmap(self.addr, self.len);
        }
    }
}

/// Loads the module and asks for the compute memory once, before anything that could take the
/// process out of its sandbox (the 13.60 trap). Call first thing at start-up.
pub fn warm_up() {
    // SAFETY: plain calls; `mem` is a local out-parameter.
    unsafe {
        let rc = pf_ps5_sys::sysmodule::sceSysmoduleLoadModule(pf_ps5_sys::sysmodule::VIDEODEC2);
        if rc != 0 {
            tracing::warn!("videodec2: load module: {}", pf_ps5_sys::sce_error(rc));
            return;
        }
        let mut mem = Videodec2ComputeMemory::new();
        let rc = sceVideodec2QueryComputeMemoryInfo(&mut mem);
        tracing::info!(
            "videodec2: compute memory {} bytes ({})",
            mem.cpu_gpu_size,
            pf_ps5_sys::sce_error(rc)
        );
    }
}

/// How a slot reaches the GPU copy.
enum Upload {
    /// The slot's own memory, imported.
    Imported {
        buffer: vk::Buffer,
        memory: vk::DeviceMemory,
    },
    /// A host-visible copy of it.
    Staged {
        buffer: vk::Buffer,
        memory: vk::DeviceMemory,
        ptr: *mut u8,
    },
}

struct FrameSlot {
    mem: Direct,
    upload: Upload,
}

/// The device-local planes one picture is copied into.
struct PlaneSet {
    dev: Arc<GpuDevice>,
    images: [vk::Image; 2],
    memory: [vk::DeviceMemory; 2],
    views: [vk::ImageView; 2],
}

impl Drop for PlaneSet {
    fn drop(&mut self) {
        // SAFETY: the last holder drops this after its sampling frame finished; the lane waits
        // its own copies before publishing.
        unsafe {
            for i in 0..2 {
                self.dev.device.destroy_image_view(self.views[i], None);
                self.dev.device.destroy_image(self.images[i], None);
                self.dev.device.free_memory(self.memory[i], None);
            }
        }
    }
}

struct Decoder {
    dev: Arc<GpuDevice>,
    decoder: *mut c_void,
    queue: *mut c_void,
    is_h264: bool,
    ten_bit: bool,
    /// The visible picture.
    width: u32,
    height: u32,
    input: Direct,
    frames: Vec<FrameSlot>,
    next_frame: usize,
    planes: Vec<Arc<PlaneSet>>,
    pool: vk::CommandPool,
    cmd: vk::CommandBuffer,
    fence: vk::Fence,
    _compute: Direct,
    _gpu: Option<Direct>,
    _cpu_gpu: Option<Direct>,
    _cpu: Option<Flexible>,
    color: ColorDesc,
    needs_reset: bool,
}

// SAFETY: the decoder handle and mappings are used only under the sink's mutex.
unsafe impl Send for Decoder {}

/// Opens Videodec2 sinks that publish to the presenter's slot.
#[derive(Clone)]
pub struct Vd2Lane {
    dev: Arc<GpuDevice>,
    slot: Arc<VideoSlot>,
}

impl Vd2Lane {
    pub fn new(dev: Arc<GpuDevice>, slot: Arc<VideoSlot>) -> Self {
        Self { dev, slot }
    }

    pub fn open(&self, spec: &StreamSpec) -> Result<Box<dyn VideoSink>> {
        let decoder = Decoder::new(self.dev.clone(), spec)?;
        Ok(Box::new(Vd2Sink {
            decoder: Mutex::new(decoder),
            slot: self.slot.clone(),
            hud: spec.hud.clone(),
            started: Instant::now(),
        }))
    }
}

/// Profile and level for a mode, from the table console titles decode with.
fn config_for(codec: u8, ten_bit: bool, width: u32, height: u32, hz: u32) -> Result<(u32, u32, u32)> {
    let fast = hz > 60;
    let big = width * height;
    match codec {
        quic::CODEC_H264 => {
            let level = if big > 2560 * 1440 {
                if fast {
                    60
                } else {
                    52
                }
            } else if big > 1920 * 1088 && fast {
                52
            } else {
                51
            };
            Ok((CODEC_AVC, AVC_HIGH, level))
        }
        quic::CODEC_HEVC => {
            let level = match (big > 2560 * 1440, big > 1920 * 1088, fast) {
                (true, _, true) => 156,
                (true, _, false) | (false, true, true) => 153,
                (false, true, false) | (false, false, true) => 150,
                (false, false, false) => 123,
            };
            Ok((CODEC_HEVC, if ten_bit { HEVC_MAIN10 } else { HEVC_MAIN }, level))
        }
        other => bail!("Videodec2 decodes H.264 and HEVC, not codec 0x{other:02x}"),
    }
}

impl Decoder {
    fn new(dev: Arc<GpuDevice>, spec: &StreamSpec) -> Result<Self> {
        let ten_bit = spec.bit_depth >= 10;
        let (codec_type, profile, level) = config_for(spec.codec, ten_bit, spec.width, spec.height, spec.refresh_hz)?;
        // HEVC codes 32-row blocks: at 2160 rows a 2160-high decoder refused every picture
        // (0x811d0302), and console titles size it 2176 (H.264's 16-row macroblocks fit 2160).
        let align = if codec_type == CODEC_HEVC { 32 } else { 16 };
        let coded_height = spec.height.div_ceil(align) * align;
        // SAFETY: every out-parameter is a local; memory handed to the decoder is owned by the
        // returned `Decoder`, which deletes the decoder before dropping it.
        unsafe {
            let mut cm = Videodec2ComputeMemory::new();
            check(sceVideodec2QueryComputeMemoryInfo(&mut cm), "query compute memory")?;
            let compute = Direct::new(cm.cpu_gpu_size as usize, PROT_CPU_GPU_RW)?;
            cm.cpu_gpu = compute.addr;
            cm.cpu_gpu_size = compute.len as u64;
            let mut queue = std::ptr::null_mut();
            check(
                sceVideodec2AllocateComputeQueue(&Videodec2ComputeConfig::new(), &cm, &mut queue),
                "allocate compute queue",
            )?;
            let mut cfg = Videodec2DecoderConfig::new();
            cfg.resource_type = RESOURCE_COMPUTE;
            cfg.codec_type = codec_type;
            cfg.profile = profile;
            cfg.max_level = level;
            cfg.max_width = spec.width as i32;
            cfg.max_height = coded_height as i32;
            cfg.max_dpb_frames = MAX_DPB_FRAMES;
            cfg.pipeline_depth = 1;
            cfg.compute_queue = queue as u64;
            cfg.cpu_affinity = DEFAULT_AFFINITY;
            cfg.cpu_priority = DEFAULT_PRIORITY;
            cfg.optimize_progressive = 1;
            let mut mem = Videodec2DecoderMemory::new();
            check(
                sceVideodec2QueryDecoderMemoryInfo(&cfg, &mut mem),
                "query decoder memory",
            )?;
            let cpu = if mem.cpu_size > 0 {
                let len = page_align(mem.cpu_size as usize);
                let mut addr = std::ptr::null_mut();
                check(
                    sceKernelMapNamedFlexibleMemory(&mut addr, len, PROT_CPU_RW, 0, c"pf-vd2-cpu".as_ptr()),
                    "map decoder workspace",
                )?;
                mem.cpu = addr;
                mem.cpu_size = len as u64;
                Some(Flexible { addr, len })
            } else {
                None
            };
            let gpu = (mem.gpu_size > 0)
                .then(|| Direct::new(mem.gpu_size as usize, PROT_CPU_W_GPU_RW))
                .transpose()?;
            if let Some(g) = &gpu {
                mem.gpu = g.addr;
                mem.gpu_size = g.len as u64;
            }
            let cpu_gpu = (mem.cpu_gpu_size > 0)
                .then(|| Direct::new(mem.cpu_gpu_size as usize, PROT_CPU_GPU_RW))
                .transpose()?;
            if let Some(g) = &cpu_gpu {
                mem.cpu_gpu = g.addr;
                mem.cpu_gpu_size = g.len as u64;
            }
            let mut decoder = std::ptr::null_mut();
            check(sceVideodec2CreateDecoder(&cfg, &mem, &mut decoder), "create decoder")?;
            let frame_len = page_align(mem.max_frame_size as usize);
            let input = Direct::new(INPUT_SLOT_BYTES, PROT_CPU_W_GPU_RW)?;
            let mut frames = Vec::with_capacity(FRAME_SLOTS);
            for _ in 0..FRAME_SLOTS {
                let slot = Direct::new(frame_len, PROT_CPU_W_GPU_RW)?;
                let upload = upload_for(&dev, &slot)?;
                frames.push(FrameSlot { mem: slot, upload });
            }
            check(sceVideodec2Reset(decoder), "reset")?;
            let pool = dev.device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(dev.qf)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?;
            let cmd = dev.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )?[0];
            let fence = dev.device.create_fence(&vk::FenceCreateInfo::default(), None)?;
            let imported = frames.iter().all(|f| matches!(f.upload, Upload::Imported { .. }));
            tracing::info!(
                "videodec2: {}x{} (coded {}) codec {codec_type:#x} profile {profile} level {level}, frame slot {frame_len} bytes, {}",
                spec.width,
                spec.height,
                coded_height,
                if imported { "imported (step 2)" } else { "staged (step 3)" }
            );
            Ok(Self {
                dev,
                decoder,
                queue,
                is_h264: codec_type == CODEC_AVC,
                ten_bit,
                width: spec.width,
                height: spec.height,
                input,
                frames,
                next_frame: 0,
                planes: Vec::new(),
                pool,
                cmd,
                fence,
                _compute: compute,
                _gpu: gpu,
                _cpu_gpu: cpu_gpu,
                _cpu: cpu,
                color: ColorDesc {
                    primaries: spec.color.primaries,
                    transfer: spec.color.transfer,
                    matrix: spec.color.matrix,
                    full_range: spec.color.full_range != 0,
                },
                needs_reset: false,
            })
        }
    }

    /// Decodes one access unit; `Some` is a picture ready on the GPU.
    fn decode(&mut self, au: &[u8]) -> Result<Option<(Arc<PlaneSet>, u32)>> {
        if au.len() > self.input.len {
            bail!(
                "access unit of {} bytes exceeds the {}-byte input slot",
                au.len(),
                self.input.len
            );
        }
        if self.needs_reset {
            // SAFETY: a live decoder handle.
            check(unsafe { sceVideodec2Reset(self.decoder) }, "reset")?;
            self.needs_reset = false;
        }
        let slot = self.next_frame;
        self.next_frame = (self.next_frame + 1) % FRAME_SLOTS;
        // SAFETY: the input slot is writable for its length and not read by the decoder between
        // calls; every struct is a local the calls borrow.
        let out = unsafe {
            std::ptr::copy_nonoverlapping(au.as_ptr(), self.input.addr.cast::<u8>(), au.len());
            let mut input = Videodec2Input::new();
            input.au = self.input.addr;
            input.au_size = au.len() as u64;
            input.dts = u64::MAX;
            let mut frame = Videodec2Frame::new();
            frame.buffer = self.frames[slot].mem.addr;
            frame.buffer_size = self.frames[slot].mem.len as u64;
            let mut out = Videodec2Output::new();
            let mut rc = sceVideodec2Decode(self.decoder, &mut input, &mut frame, &mut out);
            if rc == 0 && out.valid == 0 && self.is_h264 {
                out = Videodec2Output::new();
                rc = sceVideodec2Flush(self.decoder, &mut frame, &mut out);
            }
            if rc != 0 || out.error != 0 {
                self.needs_reset = true;
                bail!("decode: {} (error flag {})", pf_ps5_sys::sce_error(rc), out.error);
            }
            out
        };
        if out.valid == 0 || out.picture_count != 1 {
            return Ok(None);
        }
        let index = self
            .frames
            .iter()
            .position(|f| f.mem.addr == out.buffer)
            .ok_or_else(|| anyhow!("decoder wrote outside the frame pool"))?;
        Ok(self.copy(index, &out)?.map(|planes| (planes, out.height)))
    }

    /// A plane set nothing else holds, or a new one while there are fewer than [`PLANE_SETS`].
    fn free_planes(&mut self, coded_w: u32, coded_h: u32) -> Result<Option<Arc<PlaneSet>>> {
        if let Some(p) = self.planes.iter().find(|p| Arc::strong_count(p) == 1) {
            return Ok(Some(p.clone()));
        }
        if self.planes.len() >= PLANE_SETS {
            return Ok(None);
        }
        let (yf, cf) = if self.ten_bit {
            (vk::Format::R16_UNORM, vk::Format::R16G16_UNORM)
        } else {
            (vk::Format::R8_UNORM, vk::Format::R8G8_UNORM)
        };
        let usage = vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST;
        let local = vk::MemoryPropertyFlags::DEVICE_LOCAL;
        let (yi, ym) = pf_ps5_vk::util::image(
            &self.dev,
            vk::Extent2D {
                width: coded_w,
                height: coded_h,
            },
            yf,
            usage,
            local,
        )?;
        let (ci, cm) = pf_ps5_vk::util::image(
            &self.dev,
            vk::Extent2D {
                width: coded_w / 2,
                height: coded_h / 2,
            },
            cf,
            usage,
            local,
        )?;
        let set = Arc::new(PlaneSet {
            dev: self.dev.clone(),
            images: [yi, ci],
            memory: [ym, cm],
            views: [
                pf_ps5_vk::util::view(&self.dev, yi, yf)?,
                pf_ps5_vk::util::view(&self.dev, ci, cf)?,
            ],
        });
        self.planes.push(set.clone());
        Ok(Some(set))
    }

    /// Copies frame slot `index` into a free plane set and waits for the copy. `None` skips the
    /// picture when the presenter still holds every set: the decoder's state is intact, so this
    /// is a dropped frame, never a reason to freeze or ask for a keyframe.
    fn copy(&mut self, index: usize, out: &Videodec2Output) -> Result<Option<Arc<PlaneSet>>> {
        let bytes_per_sample = if self.ten_bit { 2 } else { 1 };
        let pitch_bytes = if out.pitch_bytes != 0 {
            out.pitch_bytes as usize
        } else {
            out.pitch as usize * bytes_per_sample
        };
        let coded_h = out.height;
        let y_bytes = pitch_bytes * coded_h as usize;
        let uv_bytes = pitch_bytes * (coded_h as usize).div_ceil(2);
        let Some(planes) = self.free_planes(out.width, coded_h)? else {
            tracing::warn!("videodec2: every plane set is still on screen — skipping this picture");
            return Ok(None);
        };
        let buffer = match &self.frames[index].upload {
            Upload::Imported { buffer, .. } => *buffer,
            Upload::Staged { buffer, ptr, .. } => {
                // SAFETY: the staging buffer maps at least the slot's length; the decoder finished
                // writing the slot before Decode returned.
                unsafe {
                    std::ptr::copy_nonoverlapping(self.frames[index].mem.addr.cast::<u8>(), *ptr, y_bytes + uv_bytes);
                }
                *buffer
            }
        };
        let dev = &self.dev;
        let row = |pitch: usize, texel: usize| (pitch / texel) as u32;
        // SAFETY: the command buffer and fence are this decoder's and idle (each copy is waited
        // below); the images belong to the plane set, which only this lane writes while it is free.
        unsafe {
            dev.device
                .reset_command_buffer(self.cmd, vk::CommandBufferResetFlags::empty())?;
            dev.device.begin_command_buffer(
                self.cmd,
                &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            for image in planes.images {
                pf_ps5_vk::util::barrier(
                    dev,
                    self.cmd,
                    image,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    (vk::PipelineStageFlags2::FRAGMENT_SHADER, vk::AccessFlags2::NONE),
                    (vk::PipelineStageFlags2::COPY, vk::AccessFlags2::TRANSFER_WRITE),
                );
            }
            let layers = vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                layer_count: 1,
                ..Default::default()
            };
            let y = [vk::BufferImageCopy::default()
                .buffer_row_length(row(pitch_bytes, bytes_per_sample))
                .image_subresource(layers)
                .image_extent(vk::Extent3D {
                    width: out.width,
                    height: coded_h,
                    depth: 1,
                })];
            let uv = [vk::BufferImageCopy::default()
                .buffer_offset(y_bytes as u64)
                .buffer_row_length(row(pitch_bytes, bytes_per_sample * 2))
                .image_subresource(layers)
                .image_extent(vk::Extent3D {
                    width: out.width / 2,
                    height: coded_h / 2,
                    depth: 1,
                })];
            dev.device.cmd_copy_buffer_to_image(
                self.cmd,
                buffer,
                planes.images[0],
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &y,
            );
            dev.device.cmd_copy_buffer_to_image(
                self.cmd,
                buffer,
                planes.images[1],
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &uv,
            );
            for image in planes.images {
                pf_ps5_vk::util::barrier(
                    dev,
                    self.cmd,
                    image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    (vk::PipelineStageFlags2::COPY, vk::AccessFlags2::TRANSFER_WRITE),
                    (vk::PipelineStageFlags2::FRAGMENT_SHADER, vk::AccessFlags2::SHADER_READ),
                );
            }
            dev.device.end_command_buffer(self.cmd)?;
            let cmds = [self.cmd];
            dev.vkd.queue_lock.yield_to_present();
            {
                let _q = dev.vkd.queue_lock.guard();
                dev.device.queue_submit(
                    dev.queue,
                    &[vk::SubmitInfo::default().command_buffers(&cmds)],
                    self.fence,
                )?;
            }
            dev.device
                .wait_for_fences(&[self.fence], true, 1_000_000_000)
                .context("plane copy")?;
            dev.device.reset_fences(&[self.fence])?;
        }
        Ok(Some(planes))
    }
}

/// The slot imported as a buffer, or a staging buffer when the driver refuses the pointer.
fn upload_for(dev: &GpuDevice, slot: &Direct) -> Result<Upload> {
    if dev.host_memory_import {
        match import(dev, slot) {
            Ok(u) => return Ok(u),
            Err(e) => tracing::warn!("videodec2: host-pointer import refused, staging instead: {e:#}"),
        }
    }
    let (buffer, memory, ptr) = pf_ps5_vk::util::host_buffer(dev, slot.len as u64, vk::BufferUsageFlags::TRANSFER_SRC)?;
    Ok(Upload::Staged { buffer, memory, ptr })
}

fn import(dev: &GpuDevice, slot: &Direct) -> Result<Upload> {
    let ext = ash::ext::external_memory_host::Device::new(&dev.instance, &dev.device);
    let mut props = vk::MemoryHostPointerPropertiesEXT::default();
    // SAFETY: the pointer is a live direct-memory mapping of `slot.len` bytes, 16 KiB aligned.
    unsafe {
        (ext.fp().get_memory_host_pointer_properties_ext)(
            dev.device.handle(),
            vk::ExternalMemoryHandleTypeFlags::HOST_ALLOCATION_EXT,
            slot.addr,
            &mut props,
        )
    }
    .result()?;
    let mut external = vk::ExternalMemoryBufferCreateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::HOST_ALLOCATION_EXT);
    // SAFETY: builders are locals; the buffer and memory are destroyed with the frame slot.
    unsafe {
        let buffer = dev.device.create_buffer(
            &vk::BufferCreateInfo::default()
                .size(slot.len as u64)
                .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .push_next(&mut external),
            None,
        )?;
        let req = dev.device.get_buffer_memory_requirements(buffer);
        let index = dev
            .memory_type(
                req.memory_type_bits & props.memory_type_bits,
                vk::MemoryPropertyFlags::empty(),
            )
            .ok_or_else(|| anyhow!("no memory type takes the host pointer"))?;
        let mut import = vk::ImportMemoryHostPointerInfoEXT::default()
            .handle_type(vk::ExternalMemoryHandleTypeFlags::HOST_ALLOCATION_EXT)
            .host_pointer(slot.addr);
        let memory = dev.device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(slot.len as u64)
                .memory_type_index(index)
                .push_next(&mut import),
            None,
        )?;
        dev.device.bind_buffer_memory(buffer, memory, 0)?;
        Ok(Upload::Imported { buffer, memory })
    }
}

fn check(rc: i32, what: &str) -> Result<()> {
    if rc == 0 {
        Ok(())
    } else {
        Err(anyhow!("videodec2 {what}: {}", pf_ps5_sys::sce_error(rc)))
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let dev = self.dev.clone();
        {
            let _q = dev.vkd.queue_lock.guard();
            // SAFETY: idling the shared device before the slots and buffers go.
            let _ = unsafe { dev.device.device_wait_idle() };
        }
        // SAFETY: the decoder is deleted before the memory it was given; the Vulkan objects are
        // this decoder's and idle.
        unsafe {
            sceVideodec2DeleteDecoder(self.decoder);
            sceVideodec2ReleaseComputeQueue(self.queue);
            for f in self.frames.drain(..) {
                match f.upload {
                    Upload::Imported { buffer, memory } | Upload::Staged { buffer, memory, .. } => {
                        dev.device.destroy_buffer(buffer, None);
                        dev.device.free_memory(memory, None);
                    }
                }
            }
            dev.device.destroy_fence(self.fence, None);
            dev.device.destroy_command_pool(self.pool, None);
        }
    }
}

struct Vd2Sink {
    decoder: Mutex<Decoder>,
    slot: Arc<VideoSlot>,
    hud: Arc<punktfunk_core::hud::Stats>,
    started: Instant,
}

impl VideoSink for Vd2Sink {
    fn name(&self) -> &'static str {
        "Videodec2"
    }

    fn caps(&self) -> VideoSinkCaps {
        VideoSinkCaps {
            pts: true,
            partial_au: false,
            flush: false,
            intra_only: false,
        }
    }

    fn feed(&self, au: &[u8], _pts_ns: u64, flags: FeedFlags) -> Result<()> {
        let mut d = self.decoder.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some((planes, coded_h)) = d.decode(au)? else {
            return Ok(());
        };
        let views = planes.views;
        let uv_scale = [1.0, d.height as f32 / coded_h.max(1) as f32];
        self.slot.publish(Picture {
            planes: Planes::Two(views),
            layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            width: d.width,
            height: d.height,
            color: d.color,
            depth: if d.ten_bit { 10 } else { 8 },
            msb_packed: d.ten_bit,
            sample_scale: if d.ten_bit { 64.0 } else { 1.0 },
            uv_scale,
            decoded_at: Instant::now(),
            pts_ns: flags.host_pts_ns,
            decoded_ns: u64::try_from(punktfunk_core::client::now_realtime_ns()).unwrap_or(0),
            turn: None,
            hud: Some(self.hud.clone()),
            hold: Box::new(planes),
        });
        Ok(())
    }

    fn clock(&self) -> Option<&dyn MediaClock> {
        Some(self)
    }
}

impl MediaClock for Vd2Sink {
    fn now_ns(&self) -> u64 {
        self.started.elapsed().as_nanos() as u64
    }
}

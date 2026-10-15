//! The console's implementations, assembled into one [`Platform`].

mod audio;
mod klog;
mod lifecycle;
mod pads;
mod videodec2;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use pf_ps5_app::core::caps::VideoCaps;
use pf_ps5_app::core::media::VideoSink;
use pf_ps5_app::platform::{DeviceInfo, Platform, Storage, StreamSpec, VideoDecoders};
use pf_ps5_vk::{GpuDevice, Presenter, PyroLane, Target, VideoSlot};
use punktfunk_core::quic;

/// Title-local writable storage; present when `param.json` sets `downloadDataSize`.
const DOWNLOAD0: &str = "/download0";
/// Where a payload launch (ELF loader, no title sandbox) keeps its files instead.
const PAYLOAD_DIR: &str = "/data/punktfunk-ps5";

/// The pins this build was made from, for the log and the About screen.
const MESA_PIN: &str = match option_env!("PF_MESA_PIN") {
    Some(p) => p,
    None => "unpinned",
};

struct Dir(PathBuf);

impl Storage for Dir {
    fn data_dir(&self) -> &Path {
        &self.0
    }
}

fn storage() -> Dir {
    if Path::new(DOWNLOAD0).is_dir() {
        Dir(PathBuf::from(DOWNLOAD0))
    } else {
        let _ = std::fs::create_dir_all(PAYLOAD_DIR);
        Dir(PathBuf::from(PAYLOAD_DIR))
    }
}

/// The statically linked RADV as an `ash::Entry`: no loader on the console.
fn entry() -> ash::Entry {
    unsafe extern "system" fn gipa(
        instance: ash::vk::Instance,
        name: *const std::ffi::c_char,
    ) -> ash::vk::PFN_vkVoidFunction {
        use ash::vk::Handle as _;
        // SAFETY: forwards Vulkan's own contract: `name` is NUL-terminated, `instance` null or live.
        let f = unsafe { pf_ps5_sys::vk::vk_icdGetInstanceProcAddr(instance.as_raw() as *mut _, name) };
        // SAFETY: both are `Option<unsafe extern "system" fn()>`.
        unsafe { std::mem::transmute::<pf_ps5_sys::vk::PFN_vkVoidFunction, ash::vk::PFN_vkVoidFunction>(f) }
    }
    // SAFETY: `gipa` resolves every global and instance command, as a loader would.
    unsafe {
        ash::Entry::from_static_fn(ash::StaticFn {
            get_instance_proc_addr: gipa,
        })
    }
}

/// PyroWave on the GPU, H.264/HEVC through Videodec2.
struct Ps5Video {
    pyro: PyroLane,
    vd2: videodec2::Vd2Lane,
}

impl VideoDecoders for Ps5Video {
    fn caps(&self) -> VideoCaps {
        VideoCaps {
            pyrowave: self.pyro.available(),
            ..VideoCaps::PS5
        }
    }

    fn open(&self, spec: &StreamSpec) -> Result<Box<dyn VideoSink>> {
        if spec.codec == quic::CODEC_PYROWAVE {
            self.pyro.open(spec)
        } else {
            self.vd2.open(spec)
        }
    }
}

/// The data directory and the kernel log, for the probe, which needs nothing else.
pub fn probe_parts() -> (PathBuf, Box<dyn std::io::Write + Send>) {
    (storage().0, Box::new(klog::Klog::default()))
}

/// Builds the console platform. Order matters: Videodec2 loads before anything else touches the
/// process (the 13.60 trap), and Mesa's environment is set before the instance exists.
pub fn platform() -> Result<Platform> {
    videodec2::warm_up();
    let dir = storage();
    let cache = dir.0.join("radv-cache");
    let _ = std::fs::create_dir_all(&cache);
    std::env::set_var("MESA_SHADER_CACHE_DIR", &cache);
    let dev = Arc::new(GpuDevice::new(entry(), Target::Display).context("bring up the GPU")?);
    let slot = Arc::new(VideoSlot::default());
    let presenter = Presenter::new(dev.clone(), slot.clone()).context("bring up the display")?;
    let device = DeviceInfo {
        platform: "ps5",
        model: "PlayStation 5".into(),
        firmware: String::new(),
        gpu: format!("{} (mesa {MESA_PIN})", dev.name),
    };
    Ok(Platform {
        display: Box::new(presenter),
        video: Arc::new(Ps5Video {
            pyro: PyroLane::new(dev.clone(), slot.clone()),
            vd2: videodec2::Vd2Lane::new(dev, slot),
        }),
        audio: Arc::new(audio::Ps5Audio::new()?),
        pads: Box::new(pads::Ps5Pads::new()?),
        storage: Arc::new(dir),
        lifecycle: Box::new(lifecycle::Ps5Lifecycle),
        log: Some(Box::new(klog::Klog::default())),
        device,
    })
}

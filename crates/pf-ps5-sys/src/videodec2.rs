//! libSceVideodec2: H.264 and HEVC decode into direct memory the caller owns.
//!
//! Works only in a registered title process, after `sceSysmoduleLoadModule(VIDEODEC2)`. The
//! decoder writes a LINEAR picture into the frame slot it is offered: NV12 (8-bit) or two-plane
//! 4:2:0 16-bit words with the 10 bits LOW-aligned (Main10, not P010). Chroma starts after the
//! CODED height.

use core::ffi::c_void;

pub const CODEC_AVC: u32 = 1;
pub const CODEC_HEVC: u32 = 0x000E_E049;
pub const RESOURCE_COMPUTE: u32 = 1;
pub const AVC_HIGH: u32 = 100;
pub const HEVC_MAIN: u32 = 1;
pub const HEVC_MAIN10: u32 = 2;
/// Hosts that reserve five references need a six-deep DPB, or decode fails `OVERSIZE_DECODE`.
pub const MAX_DPB_FRAMES: i32 = 6;
pub const DEFAULT_PRIORITY: i32 = 700;
/// Three physical cores (SMT siblings adjacent): measured best for one decoder.
pub const DEFAULT_AFFINITY: u64 = 0x3f;
/// One access-unit input slot.
pub const INPUT_SLOT_BYTES: usize = 0x80_0000;

pub const ERROR_FATAL_STATE: i32 = 0x811D_0111_u32 as i32;
pub const ERROR_OVERSIZE_DECODE: i32 = 0x811D_0302_u32 as i32;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Videodec2ComputeConfig {
    pub size: u64,
    pub pipe_id: u16,
    pub queue_id: u16,
    pub check_memory_type: u8,
    pub reserved0: u8,
    pub reserved1: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Videodec2ComputeMemory {
    pub size: u64,
    pub cpu_gpu_size: u64,
    pub cpu_gpu: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Videodec2DecoderConfig {
    pub size: u64,
    pub resource_type: u32,
    pub codec_type: u32,
    pub profile: u32,
    pub max_level: u32,
    pub max_width: i32,
    pub max_height: i32,
    pub max_dpb_frames: i32,
    pub pipeline_depth: u32,
    pub compute_queue: u64,
    pub cpu_affinity: u64,
    pub cpu_priority: i32,
    pub optimize_progressive: u32,
    /// Stays zero: no console source enables the memory-type checks.
    pub check_memory_type: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Videodec2DecoderMemory {
    pub size: u64,
    pub cpu_size: u64,
    pub cpu: *mut c_void,
    pub gpu_size: u64,
    pub gpu: *mut c_void,
    pub cpu_gpu_size: u64,
    pub cpu_gpu: *mut c_void,
    pub max_frame_size: u64,
    pub frame_alignment: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Videodec2Input {
    pub size: u64,
    pub au: *mut c_void,
    pub au_size: u64,
    pub pts: u64,
    /// `u64::MAX`: no DTS.
    pub dts: u64,
    pub attached: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Videodec2Frame {
    pub size: u64,
    pub buffer: *mut c_void,
    pub buffer_size: u64,
    pub accepted: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Videodec2Output {
    pub size: u64,
    pub valid: u8,
    pub error: u8,
    pub picture_count: u8,
    pub padding: u8,
    pub codec: u32,
    pub width: u32,
    /// Samples per row.
    pub pitch: u32,
    /// CODED rows (1088 for 1080p; 2160 or 2176 for 2160p).
    pub height: u32,
    pub reserved: u32,
    pub buffer: *mut c_void,
    pub buffer_size: u64,
    pub frame_format: u32,
    pub pitch_bytes: u32,
}

macro_rules! zeroed_with_size {
    ($($t:ty),*) => {$(
        impl $t {
            /// Zeroed, with `size` set as every Videodec2 call requires.
            pub fn new() -> Self {
                // SAFETY: plain-data C struct of integers and raw pointers; all-zero is valid.
                let mut v: Self = unsafe { core::mem::zeroed() };
                v.size = core::mem::size_of::<Self>() as u64;
                v
            }
        }

        impl Default for $t {
            fn default() -> Self {
                Self::new()
            }
        }
    )*};
}
zeroed_with_size!(
    Videodec2ComputeConfig,
    Videodec2ComputeMemory,
    Videodec2DecoderConfig,
    Videodec2DecoderMemory,
    Videodec2Input,
    Videodec2Frame,
    Videodec2Output
);

extern "C" {
    pub fn sceVideodec2QueryComputeMemoryInfo(mem: *mut Videodec2ComputeMemory) -> i32;
    pub fn sceVideodec2AllocateComputeQueue(
        cfg: *const Videodec2ComputeConfig,
        mem: *const Videodec2ComputeMemory,
        queue: *mut *mut c_void,
    ) -> i32;
    pub fn sceVideodec2ReleaseComputeQueue(queue: *mut c_void) -> i32;
    pub fn sceVideodec2QueryDecoderMemoryInfo(
        cfg: *const Videodec2DecoderConfig,
        mem: *mut Videodec2DecoderMemory,
    ) -> i32;
    pub fn sceVideodec2CreateDecoder(
        cfg: *const Videodec2DecoderConfig,
        mem: *const Videodec2DecoderMemory,
        decoder: *mut *mut c_void,
    ) -> i32;
    pub fn sceVideodec2DeleteDecoder(decoder: *mut c_void) -> i32;
    pub fn sceVideodec2Reset(decoder: *mut c_void) -> i32;
    pub fn sceVideodec2Decode(
        decoder: *mut c_void,
        input: *mut Videodec2Input,
        frame: *mut Videodec2Frame,
        output: *mut Videodec2Output,
    ) -> i32;
    pub fn sceVideodec2Flush(decoder: *mut c_void, frame: *mut Videodec2Frame, output: *mut Videodec2Output) -> i32;
}

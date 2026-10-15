//! libkernel: direct memory (Videodec2's buffers), flexible memory, the kernel log.

use core::ffi::{c_char, c_void};

/// The direct-memory type every console source uses: CPU-cached, coherent with the GPU.
pub const MEMORY_TYPE: i32 = 12;
/// Direct memory is allocated and mapped in 16 KiB pages.
pub const PAGE: usize = 0x4000;
pub const PROT_CPU_READ: i32 = 0x01;
pub const PROT_CPU_WRITE: i32 = 0x02;
pub const PROT_GPU_READ: i32 = 0x10;
pub const PROT_GPU_WRITE: i32 = 0x20;
/// CPU read/write, GPU read/write.
pub const PROT_CPU_GPU_RW: i32 = 0x33;
/// CPU write, GPU read/write: decoder input and frame pools.
pub const PROT_CPU_W_GPU_RW: i32 = 0x32;
pub const PROT_CPU_RW: i32 = 0x03;

/// Rounds `len` up to [`PAGE`].
pub const fn page_align(len: usize) -> usize {
    (len + PAGE - 1) & !(PAGE - 1)
}

extern "C" {
    pub fn sceKernelGetDirectMemorySize() -> i64;
    pub fn sceKernelAllocateDirectMemory(
        search_start: i64,
        search_end: i64,
        len: usize,
        alignment: usize,
        memory_type: i32,
        phys_start: *mut i64,
    ) -> i32;
    pub fn sceKernelMapDirectMemory(
        addr: *mut *mut c_void,
        len: usize,
        prot: i32,
        flags: i32,
        phys_start: i64,
        alignment: usize,
    ) -> i32;
    pub fn sceKernelReleaseDirectMemory(phys_start: i64, len: usize) -> i32;
    pub fn sceKernelMunmap(addr: *mut c_void, len: usize) -> i32;
    pub fn sceKernelMapNamedFlexibleMemory(
        addr: *mut *mut c_void,
        len: usize,
        prot: i32,
        flags: i32,
        name: *const c_char,
    ) -> i32;
    pub fn sceKernelReleaseFlexibleMemory(addr: *mut c_void, len: usize) -> i32;
    /// One line to the kernel log (klog, TCP 3232). Append the newline yourself.
    pub fn sceKernelDebugOutText(channel: i32, text: *const c_char) -> i32;
    pub fn sceKernelUsleep(micros: u32) -> i32;
}

//! libSceSysmodule: system libraries a title loads before first use.

/// Videodec2 (H.264/HEVC decode). Load before the first Videodec2 call and before anything
/// that could take the process out of its sandbox.
pub const VIDEODEC2: u32 = 0x00CF;

extern "C" {
    pub fn sceSysmoduleLoadModule(id: u32) -> i32;
}

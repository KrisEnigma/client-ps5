//! The statically linked RADV's one entry point. There is no Vulkan loader on the console: every
//! other command resolves through this.

use core::ffi::{c_char, c_void};

pub type PFN_vkVoidFunction = Option<unsafe extern "system" fn()>;

extern "system" {
    /// Exported by RADV's archive (`libvulkan_radeon.ps5.a`), linked whole into the title.
    pub fn vk_icdGetInstanceProcAddr(instance: *mut c_void, name: *const c_char) -> PFN_vkVoidFunction;
}

//! The title's life: the splash, system events, and the one way out.

use pf_ps5_app::platform::{Lifecycle, SystemEvent};
use pf_ps5_sys::kernel::sceKernelUsleep;
use pf_ps5_sys::system::*;

pub struct Ps5Lifecycle;

impl Lifecycle for Ps5Lifecycle {
    fn hide_splash(&self) {
        // SAFETY: no arguments.
        let rc = unsafe { sceSystemServiceHideSplashScreen() };
        if rc != 0 {
            tracing::warn!("hide splash: {}", pf_ps5_sys::sce_error(rc));
        }
    }

    /// Nothing yet: the home menu shows in the pads' samples (intercepted), and rest mode is the
    /// shell's. System-event polling waits for a probe to prove its layout.
    fn poll_events(&mut self, _out: &mut Vec<SystemEvent>) {}

    fn exit(&self) -> ! {
        // SAFETY: the literal path and a null argv, as the shell expects.
        unsafe { sceSystemServiceLoadExec(c"exit".as_ptr(), std::ptr::null()) };
        loop {
            // SAFETY: plain sleep; the shell ends the process.
            unsafe { sceKernelUsleep(100_000) };
        }
    }
}

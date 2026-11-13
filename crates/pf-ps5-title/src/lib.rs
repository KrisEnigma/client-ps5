//! The title's entry: the C `main` the payload SDK's CRT calls, linked by `scripts/link-title.sh`.
//!
//! std's runtime start-up does not run for a static library, so what it would have done is
//! done here: SIGPIPE ignored (a write to a closed socket must not end the title). The title
//! never returns: `pf_ps5_app::run` and every failure path end in `sceSystemServiceLoadExec`.
#![cfg(target_vendor = "sie")]

use core::ffi::{c_char, c_int};

/// Ends the title the one way the shell accepts, after `why` reaches the kernel log.
fn leave(why: &str) -> ! {
    let line = format!("punktfunk-ps5: {why}\n\0");
    // SAFETY: NUL-terminated and outlives the call; then the documented exit sequence.
    unsafe {
        pf_ps5_sys::kernel::sceKernelDebugOutText(0, line.as_ptr().cast());
        pf_ps5_sys::system::sceSystemServiceLoadExec(c"exit".as_ptr(), core::ptr::null());
        loop {
            pf_ps5_sys::kernel::sceKernelUsleep(100_000);
        }
    }
}

#[no_mangle]
pub extern "C" fn main(_argc: c_int, _argv: *const *const c_char) -> c_int {
    // SAFETY: setting a signal disposition at start-up, before any thread exists.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
    #[cfg(feature = "probe")]
    {
        let (dir, klog) = pf_ps5_platform::probe_parts();
        pf_ps5_app::services::paths::install(&dir);
        pf_ps5_app::install_tls_provider();
        // probe.json in, probe.log out, where FTP reaches them.
        let exchange = pf_ps5_app::runtime::testrun::exchange_dir();
        let ctx = pf_ps5_probe::Ctx {
            config: pf_ps5_probe::config(&exchange),
            dir,
        };
        let failed = match pf_ps5_probe::Out::new(&exchange, Some(klog)) {
            Ok(mut out) => pf_ps5_probe::run(&ctx, &mut out),
            Err(e) => leave(&format!("probe log: {e:#}")),
        };
        leave(&format!("probe finished, {failed} failed"));
    }
    #[cfg(not(feature = "probe"))]
    match pf_ps5_platform::platform() {
        Ok(platform) => pf_ps5_app::run(platform),
        Err(e) => leave(&format!("platform bring-up: {e:#}")),
    }
}

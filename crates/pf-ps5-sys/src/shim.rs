//! The few libc functions std needs that the console gets wrong, each bound by the link
//! (`scripts/link-title.sh`, `--defsym=name=pf_ps5_name`) rather than exported: a title that
//! exports symbols is refused by the title converter.

use core::ffi::{c_char, c_int, c_void};

/// std names every thread from inside the new thread (`pthread_set_name_np`), and on the console
/// a thread that renames itself hangs before its entry point. Threads stay unnamed; the log names
/// them by what they do.
#[no_mangle]
pub extern "C" fn pf_ps5_pthread_set_name_np(_thread: *mut c_void, _name: *const c_char) {}

/// Fills `buf` from `kern.rng_pseudo`, the entropy source console titles have proven; the
/// SDK's `arc4random` points at nothing in a title. At most 64 bytes per sysctl.
#[cfg(any(target_os = "freebsd", target_vendor = "apple"))]
pub fn fill_random(buf: &mut [u8]) -> Result<(), c_int> {
    for chunk in buf.chunks_mut(64) {
        let mut len = chunk.len();
        // SAFETY: the name is NUL-terminated; `chunk` is writable for `len` bytes and `len` is
        // updated in place by the call.
        let rc = unsafe {
            libc::sysctlbyname(
                c"kern.rng_pseudo".as_ptr(),
                chunk.as_mut_ptr().cast(),
                &mut len,
                core::ptr::null_mut::<c_void>() as _,
                0,
            )
        };
        if rc != 0 || len != chunk.len() {
            return Err(rc);
        }
    }
    Ok(())
}

/// No `kern.rng_pseudo` off the BSDs: the probe reports the miss.
#[cfg(not(any(target_os = "freebsd", target_vendor = "apple")))]
pub fn fill_random(_buf: &mut [u8]) -> Result<(), c_int> {
    Err(-1)
}

/// `getrandom`'s custom backend (`--cfg getrandom_backend="custom"`), for 0.3 and 0.4 alike.
#[cfg(target_vendor = "sie")]
#[no_mangle]
unsafe extern "Rust" fn __getrandom_v03_custom(dest: *mut u8, len: usize) -> Result<(), getrandom::Error> {
    // SAFETY: getrandom passes a writable buffer of `len` bytes.
    let buf = unsafe { core::slice::from_raw_parts_mut(dest, len) };
    fill_random(buf).map_err(|_| getrandom::Error::UNEXPECTED)
}

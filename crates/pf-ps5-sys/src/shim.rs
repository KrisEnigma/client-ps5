//! The few libc functions std needs that the console gets wrong, each bound by the link
//! (`scripts/link-title.sh`, `--defsym=name=pf_ps5_name`) rather than exported: a title that
//! exports symbols is refused by the title converter.

use core::ffi::{c_char, c_int, c_uint, c_void};

/// std names every thread from inside the new thread (`pthread_set_name_np`), and on the console
/// a thread that renames itself hangs before its entry point. Threads stay unnamed; the log names
/// them by what they do.
#[no_mangle]
pub extern "C" fn pf_ps5_pthread_set_name_np(_thread: *mut c_void, _name: *const c_char) {}

/// POSIX `getentropy` for aws-lc: FreeBSD 11's libc has none. At most 256 bytes, as POSIX says.
///
/// # Safety
/// `buf` is writable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn pf_ps5_getentropy(buf: *mut c_void, len: usize) -> c_int {
    if len > 256 {
        return -1;
    }
    // SAFETY: the caller passes a writable buffer of `len` bytes.
    let buf = unsafe { core::slice::from_raw_parts_mut(buf.cast::<u8>(), len) };
    if fill_random(buf).is_ok() {
        0
    } else {
        -1
    }
}

/// std's random source on FreeBSD 12+; the console's FreeBSD 11 libc has no `getrandom`.
///
/// # Safety
/// `buf` is writable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn pf_ps5_getrandom(buf: *mut c_void, len: usize, _flags: c_uint) -> isize {
    // SAFETY: the caller passes a writable buffer of `len` bytes.
    let slice = unsafe { core::slice::from_raw_parts_mut(buf.cast::<u8>(), len) };
    if fill_random(slice).is_ok() {
        len as isize
    } else {
        -1
    }
}

/// FreeBSD's `struct mmsghdr`.
#[repr(C)]
pub struct Mmsghdr {
    pub hdr: libc::msghdr,
    pub len: isize,
}

/// quinn-udp's batched receive, which the console's libc lacks: one `recvmsg` per slot, as
/// FreeBSD's own libc does it. Only the first may block; an error after it ends the batch.
///
/// # Safety
/// `msgs` points at `vlen` initialised headers whose buffers are writable.
#[no_mangle]
pub unsafe extern "C" fn pf_ps5_recvmmsg(
    fd: c_int,
    msgs: *mut Mmsghdr,
    vlen: usize,
    flags: c_int,
    _timeout: *const c_void,
) -> isize {
    let mut n = 0;
    while n < vlen {
        let flags = if n == 0 { flags } else { flags | libc::MSG_DONTWAIT };
        // SAFETY: slot `n` is one of the caller's `vlen` initialised headers.
        let m = unsafe { &mut *msgs.add(n) };
        // SAFETY: the header's buffers are the caller's, writable as `recvmsg` requires.
        let got = unsafe { libc::recvmsg(fd, &mut m.hdr, flags) };
        if got < 0 {
            return if n == 0 { -1 } else { n as isize };
        }
        m.len = got;
        n += 1;
    }
    n as isize
}

/// mdns-sd names IPv6 scopes by interface. With none of this on the console every name is
/// unknown (0), and link-local IPv6 stays unscoped.
#[no_mangle]
pub extern "C" fn pf_ps5_if_nametoindex(_name: *const c_char) -> c_uint {
    0
}

/// aws-lc's fork detection. A title never forks; -1 tells aws-lc the detector is unavailable.
#[no_mangle]
pub extern "C" fn pf_ps5_minherit(_addr: *mut c_void, _len: usize, _inherit: c_int) -> c_int {
    -1
}

/// Skia pins the C locale around shader text. The console has one locale: this answers
/// `LC_GLOBAL_LOCALE`, so Skia's restore is a no-op too.
#[no_mangle]
pub extern "C" fn pf_ps5_uselocale(_locale: *mut c_void) -> *mut c_void {
    usize::MAX as *mut c_void
}

/// Reports no loaded objects: std's symbolizer names no frames, and the panic hook's raw
/// addresses go through `task symbolize`. C++ exceptions cannot unwind through it.
#[no_mangle]
pub extern "C" fn pf_ps5_dl_iterate_phdr(_callback: *mut c_void, _data: *mut c_void) -> c_int {
    0
}

extern "C" {
    fn vsnprintf(buf: *mut c_char, len: usize, fmt: *const c_char, args: *mut c_void) -> c_int;
}

/// Skia's log port (`SkLogVAList`), which Skia has none of for the console: one line to stderr.
///
/// # Safety
/// `fmt` and `args` are a printf format and its `va_list`, as Skia passes them.
#[no_mangle]
pub unsafe extern "C" fn pf_ps5_sk_log_va_list(_priority: c_int, fmt: *const c_char, args: *mut c_void) {
    let mut buf = [0u8; 512];
    // SAFETY: `buf` is writable for its length; `fmt` and `args` are the caller's pair.
    let n = unsafe { vsnprintf(buf.as_mut_ptr().cast(), buf.len(), fmt, args) };
    if n > 0 {
        let n = (n as usize).min(buf.len() - 1);
        // SAFETY: `buf` holds `n` initialised bytes.
        unsafe { libc::write(2, buf.as_ptr().cast(), n) };
    }
}

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

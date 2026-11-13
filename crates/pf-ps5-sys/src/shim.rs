//! The few libc functions std needs that the console gets wrong, each bound by the link
//! (`scripts/link-title.sh`, `--defsym=name=pf_ps5_name`) rather than exported: a title that
//! exports symbols is refused by the title converter.

use core::ffi::{c_char, c_int, c_uint, c_void};

/// std names every thread from inside the new thread (`pthread_set_name_np`), and on the console
/// a thread that renames itself hangs before its entry point. Threads stay unnamed; the log names
/// them by what they do.
#[no_mangle]
pub extern "C" fn pf_ps5_pthread_set_name_np(_thread: *mut c_void, _name: *const c_char) {}

/// The same for `pthread_setname_np`, which this std calls: only libScePosixForWebKit's stub
/// defines it, and a title's import from that module is null, so naming jumped to address 0.
#[no_mangle]
pub extern "C" fn pf_ps5_pthread_setname_np(_thread: *mut c_void, _name: *const c_char) -> c_int {
    0
}

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

/// std formats a failed `getaddrinfo` with this, which only libScePosixForWebKit's stub defines:
/// a name lookup that failed would have jumped to address 0.
#[no_mangle]
pub extern "C" fn pf_ps5_gai_strerror(_code: c_int) -> *const c_char {
    c"address lookup failed".as_ptr()
}

/// `mkstemp` resolves only to libScePosixForWebKit, which gives a title nothing; RADV imports it
/// for a debug dump. It fails with ENOSYS.
#[cfg(target_os = "freebsd")]
#[no_mangle]
pub extern "C" fn pf_ps5_mkstemp(_template: *mut c_char) -> c_int {
    enosys()
}

/// `readlink` resolves only to `libkernel_sys`, which gives a title nothing. It fails with ENOSYS.
#[cfg(target_os = "freebsd")]
#[no_mangle]
pub extern "C" fn pf_ps5_readlink(_path: *const c_char, _buf: *mut c_char, _len: usize) -> isize {
    enosys() as isize
}

#[cfg(target_os = "freebsd")]
fn enosys() -> c_int {
    // SAFETY: errno is this thread's.
    unsafe { *libc::__error() = libc::ENOSYS };
    -1
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
/// SDK's `arc4random` points at nothing in a title. The sysctl hands out 64-byte blocks and
/// refuses a smaller buffer, so each chunk is read whole and its prefix copied.
#[cfg(any(target_os = "freebsd", target_vendor = "apple"))]
pub fn fill_random(buf: &mut [u8]) -> Result<(), c_int> {
    let mut block = [0u8; 64];
    for chunk in buf.chunks_mut(64) {
        let mut len = block.len();
        // SAFETY: the name is NUL-terminated; `block` is writable for `len` bytes and `len` is
        // updated in place by the call.
        let rc = unsafe {
            libc::sysctlbyname(
                c"kern.rng_pseudo".as_ptr(),
                block.as_mut_ptr().cast(),
                &mut len,
                core::ptr::null_mut::<c_void>() as _,
                0,
            )
        };
        if rc != 0 || len < chunk.len() {
            block.fill(0);
            return Err(rc);
        }
        chunk.copy_from_slice(&block[..chunk.len()]);
    }
    block.fill(0);
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

/// The console's own non-blocking socket option (`console_curl.c` in the Moonlight port).
#[cfg(target_vendor = "sie")]
const SO_NBIO: c_int = 0x1200;

#[cfg(target_vendor = "sie")]
extern "C" {
    fn __real_fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    fn __real_ioctl(fd: c_int, request: core::ffi::c_ulong, ...) -> c_int;
    fn __real_close(fd: c_int) -> c_int;
    fn __real_setsockopt(fd: c_int, level: c_int, name: c_int, value: *const c_void, len: libc::socklen_t) -> c_int;
}

/// Below this a refused socket buffer keeps its default rather than shrink further.
#[cfg(target_vendor = "sie")]
const SOCKBUF_FLOOR: c_int = 64 << 10;

/// `setsockopt` for `SO_RCVBUF`/`SO_SNDBUF`: FreeBSD refuses a size above its cap where Linux
/// clamps it, so core's 32 MiB ask left the 9/41 KiB defaults. A refused size steps down by a
/// quarter until one takes, to [`SOCKBUF_FLOOR`]. Bound with `--wrap=setsockopt`.
///
/// # Safety
/// As `setsockopt`.
#[cfg(target_vendor = "sie")]
#[no_mangle]
pub unsafe extern "C" fn __wrap_setsockopt(
    fd: c_int,
    level: c_int,
    name: c_int,
    value: *const c_void,
    len: libc::socklen_t,
) -> c_int {
    // SAFETY: the caller's arguments, passed through.
    let rc = unsafe { __real_setsockopt(fd, level, name, value, len) };
    let buffer = level == libc::SOL_SOCKET && matches!(name, libc::SO_RCVBUF | libc::SO_SNDBUF);
    if rc == 0 || !buffer || value.is_null() || len as usize != size_of::<c_int>() {
        return rc;
    }
    // SAFETY: errno is this thread's; `value` holds an int, as `len` says.
    let (errno, mut size) = unsafe { (*libc::__error(), value.cast::<c_int>().read_unaligned()) };
    while size > SOCKBUF_FLOOR {
        size -= size / 4;
        // SAFETY: `size` is a readable int of `len` bytes.
        if unsafe { __real_setsockopt(fd, level, name, (&raw const size).cast(), len) } == 0 {
            return 0;
        }
    }
    // SAFETY: as above.
    unsafe { *libc::__error() = errno };
    rc
}

/// Descriptors handed out again because the console refused to duplicate them, with how many
/// extra holders each has.
#[cfg(target_vendor = "sie")]
static SHARED: std::sync::Mutex<Vec<(c_int, u32)>> = std::sync::Mutex::new(Vec::new());

/// A duplicate of `fd`: `dup`, or where the console refuses that too (EPERM on sockets and
/// kqueues), `fd` itself with one more holder, so that only its last `close` closes it.
#[cfg(target_vendor = "sie")]
fn duplicate(fd: c_int) -> c_int {
    // SAFETY: `fd` is the caller's open descriptor.
    let copy = unsafe { libc::dup(fd) };
    if copy >= 0 {
        return copy;
    }
    let mut shared = SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    match shared.iter_mut().find(|(f, _)| *f == fd) {
        Some((_, extra)) => *extra += 1,
        None => shared.push((fd, 1)),
    }
    fd
}

/// `close` for the descriptors `duplicate` shared: a holder's close only drops its count.
/// Bound with `--wrap=close`.
///
/// # Safety
/// As `close`.
#[cfg(target_vendor = "sie")]
#[no_mangle]
pub unsafe extern "C" fn __wrap_close(fd: c_int) -> c_int {
    {
        let mut shared = SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(i) = shared.iter().position(|(f, _)| *f == fd) {
            if shared[i].1 > 1 {
                shared[i].1 -= 1;
            } else {
                shared.swap_remove(i);
            }
            return 0;
        }
    }
    // SAFETY: the caller's descriptor, passed through.
    unsafe { __real_close(fd) }
}

#[cfg(target_vendor = "sie")]
fn set_nbio(fd: c_int, on: bool) -> c_int {
    let on = c_int::from(on);
    // SAFETY: `on` is a readable int of the size passed.
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            SO_NBIO,
            (&raw const on).cast(),
            size_of::<c_int>() as libc::socklen_t,
        )
    }
}

/// In a title the console's libc refuses fcntl on sockets and kqueues (EINVAL or EACCES):
/// socket2's `O_NONBLOCK` failed quinn-udp, and tokio's I/O driver failed every runtime cloning
/// mio's kqueue (`F_DUPFD_CLOEXEC`). A refused descriptor-flag call succeeds (a title never
/// execs), a refused duplicate goes to `duplicate`, and the status flags map onto `SO_NBIO`.
/// Bound with `--wrap=fcntl`; the third argument is an int or a pointer.
///
/// # Safety
/// As `fcntl`.
#[cfg(target_vendor = "sie")]
#[no_mangle]
pub unsafe extern "C" fn __wrap_fcntl(fd: c_int, cmd: c_int, arg: usize) -> c_int {
    // SAFETY: the caller's arguments, passed through.
    let result = unsafe { __real_fcntl(fd, cmd, arg) };
    let refused = matches!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EINVAL | libc::EACCES)
    );
    if result >= 0 || !refused {
        return result;
    }
    match cmd {
        libc::F_GETFD | libc::F_SETFD => 0,
        libc::F_DUPFD | libc::F_DUPFD_CLOEXEC => duplicate(fd),
        libc::F_SETFL => set_nbio(fd, arg as c_int & libc::O_NONBLOCK != 0),
        libc::F_GETFL => {
            let mut on: c_int = 0;
            let mut len = size_of::<c_int>() as libc::socklen_t;
            // SAFETY: `on` and `len` are writable and `len` holds the size of `on`.
            let rc = unsafe { libc::getsockopt(fd, libc::SOL_SOCKET, SO_NBIO, (&raw mut on).cast(), &mut len) };
            if rc < 0 {
                return -1;
            }
            libc::O_RDWR | if on != 0 { libc::O_NONBLOCK } else { 0 }
        }
        _ => result,
    }
}

/// std's `set_nonblocking` is `ioctl(FIONBIO)`, which the console refuses on sockets (EACCES):
/// quinn-udp's sockets and mdns-sd's failed with it. A refused FIONBIO goes to `SO_NBIO`.
/// Bound with `--wrap=ioctl`.
///
/// # Safety
/// As `ioctl`.
#[cfg(target_vendor = "sie")]
#[no_mangle]
pub unsafe extern "C" fn __wrap_ioctl(fd: c_int, request: core::ffi::c_ulong, arg: *mut c_void) -> c_int {
    // SAFETY: the caller's arguments, passed through.
    let result = unsafe { __real_ioctl(fd, request, arg) };
    if result == 0 || request != libc::FIONBIO || arg.is_null() {
        return result;
    }
    // SAFETY: FIONBIO's argument is the caller's int.
    set_nbio(fd, unsafe { *arg.cast::<c_int>() } != 0)
}

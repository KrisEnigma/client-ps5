//! libSceNet: the console's socket API. Used by the probe to compare against BSD sockets; the
//! client reaches the network through std until a probe says otherwise.

use core::ffi::{c_char, c_void};

/// `setsockopt(SOL_SOCKET, …)`: non-blocking I/O on an sceNet socket.
pub const SO_NBIO: i32 = 0x1200;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SceNetEpollEvent {
    pub events: u32,
    pub reserved: u32,
    pub ident: u64,
    pub data: u64,
}

extern "C" {
    /// `type_` is the bare type: no `SOCK_NONBLOCK` / `SOCK_CLOEXEC`.
    pub fn sceNetSocket(name: *const c_char, domain: i32, type_: i32, protocol: i32) -> i32;
    pub fn sceNetSocketClose(fd: i32) -> i32;
    pub fn sceNetBind(fd: i32, addr: *const c_void, len: u32) -> i32;
    pub fn sceNetSendto(
        fd: i32,
        buf: *const c_void,
        len: usize,
        flags: i32,
        addr: *const c_void,
        addr_len: u32,
    ) -> isize;
    pub fn sceNetRecvfrom(
        fd: i32,
        buf: *mut c_void,
        len: usize,
        flags: i32,
        addr: *mut c_void,
        addr_len: *mut u32,
    ) -> isize;
    pub fn sceNetErrnoLoc() -> *mut i32;
}

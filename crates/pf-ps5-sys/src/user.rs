//! libSceUserService: who is signed in. A pad belongs to a user; an unassigned pad is invisible.

use core::ffi::c_void;

pub const USER_ID_SYSTEM: i32 = 0xFF;
pub const USER_ID_INVALID: i32 = -1;
/// `sceUserServiceInitialize` was already called in this process: fine.
pub const ERROR_ALREADY_INITIALIZED: i32 = 0x8096_0003_u32 as i32;

/// Four slots; an empty one reads `-1`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SceUserServiceLoginUserIdList {
    pub user_id: [i32; 4],
}

impl Default for SceUserServiceLoginUserIdList {
    fn default() -> Self {
        Self {
            user_id: [USER_ID_INVALID; 4],
        }
    }
}

extern "C" {
    pub fn sceUserServiceInitialize(params: *const c_void) -> i32;
    pub fn sceUserServiceGetInitialUser(user_id: *mut i32) -> i32;
    pub fn sceUserServiceGetLoginUserIdList(list: *mut SceUserServiceLoginUserIdList) -> i32;
}

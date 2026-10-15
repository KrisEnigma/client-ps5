//! libSceSystemService: the splash, and the one correct way out of a title.

use core::ffi::c_char;

extern "C" {
    /// Before the first present: the shell's splash covers the title until then.
    pub fn sceSystemServiceHideSplashScreen() -> i32;
    /// `("exit", null)` returns the console to the home screen and the shell ends the process
    /// asynchronously. Returning from `main` or calling `exit` is recorded as a crash.
    pub fn sceSystemServiceLoadExec(path: *const c_char, argv: *const *const c_char) -> i32;
}

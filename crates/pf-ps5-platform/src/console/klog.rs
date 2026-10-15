//! The kernel log (TCP 3232) as a `Write`: one `sceKernelDebugOutText` per line, long lines cut.

use std::io::Write;

/// The longest record one call carries; longer lines are split.
const RECORD: usize = 384;

#[derive(Default)]
pub struct Klog {
    line: Vec<u8>,
}

impl Klog {
    fn emit(&mut self) {
        for chunk in self.line.chunks(RECORD) {
            let mut c = Vec::with_capacity(chunk.len() + 2);
            c.extend(chunk.iter().copied().filter(|&b| b != 0));
            c.extend_from_slice(b"\n\0");
            // SAFETY: `c` is NUL-terminated and outlives the call.
            unsafe { pf_ps5_sys::kernel::sceKernelDebugOutText(0, c.as_ptr().cast()) };
        }
        self.line.clear();
    }
}

impl Write for Klog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for &b in buf {
            if b == b'\n' {
                self.emit();
            } else {
                self.line.push(b);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.line.is_empty() {
            self.emit();
        }
        Ok(())
    }
}

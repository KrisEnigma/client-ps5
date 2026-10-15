//! AudioOut: a MAIN port at 48 kHz, S16 (the format console titles have proven), one 256-frame
//! grain per blocking write.

use anyhow::{bail, Result};
use pf_ps5_app::platform::{AudioOut, AudioPort};
use pf_ps5_sys::audio::*;
use pf_ps5_sys::user::USER_ID_SYSTEM;

pub struct Ps5Audio;

impl Ps5Audio {
    pub fn new() -> Result<Self> {
        // SAFETY: no arguments.
        let rc = unsafe { sceAudioOutInit() };
        if rc != 0 && rc != ERROR_ALREADY_INIT {
            bail!("sceAudioOutInit: {}", pf_ps5_sys::sce_error(rc));
        }
        Ok(Self)
    }
}

impl AudioOut for Ps5Audio {
    fn open(&self, channels: u8, rate: u32) -> Result<Box<dyn AudioPort>> {
        let format = match channels {
            2 => FORMAT_S16_STEREO,
            8 => FORMAT_S16_8CH,
            n => bail!("AudioOut takes stereo or 8 channels, not {n}"),
        };
        // SAFETY: plain integer arguments.
        let handle = unsafe { sceAudioOutOpen(USER_ID_SYSTEM, PORT_MAIN, 0, GRAIN_FRAMES, rate, format) };
        if handle < 1 {
            bail!("sceAudioOutOpen: {}", pf_ps5_sys::sce_error(handle));
        }
        tracing::info!("audio: port {handle}, {channels} ch S16 @ {rate} Hz, grain {GRAIN_FRAMES}");
        Ok(Box::new(Port {
            handle,
            channels: usize::from(channels),
            pcm: vec![0; GRAIN_FRAMES as usize * usize::from(channels)],
        }))
    }
}

struct Port {
    handle: i32,
    channels: usize,
    pcm: Vec<i16>,
}

impl AudioPort for Port {
    fn grain_frames(&self) -> usize {
        GRAIN_FRAMES as usize
    }

    fn write(&mut self, pcm: &[f32]) -> Result<()> {
        if pcm.len() != self.pcm.len() {
            bail!("grain of {} samples, port takes {}", pcm.len(), self.pcm.len());
        }
        for (dst, src) in self.pcm.iter_mut().zip(pcm) {
            *dst = (src.clamp(-1.0, 1.0) * 32767.0) as i16;
        }
        // SAFETY: `pcm` holds exactly one grain at the opened width and outlives the call.
        let rc = unsafe { sceAudioOutOutput(self.handle, self.pcm.as_ptr().cast()) };
        if rc < 0 {
            bail!("sceAudioOutOutput: {}", pf_ps5_sys::sce_error(rc));
        }
        let _ = self.channels;
        Ok(())
    }
}

impl Drop for Port {
    fn drop(&mut self) {
        // SAFETY: draining and closing a handle this port opened.
        unsafe {
            sceAudioOutOutput(self.handle, std::ptr::null());
            sceAudioOutClose(self.handle);
        }
    }
}

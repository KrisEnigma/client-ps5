//! Assembling one session's media pipeline: the decoder the platform opens for the negotiated
//! stream, the audio port, and the threads that feed them. The pumps and stages above are
//! written against `core::media`'s traits and never learn which lane they got.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use punktfunk_core::client::NativeClient;

use crate::core::media::{AudioSink, VideoSink};
use crate::platform::{AudioOut, StreamSpec, VideoDecoders};
use crate::services::join::{join_with_timeout, SHUTDOWN_JOIN_TIMEOUT};
use crate::session::audio::AudioStage;
use crate::session::audio_out::AudioOutSink;
use crate::session::connect::ConnectParams;
use crate::session::pump::{spawn_audio_feed, video_pump};
use crate::session::stage::{SinkConfig, VideoStage};
use crate::session::StreamStats;

/// The platform's media halves a session builds on.
#[derive(Clone)]
pub struct Media {
    pub video: Arc<dyn VideoDecoders>,
    pub audio: Arc<dyn AudioOut>,
}

/// The threads driving one session. Dropping this does NOT stop them — the session's `stop`
/// flag does, and [`Self::join`] waits them out.
pub struct MediaPipeline {
    video_thread: std::thread::JoinHandle<()>,
    audio_thread: std::thread::JoinHandle<()>,
}

impl MediaPipeline {
    /// Open the decoder and the audio port, and start both pumps. A thread already started is
    /// stopped and joined before an error returns.
    pub fn build(
        params: &ConnectParams,
        media: &Media,
        client: &Arc<NativeClient>,
        stop: &Arc<AtomicBool>,
        stats: &Arc<StreamStats>,
    ) -> Result<Self> {
        let mode = client.mode();
        let spec = StreamSpec {
            codec: client.codec,
            width: mode.width,
            height: mode.height,
            refresh_hz: mode.refresh_hz,
            color: client.color,
            bit_depth: client.bit_depth,
            chroma444: client.chroma_format == punktfunk_core::quic::CHROMA_IDC_444,
            shard_payload: client.shard_payload as usize,
            hud: client.hud_shared(),
        };
        let sink = media.video.open(&spec).context("open the video decoder")?;
        tracing::info!(
            "{} opened (codec 0x{:02x} {}x{}@{}fps, colour {:?})",
            sink.name(),
            spec.codec,
            spec.width,
            spec.height,
            spec.refresh_hz,
            spec.color,
        );
        let port = media
            .audio
            .open(client.audio_channels, crate::session::audio::SAMPLE_RATE)
            .context("open the audio port")?;
        let audio_sink: Arc<dyn AudioSink> = Arc::new(AudioOutSink::start(port, client.audio_channels)?);
        let stage = AudioStage::new(audio_sink, client.audio_channels, client.audio_layout)?;
        let video_thread = spawn_video_thread(client, sink, stop, stats, params.present_priority)?;
        let audio_thread = match spawn_audio_feed(client.clone(), stage, stop.clone()) {
            Ok(handle) => handle,
            Err(e) => {
                stop.store(true, Ordering::Relaxed);
                join_thread(video_thread, "video");
                return Err(e);
            }
        };
        Ok(Self {
            video_thread,
            audio_thread,
        })
    }

    /// Waits the threads out, bounded. `false` means one is still running.
    pub fn join(self) -> bool {
        join_thread(self.video_thread, "video") & join_thread(self.audio_thread, "audio")
    }
}

fn join_thread(handle: std::thread::JoinHandle<()>, what: &'static str) -> bool {
    join_with_timeout(handle, SHUTDOWN_JOIN_TIMEOUT, what, || ())
}

fn spawn_video_thread(
    client: &Arc<NativeClient>,
    sink: Box<dyn VideoSink>,
    stop: &Arc<AtomicBool>,
    stats: &Arc<StreamStats>,
    present_priority: pf_client_core::trust::PresentPriority,
) -> Result<std::thread::JoinHandle<()>> {
    let cfg = SinkConfig {
        stream_hz: client.mode().refresh_hz,
        report_decode_latency: client.wants_decode_latency(),
        present_priority,
    };
    // HDR scan-out does not exist yet, so host mastering metadata has nowhere to go.
    let is_hdr = false;
    let (client, stop, stats) = (client.clone(), stop.clone(), stats.clone());
    std::thread::Builder::new()
        .name("pf-video-pump".into())
        .spawn(move || {
            let stage = VideoStage::new(sink, stats.clone(), &cfg);
            video_pump(client, stage, stop, stats, is_hdr);
        })
        .context("spawn video thread")
}

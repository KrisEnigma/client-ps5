//! WP0's probe: one numbered line per check, `probe NN ok|fail|skip <name>: <detail>`.
//!
//! Each check proves one thing the client assumes about the console, in the order the client
//! leans on them: memory, threads, sync, clocks, files, entropy, sockets, TLS, discovery, an async
//! runtime, then a headless session against a real host. A check prints its `start` line first,
//! so a probe that dies (the console aborts on a refused syscall) names the check it died in.
//!
//! Runs the same on a developer box, where every line should read `ok`.

use std::collections::HashMap;
use std::io::{IoSliceMut, Write};
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

/// What the tester's `probe.json` says about their host. Every field optional: without a host
/// the network-to-host checks print `skip`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// A punktfunk host's address (no hostnames: the console has no usable resolver).
    pub host: Option<String>,
    pub port: Option<u16>,
    pub mgmt_port: Option<u16>,
    /// The host's pairing PIN, armed on its Pairing page, for the first run.
    pub pin: Option<String>,
    /// Frames the headless session waits for.
    pub frames: Option<u32>,
    /// End with `std::process::abort` after the log is written, to prove the crash path.
    pub abort: bool,
}

/// Where a check's line goes: the log file, stdout, and the platform's own log.
pub struct Out {
    file: std::fs::File,
    extra: Option<Box<dyn Write + Send>>,
    pub failed: u32,
}

impl Out {
    pub fn new(dir: &Path, extra: Option<Box<dyn Write + Send>>) -> Result<Self> {
        let path = dir.join("probe.log");
        let file = std::fs::File::create(&path).with_context(|| format!("create {}", path.display()))?;
        Ok(Self { file, extra, failed: 0 })
    }

    pub fn line(&mut self, text: &str) {
        let line = format!("{text}\n");
        let _ = self.file.write_all(line.as_bytes());
        let _ = self.file.sync_data();
        print!("{line}");
        if let Some(extra) = self.extra.as_mut() {
            let _ = extra.write_all(line.as_bytes());
            let _ = extra.flush();
        }
    }
}

type Check = fn(&Ctx) -> Result<Outcome>;

/// What one check reports.
pub enum Outcome {
    Ok(String),
    Skip(String),
}

/// What a check may use.
pub struct Ctx {
    pub dir: PathBuf,
    pub config: Config,
}

const CHECKS: &[(&str, Check)] = &[
    ("alloc", alloc),
    ("threads", threads),
    ("thread-locals", thread_locals),
    ("mutex-condvar", mutex_condvar),
    ("clocks", clocks),
    ("parallelism", parallelism),
    ("files", files),
    ("entropy", entropy),
    ("hashmap-seed", hashmap_seed),
    ("udp-quinn", udp_quinn),
    ("udp-rate", udp_rate),
    ("tokio", tokio_runtime),
    ("tcp-mgmt", tcp_mgmt),
    ("quic-tls", quic_tls),
    ("mdns", mdns),
    ("session", session),
];

/// Runs every check in order. Returns how many failed.
pub fn run(ctx: &Ctx, out: &mut Out) -> u32 {
    out.line(&format!(
        "probe start {} host={:?}",
        pf_ps5_app::core::build_id(),
        ctx.config.host
    ));
    for (i, (name, check)) in CHECKS.iter().enumerate() {
        let n = i + 1;
        out.line(&format!("probe {n:02} start {name}"));
        let started = Instant::now();
        match check(ctx) {
            Ok(Outcome::Ok(detail)) => out.line(&format!(
                "probe {n:02} ok {name}: {detail} ({} ms)",
                started.elapsed().as_millis()
            )),
            Ok(Outcome::Skip(why)) => out.line(&format!("probe {n:02} skip {name}: {why}")),
            Err(e) => {
                out.failed += 1;
                out.line(&format!("probe {n:02} fail {name}: {e:#}"));
            }
        }
    }
    out.line(&format!("probe end: {} failed", out.failed));
    if ctx.config.abort {
        out.line("probe abort: calling std::process::abort after this line");
        std::process::abort();
    }
    out.failed
}

/// Reads `probe.json` from the data directory; absent reads as no host.
pub fn config(dir: &Path) -> Config {
    std::fs::read(dir.join("probe.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn ok(detail: impl Into<String>) -> Result<Outcome> {
    Ok(Outcome::Ok(detail.into()))
}

fn alloc(_: &Ctx) -> Result<Outcome> {
    let mut sizes = Vec::new();
    for shift in [10u32, 16, 20, 24, 28] {
        let len = 1usize << shift;
        let mut v = vec![0u8; len];
        v[len - 1] = 1;
        sizes.push(format!("{}K", len >> 10));
        drop(v);
    }
    // Many small ones, as a parser makes.
    let many: Vec<Box<[u8; 64]>> = (0..100_000).map(|_| Box::new([0u8; 64])).collect();
    ok(format!("{} and 100k x 64 B ({})", sizes.join(" "), many.len()))
}

fn threads(_: &Ctx) -> Result<Outcome> {
    let handles: Vec<_> = (0..16)
        .map(|i| {
            std::thread::Builder::new()
                .name(format!("probe-{i}"))
                .stack_size(256 * 1024)
                .spawn(move || i * 2)
        })
        .collect::<std::io::Result<_>>()?;
    let sum: usize = handles.into_iter().map(|h| h.join().unwrap_or(0)).sum();
    if sum != (0..16).map(|i| i * 2).sum::<usize>() {
        bail!("joined sum {sum}");
    }
    ok("16 named threads spawned and joined")
}

fn thread_locals(_: &Ctx) -> Result<Outcome> {
    thread_local!(static CELL: std::cell::Cell<u64> = const { std::cell::Cell::new(0) });
    CELL.with(|c| c.set(7));
    let other = std::thread::spawn(|| CELL.with(std::cell::Cell::get))
        .join()
        .map_err(|_| anyhow!("thread panicked"))?;
    let here = CELL.with(std::cell::Cell::get);
    if (here, other) != (7, 0) {
        bail!("main {here}, other {other}");
    }
    ok("per-thread values kept apart")
}

fn mutex_condvar(_: &Ctx) -> Result<Outcome> {
    let shared = Arc::new((Mutex::new(0u64), Condvar::new()));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let s = shared.clone();
            std::thread::spawn(move || {
                for _ in 0..10_000 {
                    *s.0.lock().unwrap() += 1;
                }
                s.1.notify_all();
            })
        })
        .collect();
    let started = Instant::now();
    {
        let (lock, cv) = &*shared;
        let mut n = lock.lock().unwrap();
        while *n < 80_000 {
            let (g, timeout) = cv.wait_timeout(n, Duration::from_secs(10)).unwrap();
            n = g;
            if timeout.timed_out() {
                bail!("stuck at {}", *n);
            }
        }
    }
    for w in workers {
        let _ = w.join();
    }
    ok(format!(
        "80k contended increments in {} ms",
        started.elapsed().as_millis()
    ))
}

fn clocks(_: &Ctx) -> Result<Outcome> {
    let a = Instant::now();
    std::thread::sleep(Duration::from_millis(20));
    let slept = a.elapsed();
    let wall = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
    if slept < Duration::from_millis(19) || slept > Duration::from_millis(200) {
        bail!("a 20 ms sleep took {slept:?}");
    }
    ok(format!("sleep 20 ms = {slept:?}, wall {}s", wall.as_secs()))
}

fn parallelism(_: &Ctx) -> Result<Outcome> {
    let n = std::thread::available_parallelism()?;
    ok(format!("{n} threads available"))
}

fn files(ctx: &Ctx) -> Result<Outcome> {
    let marker = ctx.dir.join("probe-marker.txt");
    let previous = std::fs::read_to_string(&marker).ok();
    let tmp = ctx.dir.join("probe-marker.tmp");
    let stamp = format!("{:?}", std::time::SystemTime::now());
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(stamp.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, &marker)?;
    let back = std::fs::read_to_string(&marker)?;
    if back != stamp {
        bail!("read back {back:?}");
    }
    ok(match previous {
        Some(p) => format!("rewrote the marker; the last launch left {p}"),
        None => "wrote the marker; relaunch to see it survive".into(),
    })
}

fn entropy(_: &Ctx) -> Result<Outcome> {
    let mut a = [0u8; 32];
    let filled = pf_ps5_sys::shim::fill_random(&mut a);
    let sysctl = filled.map_or_else(
        |rc| format!("kern.rng_pseudo failed ({rc})"),
        |()| "kern.rng_pseudo ok".into(),
    );
    let mut b = [0u8; 32];
    let g02 = getrandom02::getrandom(&mut b).map_or_else(
        |e| format!("getrandom 0.2 failed ({e})"),
        |()| "getrandom 0.2 ok".into(),
    );
    // The client's source is the sysctl (aws-lc's getentropy, std's getrandom); a buffer it
    // left partly written is still a failure.
    if filled.is_err() || a == [0; 32] {
        bail!("{sysctl}, {g02}");
    }
    ok(format!("{sysctl}, {g02}"))
}

fn hashmap_seed(_: &Ctx) -> Result<Outcome> {
    let mut m = HashMap::new();
    m.insert("a", 1);
    ok(format!("RandomState seeded ({} entry)", m.len()))
}

/// UDP through quinn-udp, as core's media reader uses it: state, options, a batch round trip.
fn udp_quinn(_: &Ctx) -> Result<Outcome> {
    let rx = UdpSocket::bind("127.0.0.1:0")?;
    let tx = UdpSocket::bind("127.0.0.1:0")?;
    let state = quinn_udp::UdpSocketState::new((&rx).into()).context("UdpSocketState::new")?;
    let to = rx.local_addr()?;
    let sender = quinn_udp::UdpSocketState::new((&tx).into())?;
    for i in 0..8u8 {
        sender.try_send(
            (&tx).into(),
            &quinn_udp::Transmit {
                destination: to,
                ecn: None,
                contents: &[i; 1200],
                segment_size: None,
                src_ip: None,
            },
        )?;
    }
    rx.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut storage = vec![[0u8; 1500]; quinn_udp::BATCH_SIZE];
    let mut bufs: Vec<IoSliceMut<'_>> = storage.iter_mut().map(|b| IoSliceMut::new(b)).collect();
    let mut meta = vec![quinn_udp::RecvMeta::default(); quinn_udp::BATCH_SIZE];
    let got = state.recv((&rx).into(), &mut bufs, &mut meta).context("recv")?;
    ok(format!(
        "batch {} got {got}, gro {}, gso {}, may_fragment {}, rcvbuf {:?}",
        quinn_udp::BATCH_SIZE,
        state.gro_segments(),
        state.max_gso_segments(),
        state.may_fragment(),
        state.recv_buffer_size((&rx).into()).ok(),
    ))
}

/// Datagrams per receive call and the cost per datagram at 10k/s over loopback: R7.
fn udp_rate(_: &Ctx) -> Result<Outcome> {
    let rx = UdpSocket::bind("127.0.0.1:0")?;
    let to = rx.local_addr()?;
    let state = quinn_udp::UdpSocketState::new((&rx).into())?;
    let _ = state.set_recv_buffer_size((&rx).into(), 8 << 20);
    rx.set_read_timeout(Some(Duration::from_millis(50)))?;
    let sent = Arc::new(AtomicU64::new(0));
    let s2 = sent.clone();
    let sender = std::thread::spawn(move || -> Result<()> {
        let tx = UdpSocket::bind("127.0.0.1:0")?;
        let payload = [0u8; 1200];
        let start = Instant::now();
        for i in 0..10_000u64 {
            let due = start + Duration::from_micros(i * 100);
            if let Some(wait) = due.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            tx.send_to(&payload, to)?;
            s2.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    });
    let mut storage = vec![[0u8; 1500]; quinn_udp::BATCH_SIZE];
    let mut meta = vec![quinn_udp::RecvMeta::default(); quinn_udp::BATCH_SIZE];
    let (mut got, mut calls, mut busy) = (0u64, 0u64, Duration::ZERO);
    let deadline = Instant::now() + Duration::from_secs(3);
    while got < 10_000 && Instant::now() < deadline {
        let mut bufs: Vec<IoSliceMut<'_>> = storage.iter_mut().map(|b| IoSliceMut::new(b)).collect();
        let t = Instant::now();
        if let Ok(n) = state.recv((&rx).into(), &mut bufs, &mut meta) {
            busy += t.elapsed();
            calls += 1;
            got += n as u64;
        }
    }
    sender.join().map_err(|_| anyhow!("sender panicked"))??;
    let per = busy.as_nanos() as u64 / got.max(1);
    ok(format!(
        "{got}/{} received in {calls} calls ({:.1}/call), {per} ns per datagram",
        sent.load(Ordering::Relaxed),
        got as f64 / calls.max(1) as f64
    ))
}

fn tokio_runtime(_: &Ctx) -> Result<Outcome> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let sum = rt.block_on(async {
        let tasks: Vec<_> = (0..64u64)
            .map(|i| {
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    i
                })
            })
            .collect();
        let mut sum = 0;
        for t in tasks {
            sum += t.await.unwrap_or(0);
        }
        sum
    });
    if sum != (0..64).sum::<u64>() {
        bail!("tasks summed to {sum}");
    }
    ok("64 timers on 2 workers")
}

fn host(ctx: &Ctx) -> Option<(&str, u16)> {
    ctx.config
        .host
        .as_deref()
        .map(|h| (h, ctx.config.port.unwrap_or(47_989)))
}

fn tcp_mgmt(ctx: &Ctx) -> Result<Outcome> {
    let Some((h, _)) = host(ctx) else {
        return Ok(Outcome::Skip("no host in probe.json".into()));
    };
    let mgmt = ctx
        .config
        .mgmt_port
        .unwrap_or(pf_ps5_app::services::library::DEFAULT_MGMT_PORT);
    let addr: SocketAddr = format!("{h}:{mgmt}").parse()?;
    let t = Instant::now();
    std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(3))?;
    ok(format!("connected to {addr} in {} ms", t.elapsed().as_millis()))
}

fn quic_tls(ctx: &Ctx) -> Result<Outcome> {
    let Some((h, port)) = host(ctx) else {
        return Ok(Outcome::Skip("no host in probe.json".into()));
    };
    let fp = punktfunk_core::client::NativeClient::probe_identity(h, port, Duration::from_secs(5))
        .ok_or_else(|| anyhow!("no QUIC/TLS answer from {h}:{port}"))?;
    ok(format!("host fingerprint {:02x}{:02x}{:02x}…", fp[0], fp[1], fp[2]))
}

fn mdns(_: &Ctx) -> Result<Outcome> {
    let Some(mut d) = pf_ps5_app::services::discovery::Discovery::start() else {
        bail!("the mDNS daemon did not start");
    };
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        for h in d.poll() {
            seen.push(format!("{}@{}", h.name, h.addr));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    ok(format!("{} host(s): {}", seen.len(), seen.join(", ")))
}

/// Pairs on the first run (with the PIN), reconnects with the saved pin afterwards, receives
/// frames at 1080p60 and sends input: the headless client.
fn session(ctx: &Ctx) -> Result<Outcome> {
    let Some((h, port)) = host(ctx) else {
        return Ok(Outcome::Skip("no host in probe.json".into()));
    };
    let identity = pf_ps5_app::services::store::load_or_create_identity()?;
    let pin_file = ctx.dir.join("probe-host-pin.hex");
    let saved = std::fs::read_to_string(&pin_file)
        .ok()
        .and_then(|s| pf_client_hex(s.trim()));
    let (pin, how) = match (saved, ctx.config.pin.as_deref()) {
        (Some(pin), _) => (pin, "saved pairing"),
        (None, Some(code)) => {
            let fp = punktfunk_core::client::NativeClient::pair(
                h,
                port,
                (&identity.0, &identity.1),
                code,
                "PS5 probe",
                Duration::from_secs(15),
            )?;
            let hex = fp.iter().fold(String::with_capacity(64), |mut s, b| {
                use std::fmt::Write as _;
                let _ = write!(s, "{b:02x}");
                s
            });
            std::fs::write(&pin_file, hex)?;
            (fp, "paired now")
        }
        (None, None) => return Ok(Outcome::Skip("not paired and no pin in probe.json".into())),
    };
    let mode = punktfunk_core::config::Mode {
        width: 1920,
        height: 1080,
        refresh_hz: 60,
    };
    let client = punktfunk_core::client::NativeClient::connect(punktfunk_core::client::ConnectParams {
        pin: Some(pin),
        identity: Some(identity),
        video_codecs: punktfunk_core::quic::CODEC_HEVC | punktfunk_core::quic::CODEC_H264,
        ..punktfunk_core::client::ConnectParams::new(h, port, mode, Duration::from_secs(10))
    })?;
    let want = ctx.config.frames.unwrap_or(600);
    let (mut frames, mut bytes) = (0u32, 0usize);
    let started = Instant::now();
    let deadline = started + Duration::from_secs(30);
    while frames < want && Instant::now() < deadline {
        match client.next_frame(Duration::from_millis(100)) {
            Ok(f) => {
                frames += 1;
                bytes += f.data.len();
            }
            Err(punktfunk_core::PunktfunkError::NoFrame) => {}
            Err(e) => bail!("after {frames} frames: {e}"),
        }
        if frames == 60 {
            let _ = client.send_input(&pf_ps5_app::runtime::pads::bit_event(
                punktfunk_core::input::gamepad::BTN_A,
                true,
                0,
            ));
            let _ = client.send_input(&pf_ps5_app::runtime::pads::bit_event(
                punktfunk_core::input::gamepad::BTN_A,
                false,
                0,
            ));
        }
    }
    let secs = started.elapsed().as_secs_f64();
    client.disconnect_quit();
    if frames < want {
        bail!("{frames}/{want} frames in {secs:.1}s");
    }
    ok(format!(
        "{how}: {frames} frames, {:.1} fps, {:.1} Mbit/s, codec 0x{:02x}, input sent",
        f64::from(frames) / secs,
        bytes as f64 * 8.0 / secs / 1e6,
        client.codec
    ))
}

fn pf_client_hex(s: &str) -> Option<[u8; 32]> {
    let mut out = [0u8; 32];
    if s.len() != 64 {
        return None;
    }
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On a developer box every local check passes; the host checks skip without a probe.json.
    #[test]
    fn local_checks_pass_here() {
        let dir = std::env::temp_dir().join(format!("pf-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        pf_ps5_app::services::paths::install(&dir);
        let ctx = Ctx {
            dir: dir.clone(),
            config: Config::default(),
        };
        for (name, check) in CHECKS {
            if matches!(*name, "entropy" | "mdns") {
                continue; // kern.rng_pseudo is the console's; mDNS needs a LAN.
            }
            match check(&ctx) {
                Ok(_) => {}
                Err(e) => panic!("{name}: {e:#}"),
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}

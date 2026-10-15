# Notes

What this client knows about the console, where each fact comes from, and what still needs a
console run to prove. Plan of record: `plan.md`. A claim proven on the console cites its bundle
id; until the first bundle, every console fact below is **sourced, not proven**.

## Where the code stands (2026-10-15)

| WP | Written | Proven without a console | Needs a console run for |
| --- | --- | --- | --- |
| 0 | probe (16 checks), target JSON, toolchain + link scripts, FFI with layout tests | probe passes on macOS and Linux; layouts pinned | every probe line on 13.60, both launch forms |
| 1 | device, `VK_KHR_display` output, presenter, read-back, PyroWave lane | presenter renders the console on lavapipe (CI artifact) | swapchain at 119.88 Hz, pacing record, PyroWave feature line |
| 2 | console hosting, service loop, store, pairing, library, art | fakes and the host platform; menu test run produces a PNG | Skia's Vulkan backend on RADV (R4) |
| 3 | stage/timeline/pump (lifted), Videodec2 lane, PyroWave lane | stage tests incl. intra-only anchors | Videodec2 decode, the import step taken, 4K120 budgets |
| 4 | stream loop, input thread, dial, chord, rumble/light/triggers, AudioOut, reconnect | pad routing, trigger translation, audio ring | input feel, triggers, audio drift |

Not built yet: anything in plan §9 (HDR, surround, haptics, keyboard/mouse, VRR), system-event
polling (layout unproven, below), a hostname resolver (addresses only).

## Deviations from the plan, and why

- **The app does not depend on `pf-ps5-vk`.** `pf-client-video` compiles to nothing on macOS, so
  an app built on the Vulkan crate could not be tested on the Mac. The `Display` and
  `VideoDecoders` seams carry what the app needs; the platform crates own the Vulkan plane.
- **The title is a static library linked by `scripts/link-title.sh`,** not a binary linked through
  the target JSON's `linker`. RADV titles link with a raw `prospero-lld` call (CRT objects,
  whole-archive RADV, wraps, defsyms, version scripts); the script reuses PS5_Vulkan's
  `radv_link_recipe` at a pinned revision instead of re-deriving it.
- **RADV comes from PS5_Vulkan's Mesa fork**, built by that repository's `build-radv.sh`, not from
  the pacbrew package: it is the tree the 1.4 CTS ran on, and it ships the link recipe that goes
  with it.
- **Videodec2 lands through a buffer import and one GPU copy** (plan §3.3 step 2), with a staging
  copy (step 3) where the import is refused. No repository imports decoder memory as an image.
- **The dial opens on Touchpad + Cross, and the escape chord is L1+R1+Options+Touchpad.** Create
  is system-owned on the console (bit 0x1, see below).
- **Pacing is newest-picture-at-vblank** under FIFO. `CadenceClock`/`pace_slot` phase pacing waits
  for the first pacing record to show whether it is needed.
- **Audio is S16** through AudioOut: float formats appear only in the SDL port's code.

## Upstream changes (plan §7)

Merged into monorepo main (#1966–#1968); `Cargo.toml` pins `770504dbc`, which carries all three:

- #1966: `Platform::Ps5` and its rows; `shortcuts::Client::Ps5`.
- #1967: `pf-client-core` (portable half, discovery) and `pf-console-ui` build for
  `all(target_os = "freebsd", target_vendor = "sie")`.
- #1968: `pyrowave-sys` and `pf-client-video`'s PyroWave lane on that target.

Not started: the PS5 Skia archive on `skia-binaries` (§7.4, `task skia` builds it), the install
page entry (§7.5, at WP6). §7.6 (a `ring` provider) only if aws-lc-sys will not cross-build.

## Console facts the code relies on (sourced, not yet proven)

Sources: ProsperoLight (PL), PS5_Vulkan (VK), PS5_VulkanTemplate (TPL), the payload SDK (SDK),
OpenNOW-PS5 (ONW). Section numbers refer to the extraction kept with the plan's research.

- **Exit:** `exit()` and returning from `main` are recorded as crashes. The only clean end is
  `sceSystemServiceLoadExec("exit", NULL)` then sleeping until the shell ends the process
  (TPL platform-contracts, PL main.cpp, VK DEPLOYMENT).
- **Splash:** `sceSystemServiceHideSplashScreen` before the first present (TPL, VK).
- **RADV entry:** no loader; `vk_icdGetInstanceProcAddr` from the archive (VK radv_smoke.c).
  One display "PS5 VideoOut", modes largest first with each 119.88 Hz mode before its 59.94 Hz
  one; 119.88 only when `param.json` `attribute3` declares HFR (0x40; VK/TPL ship 0x80040).
- **Videodec2:** title process only (payload contexts fail), `sceSysmoduleLoadModule(207)` first,
  memory type 12 in 16 KiB pages, six-deep DPB for five-reference hosts, H.264 needs `Flush`
  after each `Decode` at depth 1. Output is LINEAR NV12, or two-plane 16-bit with the 10 bits
  LOW-aligned for Main10 (not P010). On 13.60 it fails `811d0111` once a process leaves its
  sandbox (PL TROUBLESHOOTING). 4K120 HEVC holds to about 80 Mbit/s (PL PERFORMANCE_GUIDE).
- **Pads:** one handle per signed-in user, 120-byte state; Create (0x1) and PS (0x10000) are the
  system's; bit 0x80000000 marks a sample the shell intercepted. Trigger effects take Sony's
  high-level modes, 120-byte parameter (PL ps5_dualsense, duaLib).
- **AudioOut:** user 0xFF, MAIN port, 48 kHz, grains of 256..2048 frames in steps of 256,
  `sceAudioOutOutput` blocks one grain; S16 stereo and 8-channel are hardware-verified.
- **klog:** `sceKernelDebugOutText(0, line)`; a title's stderr does not reach it.
- **libc traps for std:** `arc4random*` resolve to nothing in a title (bound to the SDK fork's
  `ps5_` versions by the link recipe); `pthread_set_name_np` from inside the new thread hangs it
  (bound to a no-op here); malloc is wrapped onto the platform heap; TLS is emulated
  (`has-thread-local: false` in the target); `kern.rng_pseudo` is the proven entropy sysctl.
- **Packaging:** `/data/homebrew/<TITLE_ID>/` with `eboot.bin` (fake-signed SELF from
  `ps5-native-tool`), `sce_module/libc.prx`, `sce_sys/param.json`, `sce_sys/icon0.png` (512²).
  `/download0` exists when `downloadDataSize` > 0. Files FTP must read are 0666/0777.
- **Ids in use elsewhere:** PPSA99002 (ProsperoLight), 99008, 99010, 99014, 99015, 99130, 99169,
  99988, 99996–99999. This client is PPSA99724 and its probe PPSA99725: no public repository
  used either on 2026-10-15.

## Open questions the probe answers first

1. **BSD sockets in a title.** ProsperoLight routes every socket through libSceNet
   (`sceNetSocket`, `sceNetEpoll*`) and never uses BSD fds; std, mio and quinn-udp use BSD fds.
   Probe lines `udp-quinn`, `udp-rate`, `tcp-mgmt`, `quic-tls`, `mdns` and `tokio` answer it. If
   they fail with refused syscalls, the shim is a libc-level redirect of socket calls onto sceNet
   in `pf-ps5-sys` (R1); `pf-ps5-sys::net` already declares the calls.
2. **`_umtx_op` for std's locks.** std's FreeBSD port parks threads with `_umtx_op`; PL and VK
   only prove pthread primitives. Probe `mutex-condvar`.
3. **`kqueue` under mio/tokio.** Unproven in a title; the console's own queues are
   `sceKernelCreateEqueue`. Probe `tokio`.
4. **getrandom 0.2's FreeBSD path** (`kern.arandom` or `getrandom(2)`), used by `rand_core` under
   curve25519. Probe `entropy` reports it beside `kern.rng_pseudo`.
5. **recvmmsg batch size** at 10k datagrams/s and the per-datagram cost with the loader's hooks
   (R7). Probe `udp-rate`.
6. **`dlopen` at link time.** Granite references `dlopen` even when handed a
   `GetInstanceProcAddr`; if the console's libkernel stub lacks it, the title link needs a
   `--defsym` to a stub returning null (WP1).
7. **System events.** `sceSystemServiceGetStatus`/`ReceiveEvent` layouts are emulator-derived;
   `Lifecycle::poll_events` returns nothing until a probe line proves them. Focus loss shows up
   as intercepted pad samples, which release held input.

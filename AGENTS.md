# AGENTS.md

PlayStation 5 homebrew client for [punktfunk](https://git.unom.io/unom/punktfunk): a Rust title
on Vulkan (RADV linked statically), the shared console UI, PyroWave and Videodec2. Plan of record:
`docs/plan.md`. Console facts and what is proven: `docs/NOTES.md`.

## Layout (dependencies point down)

| Crate | What | `unsafe` |
| --- | --- | --- |
| `pf-ps5-app` | session, console hosting, services, run loop, **seams** (`platform/`) + fakes | forbidden |
| `pf-ps5-vk` | device, presenter, UI layer, PyroWave lane (Linux + console only) | with `// SAFETY:` |
| `pf-ps5-sys` | hand-written console FFI, layout tests, libc shims | with `// SAFETY:` |
| `pf-ps5-platform` | the console's seam implementations (pure maps build anywhere) | via sys/vk |
| `pf-ps5-host` | the seams on a dev box (Vulkan offscreen on Linux, CPU raster on macOS) | none |
| `pf-ps5-probe` | WP0's numbered checks | none |
| `pf-ps5-title` | the console's `main`, a static library | via sys |

The app never names a console API or a Vulkan handle; the binary hands it a `Platform`.

## Commands

`task --list`. `task test` / `task lint` run here; `task docker:*` the same in the toolchain image.
`task cross-check` type-checks the console code a host check never compiles. `task package` builds
`dist/<TITLE_ID>/`; `task deploy`, `task test-run -- menu`, `task fetch-bundle` drive the console.

## Invariants

- The title never returns from `main` and never calls `exit`: `Lifecycle::exit` loads `"exit"`.
- Videodec2 loads before anything else (`console::platform`), and nothing leaves the sandbox.
- Threads are never named from inside themselves on the console (`pf_ps5_pthread_set_name_np`).
- A console claim in `docs/NOTES.md` cites a bundle id. No workaround without a probe line.
- Writing rules are the monorepo's `docs/writing.md`: commits, comments, the two error registers.

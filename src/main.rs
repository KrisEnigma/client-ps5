//! The client on a developer box: the same app on the host platform.
//!
//! `PF_PS5_DATA` names the data directory (identity, settings, logs, bundles), `PF_PS5_SIZE`
//! the output (`1920x1080`), `PF_PS5_PADS` a scripted pad file. A `test-run.json` in the data
//! directory runs a scenario and leaves its bundle in `verdict/`. The console's entry is
//! `crates/pf-ps5-title`, linked by `scripts/link-title.sh`.

fn main() -> anyhow::Result<()> {
    let opts = pf_ps5_host::Options::from_env();
    let platform = pf_ps5_host::platform(&opts)?;
    pf_ps5_app::run(platform)
}

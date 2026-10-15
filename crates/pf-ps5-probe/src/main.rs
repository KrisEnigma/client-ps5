//! The probe on a developer box: `pf-ps5-probe [data-dir]`, reading `probe.json` there.

fn main() -> anyhow::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .map_or_else(|| std::env::temp_dir().join("pf-ps5-probe"), std::path::PathBuf::from);
    std::fs::create_dir_all(&dir)?;
    pf_ps5_app::services::paths::install(&dir);
    punktfunk_core::tls::install_default_provider();
    let ctx = pf_ps5_probe::Ctx {
        config: pf_ps5_probe::config(&dir),
        dir: dir.clone(),
    };
    let mut out = pf_ps5_probe::Out::new(&dir, None)?;
    let failed = pf_ps5_probe::run(&ctx, &mut out);
    std::process::exit(i32::from(failed > 0));
}

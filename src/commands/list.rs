use tracing::info;

use crate::{config::AppConfig, core::local};

/// Lists currently installed mods.
pub fn run(config: &AppConfig) -> anyhow::Result<()> {
    info!("scanning installed mods");
    let mods = local::scan_mods(&config.mods_dir())?;

    for installed in &mods {
        println!("{}", installed)
    }

    info!("found {} mods", mods.len());
    Ok(())
}

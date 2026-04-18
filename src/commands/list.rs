use tracing::info;

use crate::{config::AppConfig, core::local};

/// Lists currently installed mods.
pub fn run(config: &AppConfig) -> anyhow::Result<()> {
    info!("loading installed mods");
    let mods = local::scan_mods(&config.mods_dir())?;

    info!("listing installed mods");
    for installed in mods {
        println!("{}", installed)
    }

    Ok(())
}

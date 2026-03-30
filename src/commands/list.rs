use tracing::info;

use crate::{config::AppConfig, local_mods::LocalMod};

/// Lists currently installed mods.
pub fn run(config: &AppConfig) -> anyhow::Result<()> {
    info!("loading installed mods");
    let installed_mods = LocalMod::load_local_mods(config)?;

    info!("listing installed mods");
    for installed in installed_mods {
        println!("{}", installed)
    }

    Ok(())
}

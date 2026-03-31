use tracing::info;

use crate::{config::AppConfig, core::loader::ModLoader};

/// Lists currently installed mods.
pub fn run(config: &AppConfig) -> anyhow::Result<()> {
    info!("loading installed mods");
    let installed_mods = ModLoader::load_from_config(config)?;

    info!("listing installed mods");
    for installed in installed_mods {
        println!("{}", installed)
    }

    Ok(())
}

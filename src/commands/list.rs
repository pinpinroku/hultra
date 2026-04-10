use tracing::info;

use crate::{
    config::AppConfig,
    core::loader::{ModResolver, ModsDirectoryScanner},
};

/// Lists currently installed mods.
pub fn run(config: &AppConfig) -> anyhow::Result<()> {
    info!("loading installed mods");
    let paths = ModsDirectoryScanner::scan(&config.mods_dir())?;
    let mods = ModResolver::resolve_from_paths(&paths)?;

    info!("listing installed mods");
    for installed in mods {
        println!("{}", installed)
    }

    Ok(())
}

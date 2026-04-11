use tracing::info;

use crate::{config::AppConfig, core::loader::ModResolver, service::ModsDirectoryScanner};

/// Lists currently installed mods.
pub fn run(config: &AppConfig) -> anyhow::Result<()> {
    info!("loading installed mods");
    let files = ModsDirectoryScanner::scan(&config.mods_dir())?;
    let mods = ModResolver::resolve(&files)?;

    info!("listing installed mods");
    for installed in mods {
        println!("{}", installed)
    }

    Ok(())
}

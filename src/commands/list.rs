use tracing::info;

use crate::{
    config::AppConfig,
    core::{LocalFileSystemScanner, loader},
};

/// Lists currently installed mods.
pub fn run(config: &AppConfig) -> anyhow::Result<()> {
    info!("loading installed mods");
    let mods = loader::scan_mods(&LocalFileSystemScanner, &config.mods_dir())?;

    info!("listing installed mods");
    for installed in mods {
        println!("{}", installed)
    }

    Ok(())
}

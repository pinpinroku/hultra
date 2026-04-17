//! Handle update command.
use tracing::info;

use crate::{
    commands::DownloadOption,
    config::AppConfig,
    core::{
        LocalFileSystemScanner, cache, loader,
        network::{SharedHttpClient, api, downloader},
        update,
    },
    service::{LocalFileSystemService, fs::fetch_updater_blacklist},
};

/// Checks update for the mods and download the latest one if available.
pub async fn run(args: DownloadOption, config: &AppConfig) -> anyhow::Result<()> {
    info!("updating mods");

    info!("loading installed mods");
    let mut local_mods = loader::scan_mods(&LocalFileSystemScanner, &config.mods_dir())?;

    let initial_count = local_mods.len();
    info!("loaded {} mods", initial_count);

    info!("checking updater blacklist");
    let blacklist = fetch_updater_blacklist(&config.mods_dir())?;
    local_mods.retain(|local_mod| !local_mod.file().is_blacklisted(&blacklist));

    let ignored_count = initial_count - local_mods.len();
    if ignored_count > 0 {
        info!("{} mods were ignored due to blacklist", ignored_count);
    }
    if local_mods.is_empty() {
        println!("All mods are blacklisted")
    }

    info!("syncing file cache");
    let cache_db = cache::sync(config)?;

    // Initialize shared client
    let shared_client = SharedHttpClient::new();

    info!("fetching database...");
    let registry = api::fetch_registry(shared_client.inner().clone(), &args).await?;

    info!("checking updates");
    let contexts = registry.into_update_context(&local_mods, LocalFileSystemService);
    let report = update::scan_updates(&cache_db, &contexts)?;

    if report.updates.is_empty() {
        info!("all mods are up-to-date");
        return Ok(());
    } else {
        // send update info to stdout
        info!("available updates:");
        for update_info in report.updates {
            info!("{}", update_info);
        }
    }

    // Download updates
    info!("downloading mods");
    downloader::download_all(
        shared_client.inner().clone(),
        args,
        report.download_files,
        &config.mods_dir(),
    )
    .await?;

    info!("updating completed");
    Ok(())
}

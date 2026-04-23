//! Handle update command.
use tracing::info;

use crate::{
    commands::DownloadOption,
    config::AppConfig,
    core::{
        blacklist::{self, LocalUpdaterBlacklistSource},
        cache,
        local::{self, LocalFileSystemService, LocalModExt},
        network::{SharedHttpClient, api, downloader},
        update,
    },
};

/// Checks update for the mods and download the latest one if available.
pub async fn run(args: DownloadOption, config: &AppConfig) -> anyhow::Result<()> {
    let mods_dir = config.mods_dir();

    info!("scanning installed mods");
    let mut local_mods = local::scan_mods(&mods_dir)?;
    info!("found {} mods", local_mods.len());

    info!("checking updater's blacklist");
    let source = LocalUpdaterBlacklistSource::new(&mods_dir);
    let ublist = blacklist::fetch(&source)?;

    local_mods.apply_blacklist(&ublist)?;

    if local_mods.is_empty() {
        println!("All mods are blacklisted")
    }

    info!("syncing file cache");
    let cache_db = cache::sync(config)?;

    // Initialize shared client
    let shared_client = SharedHttpClient::new();

    info!("fetching database");
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
        &mods_dir,
    )
    .await?;

    info!("updating completed");
    Ok(())
}

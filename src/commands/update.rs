//! Handle update command.
use reqwest::Client;
use tracing::info;

use crate::{
    cache,
    commands::DownloadOption,
    config::AppConfig,
    core::{
        loader::ModLoader,
        network::{
            api::{ApiClient, ApiSource},
            downloader::ModDownloader,
        },
    },
    mirror::DomainMirror,
    ui::create_spinner,
    update,
};

/// Checks update for the mods and download the latest one if available.
pub async fn run(args: &DownloadOption, config: &AppConfig) -> anyhow::Result<()> {
    info!("updating mods");

    info!("loading installed mods");
    let mut local_mods = ModLoader::load(&config.mods_dir())?;

    info!("checking updater blacklist");
    let blacklist = update::fetch_updater_blacklist(&config.mods_dir())?;
    local_mods.retain(|local_mod| {
        let Some(name) = local_mod.path().file_name() else {
            return true;
        };
        !blacklist.contains(name)
    });

    if local_mods.is_empty() {
        println!("All mods are blacklisted")
    }

    info!("syncing file cache");
    let cache_db = cache::sync(config)?;

    let client = Client::builder()
        .https_only(true)
        .gzip(true)
        .build()
        .unwrap_or_default();

    let fetcher = ApiClient::new(client.clone());
    let source = ApiSource::from(args.use_api_mirror);

    let spinner = create_spinner();
    let registry = fetcher.fetch_registry(source).await?;
    spinner.finish_and_clear();

    // check updates
    info!("checking updates");
    let report = update::UpdateScanner::new(cache_db, registry.mods).scan(&local_mods);

    // TODO make `display_updates()` function
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
    let mirrors = args
        .mirror_priority
        .iter()
        .map(DomainMirror::from)
        .collect();
    let downloader = ModDownloader::new(client.clone(), args.jobs, config.mods_dir(), mirrors);
    downloader.download_all(&report.download_tasks).await;

    info!("updating completed");
    Ok(())
}

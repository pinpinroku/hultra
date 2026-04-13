//! Handle update command.
use reqwest::Client;
use tracing::info;

use crate::{
    cache,
    commands::DownloadOption,
    config::AppConfig,
    core::{
        loader::ModResolver,
        network::{
            api::{ApiClient, ApiSource},
            downloader::ModDownloader,
        },
        update,
    },
    service::{ModsDirectoryScanner, fs::fetch_updater_blacklist},
    ui::create_spinner,
};

/// Checks update for the mods and download the latest one if available.
pub async fn run(args: DownloadOption, config: &AppConfig) -> anyhow::Result<()> {
    info!("updating mods");

    info!("loading installed mods");
    let files = ModsDirectoryScanner::scan(&config.mods_dir())?;
    let mut local_mods = ModResolver::resolve(&files)?;

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

    let client = Client::builder()
        .https_only(true)
        .gzip(true)
        .build()
        .unwrap_or_default();

    let fetcher = ApiClient::new(client.clone());
    let source = ApiSource::from(&args);

    let spinner = create_spinner();
    let registry = fetcher.fetch_everest_update_yaml(source).await?;
    spinner.finish_and_clear();

    // check updates
    info!("checking updates");
    let report = update::UpdateScanner::new(cache_db, registry).scan(&local_mods)?;

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
    let downloader = ModDownloader::new(client.clone(), args, config.mods_dir());
    downloader.download_all(report.download_tasks).await;

    info!("updating completed");
    Ok(())
}

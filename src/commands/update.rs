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
            downloader::{DownloadTask, ModDownloader},
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

    info!("reading updater blacklist file");
    let blacklist = config.read_updater_blacklist()?;
    local_mods.retain(|local_mod| {
        local_mod
            .path()
            .file_name()
            .map(|name| !blacklist.contains(name.to_string_lossy().as_ref()))
            .unwrap_or(true)
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
    let (targets, update_info_list) = update::detect(cache_db, registry.mods, &local_mods);

    if targets.is_empty() {
        info!("all mods are up-to-date");
        return Ok(());
    } else {
        // send update info to stdout
        info!("available updates:");
        for update_info in update_info_list {
            info!("{}", update_info);
        }
    }

    let tasks: Vec<DownloadTask> = targets.into_iter().map(DownloadTask::from).collect();

    // Download updates
    info!("downloading mods");
    let mirrors = args
        .mirror_priority
        .iter()
        .map(DomainMirror::from)
        .collect();
    let downloader = ModDownloader::new(client.clone(), args.jobs, config.mods_dir(), mirrors);
    downloader.download_all(&tasks).await;

    info!("updating completed");
    Ok(())
}

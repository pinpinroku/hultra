//! Handle install command.
use reqwest::Client;
use tokio::try_join;
use tracing::{info, instrument};

use crate::{
    config::AppConfig,
    core::{
        loader::ModLoader,
        network::{
            api::{ApiClient, ApiSource},
            downloader::{DownloadTask, ModDownloader},
        },
        resolver,
    },
    mirror::DomainMirror,
    ui::create_spinner,
};

use super::InstallArgs;

#[instrument(skip(config))]
pub async fn run(args: &InstallArgs, config: &AppConfig) -> anyhow::Result<()> {
    info!("installing mods");

    info!("loading installed mods");
    let mods = ModLoader::load(&config.mods_dir())?;

    // Initialize client
    let client = Client::builder()
        .https_only(true)
        .gzip(true)
        .build()
        .unwrap_or_default();

    // Parse mod page URLs to get mod IDs
    let ids: Vec<u32> = args
        .urls
        .iter()
        .filter_map(|url| url.extract_id().ok())
        .collect();

    let api_client = ApiClient::new(client.clone());
    let source = ApiSource::from(args.option.use_api_mirror);

    info!("fetching database");
    let spinner = create_spinner();
    let (registry, graph) = try_join!(
        api_client.fetch_registry(source),
        api_client.fetch_graph(source)
    )?;
    spinner.finish_and_clear();

    // Resolve missing deps
    info!("resolving missing dependencies");
    let targets = resolver::resolve_missing_mods(&ids, registry, &graph, &mods);

    if targets.is_empty() {
        println!("You have already installed the mod and its dependencies");
        return Ok(());
    }

    // Convert targets into tasks
    let tasks: Vec<DownloadTask> = targets.into_iter().map(DownloadTask::from).collect();

    let mirrors: Vec<DomainMirror> = args
        .option
        .mirror_priority
        .iter()
        .map(DomainMirror::from)
        .collect();

    // Construct download context
    let downloader =
        ModDownloader::new(client.clone(), args.option.jobs, config.mods_dir(), mirrors);

    // Download all mods
    info!("downloading mods");
    downloader.download_all(&tasks).await;

    info!("installation completed");
    Ok(())
}

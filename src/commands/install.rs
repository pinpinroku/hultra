//! Handle install command.
use std::{collections::HashSet, ops::Deref, str::FromStr};

use clap::Args;
use reqwest::Client;
use tokio::try_join;
use tracing::info;

use crate::{
    config::AppConfig,
    core::{
        loader::ModResolver,
        network::{
            api::{ApiClient, ApiSource},
            downloader::{DownloadTask, ModDownloader},
        },
        resolver,
    },
    service::ModsDirectoryScanner,
    ui::create_spinner,
};

use super::DownloadOption;

#[derive(Debug, Args, Clone)]
pub struct InstallArgs {
    /// URL(s) of mod page on GameBanana.
    #[arg(required = true, num_args = 1..20)]
    pub urls: Vec<GamebananaUrl>,

    #[command(flatten)]
    pub option: DownloadOption,
}

#[derive(thiserror::Error, Debug)]
pub enum ArgumentError {
    #[error(
        "last path segment of URL must be a positive integer up to {}",
        u32::MAX
    )]
    ParseLastSegAsInt(#[from] std::num::ParseIntError),
    #[error("it must be starts with 'https://gamebanana.com/mods/'")]
    InvalidUrl,
}

#[derive(Debug, Clone)]
pub struct GamebananaUrl(String);

impl FromStr for GamebananaUrl {
    type Err = ArgumentError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        s.strip_prefix("https://gamebanana.com/mods/")
            .ok_or(ArgumentError::InvalidUrl)?
            .parse::<u32>()?;
        Ok(GamebananaUrl(s.to_string()))
    }
}

impl Deref for GamebananaUrl {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl GamebananaUrl {
    pub fn extract_id(&self) -> Result<u32, ArgumentError> {
        let id_part = self
            .0
            .strip_prefix("https://gamebanana.com/mods/")
            .ok_or(ArgumentError::InvalidUrl)?;
        let id = id_part.parse()?;
        Ok(id)
    }
}

pub async fn run(args: &InstallArgs, config: &AppConfig) -> anyhow::Result<()> {
    info!("installing mods");

    // Initialize client
    let client = Client::builder()
        .https_only(true)
        .gzip(true)
        .build()
        .unwrap_or_default();

    // Parse mod page URLs to get mod IDs
    let ids: HashSet<u32> = args
        .urls
        .iter()
        .filter_map(|url| url.extract_id().ok())
        .collect();

    let api_client = ApiClient::new(client.clone());
    let source = ApiSource::from(&args.option);

    info!("fetching database");
    let spinner = create_spinner();
    let (registry, graph) = try_join!(
        api_client.fetch_everest_update_yaml(source),
        api_client.fetch_graph(source)
    )?;
    spinner.finish_and_clear();

    info!("extracting installed mod names");
    let files = ModsDirectoryScanner::scan(&config.mods_dir())?;
    let installed_names = ModResolver::resolve_names(&files)?;

    // Resolve missing deps
    info!("resolving missing dependencies");
    // TODO this method should be `graph::traverse()`
    let targets = resolver::resolve_missing_mods(&ids, &registry, &graph, &installed_names);

    if targets.is_empty() {
        println!("You have already installed the mod and its dependencies");
        return Ok(());
    }

    // Convert targets into tasks
    let tasks: Vec<DownloadTask> = registry.into_download_tasks(targets, installed_names)?;

    info!("generating mirror urls");

    // Construct download context
    let downloader = ModDownloader::new(client.clone(), args.option.clone(), config.mods_dir());

    // Download all mods
    info!("downloading mods");
    downloader.download_all(tasks).await;

    info!("installation completed");
    Ok(())
}

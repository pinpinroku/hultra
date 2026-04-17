//! Handle install command.
use std::{collections::HashSet, ops::Deref, str::FromStr};

use clap::Args;
use tracing::info;

use crate::{
    config::AppConfig,
    core::{
        LocalFileSystemScanner, loader,
        network::{SharedHttpClient, api, downloader},
    },
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

pub async fn run(args: InstallArgs, config: &AppConfig) -> anyhow::Result<()> {
    info!("installing mods");

    // Initialize client
    let shared_client = SharedHttpClient::new();

    // Parse mod page URLs to get mod IDs
    let ids: HashSet<u32> = args
        .urls
        .iter()
        .filter_map(|url| url.extract_id().ok())
        .collect();

    info!("fetching databases...");
    let (registry, graph) = api::fetch(shared_client.inner().clone(), &args.option).await?;

    info!("extracting installed mod names");
    let installed_names: HashSet<String> =
        loader::scan_mods(&LocalFileSystemScanner, &config.mods_dir())?
            .iter()
            .map(|m| m.name().to_string())
            .collect();

    // Resolve missing deps
    info!("resolving missing dependencies");
    let targets = graph.resolve_missing_mods(&ids, &registry, &installed_names);

    if targets.is_empty() {
        println!("You have already installed the mod and its dependencies");
        return Ok(());
    }

    // Convert targets into tasks
    let tasks = registry.into_download_files(targets, installed_names)?;

    // Download all mods
    info!("downloading mods");
    downloader::download_all(
        shared_client.inner().clone(),
        args.option,
        tasks,
        &config.mods_dir(),
    )
    .await?;

    info!("installation completed");
    Ok(())
}

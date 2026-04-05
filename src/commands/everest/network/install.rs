use anyhow::Context;
use clap::Args;
use reqwest::Client;

use crate::{
    config::AppConfig,
    core::everest::{
        EverestBuild,
        network::{
            downloader::{DownloadTask, EverestDownloader},
            install,
        },
    },
};

use super::NetworkOption;

#[derive(Debug, Clone, Args)]
pub struct InstallArgs {
    /// The version of Everest to install (e.g., "6194")
    version: u32,
    #[command(flatten)]
    pub option: NetworkOption,
}

pub async fn run(
    args: &InstallArgs,
    builds: Vec<EverestBuild>,
    client: Client,
    config: AppConfig,
) -> anyhow::Result<()> {
    let target_build = builds
        .into_iter()
        .find(|b| b.version == args.version)
        .context("Specified version is not available")?;

    let downloader = EverestDownloader::new(client, config.root_dir());

    let task = DownloadTask::from(target_build);

    install(&downloader, task).await?;
    Ok(())
}

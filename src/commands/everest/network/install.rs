use anyhow::Context;
use clap::Args;

use crate::{
    config::AppConfig,
    core::everest::{
        EverestBuild, EverestBuildExt,
        network::{
            self, EverestHttpClient,
            downloader::{DownloadTask, EverestDownloader},
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
    builds: &[EverestBuild],
    client: &EverestHttpClient,
    config: &AppConfig,
) -> anyhow::Result<()> {
    let target_build = builds
        .get_build_for_version(args.version)
        .context("Specified version is not available")?;

    let downloader = EverestDownloader::new(client.inner.clone(), config.root_dir());

    let task = DownloadTask::from(target_build);

    network::install(&downloader, &task).await?;
    Ok(())
}

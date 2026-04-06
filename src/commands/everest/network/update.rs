use anyhow::Context;
use reqwest::Client;
use tracing::debug;

use crate::{
    config::AppConfig,
    core::everest::{
        EverestBuild, get_installed_branch, get_latest_build_on_branch,
        network::{
            self,
            downloader::{DownloadTask, EverestDownloader},
        },
        version::{self, FileVersionRepository},
    },
};

pub async fn run(
    config: &AppConfig,
    builds: Vec<EverestBuild>,
    client: Client,
) -> anyhow::Result<()> {
    let repo = FileVersionRepository::new(config);
    let current_v = version::fetch_installed_version(&repo)?.value();
    let current_b = get_installed_branch(&builds, current_v)
        .context("Installed version not found on the database")?;

    let target_build =
        get_latest_build_on_branch(&builds, current_b).context("No builds found on the branch")?;
    debug!(?target_build, ?current_v, ?current_b);

    if current_v == target_build.version {
        println!("Everest is up-to-date");
        println!("  {}", target_build);
        return Ok(());
    }

    let downloader = EverestDownloader::new(client, config.root_dir());
    let task = DownloadTask::from(target_build.clone());

    network::install(&downloader, task).await
}

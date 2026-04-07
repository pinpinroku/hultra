use anyhow::Context;
use tracing::debug;

use crate::{
    config::AppConfig,
    core::everest::{
        EverestBuild, EverestBuildExt,
        network::{
            self, EverestHttpClient,
            downloader::{DownloadTask, EverestDownloader},
        },
        version::{self, FileVersionRepository},
    },
};

pub async fn run(
    config: &AppConfig,
    builds: &[EverestBuild],
    client: &EverestHttpClient,
) -> anyhow::Result<()> {
    let repo = FileVersionRepository::new(config);
    let current_v = version::fetch_installed_version(&repo)?.value();
    let current_b = builds
        .get_installed_branch(current_v)
        .context("Installed version not found on the database")?;

    let target_build = builds
        .get_latest_build_for_branch(current_b)
        .context("No builds found on the branch")?;
    debug!(?target_build, ?current_v, ?current_b);

    if current_v == target_build.version {
        println!("Everest is up-to-date");
        println!("  {}", target_build);
        return Ok(());
    }

    let downloader = EverestDownloader::new(client.inner.clone(), config.root_dir());
    let task = DownloadTask::from(target_build);

    network::install(&downloader, &task).await
}

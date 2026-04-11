use anyhow::Context;
use tracing::debug;

use crate::{
    commands::everest::fetch_installed_version,
    config::AppConfig,
    core::everest::{
        EverestBuild, EverestBuildExt,
        network::{
            self, EverestHttpClient,
            downloader::{DownloadTask, EverestDownloader},
        },
    },
    service::fs::FileVersionRepository,
};

pub async fn run(
    config: &AppConfig,
    builds: &[EverestBuild],
    client: &EverestHttpClient,
) -> anyhow::Result<()> {
    let repo = FileVersionRepository::new(config);
    let current_v = fetch_installed_version(&repo)?.value();
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

    network::install(&downloader, &task).await?;

    // update current build version
    std::fs::write(
        config.root_dir().join("update-build.txt"),
        target_build.version.to_string(),
    )?;
    Ok(())
}

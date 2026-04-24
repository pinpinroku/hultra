use anyhow::Context;
use tracing::debug;

use crate::{
    config::AppConfig,
    everest::{
        self, EverestHttpClient,
        build::{EverestBuild, EverestBuildExt},
        version::{FileVersionRepository, fetch_installed_version},
    },
};

pub async fn run(
    config: &AppConfig,
    builds: &[EverestBuild],
    client: &EverestHttpClient,
) -> anyhow::Result<()> {
    // Check if update is available
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

    // Download Everest
    everest::download(client.inner().clone(), target_build, config).await?;

    // Install Everest
    everest::install(config.root_dir())?;

    // Update build version
    std::fs::write(config.update_build_path(), target_build.version.to_string())?;
    Ok(())
}

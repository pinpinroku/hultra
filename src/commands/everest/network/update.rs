//! write update logic here
//! move logic from main.rs

use anyhow::Context;
use tracing::debug;

use crate::{
    config::AppConfig,
    everest::{EverestBuild, client::EverestClient, version},
};

pub async fn run(
    config: &AppConfig,
    builds: &[EverestBuild],
    client: &EverestClient,
) -> anyhow::Result<()> {
    let current_v = version::ensure_installed_version(config.root_dir())?;
    let current_b = version::get_installed_branch(builds, &current_v)
        .context("Installed version not found on the database")?;

    let target_build = version::get_latest_build_on_branch(builds, current_b)
        .context("No builds found on the branch")?;
    debug!(?target_build, ?current_v, ?current_b);

    if current_v == target_build.version {
        println!("Everest is up-to-date");
        println!("  {}", target_build);
        return Ok(());
    }
    client
        .download_and_run_installer(target_build, config)
        .await?;

    Ok(())
}

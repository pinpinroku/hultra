use anyhow::Context;
use clap::Args;

use crate::{
    config::AppConfig,
    everest::EverestHttpClient,
    everest::{
        self,
        build::{EverestBuild, EverestBuildExt},
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

    // Download Everest
    everest::download(client.inner().clone(), target_build, config).await?;

    // Install Everest
    everest::install(config.root_dir())?;

    // Cache build version
    std::fs::write(config.update_build_path(), target_build.version.to_string())?;
    Ok(())
}

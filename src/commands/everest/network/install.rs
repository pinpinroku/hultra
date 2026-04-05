use anyhow::Context;
use clap::Args;

use crate::{
    config::AppConfig,
    everest::{EverestBuild, client::EverestClient},
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
    client: &EverestClient,
    config: &AppConfig,
) -> anyhow::Result<()> {
    let target_build = builds
        .iter()
        .find(|b| b.version == args.version)
        .context("Specified version is not available")?;

    client
        .download_and_run_installer(target_build, config)
        .await?;
    Ok(())
}

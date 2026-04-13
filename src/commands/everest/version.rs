//! Everest version command handler.
use crate::{
    config::AppConfig, everest::fetch_installed_version, service::fs::FileVersionRepository,
};

pub fn run(config: &AppConfig) -> anyhow::Result<()> {
    let repo = FileVersionRepository::new(config);
    let number = fetch_installed_version(&repo)?;

    println!("{}", number.value());
    Ok(())
}

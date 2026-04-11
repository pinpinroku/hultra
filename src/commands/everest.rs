//! Everest commands and the sub commands.
use std::str::FromStr;

use clap::Subcommand;

use crate::{
    commands::everest::network::NetworkCommand,
    core::everest::version::{InstalledVersionProvider, VersionNumber, VersionParseError},
};

pub mod network;
pub mod version;

#[derive(Debug, Clone, Subcommand)]
pub enum EverestSubCommand {
    /// Print the current installed version
    Version,

    #[command(flatten)]
    NetworkRequired(NetworkCommand),
}

// NOTE Might move these functions to `./commands/shared.rs`

/// Fetches version number and returns it if it is valid, otherwise returns error.
fn fetch_installed_version(
    repo: &impl InstalledVersionProvider,
) -> Result<VersionNumber, VersionParseError> {
    let content = repo.fetch()?;
    let number = VersionNumber::from_str(&content)?;
    Ok(number)
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    struct MockFileRepo(&'static str);
    impl InstalledVersionProvider for MockFileRepo {
        fn fetch(&self) -> io::Result<String> {
            Ok(self.0.to_string())
        }
    }

    #[test]
    fn test_fetch_installed_version_valid() {
        let repo = MockFileRepo("6924");
        let result = fetch_installed_version(&repo);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().value(), 6924);
    }

    #[test]
    fn test_fetch_installed_version_too_low() {
        let repo = MockFileRepo("1000");
        let result = fetch_installed_version(&repo);

        assert!(matches!(
            result,
            Err(VersionParseError::InvalidVersionRange { .. })
        ));
    }
}

//! Domain model of version number for the `everest version` command.
use std::{fs, io, path::PathBuf, str::FromStr};

use crate::config::AppConfig;

#[derive(Debug, thiserror::Error)]
pub enum VersionParseError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid version string '{version}' in the `update-build.txt`: {source}")]
    InvalidVersion {
        source: std::num::ParseIntError,
        version: String,
    },
    #[error("version file does not contain any strings")]
    VersionTextNotFound,
    #[error("version number '{actual}' is not in the range within {expected}")]
    InvalidVersionRange { actual: u32, expected: String },
}

/// Represents version number of Everest.
pub struct VersionNumber(u32);

impl VersionNumber {
    const MIN_VERSION: u32 = 3960;
    const MAX_VERSION: u32 = 9999;

    pub fn value(&self) -> u32 {
        self.0
    }
}

impl FromStr for VersionNumber {
    type Err = VersionParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(VersionParseError::VersionTextNotFound);
        }

        let version = trimmed
            .parse::<u32>()
            .map_err(|e| VersionParseError::InvalidVersion {
                source: e,
                version: trimmed.to_string(),
            })?;

        if !(Self::MIN_VERSION..=Self::MAX_VERSION).contains(&version) {
            return Err(VersionParseError::InvalidVersionRange {
                actual: version,
                expected: format!("{}-{}", Self::MIN_VERSION, Self::MAX_VERSION),
            });
        }

        Ok(Self(version))
    }
}

pub trait InstalledVersionProvider {
    fn fetch(&self) -> Result<String, io::Error>;
}

/// Represents version file of Everest.
pub struct FileVersionRepository {
    path: PathBuf,
}

impl FileVersionRepository {
    pub fn new(config: &AppConfig) -> Self {
        let path = config.update_build_path();
        Self { path }
    }
}

impl InstalledVersionProvider for FileVersionRepository {
    fn fetch(&self) -> Result<String, io::Error> {
        fs::read_to_string(&self.path)
    }
}

/// Fetches version number and returns it if it is valid, otherwise returns error.
pub fn fetch_installed_version(
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

//! Core logic and infrastructure for `everest version` command.
use std::{fs, io, path::PathBuf};

use crate::config::AppConfig;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read version file")]
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

    fn parse(raw: &str) -> Result<Self, Error> {
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(Error::VersionTextNotFound);
        }

        let version = trimmed.parse::<u32>().map_err(|e| Error::InvalidVersion {
            source: e,
            version: trimmed.to_string(),
        })?;

        if !(Self::MIN_VERSION..=Self::MAX_VERSION).contains(&version) {
            return Err(Error::InvalidVersionRange {
                actual: version,
                expected: format!("{}-{}", Self::MIN_VERSION, Self::MAX_VERSION),
            });
        }

        Ok(Self(version))
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}

pub trait InstalledVersionProvider {
    fn fetch(&self) -> Result<String, io::Error>;
}

/// Fetches version number and returns it if it is valid, otherwise returns error.
pub fn fetch_installed_version(
    repo: &impl InstalledVersionProvider,
) -> Result<VersionNumber, Error> {
    let content = repo.fetch()?;
    let number = VersionNumber::parse(&content)?;
    Ok(number)
}

/// Represents file format version repository.
pub struct FileVersionRepository {
    path: PathBuf,
}

impl FileVersionRepository {
    pub fn new(config: &AppConfig) -> Self {
        let path = config.root_dir().join("update-build.txt");
        Self { path }
    }
}

impl InstalledVersionProvider for FileVersionRepository {
    fn fetch(&self) -> Result<String, io::Error> {
        fs::read_to_string(&self.path)
    }
}

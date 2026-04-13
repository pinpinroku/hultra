//! Domain model of version number for the `everest version` command.
use std::{io, str::FromStr};

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

mod api;
pub mod build;
mod downloader;
mod error;
mod installer;
pub mod version;

use std::str::FromStr;

pub use api::fetch;
pub use downloader::download;
pub use installer::install;
use reqwest::Client;

use crate::everest::version::{InstalledVersionProvider, VersionNumber, VersionParseError};

#[derive(Debug, Clone)]
pub struct EverestHttpClient {
    pub inner: Client,
}

impl EverestHttpClient {
    pub fn new() -> reqwest::Result<Self> {
        let client = Client::builder().https_only(true).gzip(true).build()?;
        Ok(Self { inner: client })
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

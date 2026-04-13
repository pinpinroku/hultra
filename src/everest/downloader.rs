use std::{fmt::Display, path::Path, str::FromStr, time::Duration};

use futures_util::StreamExt;
use indicatif::ProgressBar;
use reqwest::{Client, header::ACCEPT};
use tempfile::{Builder, NamedTempFile};
use tokio::io::AsyncWriteExt;
use tracing::instrument;

use crate::{
    config::{AppConfig, CARGO_PKG_NAME},
    everest::build::EverestBuild,
    log::anonymize,
    service::archive::{self, ExtractError},
};

/// Downloads Everest and extracts it to the root directory of Celeste.
pub async fn download(
    client: Client,
    build: &EverestBuild,
    config: &AppConfig,
) -> anyhow::Result<()> {
    let downloader = EverestDownloader::new(client);
    let resource = DownloadResource::try_from(build)?;

    let extract_dir = config.root_dir();
    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(120));
    spinner.set_message("Downloading Everest");

    downloader.run(&resource, extract_dir, &spinner).await?;
    Ok(())
}

/// Download reasource for the Everest.
#[derive(Debug, Clone)]
struct DownloadResource {
    /// Download URL of the Everest.
    url: EverestDownloadUrl,
    /// Validation for file integrity.
    size: u64,
}

impl Display for DownloadResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "URL: {}, Expected size: {}", self.url(), self.filesize())
    }
}

#[derive(Debug, Clone)]
struct EverestDownloadUrl(String);

impl Display for EverestDownloadUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// TODO make it enum and add more variants
// URL must contains either github or azure
#[derive(Debug, thiserror::Error)]
#[error("failed to convert given string to EversestDownloadUrl")]
struct ParseError;

impl FromStr for EverestDownloadUrl {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.ends_with("main.zip") {
            Ok(Self(s.to_string()))
        } else {
            Err(ParseError)
        }
    }
}

impl TryFrom<&EverestBuild> for DownloadResource {
    type Error = ParseError;

    fn try_from(build: &EverestBuild) -> Result<Self, ParseError> {
        let url = EverestDownloadUrl::from_str(&build.main_download)?;
        Ok(Self {
            url,
            size: build.main_file_size,
        })
    }
}

impl DownloadResource {
    fn url(&self) -> &str {
        &self.url.0
    }
    fn filesize(&self) -> u64 {
        self.size
    }
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("failed to download the mod")]
    Network(#[from] reqwest::Error),
    #[error("failed to save the mod")]
    Io(#[from] std::io::Error),
    #[error("failed to extract Everest to the root directory")]
    Archive(#[from] ExtractError),
}

/// Download client for Everest update.
#[derive(Debug, Clone)]
struct EverestDownloader {
    client: Client,
}

impl EverestDownloader {
    fn new(client: Client) -> Self {
        Self { client }
    }
}

impl EverestDownloader {
    #[instrument(skip(spinner), fields(resource = %resource, extract_dir = %anonymize(extract_dir)))]
    async fn run(
        &self,
        resource: &DownloadResource,
        extract_dir: &Path,
        spinner: &ProgressBar,
    ) -> Result<(), Error> {
        let response = self
            .client
            .get(resource.url())
            .timeout(Duration::from_secs(90))
            .header(ACCEPT, "application/octet-stream")
            .send()
            .await?
            .error_for_status()?;

        // Use a temp file for "Verify-then-Commit" strategy.
        let temp_dir = Builder::new()
            .prefix(&format!("{}-", CARGO_PKG_NAME))
            .rand_bytes(6)
            .tempdir()?;
        let named_temp_file = NamedTempFile::new_in(temp_dir.path())?;
        let temp_path = named_temp_file.path();

        // Reopen handle to keep `named_temp_file` (and its path) alive for the final copy.
        let std_file = named_temp_file.reopen()?;
        let mut file = tokio::fs::File::from_std(std_file);

        let mut stream = response.bytes_stream();
        let mut downloaded = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
        }
        file.flush().await?;
        // TODO implement actucal validation
        debug_assert_eq!(downloaded, resource.filesize());

        archive::extract(temp_path, extract_dir)?;
        spinner.finish_and_clear();
        Ok(())
    }
}

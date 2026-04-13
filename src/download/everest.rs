use std::{path::Path, time::Duration};

use futures_util::StreamExt;
use indicatif::ProgressBar;
use reqwest::Client;
use reqwest::header::ACCEPT;
use tempfile::{Builder, NamedTempFile};
use tokio::io::AsyncWriteExt;
use tracing::instrument;

use crate::{
    config::CARGO_PKG_NAME,
    core::everest::EverestBuild,
    log::anonymize,
    service::archive::{self, ExtractError},
};

/// Metadata of target build of Everest to be downloaded.
#[derive(Debug, Clone)]
pub struct DownloadResource {
    // NOTE this is called resource since we will delete an item instead of keep
    url: String,
    filesize: u64, // this is for file validation since we don't have checksum for this item
} // NOTE we don't need to have a filename because we will extracting this item to the specific directory and delete the original

impl From<&EverestBuild> for DownloadResource {
    /// Converts EverestBuild into this type.
    fn from(build: &EverestBuild) -> Self {
        Self {
            url: build.main_download.clone(),
            filesize: build.main_file_size,
        }
    }
}

impl DownloadResource {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn filesize(&self) -> u64 {
        self.filesize
    }
}

/// Download client for Everest update.
#[derive(Debug, Clone)]
pub struct EverestDownloader {
    client: Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to download Everest")]
    Network(#[from] reqwest::Error),
    #[error("failed to save Everest to the path")]
    Io(#[from] std::io::Error),
    #[error("failed to save Everest to the path")]
    Archive(#[from] ExtractError),
}

impl EverestDownloader {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl EverestDownloader {
    #[instrument(skip_all, fields(path = %anonymize(dest)))]
    async fn download(
        &self,
        item: &DownloadResource,
        dest: &Path,
        pb: &ProgressBar,
    ) -> Result<(), Error> {
        // TODO add progress bar in argument
        // let pb = ProgressBar::new_spinner();
        // pb.enable_steady_tick(Duration::from_millis(120));
        // pb.set_message("Downloading Everest");

        let response = self
            .client
            .get(item.url())
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
        debug_assert_eq!(downloaded, item.filesize());

        archive::extract(temp_path, dest)?;
        pb.finish_and_clear();
        Ok(())
    }
}

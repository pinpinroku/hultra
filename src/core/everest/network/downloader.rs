use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use futures_util::StreamExt;
use indicatif::ProgressBar;
use reqwest::Client;
use reqwest::header::ACCEPT;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
};
use tracing::{info, instrument};

use crate::{core::everest::EverestBuild, log::anonymize};

/// Metadata of target build of Everest to be downloaded.
#[derive(Debug, Clone)]
pub struct DownloadTask {
    url: String,
    filesize: u64,
}

impl From<EverestBuild> for DownloadTask {
    /// Converts EverestBuild into this type.
    fn from(build: EverestBuild) -> Self {
        Self {
            url: build.main_download,
            filesize: build.main_file_size,
        }
    }
}

impl DownloadTask {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn filesize(self) -> u64 {
        self.filesize
    }
}

/// Download client for Everest update.
#[derive(Debug, Clone)]
pub struct EverestDownloader {
    client: Client,
    output_dir: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to download Everest")]
    Network(#[from] reqwest::Error),
    #[error("failed to save Everest to the path")]
    Io(#[from] std::io::Error),
}

impl EverestDownloader {
    pub fn new(client: Client, output_dir: &Path) -> Self {
        Self {
            client,
            output_dir: output_dir.to_path_buf(),
        }
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    #[instrument(skip(self), fields(url, path = %anonymize(dest)))]
    pub async fn download_everest(&self, url: &str, dest: &Path) -> Result<u64, Error> {
        info!("Downloading Everest");
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_message("Downloading Everest");

        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(90))
            .header(ACCEPT, "application/octet-stream")
            .send()
            .await?
            .error_for_status()?;

        let file = File::create(dest).await?;
        let mut writer = BufWriter::new(file);
        let mut stream = response.bytes_stream();
        let mut downloaded = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            writer.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
        }
        writer.flush().await?;

        pb.finish_and_clear();
        Ok(downloaded)
    }
}

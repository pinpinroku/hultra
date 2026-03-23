use std::{path::Path, time::Duration};

use futures_util::StreamExt;
use reqwest::{
    Client,
    header::{ACCEPT, ACCEPT_ENCODING, HeaderValue},
};
use tempfile::NamedTempFile;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
};
use tracing::{info, instrument};
use url::Url;

use crate::{config::AppConfig, everest::installer};

use super::EverestBuild;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
    #[error(transparent)]
    Extract(#[from] super::ExtractError),
}

/// Download client for Everest update.
pub struct EverestClient {
    client: Client,
}

impl EverestClient {
    const ENDPOINT_MIRROR: &str =
        "https://everestapi.github.io/updatermirror/everest_versions.json";
    const ENDPOINT_ORIGINAL: &str = "https://everestapi.github.io/everestupdater.txt";

    pub fn new() -> Result<Self, Error> {
        let client = Client::builder()
            .https_only(true)
            .gzip(true)
            .timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self { client })
    }

    /// Downloads `main.zip` and runs `MiniInstaller-linux`.
    pub async fn download_and_run_installer(
        &self,
        build: &EverestBuild,
        config: &AppConfig,
    ) -> Result<(), Error> {
        let temp_zip = NamedTempFile::new()?;

        let downloaded = self
            .download_everest(&build.main_download, temp_zip.path())
            .await?;
        debug_assert_eq!(downloaded, build.main_file_size);

        super::extract_zip_archive(temp_zip.path(), config.root_dir())?;

        installer::run(config.root_dir())?;

        Ok(())
    }

    /// Returns API endpoint.
    #[instrument(skip(self), err(Debug))]
    pub async fn get_url(&self, is_mirror: bool) -> Result<Url, Error> {
        let url = if is_mirror {
            info!("Using mirror for the Everest updater database");
            Url::parse(Self::ENDPOINT_MIRROR)?
        } else {
            info!("Fetching Everest updater database URL");
            self.fetch_url().await?
        };

        Ok(url)
    }

    /// Fetches URL from GitHub endopint.
    #[instrument(skip_all, err(Debug))]
    async fn fetch_url(&self) -> Result<Url, Error> {
        let mut url = self
            .client
            .get(Self::ENDPOINT_ORIGINAL)
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .header(ACCEPT_ENCODING, HeaderValue::from_static("gzip"))
            .send()
            .await?
            .text()
            .await?
            .trim()
            .parse::<Url>()?;

        url.query_pairs_mut()
            .append_pair("supportsNativeBuilds", "true");

        Ok(url)
    }

    // 1. Returns list of builds by sending request to endpoint.
    #[instrument(skip(self), err(Debug))]
    pub async fn fetch_update_list(&self, url: Url) -> Result<Vec<EverestBuild>, Error> {
        let response = self.client.get(url).send().await?;
        let builds: Vec<EverestBuild> = response.json().await?;
        Ok(builds)
    }

    // 2. Downloads file and save it to given destination. Returns actual downloaded size in bytes.
    #[instrument(skip(self), err(Debug))]
    pub async fn download_everest(&self, url: &str, dest: &Path) -> Result<u64, Error> {
        let response = self
            .client
            .get(url)
            .header(ACCEPT, "application/octet-stream")
            .send()
            .await?;

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

        Ok(downloaded)
    }
}

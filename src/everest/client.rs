use std::{path::Path, time::Duration};

use futures_util::StreamExt;
use indicatif::ProgressBar;
use reqwest::{
    Client,
    header::{ACCEPT, ACCEPT_ENCODING, HeaderValue},
};
use tempfile::NamedTempFile;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
};
use tracing::{error, info, instrument};
use url::Url;

use crate::{config::AppConfig, everest::installer, log::anonymize};

use super::EverestBuild;

/// Download client for Everest update.
pub struct EverestClient {
    client: Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("failed to parse string as valid URL")]
    UrlParse(#[from] url::ParseError),
    #[error(transparent)]
    Extract(#[from] crate::archive::ExtractError),
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

    #[instrument(skip(self))]
    pub async fn fetch_database(&self, is_mirror: bool) -> Result<Vec<EverestBuild>, Error> {
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_message("Fetching database...");

        let endpoint = self.get_url(is_mirror).await?;
        let builds = self.fetch_update_list(endpoint).await?;

        pb.finish_and_clear();
        Ok(builds)
    }

    /// Downloads `main.zip` and runs `MiniInstaller-linux`.
    #[instrument(skip(self))]
    pub async fn download_and_run_installer(
        &self,
        build: &EverestBuild,
        config: &AppConfig,
    ) -> Result<(), Error> {
        let temp_zip = NamedTempFile::new()?;

        let downloaded = self
            .download_everest(&build.main_download, temp_zip.path())
            .await
            .inspect_err(|err| error!(?err, "failed to download Everest"))?;
        debug_assert_eq!(downloaded, build.main_file_size);

        crate::archive::extract_zip_archive(temp_zip.path(), config.root_dir())
            .inspect_err(|err| error!(?err, "failed to extract ZIP archive"))?;
        drop(temp_zip);

        installer::run(config)?;
        Ok(())
    }

    /// Returns API endpoint.
    async fn get_url(&self, is_mirror: bool) -> Result<Url, Error> {
        let url = if is_mirror {
            info!("Using mirror for the Everest updater database");
            Url::parse(Self::ENDPOINT_MIRROR)?
        } else {
            info!("Fetching Everest updater database URL");
            let text = self.fetch_url().await?;
            let mut url = text.trim().parse::<Url>()?;

            url.query_pairs_mut()
                .append_pair("supportsNativeBuilds", "true");
            url
        };
        Ok(url)
    }

    /// Fetches URL from GitHub endopint.
    #[instrument(skip_all)]
    async fn fetch_url(&self) -> reqwest::Result<String> {
        self.client
            .get(Self::ENDPOINT_ORIGINAL)
            .timeout(Duration::from_secs(10))
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .header(ACCEPT_ENCODING, HeaderValue::from_static("gzip"))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await
    }

    // 1. Returns list of builds by sending request to endpoint.
    #[instrument(skip(self), fields(url = %url))]
    async fn fetch_update_list(&self, url: Url) -> Result<Vec<EverestBuild>, Error> {
        info!("Fetching version list");
        let response = self.client.get(url).send().await?;
        let builds: Vec<EverestBuild> = response.json().await?;
        Ok(builds)
    }

    // 2. Downloads file and save it to given destination. Returns actual downloaded size in bytes.
    #[instrument(skip(self), fields(url, path = %anonymize(dest)))]
    pub async fn download_everest(&self, url: &str, dest: &Path) -> Result<u64, Error> {
        info!("Downloading Everest");
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_message("downloading Everest");

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

#![allow(dead_code, unused_variables)]
use std::time::Duration;

use reqwest::{
    Client,
    header::{ACCEPT, ACCEPT_ENCODING, HeaderValue},
};
use tracing::{info, instrument};
use url::Url;

use super::EverestBuild;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] reqwest::Error),
    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
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

    // 2. Downloads file for specific version.
    #[instrument(skip(self), err(Debug))]
    pub async fn download_everest(&self, version: &str) -> Result<Vec<u8>, Error> {
        // TODO: Generates `update-build.txt` in the destination directory
        // NOTE: Consider both `./Celsete.Mod.mm/Mod/Everest/Everest.Updater.cs` for GUI and `./MiniInstaller/` for CLI to implement this logic
        todo!()
    }
}

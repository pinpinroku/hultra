use std::time::Duration;

use indicatif::ProgressBar;
use reqwest::{
    Client,
    header::{ACCEPT, ACCEPT_ENCODING, HeaderValue},
};
use tracing::{debug, instrument};
use url::Url;

use crate::{commands::everest::network::NetworkOption, everest::build::EverestBuild};

pub async fn fetch(client: Client, opts: &NetworkOption) -> anyhow::Result<Vec<EverestBuild>> {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_message("Fetching database...");

    let fetcher = EverestApiClient::new(client);
    let endpoint = fetcher.get_url(opts.use_api_mirror).await?;
    let builds = fetcher.fetch_update_list(endpoint).await?;

    pb.finish_and_clear();
    Ok(builds)
}

/// API client for Everest.
#[derive(Debug, Clone)]
struct EverestApiClient {
    client: Client,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("failed to fetch database of Everest builds")]
    Network(#[from] reqwest::Error),
    #[error("failed to parse string as valid URL of Everest API")]
    UrlParse(#[from] url::ParseError),
    #[error("failed to extract Everest file: `main.zip`")]
    Extract(#[from] crate::service::archive::ExtractError),
}

impl EverestApiClient {
    const ENDPOINT_MIRROR: &str =
        "https://everestapi.github.io/updatermirror/everest_versions.json";
    const ENDPOINT_ORIGINAL: &str = "https://everestapi.github.io/everestupdater.txt";

    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Returns API endpoint.
    async fn get_url(&self, is_mirror: bool) -> Result<Url, Error> {
        let url = if is_mirror {
            debug!("Using mirror for the Everest updater database");
            Url::parse(Self::ENDPOINT_MIRROR)?
        } else {
            debug!("Fetching Everest updater database URL");
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

    // Returns list of builds by sending request to endpoint.
    #[instrument(skip(self), fields(url = %url))]
    async fn fetch_update_list(&self, url: Url) -> Result<Vec<EverestBuild>, Error> {
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?;
        let builds: Vec<EverestBuild> = response.json().await?;
        Ok(builds)
    }
}

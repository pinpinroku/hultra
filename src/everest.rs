mod api;
pub mod build;
mod downloader;
mod error;
mod installer;
pub mod version;

pub use api::fetch;
pub use downloader::download;
pub use installer::install;
use reqwest::Client;

#[derive(Debug, Clone)]
pub struct EverestHttpClient {
    inner: Client,
}

impl EverestHttpClient {
    pub fn new() -> reqwest::Result<Self> {
        let client = Client::builder().https_only(true).gzip(true).build()?;
        Ok(Self { inner: client })
    }

    pub fn inner(&self) -> &Client {
        &self.inner
    }
}

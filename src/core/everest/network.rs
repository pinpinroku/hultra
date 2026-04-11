use reqwest::Client;

use downloader::{DownloadTask, EverestDownloader};
use tempfile::NamedTempFile;
use tracing::error;

use crate::service::archive;

pub mod api;
pub mod downloader;

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

/// Downloads `main.zip` and runs `MiniInstaller-linux`.
pub async fn install(downloader: &EverestDownloader, task: &DownloadTask) -> anyhow::Result<()> {
    let temp_zip = NamedTempFile::new()?;

    let downloaded = downloader
        .download_everest(task.url(), temp_zip.path())
        .await
        .inspect_err(|err| error!(?err, "failed to download Everest"))?;
    debug_assert_eq!(downloaded, task.filesize());

    archive::extract(temp_zip.path(), downloader.output_dir())
        .inspect_err(|err| error!(?err, "failed to extract ZIP archive"))?;
    drop(temp_zip);

    super::installer::run(downloader.output_dir())?;
    Ok(())
}

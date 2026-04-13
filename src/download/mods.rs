//! Business logic to download mods.
use std::{path::Path, sync::Arc, time::Duration};

use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar};
use reqwest::Client;
use tempfile::{self, Builder, NamedTempFile};
use tokio::{
    io::AsyncWriteExt,
    sync::{AcquireError, Semaphore},
    task::JoinSet,
};
use tracing::{error, instrument};
use xxhash_rust::xxh64::Xxh64;

use crate::{
    commands::{DownloadOption, Mirrors},
    config::CARGO_PKG_NAME,
    core::{ChecksumVerificationError, Checksums, update::UpdateTask},
    log::anonymize,
    ui::create_download_progress_bar,
    utils,
};

// public function to handle downloading multiple files concurrently.
pub async fn download_all(
    downloader: Arc<ModDownloader>,
    items: Vec<DownloadFile>,
    mods_dir: &Path,
) -> Result<(), DownloadError> {
    let mut set = JoinSet::new();
    let mp = MultiProgress::new();

    for item in items {
        // TODO this will be already generated `Vec<MirrorUrl>` and can be converted into `Vec<String>`
        let downloader = downloader.clone();

        let name = item.filename.clone();
        let clean_name = utils::sanitize_stem(&name);
        // construct full path before actually start downloading
        let dest = mods_dir.join(&clean_name).with_extension("zip");

        let pb = mp.add(create_download_progress_bar(&name, item.filesize));

        set.spawn(async move { download_with_fallbacks(&downloader, &item, &dest, &pb).await });
    }

    while let Some(result) = set.join_next().await {
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => error!("{:?}", e),
            Err(e) => error!(?e, "failed to complete task, canceled or pacnicked"),
        }
    }
    Ok(())
}

/// Metadata of target mod to be downloaded.
#[derive(Debug, Clone)]
pub struct DownloadFile {
    // NOTE this is called file since we will keep it in the disk
    pub url: String, // TODO define DownloadUrl to validate the value
    /// A name of the target. Just a stem instead of full path. FileStem
    pub filename: String, // TODO sanitize when convert from (String, Entry)
    pub filesize: u64, // this is for the progress bar
    pub checksums: Checksums,
}

impl From<UpdateTask> for DownloadFile {
    fn from(value: UpdateTask) -> Self {
        Self {
            url: value.url,
            filename: value.name,
            filesize: value.size,
            checksums: value.checksums,
        }
    }
}

/// Context for downloading mods.
#[derive(Debug)]
pub struct ModDownloader {
    client: Client,
    semaphore: Arc<Semaphore>,
    mirror_priority: Mirrors,
}

impl ModDownloader {
    pub fn new(client: Client, args: DownloadOption) -> Self {
        Self {
            client,
            semaphore: Arc::new(Semaphore::new(args.jobs as usize)),
            mirror_priority: args.mirror_priority,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("All mirrors failed for '{name}'")]
pub struct AllMirrorsFailedError {
    name: String,
    errors: Vec<(String, DownloadError)>,
}

/// Retry downloading a file for given mirror urls until success or all mirrors are exhausted.
async fn download_with_fallbacks(
    client: &ModDownloader,
    item: &DownloadFile,
    dest: &Path,
    pb: &ProgressBar,
) -> Result<(), AllMirrorsFailedError> {
    let mut errors = Vec::new();

    let urls = &client.mirror_priority.resolve(&item.url);

    for url in urls {
        match client.download(item, dest, pb).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                errors.push((url.clone(), e));
                pb.reset();
            }
        }
    }

    Err(AllMirrorsFailedError {
        name: item.filename.to_string(),
        errors,
    })
}

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("failed to download the mod")]
    Network(#[from] reqwest::Error),
    #[error("failed to save the mod")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Hash(#[from] ChecksumVerificationError),
    #[error("failed to acquire semaphore")]
    SemaphoreClosed(#[from] AcquireError),
}

impl ModDownloader {
    /// Downloads a file while hashing, verifying its integrity before final persistence.
    ///
    /// ### Note
    /// - Uses `tempfile` (typically in `tmpfs`) to avoid polluting the destination
    ///   with corrupt/partial data if verification fails.
    /// - Performs `tokio::fs::copy` instead of `tempfile::persist` because `temp_path` and `dest`
    ///   often reside on different filesystems (e.g., RAM vs. Disk).
    #[instrument(skip_all, fields(path = %anonymize(path)))]
    async fn download(
        &self,
        item: &DownloadFile,
        path: &Path,
        pb: &ProgressBar,
    ) -> Result<(), DownloadError> {
        // ここでセマフォを獲得する
        let _permit = self.semaphore.acquire().await?;

        let response = self
            .client
            .get(&item.url)
            .timeout(Duration::from_secs(120))
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
        let mut writer = tokio::fs::File::from_std(std_file);

        let mut hasher = Xxh64::new(0);
        let mut stream = response.bytes_stream();

        // Stream download while hashing to minimize RAM usage.
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            hasher.update(&chunk);
            writer.write_all(&chunk).await?;
            pb.inc(chunk.len() as u64);
        }
        writer.flush().await?;

        // Abort if the file is corrupt. NamedTempFile will be auto-deleted.
        let digest = hasher.digest();
        item.checksums.verify(&digest)?;

        // Finalize the download by copying across filesystem boundaries.
        tokio::fs::copy(temp_path, path).await?;
        pb.finish_with_message(format!("{} 🍓", item.filename)); // ここでプログレバーを処理する（完結させる）
        Ok(())
    }
}

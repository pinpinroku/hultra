//! Business logic to download mods.

use std::{
    path::{Path, PathBuf},
    str,
    sync::Arc,
    time::Duration,
};

use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar};
use reqwest::Client;
use tempfile::{self, Builder, NamedTempFile};
use tokio::{io::AsyncWriteExt, sync::Semaphore, task::JoinSet};
use tracing::{error, instrument};
use xxhash_rust::xxh64::Xxh64;

use crate::{
    config::CARGO_PKG_NAME,
    core::{ChecksumVerificationError, Checksums, update::UpdateTask},
    log::anonymize,
    mirror::{self, DomainMirror},
    ui::create_download_progress_bar,
    utils,
};

/// Metadata of target mod to be downloaded.
#[derive(Debug)]
pub struct DownloadTask {
    pub url: String,      // TODO define DownloadUrl to validate the value
    pub filename: String, // TODO sanitize when convert from (String, Entry)
    pub filesize: u64,
    pub checksums: Checksums,
}

impl From<UpdateTask> for DownloadTask {
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
    mods_dir: PathBuf,
    mirror_priority: Vec<DomainMirror>, // TODO create Vec<String> from DomainMirror when init this instance
}

impl ModDownloader {
    pub fn new(
        client: Client,
        jobs: u8,
        mods_dir: PathBuf,
        mirror_priority: Vec<DomainMirror>,
    ) -> Self {
        Self {
            client,
            semaphore: Arc::new(Semaphore::new(jobs as usize)),
            mods_dir,
            mirror_priority,
        }
    }

    /// Download all mod files concurrently.
    #[instrument(skip(self))]
    pub async fn download_all(&self, tasks: &[DownloadTask]) {
        let mut set = JoinSet::new();
        let mp = MultiProgress::new();

        for task in tasks {
            let client = self.client.clone();
            let jobs = self.semaphore.clone();

            let mirror_urls = mirror::generate(&task.url, &self.mirror_priority);

            let name = task.filename.clone();
            let clean_name = utils::sanitize_stem(&name);
            let dest = self.mods_dir.join(&clean_name).with_extension("zip");

            let size = task.filesize;
            let checksums = task.checksums.clone();

            let pb = mp.add(create_download_progress_bar(&name, size));

            set.spawn(async move {
                let _permit = jobs.acquire().await.unwrap();

                download_with_fallbacks(&client, &mirror_urls, &clean_name, &checksums, &dest, &pb)
                    .await
            });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => error!("{:?}", e),
                Err(e) => error!(?e, "failed to complete task, canceled or pacnicked"),
            }
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
    client: &Client,
    urls: &[String],
    name: &str,
    checksums: &Checksums,
    dest: &Path,
    pb: &ProgressBar,
) -> Result<(), AllMirrorsFailedError> {
    let mut errors = Vec::new();

    for url in urls {
        match download(client, url, checksums, dest, pb).await {
            Ok(_) => {
                pb.finish_with_message(format!("{} 🍓", name));
                return Ok(());
            }
            Err(e) => {
                errors.push((url.clone(), e));
                pb.reset();
            }
        }
    }

    pb.finish_and_clear();

    error!("failed to download '{}' for all mirrors", name);
    Err(AllMirrorsFailedError {
        name: name.to_string(),
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
}

/// Downloads a file while hashing, verifying its integrity before final persistence.
///
/// ### Note
/// - Uses `tempfile` (typically in `tmpfs`) to avoid polluting the destination
///   with corrupt/partial data if verification fails.
/// - Performs `tokio::fs::copy` instead of `tempfile::persist` because `temp_path` and `dest`
///   often reside on different filesystems (e.g., RAM vs. Disk).
#[instrument(skip_all, fields(url = %url, path = %anonymize(dest)))]
async fn download(
    client: &Client,
    url: &str,
    checksums: &Checksums,
    dest: &Path,
    pb: &ProgressBar,
) -> Result<(), DownloadError> {
    let response = client
        .get(url)
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
    let mut file = tokio::fs::File::from_std(std_file);

    let mut hasher = Xxh64::new(0);
    let mut stream = response.bytes_stream();

    // Stream download while hashing to minimize RAM usage.
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        pb.inc(chunk.len() as u64);
    }
    file.flush().await?;

    // Abort if the file is corrupt. NamedTempFile will be auto-deleted.
    let digest = hasher.digest();
    checksums.verify(&digest)?;

    // Finalize the download by copying across filesystem boundaries.
    tokio::fs::copy(temp_path, dest).await?;
    Ok(())
}

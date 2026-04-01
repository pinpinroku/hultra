//! Business logic to download mods.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar};
use reqwest::Client;
use tempfile::{self, Builder, NamedTempFile};
use tokio::{io::AsyncWriteExt, sync::Semaphore};
use tracing::{error, info, instrument, warn};
use xxhash_rust::xxh64::Xxh64;

use crate::{
    config::CARGO_PKG_NAME,
    core::utils,
    log::anonymize,
    mirror::{self, DomainMirror},
    registry::RemoteMod,
    ui::create_download_progress_bar,
};

/// Metadata of target mod to be downloaded.
#[derive(Debug)]
pub struct DownloadTask {
    url: String,
    filename: String,
    filesize: u64,
    checksums: Vec<u64>,
}

impl From<(String, RemoteMod)> for DownloadTask {
    /// Converts HashMap<String, RemoteMod> into this type.
    fn from((name, remote): (String, RemoteMod)) -> Self {
        Self {
            url: remote.download_url,
            filename: name,
            filesize: remote.file_size,
            checksums: remote.checksums,
        }
    }
}

/// Context for downloading mods.
#[derive(Debug)]
pub struct ModDownloader {
    client: Client,
    semaphore: Arc<Semaphore>,
    mods_dir: PathBuf,
    mirror_priority: Vec<DomainMirror>,
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
        let mp = MultiProgress::new();

        let handles: Vec<_> = tasks
            .iter()
            .map(|task| {
                let client = self.client.clone();
                let jobs = self.semaphore.clone();

                let mirror_urls = mirror::generate(&task.url, &self.mirror_priority);

                let name = task.filename.clone();
                let clean_name = utils::sanitize_stem(&name);
                let dest = self.mods_dir.join(&clean_name).with_extension("zip");

                let size = task.filesize;
                let checksums = task.checksums.clone();

                let pb = mp.add(create_download_progress_bar(&name, size));

                tokio::spawn(async move {
                    let _permit = jobs.acquire().await.unwrap();

                    download_with_fallbacks(
                        &client,
                        &mirror_urls,
                        &clean_name,
                        &checksums,
                        &dest,
                        &pb,
                    )
                    .await;
                })
            })
            .collect();

        for handle in handles {
            if let Err(e) = handle.await {
                error!(?e, "failed to complete the task, canceled or panicked")
            }
        }
    }
}

/// Retry downloading a file for given mirror urls until success or all mirrors are exhausted.
#[instrument(skip_all, fields(urls = ?urls))]
async fn download_with_fallbacks(
    client: &Client,
    urls: &[String],
    name: &str,
    checksums: &[u64],
    dest: &Path,
    pb: &ProgressBar,
) {
    let mut success = false;

    for url in urls {
        match download(client, url, checksums, dest, pb).await {
            Ok(_) => {
                success = true;
                pb.finish_with_message(format!("{} 🍓", name));
                break;
            }
            Err(e) => {
                warn!(
                    ?e,
                    "failed to download '{}' from '{}', trying another mirror", name, url
                );
                pb.set_message(format!(
                    "{}: Failed to download, trying another mirror.",
                    name
                ));
                pb.reset();
            }
        }
    }

    // TODO implement error summary by collecting all the errors
    if !success {
        error!("failed to download '{}' for all mirrors", name);
        pb.finish_with_message(format!("{} ❌ Failed", name))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("failed to download the mod")]
    Network(#[from] reqwest::Error),
    #[error("failed to save the mod")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Hash(#[from] HashValidationError),
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
    checksums: &[u64],
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
    verify(digest, checksums)?;
    info!("checksum verified");

    // Finalize the download by copying across filesystem boundaries.
    tokio::fs::copy(temp_path, dest).await?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
#[error("Hash mismatch: computed: {computed}, expected: {expected:?}")]
pub struct HashValidationError {
    computed: String,
    expected: Vec<String>,
}

/// Verifies given checksums are equal.
fn verify(computed: u64, expected: &[u64]) -> Result<(), HashValidationError> {
    if expected.contains(&computed) {
        Ok(())
    } else {
        Err(HashValidationError {
            computed: format!("{:016x}", computed),
            expected: expected.iter().map(|v| format!("{:016x}", v)).collect(),
        })
    }
}

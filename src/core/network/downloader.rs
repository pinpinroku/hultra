//! Business logic to download mods.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar};
use reqwest::Client;
use tokio::{
    fs,
    io::{self, AsyncWriteExt},
    sync::Semaphore,
};
use tracing::{error, info, instrument, warn};
use xxhash_rust::xxh64::Xxh64;

use crate::{
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
                        size,
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
    size: u64,
    checksums: &[u64],
    dest: &Path,
    pb: &ProgressBar,
) {
    let mut success = false;

    for url in urls {
        match download(client, url, size, checksums, dest, pb).await {
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

    // TODO implement error summary by collection all the errors
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

/// Downloads a single mod file while hashing the file.
#[instrument(skip_all, fields(url = %url))]
async fn download(
    client: &Client,
    url: &str,
    size: u64,
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

    // TODO Use tempfile to store temporary file on RAM disk
    let mut buffer = Vec::with_capacity(size as usize);

    let mut hasher = Xxh64::new(0);
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        // TODO Write buffer directly to tempfile
        buffer.extend_from_slice(&chunk);
        pb.inc(chunk.len() as u64);
    }
    // TODO flush file

    // TODO remove the file if verification fails
    let digest = hasher.digest();
    verify(digest, checksums)?;
    info!("checksum verified");

    // TODO persist (copy) the file to the disk if verification check passes
    save_to_disk(dest, &buffer).await?;

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

/// Writes buffer to destination path.
#[instrument(skip_all, fields(path = %anonymize(dest)))]
async fn save_to_disk(dest: &Path, buffer: &[u8]) -> io::Result<()> {
    let mut file = fs::File::create(dest).await?;
    file.write_all(buffer).await?;
    file.flush().await?;

    Ok(())
}

//! Business logic to download mods.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar};
use reqwest::Client;
use tokio::{fs, io::AsyncWriteExt, sync::Semaphore};
use tracing::{debug, error, instrument, warn};
use xxhash_rust::xxh64::Xxh64;

use crate::{
    core::utils,
    log::anonymize,
    mirror::{self, DomainMirror},
    registry::RemoteMod,
    ui::create_download_progress_bar,
};

/// Metadata of target mod to be downloaded.
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
pub struct ModDownloader {
    client: Client,
    semaphore: Arc<Semaphore>,
    output_dir: PathBuf,
    mirror_priority: Vec<DomainMirror>,
}

impl ModDownloader {
    pub fn new(
        client: Client,
        jobs: u8,
        output_dir: &Path,
        mirror_priority: Vec<DomainMirror>,
    ) -> Self {
        Self {
            client,
            semaphore: Arc::new(Semaphore::new(jobs as usize)),
            output_dir: output_dir.to_path_buf(),
            mirror_priority,
        }
    }

    /// Download all mod files concurrently.
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
                let dest = self.output_dir.join(&clean_name).with_extension("zip");

                let size = task.filesize;
                let hashes = task.checksums.clone();

                let pb = mp.add(create_download_progress_bar(&name));

                tokio::spawn(async move {
                    let _permit = jobs.acquire().await.unwrap();

                    download_with_fallbacks(
                        &client,
                        &mirror_urls,
                        &clean_name,
                        size,
                        &hashes,
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
#[instrument(skip_all, fields(name))]
async fn download_with_fallbacks(
    client: &Client,
    urls: &[String],
    name: &str,
    size: u64,
    hashes: &[u64],
    dest: &Path,
    pb: &ProgressBar,
) {
    let mut success = false;

    for url in urls {
        match download(client, url, size, hashes, dest, pb).await {
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
    #[error(
        "failed to verify checksum for {file_path:?}: computed {computed}, expected {expected:?}"
    )]
    FileHashMissMatch {
        file_path: PathBuf,
        computed: u64,
        expected: Vec<u64>,
    },
}

/// Downloads a single mod file while hashing the file.
#[instrument(skip_all, fields(url = %url))]
async fn download(
    client: &Client,
    url: &str,
    size: u64,
    hashes: &[u64],
    dest: &Path,
    pb: &ProgressBar,
) -> Result<(), DownloadError> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(60))
        .send()
        .await?
        .error_for_status()?;

    let total_size = response.content_length().unwrap_or(size);
    pb.set_length(total_size);
    pb.reset();

    let mut buffer = Vec::with_capacity(total_size as usize);

    let mut hasher = Xxh64::new(0);
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        buffer.extend_from_slice(&chunk);
        pb.inc(chunk.len() as u64);
    }
    debug!(filesize = %size, "download completed");

    let computed_hash = hasher.digest();
    if !hashes.contains(&computed_hash) {
        return Err(DownloadError::FileHashMissMatch {
            file_path: dest.to_path_buf(),
            computed: computed_hash,
            expected: hashes.to_vec(),
        });
    }
    debug!(xxhash64 = %computed_hash, "hash check passed");

    let mut file = fs::File::create(dest).await?;
    file.write_all(&buffer).await?;
    file.flush().await?;

    debug!(path = %anonymize(dest), "saved to disk");

    Ok(())
}

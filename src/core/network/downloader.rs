use std::{path::Path, str::FromStr, sync::Arc, time::Duration};

use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar};
use reqwest::Client;
use tempfile::{self, Builder, NamedTempFile};
use tokio::{
    io::AsyncWriteExt,
    sync::{AcquireError, Semaphore},
    task::{JoinError, JoinSet},
};
use tracing::instrument;
use xxhash_rust::xxh64::Xxh64;

use crate::{
    commands::{DownloadOption, Mirrors},
    config::CARGO_PKG_NAME,
    core::{
        Checksum, ChecksumVerificationError, Checksums, ParseChecksumError, registry::Entry,
        update::UpdateContext,
    },
    log::anonymize,
    ui::create_download_progress_bar,
    utils,
};

/// Downloads multiple files concurrently.
pub async fn download_all(
    client: Client,
    args: DownloadOption,
    targets: Vec<DownloadFile>,
    mods_dir: &Path,
) -> anyhow::Result<()> {
    let downloader = Arc::new(ModDownloader::new(client, args));
    let mut set = JoinSet::new();
    let mp = MultiProgress::new();

    for target in targets {
        let downloader = downloader.clone();
        let dest = mods_dir.join(target.name()).with_extension("zip");
        let pb = mp.add(create_download_progress_bar(target.name(), target.size()));

        set.spawn(async move {
            downloader
                .download_with_fallbacks(&target, &dest, &pb)
                .await
        });
    }

    while let Some(result) = set.join_next().await {
        result??
    }
    Ok(())
}

/// Metadata of target mod to be downloaded.
#[derive(Debug, Clone)]
pub struct DownloadFile {
    /// Mirror name of the mod defined in `everest_update.yaml`.
    mirror_name: String,
    /// A name of the mod. Used for file name.
    name: FileStem,
    /// File size used for the progress bar.
    size: u64,
    /// A expected list of XxHash64.
    checksums: Checksums,
}

impl DownloadFile {
    fn mirror_name(&self) -> &str {
        &self.mirror_name
    }
    fn name(&self) -> &str {
        &self.name.0
    }
    fn size(&self) -> u64 {
        self.size
    }
    fn checksums(&self) -> &Checksums {
        &self.checksums
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseDownloadFileError {
    #[error(transparent)]
    Name(#[from] ParseNameError),
    #[error(transparent)]
    Checksum(#[from] ParseChecksumError),
}

impl TryFrom<&UpdateContext> for DownloadFile {
    type Error = ParseDownloadFileError;

    fn try_from(value: &UpdateContext) -> Result<Self, Self::Error> {
        let mirror_name = value.mirror_name();
        let name = FileStem::from_str(value.name())?;

        Ok(Self {
            mirror_name: mirror_name.to_string(),
            name,
            size: value.size(),
            checksums: value.checksums().to_owned(),
        })
    }
}

impl TryFrom<(String, Entry)> for DownloadFile {
    type Error = ParseDownloadFileError;

    fn try_from((name, entry): (String, Entry)) -> Result<Self, Self::Error> {
        let mirror_name = entry.mirror_name();
        let name = FileStem::from_str(&name)?;
        let checksums = entry
            .checksums()
            .iter()
            .map(|s| Checksum::from_str(s))
            .collect::<Result<Checksums, _>>()?;

        Ok(Self {
            mirror_name: mirror_name.to_string(),
            name,
            size: entry.file_size(),
            checksums,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseNameError {
    #[error(transparent)]
    NonAscii(#[from] utils::NonAsciiError),
}

#[derive(Debug, Clone)]
struct FileStem(String);

impl FromStr for FileStem {
    type Err = ParseNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let clean_s = utils::sanitize_stem(s)?;
        Ok(Self(clean_s))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to download the mod")]
    Network(#[from] reqwest::Error),
    #[error("failed to save the mod")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Hash(#[from] ChecksumVerificationError),
    #[error("failed to complete the concurrent tasks, canceled or panicked")]
    Join(#[from] JoinError),
    #[error("failed to acquire semaphore")]
    SemaphoreClosed(#[from] AcquireError),
    #[error("all mirrors failed for '{name}'")]
    AllMirrorsFailed {
        name: String,
        errors: Vec<(String, Error)>,
    },
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
            mirror_priority: Mirrors::from(args.mirror_priority),
        }
    }
}

impl ModDownloader {
    /// Retry downloading a file for given mirror urls until success or all mirrors are exhausted.
    async fn download_with_fallbacks(
        &self,
        item: &DownloadFile,
        dest: &Path,
        pb: &ProgressBar,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await?;

        let mut errors = Vec::new();

        let urls = &self.mirror_priority.resolve(item.mirror_name());

        for url in urls {
            match self.download(url, item, dest, pb).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    errors.push((url.clone(), e));
                    pb.reset();
                }
            }
        }

        Err(Error::AllMirrorsFailed {
            name: item.name().to_string(),
            errors,
        })
    }

    /// Downloads a file while hashing, verifying its integrity before final persistence.
    ///
    /// ### Note
    /// - Uses `tempfile` (typically in `tmpfs`) to avoid polluting the destination
    ///   with corrupt/partial data if verification fails.
    /// - Performs `tokio::fs::copy` instead of `tempfile::persist` because `temp_path` and `dest`
    ///   often reside on different filesystems (e.g., RAM vs. Disk).
    #[instrument(skip_all, fields(%url, ?item, path = %anonymize(dest)))]
    async fn download(
        &self,
        url: &str,
        item: &DownloadFile,
        dest: &Path,
        pb: &ProgressBar,
    ) -> Result<(), Error> {
        let response = self
            .client
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
        item.checksums().verify(&digest)?;

        // Finalize the download by copying across filesystem boundaries.
        tokio::fs::copy(temp_path, dest).await?;
        pb.finish_with_message(format!("{} 🍓", item.name()));
        Ok(())
    }
}

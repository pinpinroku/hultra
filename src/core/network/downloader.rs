//! Business logic to download mods.
use std::{
    collections::HashSet,
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
    core::{registry::Entry, update::UpdateTask},
    log::anonymize,
    mirror::{self, DomainMirror},
    ui::create_download_progress_bar,
    utils,
};

/// Metadata of target mod to be downloaded.
#[derive(Debug)]
pub struct DownloadTask {
    url: String,      // TODO define DownloadUrl to validate the value
    filename: String, // TODO sanitize when convert from (String, Entry)
    filesize: u64,
    checksums: Checksums,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksums(HashSet<Checksum>);

impl FromIterator<Checksum> for Checksums {
    fn from_iter<T: IntoIterator<Item = Checksum>>(iter: T) -> Self {
        Checksums(iter.into_iter().collect::<HashSet<Checksum>>())
    }
}

impl std::fmt::Display for Checksums {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, checksum) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", checksum)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Checksum(u64);

impl std::fmt::Display for Checksum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Hash mismatch: computed: {computed}, expected: {expected}")]
pub struct ChecksumVerificationError {
    computed: String,
    expected: Checksums,
}

pub trait ChecksumVerifier {
    fn verify(&self, target: &u64) -> Result<(), ChecksumVerificationError>;
}

impl ChecksumVerifier for Checksums {
    /// Verifies given checksums are equal.
    fn verify(&self, digest: &u64) -> Result<(), ChecksumVerificationError> {
        if self.0.contains(&Checksum(*digest)) {
            Ok(())
        } else {
            Err(ChecksumVerificationError {
                computed: format!("0x{:016x}", digest),
                expected: self.clone(),
            })
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid checksum: could not parse the '{input}' with digits in base 16")]
pub struct ChecksumError {
    pub(crate) input: String,
    #[source]
    pub(crate) source: std::num::ParseIntError,
}

impl TryFrom<String> for Checksum {
    type Error = ChecksumError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let i = utils::from_str_digest(&s).map_err(|err| ChecksumError {
            input: s.to_string(),
            source: err,
        })?;
        Ok(Self(i))
    }
}
// TODO Write tests
impl TryFrom<(String, Entry)> for DownloadTask {
    type Error = ChecksumError;

    fn try_from((filename, e): (String, Entry)) -> Result<Self, Self::Error> {
        let checksums = e
            .checksums
            .into_iter()
            .map(Checksum::try_from)
            .collect::<Result<Checksums, _>>()?;

        Ok(Self {
            url: e.url,
            filename,
            filesize: e.file_size,
            checksums,
        })
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

#[cfg(test)]
mod tests_checksum {
    use super::*;

    #[test]
    fn test_checksums_display() {
        let checksums = Checksums(HashSet::from_iter(vec![Checksum(123), Checksum(0xABCDEF)]));

        let display_string = checksums.to_string();

        let mut parts: Vec<_> = display_string.split(',').map(|s| s.trim()).collect();
        parts.sort();

        assert_eq!(parts, vec!["0x000000000000007b", "0x0000000000abcdef"]);
    }

    #[test]
    fn test_single_checksum_display() {
        let checksums = Checksums(HashSet::from_iter(vec![Checksum(0x123)]));
        assert_eq!(checksums.to_string(), "0x0000000000000123");
    }

    #[test]
    fn test_empty_checksums_display() {
        let checksums = Checksums(HashSet::new());
        assert_eq!(checksums.to_string(), "");
    }

    #[test]
    fn test_checksums_deduplication() {
        let raw = vec![Checksum(0xA), Checksum(0xB), Checksum(0xA)];
        let checksums: Checksums = raw.into_iter().collect();
        assert_eq!(checksums.0.len(), 2);
    }

    #[test]
    fn test_checksum_try_from_invalid_string() {
        let invalid = "not_a_hex_string".to_string();
        let result = Checksum::try_from(invalid);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod tests_checksum_verification {
    use super::*;

    fn setup_checksums(values: Vec<u64>) -> Checksums {
        Checksums(values.into_iter().map(Checksum).collect())
    }

    #[test]
    fn test_verify_success() {
        let checksums = setup_checksums(vec![0x123, 0xABC]);

        assert!(checksums.verify(&0x123).is_ok());
        assert!(checksums.verify(&0xABC).is_ok());
    }

    #[test]
    fn test_verify_mismatch() {
        let checksums = setup_checksums(vec![0x111]);
        let computed_val = 0x222;

        let result = checksums.verify(&computed_val);

        assert!(result.is_err());

        if let Err(e) = result {
            assert_eq!(e.computed, "0x0000000000000222");
            assert!(e.expected.0.contains(&Checksum(0x111)));

            let err_msg = e.to_string();
            assert!(err_msg.contains("computed: 0x0000000000000222"));
            assert!(err_msg.contains("expected: 0x0000000000000111"));
        }
    }

    #[test]
    fn test_verify_empty() {
        let checksums = setup_checksums(vec![]);
        assert!(checksums.verify(&0x123).is_err());
    }
}

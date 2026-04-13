#![allow(dead_code)]
use std::path::PathBuf;

use tokio::{sync::AcquireError, task::JoinError};

use crate::{
    core::{ChecksumVerificationError, Checksums, update::UpdateTask},
    service::archive::ExtractError,
};

pub mod mods;

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("failed to download the mod")]
    Network(#[from] reqwest::Error),
    #[error("failed to save the mod")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Hash(#[from] ChecksumVerificationError),
    #[error("failed to extract Everest to the root directory")]
    Archive(#[from] ExtractError),
    #[error("failed to complete the concurrent tasks, canceld or panicked")]
    Join(#[from] JoinError),
    #[error("failed to acquire semaphore")]
    SemaphoreClosed(#[from] AcquireError),
    #[error("all mirrors failed for '{name}'")]
    AllMirrorsFailedError {
        name: String,
        errors: Vec<(String, DownloadError)>,
    },
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

/// ### Example
/// ```ignore
/// let config = AppConfig::new();
/// let output_dir = match {
///     Standard {name, ..} => config.mods_dir().join(name),
///     Archive {..} => config.root_dir(),
/// }
/// ```
enum DownloadMode {
    Standard {
        url: String,
        name: String,
        size: u64,
        checksums: Checksums,
    },
    Archive {
        url: String,
        size: u64,
    },
}

struct DownloadOptions {
    verify_checksum: bool,
    progress: ProgressKind,
}

enum ProgressKind {
    ProgressBar,
    Spinner,
    None,
}

pub struct DownloadResult {
    pub final_path: PathBuf,
    pub bytes_downloaded: u64,
    pub checksum_verified: bool,
}

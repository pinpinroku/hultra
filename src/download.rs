use std::path::PathBuf;

use crate::{
    core::{ChecksumVerificationError, Checksums},
    service::archive::ExtractError,
};

pub mod everest;
pub mod mods;

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("failed to download Everest")]
    Network(#[from] reqwest::Error),
    #[error("failed to save Everest to the path")]
    Io(#[from] std::io::Error),
    #[error("failed to save Everest to the path")]
    Archive(#[from] ExtractError),
    #[error(transparent)]
    Checksum(#[from] ChecksumVerificationError),
}

enum DownloadMode {
    Standard {
        url: String,
        name: String,
        checksums: Checksums,
    },
    Archive {
        url: String,
        filesize: u64,
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

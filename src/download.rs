#![allow(dead_code)]
use std::path::PathBuf;

pub mod mods;

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

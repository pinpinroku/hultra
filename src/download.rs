#![allow(dead_code)]
use std::path::PathBuf;

use crate::core::Checksums;

pub mod mods;

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

use std::path::PathBuf;

use reqwest::Url;
use thiserror::Error;

/// The `Error` enum defines all possible error types that can occur in the application.
#[derive(Debug, Error)]
pub enum Error {
    /// Represents an I/O error, transparently wrapping `std::io::Error`
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Represents a network request error, transparently wrapping `reqwest::Error`
    #[error(transparent)]
    Request(#[from] reqwest::Error),

    /// Error indicating that a checksum verification failed for a specific file
    #[error(
        "checksum verification failed for '{file}':\
        computed checksum '{computed}' does not match\
        expected checksums {expected:#?}"
    )]
    InvalidChecksum {
        /// The file for which checksum verification failed
        file: PathBuf,
        /// The computed checksum of the file
        computed: String,
        /// A list of expected checksums for the file
        expected: Vec<String>,
    },
}

#[derive(Debug, Error)]
pub enum ModPageUrlParseError {
    /// Invalid URL
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// Unsupported scheme
    #[error("unsupported scheme: {0} (expected 'http' or 'https')")]
    UnsupportedScheme(String),

    /// Invalid GameBanana URL
    #[error("invalid GameBanana URL :{0:?}")]
    InvalidGameBananaUrl(Url),

    /// Invalid Mod ID
    #[error("invalid Mod ID :{0} (expected unsigned 32 bit integer)")]
    InvalidModId(String),
}

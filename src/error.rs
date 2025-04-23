use std::path::PathBuf;

use thiserror::Error;

/// The `Error` enum defines all possible error types that can occur in the application.
#[derive(Debug, Error)]
pub enum Error {
    /// Represents an I/O error, transparently wrapping `std::io::Error`
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Represents a ZIP archive error, transparently wrapping `zip::result::ZipError`
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),

    /// Represents a YAML parsing error, transparently wrapping `serde_yaml_ng::Error`
    #[error(transparent)]
    Yaml(#[from] serde_yaml_ng::Error),

    /// Represents a network request error, transparently wrapping `reqwest::Error`
    #[error(transparent)]
    Request(#[from] reqwest::Error),

    /// Error indicating that the home directory could not be determined
    #[error(
        "Could not determine home directory location!\
        Please specify the mods directory using --mods-dir"
    )]
    CouldNotDetermineHomeDir,

    /// Error indicating that the mods directory is missing
    #[error(
        "No mods directory found.\
        Please verify that Everest is properly installed"
    )]
    MissingModsDirectory,

    /// Error indicating that a checksum verification failed for a specific file
    #[error(
        "Checksum verification failed for '{file}':\
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

    /// Error indicating that a file is not hashed
    #[error("The file is not hashed. It seems the developer made mistake.")]
    FileIsNotHashed,
}

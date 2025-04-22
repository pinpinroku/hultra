use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(
        "Could not determine home directory location!\
        Please specify the mods directory using --mods-dir"
    )]
    CouldNotDetermineHomeDir,
    #[error(
        "No mods directory found.\
        Please verify that Everest is properly installed"
    )]
    MissingModsDirectory,
    #[error(
        "Checksum verification failed for '{file}':\
        computed checksum '{computed}' does not match\
        expected checksums {expected:#?}"
    )]
    InvalidChecksum {
        file: PathBuf,
        computed: String,
        expected: Vec<String>,
    },
    #[error("The file is not hashed. It seems the developer made mistake.")]
    FileIsNotHashed,
}

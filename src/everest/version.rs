use std::{fs, io, path::Path};

use super::{Branch, EverestBuild};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read version file")]
    Io(#[from] std::io::Error),
    #[error("invalid version string '{version}' in the `update-build.txt`: {source}")]
    InvalidVersion {
        source: std::num::ParseIntError,
        version: String,
    },
    #[error("version file not found in the root directory")]
    VersionFileNotFound,
    #[error("version file does not contain any strings")]
    VersionTextNotFound,
}

/// Ensures installed version.
pub fn ensure_installed_version(root_dir: &Path) -> Result<u32, Error> {
    get_installed_version(root_dir)
}

/// Returns installed Everst version number.
///
/// ## Errors
///
/// * VersionFileNotFound: If version file is not found in the path, it indicates Everest is not installed.
/// * VersionTextNotFound: If version file does not contain any strings. This will not going to happen. Corrupt Everest installation.
/// * InvalidVersion: If version number can not be parsed as unsigned 32 bit integer. It might occur if the beta or dev version is used but I'm not sure.
fn get_installed_version(root_dir: &Path) -> Result<u32, Error> {
    let path = root_dir.join("update-build.txt");

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(Error::VersionFileNotFound),
        Err(e) => return Err(e.into()),
    };

    let content = content.trim();
    if content.is_empty() {
        return Err(Error::VersionTextNotFound);
    }

    content.parse::<u32>().map_err(|e| Error::InvalidVersion {
        source: e,
        version: content.to_string(),
    })
}

pub fn get_installed_branch<'a>(builds: &'a [EverestBuild], version: &u32) -> Option<&'a Branch> {
    builds
        .iter()
        .find(|b| &b.version == version)
        .map(|b| &b.branch)
}

/// Returns latest build on given branch.
pub fn get_latest_build_on_branch<'a>(
    builds: &'a [EverestBuild],
    branch: &Branch,
) -> Option<&'a EverestBuild> {
    builds
        .iter()
        .filter(|b| &b.branch == branch)
        .max_by_key(|b| b.version)
}

use std::{fs, io, path::Path};

use super::{Branch, EverestBuild};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid version string '{version}' in the `update-build.txt`: {source}")]
    InvalidVersion {
        source: std::num::ParseIntError,
        version: String,
    },
    #[error("version text not found in the root directory")]
    VersionNotFound,
}

/// Ensures installed version.
pub fn ensure_installed_version(root_dir: &Path) -> Result<u32, Error> {
    get_installed_version(root_dir)?.ok_or(Error::VersionNotFound)
}

/// Returns currently installed Everst version number if the file is found.
///
/// This function will returns None if the file is empty or not found on the path.
/// Returns InvalidVersion error when the version number cannot be parsed as unsigned 32 bit integer.
fn get_installed_version(root_dir: &Path) -> Result<Option<u32>, Error> {
    let path = root_dir.join("update-build.txt");

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    trimmed
        .parse::<u32>()
        .map(Some)
        .map_err(|e| Error::InvalidVersion {
            source: e,
            version: trimmed.to_string(),
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

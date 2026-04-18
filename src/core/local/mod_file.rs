use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

/// Represents a validated path to a mod file, typically a `.zip` archive.
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct ModFile(PathBuf);

impl ModFile {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    #[cfg(test)]
    pub fn new_unchecked(path: PathBuf) -> Self {
        Self(path)
    }
}

impl ModFile {
    pub fn is_blacklisted(&self, blacklist: &HashSet<String>) -> bool {
        self.0
            .file_name()
            .and_then(|n| n.to_str())
            .map(|name| blacklist.contains(name))
            .unwrap_or(false)
    }
}

pub trait ModIdentityService {
    fn fetch_id(&self, path: &Path) -> io::Result<u64>;
}

/// A service for discovering mod files within a directory.
pub(super) trait ModFileSource {
    /// Returns a list of valid mod files.
    fn fetch_all(&self) -> io::Result<Vec<ModFile>>;
}

/// A standard implementation of [`ModFileSource`] that interacts with the local file system.
#[derive(Debug)]
pub(super) struct LocalModFileSource {
    mods_dir: PathBuf,
}

impl LocalModFileSource {
    pub(super) fn new(mods_dir: impl Into<PathBuf>) -> Self {
        Self {
            mods_dir: mods_dir.into(),
        }
    }
}

impl ModFileSource for LocalModFileSource {
    fn fetch_all(&self) -> io::Result<Vec<ModFile>> {
        let found_paths = fs::read_dir(&self.mods_dir)?
            .flatten()
            .filter(|e| {
                e.file_type().is_ok_and(|ft| ft.is_file())
                    && e.path()
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
            })
            .map(|e| ModFile::from(e.path()))
            .collect();
        Ok(found_paths)
    }
}

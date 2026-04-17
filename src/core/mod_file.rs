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
pub trait ModsDirectoryScanner {
    fn scan(&self, mods_dir: &Path) -> io::Result<Vec<ModFile>>;
}

/// A standard implementation of [`ModsDirectoryScanner`] that interacts with the local file system.
pub struct LocalFileSystemScanner;

impl ModsDirectoryScanner for LocalFileSystemScanner {
    /// Scans the specified directory and returns a list of valid mod files.
    fn scan(&self, mods_dir: &Path) -> io::Result<Vec<ModFile>> {
        let found_paths = fs::read_dir(mods_dir)?
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

use std::{
    borrow::Cow,
    fmt, fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

mod manifest;
mod resolver;

pub use resolver::scan_mods;

use crate::core::blacklist::UpdaterBlacklist;

/// Information of installed mod.
#[derive(Debug, Clone)]
pub struct LocalMod {
    /// Full path to the ZIP archive of the mod.
    file: ModFile,
    /// Mod name.
    name: String,
    /// Version label of the mod to display.
    version: DisplayVersion,
}

#[derive(Debug, Clone)]
struct DisplayVersion(String);

impl std::fmt::Display for DisplayVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.0)
    }
}

impl LocalMod {
    pub fn new(file: ModFile, name: String, version: String) -> Self {
        Self {
            file,
            name,
            version: DisplayVersion(version),
        }
    }

    pub fn file(&self) -> &ModFile {
        &self.file
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version.0
    }
}

impl fmt::Display for LocalMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self
            .file()
            .path()
            .file_stem()
            .is_some_and(|name| name.eq_ignore_ascii_case(self.name()))
        {
            write!(f, "{} (v{})", self.name(), self.version())?;
        } else {
            let filename = self
                .file()
                .path()
                .file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or(Cow::Borrowed("unknown"));

            write!(f, "*{} (v{}) [{}]", self.name(), self.version(), filename)?;
        }
        Ok(())
    }
}

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
    pub fn is_blacklisted(&self, blacklist: &UpdaterBlacklist) -> bool {
        self.0
            .file_name()
            .and_then(|n| n.to_str())
            .map(|name| blacklist.filenames().contains(name))
            .unwrap_or(false)
    }
}

pub trait ModIdentityService {
    /// Fetches inode of the file.
    fn fetch_id(&self, path: &Path) -> io::Result<u64>;
}

pub struct LocalFileSystemService;

impl ModIdentityService for LocalFileSystemService {
    fn fetch_id(&self, path: &Path) -> io::Result<u64> {
        path.metadata().map(|m| m.ino())
    }
}

#[cfg(test)]
pub struct MockFileSystemService {
    pub should_fail: bool,
}

#[cfg(test)]
impl ModIdentityService for MockFileSystemService {
    fn fetch_id(&self, _path: &Path) -> io::Result<u64> {
        if self.should_fail {
            Err(io::Error::other("intentional error"))
        } else {
            Ok(12345)
        }
    }
}

/// A service for discovering mod files within a directory.
trait ModFileSource {
    /// Returns a list of valid mod files.
    fn fetch_all(&self) -> io::Result<Vec<ModFile>>;
}

/// A standard implementation of [`ModFileSource`] that interacts with the local file system.
#[derive(Debug)]
struct LocalModFileSource {
    mods_dir: PathBuf,
}

impl LocalModFileSource {
    fn new(mods_dir: impl Into<PathBuf>) -> Self {
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

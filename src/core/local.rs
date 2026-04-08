use std::{
    borrow::Cow,
    fmt, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use crate::manifest::Manifest;

/// Information of installed mod.
#[derive(Debug)]
pub struct LocalMod {
    /// Full path for the ZIP archive of the mod.
    path: PathBuf,
    /// Mod name.
    name: String,
    /// Version label of the mod to display.
    version: DisplayVersion,
}

#[derive(Debug)]
struct DisplayVersion(String);

impl std::fmt::Display for DisplayVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.0)
    }
}

impl LocalMod {
    pub fn new(path: &Path, manifest: Manifest) -> Self {
        // TODO path.is_absolute()
        // TODO sanitize mod name as file name
        Self {
            path: path.to_path_buf(),
            name: manifest.name,
            version: DisplayVersion(manifest.version),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version.0
    }
}

pub trait FileSystemExt {
    /// Gets current inode from path.
    fn fetch_inode(&self) -> io::Result<u64>;
}

impl FileSystemExt for LocalMod {
    fn fetch_inode(&self) -> io::Result<u64> {
        self.path.metadata().map(|m| m.ino())
    }
}

impl fmt::Display for LocalMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self
            .path()
            .file_stem()
            .is_some_and(|name| name.eq_ignore_ascii_case(self.name()))
        {
            write!(f, "{} (v{})", self.name(), self.version())?;
        } else {
            let filename = self
                .path()
                .file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or(Cow::Borrowed("unknown"));

            write!(f, "*{} (v{}) [{}]", self.name(), self.version(), filename)?;
        }
        Ok(())
    }
}

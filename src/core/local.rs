use std::{
    borrow::Cow,
    fmt, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

/// Information of installed mod.
#[derive(Debug, Clone)]
pub struct LocalMod {
    /// Full path for to the ZIP archive of the mod.
    path: PathBuf,
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the path should be absolute")]
    PathIsRelative,
}

impl LocalMod {
    pub fn new(path: &Path, name: String, version: String) -> Result<Self, Error> {
        if !path.is_absolute() {
            return Err(Error::PathIsRelative);
        }
        Ok(Self {
            path: path.to_path_buf(),
            name,
            version: DisplayVersion(version),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_mod_creation() {
        let result = LocalMod::new(
            Path::new("./SpeedrunTool.zip"),
            "SpeedrunTool".into(),
            "1.0.1".into(),
        );
        assert!(result.is_err_and(|e| e.to_string().contains("should be absolute")))
    }
}

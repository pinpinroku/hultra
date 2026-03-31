//! Locally installed mods information.
use std::{
    borrow::Cow,
    collections::VecDeque,
    fmt, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use serde::Deserialize;

/// Information of locally installed mod.
#[derive(Debug)]
pub struct LocalMod {
    /// Full path of the mod.
    path: PathBuf,
    /// Metadata of the mod.
    manifest: Manifest,
}

impl LocalMod {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    pub fn new(path: &Path, manifest: Manifest) -> Self {
        Self {
            path: path.to_path_buf(),
            manifest,
        }
    }
}

/// Represents the `everest.yaml`; metadata of the mod.
#[derive(Debug, Default, Deserialize)]
pub struct Manifest {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: String,
}

impl Manifest {
    /// Deserializes an instance of Manifest from bytes of YAML text.
    pub fn from_slice(buffer: &[u8]) -> Result<VecDeque<Self>, serde_yaml_ng::Error> {
        // Remove UTF-8 BOM if present
        let clean_slice = buffer.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(buffer);

        // NOTE Use `VecDeque` for efficient `pop_front` operation (`O(1)` vs `Vec::remove(0)` which is `O(n)`)
        let manifest = serde_yaml_ng::from_slice(clean_slice)?;
        Ok(manifest)
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
mod tests_manifest_parsing {
    use anyhow::{Context, Result};

    use super::*;

    #[test]
    fn test_parse_manifest() -> Result<()> {
        let bytes = br#"
- Name: darkmoonruins
  Version: 1.1.4
  Dependencies:
    - Name: AvBdayHelper2021
      Version: 1.0.2
    - Name: CherryHelper
      Version: 1.7.1
    - Name: CollabUtils2
      Version: 1.6.13
"#;
        let manifest = Manifest::from_slice(bytes);
        assert!(manifest.is_ok());

        let mut manifest = manifest.context("failed to parse manifest from YAML")?;
        let primary = manifest.pop_front().context("should be at least one")?;
        assert_eq!(primary.name, "darkmoonruins");
        assert_eq!(primary.version, "1.1.4");
        Ok(())
    }
}

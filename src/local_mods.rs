//! Locally installed mods information.
use std::{
    borrow::Cow,
    collections::VecDeque,
    fmt, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use tracing::instrument;

use crate::config::AppConfig;

#[derive(Debug)]
pub struct LocalMod {
    /// A full path of the mod installed.
    path: PathBuf,
    /// A metadata of the mod.
    manifest: Manifest,
}

impl fmt::Display for LocalMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self
            .path()
            .file_stem()
            .is_some_and(|name| name.eq_ignore_ascii_case(self.name()))
        {
            write!(f, "✅ {} (v{})", self.name(), self.version())?;
        } else {
            let filename = self
                .path()
                .file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or(Cow::Borrowed("unknown"));

            writeln!(f, "ℹ️ {} (v{})", self.name(), self.version(),)?;
            write!(f, "📂 {}", filename)?;
        }
        Ok(())
    }
}

impl LocalMod {
    /// Returns a value of this type from the given file path by extracting and parsing the manifest.
    fn from_path(mod_path: &Path) -> Option<Self> {
        let Ok(manifest_bytes) = zip_finder::extract_file_from_zip(
            mod_path,
            b"everest.yaml",
            Some(b"everest.yml"),
        )
        .inspect_err(
            |err| tracing::error!(?err, file_name = ?mod_path.file_name(), "manifest is missing"),
        ) else {
            return None;
        };

        let Ok(mut manifest) = Manifest::from_slice(&manifest_bytes)
            .inspect(|manifest| tracing::debug!(?manifest, file_name = ?mod_path.file_name()))
            .inspect_err(|err| tracing::error!(?err, file_name = ?mod_path.file_name()))
        else {
            return None;
        };

        manifest.pop_front().map(|value| Self {
            path: mod_path.to_path_buf(),
            manifest: value,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    /// Gets current inode of path.
    pub fn get_inode(&self) -> io::Result<u64> {
        let meta = self.path.metadata()?;
        let inode = meta.ino();
        Ok(inode)
    }

    pub fn get_file_name(&self) -> Cow<'_, str> {
        self.path.file_name().unwrap_or_default().to_string_lossy()
    }

    /// Creates values of this type for each path of given paths in parallel.
    #[instrument(skip_all)]
    pub fn load_local_mods(config: &AppConfig) -> io::Result<Vec<Self>> {
        use rayon::prelude::*;

        let archive_paths = config.read_mods_dir()?;

        let local_mods: Vec<Self> = archive_paths
            .par_iter()
            .filter_map(|archive_path| Self::from_path(archive_path))
            .collect();

        tracing::debug!(
            detected_archives = archive_paths.len(),
            found_mods = local_mods.len()
        );

        Ok(local_mods)
    }
}

/// Represents the `everest.yaml`; metadata of the mod.
#[derive(Debug, Default, Deserialize)]
struct Manifest {
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

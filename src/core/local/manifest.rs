//! Raw data of `everest.yaml`.
use std::{collections::VecDeque, path::Path};

use serde::Deserialize;

/// Represents the metadata of mod.
#[derive(Debug, Default, Deserialize)]
pub(super) struct Manifest {
    #[serde(rename = "Name")]
    pub(super) name: String,
    #[serde(rename = "Version")]
    pub(super) version: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestParseError {
    #[error("manifest is parsed successfully but no entries found on the file")]
    NoEntry,
    #[error("failed to deserialize bytes as `everest.yaml`")]
    InvalidYamlStructure(#[from] serde_yaml_ng::Error),
}

impl TryFrom<Vec<u8>> for Manifest {
    type Error = ManifestParseError;

    fn try_from(buffer: Vec<u8>) -> Result<Self, Self::Error> {
        // Remove UTF-8 BOM if present
        let clean_slice = buffer.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&buffer);

        // NOTE Use `VecDeque` for efficient `pop_front` operation (`O(1)` vs `Vec::remove(0)` which is `O(n)`)
        let mut manifests: VecDeque<Manifest> = serde_yaml_ng::from_slice(clean_slice)?;

        manifests.pop_front().ok_or(ManifestParseError::NoEntry)
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
        let manifest = Manifest::try_from(bytes.to_vec());
        assert!(manifest.is_ok());

        let manifest = manifest.context("failed to parse manifest from YAML")?;
        assert_eq!(manifest.name, "darkmoonruins");
        assert_eq!(manifest.version, "1.1.4");
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MetadataReadError {
    #[error(transparent)]
    Archive(#[from] zip_finder::Error),
    #[error(transparent)]
    Parse(#[from] ManifestParseError),
}

pub trait MetadataReader {
    fn read_metadata(&self, path: &Path) -> Result<Manifest, MetadataReadError>;
}

#[derive(Debug, Clone)]
pub(super) struct LocalMetadataReader;

impl MetadataReader for LocalMetadataReader {
    fn read_metadata(&self, path: &Path) -> Result<Manifest, MetadataReadError> {
        let bytes = zip_finder::extract_file_from_zip(path, b"everest.yaml", Some(b"everest.yml"))?;
        let manifest = bytes.try_into()?;
        Ok(manifest)
    }
}

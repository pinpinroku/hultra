use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::core::{
    Checksum, ChecksumError, Checksums, network::downloader::DownloadTask, update::UpdateTask,
};

/// Mod database. The key of main map is the mod name.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct EverestUpdateYaml {
    entries: HashMap<String, Entry>,
}

/// Metadata of the mod.
#[derive(Debug, Clone, Default, Deserialize)]
struct Entry {
    /// This is a group ID of the map. It is unique but shared with assets.
    #[serde(rename = "GameBananaId")]
    id: u32,
    /// Version string. This value may not follow any specific versioning scheme. Do not expect it to be SemVer.
    #[serde(rename = "Version")]
    version: String,
    /// Download link for the mod file.
    #[serde(rename = "URL")]
    url: String,
    /// File size of the mod file, a.k.a. `Content-Length`.
    #[serde(rename = "Size")]
    file_size: u64,
    /// XxHash checksums for the file. (e.g. "f437bf0515368130")
    #[serde(rename = "xxHash")]
    checksums: Vec<String>,
}

impl EverestUpdateYaml {
    // Lenear search. `O(n)`
    pub fn get_names_by_ids(&self, ids: &HashSet<u32>) -> HashSet<String> {
        self.entries
            .iter()
            .filter(|(_, e)| ids.contains(&e.id))
            .map(|(n, _)| n.to_owned())
            .collect()
    }

    /// Converts Entry to the context for downloads.
    pub fn create_download_tasks(
        mut self,
        names: HashSet<String>,
    ) -> Result<Vec<DownloadTask>, ChecksumError> {
        names
            .into_iter()
            .filter_map(|name| {
                self.entries
                    .remove(&name)
                    .map(|entry| DownloadTask::try_from((name, entry)))
            })
            .collect()
    }

    pub fn create_update_task(&mut self, name: &str) -> Option<Result<UpdateTask, ChecksumError>> {
        self.entries.remove_entry(name).map(UpdateTask::try_from)
    }
}

impl TryFrom<(String, Entry)> for UpdateTask {
    type Error = ChecksumError;

    fn try_from((name, entry): (String, Entry)) -> Result<UpdateTask, Self::Error> {
        let checksums = entry
            .checksums
            .into_iter()
            .map(Checksum::try_from)
            .collect::<Result<Checksums, _>>()?;
        Ok(Self {
            name,
            version: entry.version,
            url: entry.url,
            size: entry.file_size,
            checksums,
        })
    }
}

impl TryFrom<(String, Entry)> for DownloadTask {
    type Error = ChecksumError;

    fn try_from((filename, entry): (String, Entry)) -> Result<Self, Self::Error> {
        let checksums = entry
            .checksums
            .into_iter()
            .map(Checksum::try_from)
            .collect::<Result<Checksums, _>>()?;
        Ok(Self {
            url: entry.url,
            filename,
            filesize: entry.file_size,
            checksums,
        })
    }
}

#[cfg(test)]
mod tests_registry {
    use super::*;

    const YAML_BYTES: &[u8; 670] = br#"
puppyposting:
  GameBananaType: Mod
  Version: 1.1.0
  LastUpdate: 1758235322
  Size: 13937408
  GameBananaId: 619550
  GameBananaFileId: 1520739
  xxHash:
  - 7f4d96733b93c52c
  URL: https://gamebanana.com/mmdl/1520739
BreezeContest:
  GameBananaType: Mod
  Version: 1.1.2
  LastUpdate: 1760568856
  Size: 234447819
  GameBananaId: 554453
  GameBananaFileId: 1539722
  xxHash:
  - e4d62f4733631949
  URL: https://gamebanana.com/mmdl/1539722
BreezeContestAudio:
  GameBananaType: Mod
  Version: 1.0.1
  LastUpdate: 1731192314
  Size: 707675460
  GameBananaId: 554453
  GameBananaFileId: 1318934
  xxHash:
  - de98a344ea44aea4
  URL: https://gamebanana.com/mmdl/1318934
"#;

    fn load_registry_from_yaml() -> EverestUpdateYaml {
        serde_yaml_ng::from_slice(YAML_BYTES).expect("YAML format should be parsed")
    }

    #[test]
    fn test_mod_registry_from_slice_and_mods() {
        let registry = load_registry_from_yaml();
        let mods = registry.entries;
        let target = mods.get("puppyposting");
        assert!(target.is_some_and(|mod_info| {
            mod_info.id == 619550 && mod_info.url == "https://gamebanana.com/mmdl/1520739"
        }));
    }

    #[test]
    fn test_get_mod_names_by_id() {
        let registry = load_registry_from_yaml();
        let ids: HashSet<u32> = HashSet::from_iter([619550]);
        let names = registry.get_names_by_ids(&ids);
        assert!(!names.is_empty());
        assert!(names.contains("puppyposting"))
    }

    #[test]
    fn test_get_mod_names_by_id_multiple() {
        let registry = load_registry_from_yaml();
        let ids: HashSet<u32> = HashSet::from_iter([554453]);
        let result = registry.get_names_by_ids(&ids);
        assert!(
            result.len() == 2
                && result.contains("BreezeContest")
                && result.contains("BreezeContestAudio")
        );
    }
}

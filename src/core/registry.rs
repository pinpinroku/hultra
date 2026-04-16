use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::core::{
    LocalMod,
    mod_file::ModIdentityService,
    network::downloader::{DownloadFile, ParseDownloadFileError},
    update::UpdateContext,
};

/// Mod database. The key of main map is the mod name.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct EverestUpdateYaml {
    entries: HashMap<String, Entry>,
}

/// Metadata of the mod.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Entry {
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

impl Entry {
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn file_size(&self) -> u64 {
        self.file_size
    }
    pub fn checksums(&self) -> &[String] {
        &self.checksums
    }
}

impl EverestUpdateYaml {
    /// Returns names corresponding to the given IDs using a linear search.
    ///
    /// Note: While this has O(n) complexity, it is more performant than
    /// building an inverted index for the expected workload.
    pub fn get_names_by_ids(&self, ids: &HashSet<u32>) -> HashSet<String> {
        self.entries
            .iter()
            .filter(|(_, e)| ids.contains(&e.id))
            .map(|(n, _)| n.to_owned())
            .collect()
    }

    /// Converts Entry to the items for downloads.
    pub fn into_download_files(
        mut self,
        required_names: HashSet<String>,
        installed_names: HashSet<String>,
    ) -> Result<Vec<DownloadFile>, ParseDownloadFileError> {
        let missing_names: HashSet<String> = required_names
            .into_iter()
            .filter(|name| !installed_names.contains(name))
            .collect();

        missing_names
            .into_iter()
            .filter_map(|name| {
                self.entries
                    .remove(&name)
                    .map(|entry| DownloadFile::try_from((name, entry)))
            })
            .collect()
    }

    pub fn into_update_context(
        &mut self,
        local_mods: &[LocalMod],
        service: impl ModIdentityService,
    ) -> Vec<UpdateContext> {
        local_mods
            .iter()
            .filter_map(|m| {
                let (n, e) = self.entries.remove_entry(m.name())?;
                let inode = service.fetch_id(m.file().path()).ok()?;
                let task = UpdateContext::new(m.version(), inode, n, e).ok()?;
                Some(task)
            })
            .collect()
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

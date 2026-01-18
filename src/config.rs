use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

use tracing::{error, info, warn};

use crate::{cli::Mirror, download::DatabaseUrlSet};

pub const CARGO_PKG_NAME: &str = env!("CARGO_PKG_NAME");
pub const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const STEAM_MODS_DIRECTORY: &str = ".local/share/Steam/steamapps/common/Celeste/Mods/";
// NOTE maybe add default path of epic games and itch.io in the future

#[derive(thiserror::Error, Debug)]
pub enum AppConfigError {
    #[error("failed to determine user home directory from environment variable")]
    DetermineHomeDirectory,
}

/// Application config.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// A mods directory where all of mod files stored.
    mods_dir: PathBuf,

    /// A path to the cache file which caches file hashes.
    cache_db_path: PathBuf,

    /// A type of database URL. (Primary or Mirror)
    api_url_type: DatabaseUrlSet,

    /// A priority of mirror list for downloading.
    mirror_priority: Vec<Mirror>,
}

impl AppConfig {
    pub fn new(
        mods_dir: Option<&Path>,
        use_api_mirror: bool,
        mirror_priority: Vec<Mirror>,
    ) -> Result<Self, AppConfigError> {
        // Determine user home directory
        let Some(home) = env::home_dir() else {
            return Err(AppConfigError::DetermineHomeDirectory);
        };

        let cache_db_path = env::var("XDG_STATE_HOME")
            .map(|value| value.into())
            .unwrap_or_else(|_| home.join(".local").join("state"))
            .join(CARGO_PKG_NAME)
            .join("checksum")
            .with_extension("cache");

        let api_url_type = if use_api_mirror {
            DatabaseUrlSet::Mirror
        } else {
            DatabaseUrlSet::Primary
        };

        let mods_dir = mods_dir
            .map(|dir| dir.into())
            .unwrap_or_else(|| home.join(STEAM_MODS_DIRECTORY));

        Ok(Self {
            mods_dir,
            cache_db_path,
            api_url_type,
            mirror_priority,
        })
    }

    pub fn mods_dir(&self) -> &Path {
        &self.mods_dir
    }

    pub fn cache_db_path(&self) -> &Path {
        &self.cache_db_path
    }

    pub fn url_set(&self) -> &DatabaseUrlSet {
        &self.api_url_type
    }

    pub fn mirror_priority(&self) -> &Vec<Mirror> {
        &self.mirror_priority
    }

    /// Returns a list of archive path by scanning mods directory.
    pub fn read_mods_dir(&self) -> io::Result<Vec<PathBuf>> {
        info!("scan mods directory for archives");

        let found_paths: Vec<PathBuf> = fs::read_dir(&self.mods_dir)
            .inspect_err(|err| error!(?err, "failed to read mods directory"))?
            .filter_map(|res| {
                res.inspect_err(|err| warn!(?err, "failed to read entry"))
                    .map(|entry| entry.path())
                    .ok() // Some(PathBuf)
            })
            .filter(|path| is_mod_archive(path))
            .collect();

        info!(found_archives = found_paths.len());

        Ok(found_paths)
    }

    const UPDATER_BLACKLIST_FILE: &str = "updaterblacklist.txt";

    /// Returns paths of blacklisted mod by reading `updaterblacklist.txt`.
    pub fn read_updater_blacklist(&self) -> io::Result<HashSet<String>> {
        info!("reading updater blacklist");

        let path = self.mods_dir.join(Self::UPDATER_BLACKLIST_FILE);
        let mut blacklist = HashSet::new();

        let mut file = match File::open(&path) {
            Ok(value) => value,
            // NOTE the file might be missing but it's ok, just returns empty list
            Err(err) if err.kind().eq(&io::ErrorKind::NotFound) => return Ok(blacklist),
            Err(err) => {
                error!(?err, "failed to open blacklist file");
                return Err(err);
            }
        };

        // NOTE default 8KiB buffer is too large to read simple text file with few lines
        let reader = BufReader::with_capacity(1024, &mut file);

        for line in reader.lines() {
            let line = line.inspect_err(|err| error!(?err, "failed to read line in blacklist"))?;
            let line = line.trim();
            if !line.starts_with('#') && !line.is_empty() {
                info!("'{}' will be excluded from updates", line);
                blacklist.insert(line.to_string());
            }
        }

        Ok(blacklist)
    }
}

fn is_mod_archive(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}

#[cfg(test)]
mod test {
    use super::*;

    use std::fs::File;

    use tempfile::TempDir;

    #[test]
    fn test_is_mod_archive() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;

        let valid_target = temp_dir.path().join("SpeedrunTool.zip");
        let valid_target_upper = temp_dir.path().join("SPEEDRUNTOOL.ZIP");
        let blacklist_file = temp_dir.path().join("updaterblacklist.txt");
        let cache_dir = temp_dir.path().join("Cache/");
        let custom_mod = temp_dir.path().join("LocalCustomMod/");

        File::create_new(&valid_target)?;
        File::create_new(&valid_target_upper)?;
        File::create_new(&blacklist_file)?;
        fs::create_dir(&cache_dir)?;
        fs::create_dir(&custom_mod)?;

        assert!(is_mod_archive(&valid_target));
        assert!(is_mod_archive(&valid_target_upper));
        assert!(!is_mod_archive(&blacklist_file));
        assert!(!is_mod_archive(&cache_dir));
        assert!(!is_mod_archive(&custom_mod));

        Ok(())
    }
}

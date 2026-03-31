use std::{
    collections::HashSet,
    env,
    fs::File,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

use tracing::{error, warn};

pub const CARGO_PKG_NAME: &str = env!("CARGO_PKG_NAME");
pub const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const STEAM_GAME_DIRECTORY: &str = ".local/share/Steam/steamapps/common/Celeste/";

#[derive(thiserror::Error, Debug)]
pub enum AppConfigError {
    #[error("failed to determine user home directory from environment variable")]
    DetermineHomeDirectory,
}

/// Application configuration.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Directory where `Celeste.exe` is installed originally.
    root_dir: PathBuf,

    /// Path to the file hash cache.
    cache_db_path: PathBuf,
}

impl AppConfig {
    pub fn new(directory: Option<&Path>) -> Result<Self, AppConfigError> {
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

        let root_dir = directory
            .map(|dir| dir.into())
            .unwrap_or_else(|| home.join(STEAM_GAME_DIRECTORY));

        let root_dir = resolve_root_dir(&root_dir);

        Ok(Self {
            root_dir: root_dir.to_path_buf(),
            cache_db_path,
        })
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn mods_dir(&self) -> PathBuf {
        self.root_dir.join("Mods")
    }

    pub fn cache_db_path(&self) -> &Path {
        &self.cache_db_path
    }

    // FIXME Create this field in AppConfig or getter
    const UPDATER_BLACKLIST_FILE: &str = "updaterblacklist.txt";

    // FIXME `AppConfig` should not read the File or perform any actions, move this method to outside
    /// Reads `updaterblacklist.txt` and returns blacklisted mod paths.
    pub fn read_updater_blacklist(&self) -> io::Result<HashSet<String>> {
        let path = self.mods_dir().join(Self::UPDATER_BLACKLIST_FILE);
        let mut blacklist = HashSet::new();

        let mut file = match File::open(&path) {
            Ok(value) => value,
            // NOTE It's safe to ignore NotFound here, as this file is optional.
            Err(err) if err.kind().eq(&io::ErrorKind::NotFound) => return Ok(blacklist),
            Err(err) => {
                error!(?err, "failed to open blacklist file");
                return Err(err);
            }
        };

        // NOTE The default 8KiB buffer is overkill for small text files.
        let reader = BufReader::with_capacity(1024, &mut file);

        for line in reader.lines() {
            let line = line.inspect_err(|err| error!(?err, "failed to read line in blacklist"))?;
            let line = line.trim();
            if !line.starts_with('#') && !line.is_empty() {
                warn!("'{}' will be excluded from updates", line);
                blacklist.insert(line.to_string());
            }
        }

        Ok(blacklist)
    }
}

/// Resolves installation path by searching Celeste executables.
fn resolve_root_dir(dir: &Path) -> &Path {
    let is_root = dir.join("Celeste.exe").exists() || dir.join("Celeste.dll").exists();

    let is_mods_dir = dir.ends_with("Mods") || dir.join("blacklist.txt").exists();

    if is_mods_dir
        && !is_root
        && let Some(parent) = dir.parent()
    {
        warn!(
            ?parent,
            "Note: 'Mods' folder detected. Using parent directory as game root",
        );
        return parent;
    }

    dir
}

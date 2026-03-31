//! Handle loading mods.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use rayon::prelude::*;
use tracing::{error, instrument};

use crate::{
    config::AppConfig,
    local_mods::{LocalMod, Manifest},
    log::anonymize,
};

pub struct ModLoader;

impl ModLoader {
    #[instrument(skip_all)]
    pub fn load_from_config(config: &AppConfig) -> io::Result<Vec<LocalMod>> {
        let mods_dir = &config.mods_dir();
        if !mods_dir.exists() {
            return Ok(Vec::new());
        }

        let paths = Self::scan_directory(mods_dir)?;

        Self::load_all(&paths)
    }

    #[instrument(skip_all)]
    fn load_all(paths: &[PathBuf]) -> io::Result<Vec<LocalMod>> {
        let mods: Vec<LocalMod> = paths
            .par_iter()
            .filter_map(|path| Self::load_single(path))
            .collect();
        Ok(mods)
    }

    #[instrument(fields(path = %anonymize(path)))]
    fn load_single(path: &Path) -> Option<LocalMod> {
        let bytes = zip_finder::extract_file_from_zip(path, b"everest.yaml", Some(b"everest.yml"))
            .inspect_err(|e| error!(?e, "Failed to extract manifest"))
            .ok()?;

        let mut manifests = Manifest::from_slice(&bytes)
            .inspect_err(|e| error!(?e, "Failed to parse YAML"))
            .ok()?;

        manifests.pop_front().map(|m| LocalMod::new(path, m))
    }

    /// Scans mods directory and returns list of archive paths.
    #[instrument]
    fn scan_directory(mods_dir: &Path) -> io::Result<Vec<PathBuf>> {
        let found_paths: Vec<_> = fs::read_dir(mods_dir)?
            .filter_map(|res| res.ok())
            .map(|e| e.path())
            .filter(|p| Self::is_mod_archive(p))
            .collect();
        Ok(found_paths)
    }

    fn is_mod_archive(path: &Path) -> bool {
        path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    }
}

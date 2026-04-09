//! Handle loading mods.
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

use rayon::prelude::*;
use tracing::{error, instrument, warn};

use crate::{core::local::LocalMod, log::anonymize, manifest::Manifest};

pub struct ModLoader;

impl ModLoader {
    /// Returns found installed mods in given directory.
    #[instrument(skip_all, fields(directory = %anonymize(mods_dir)))]
    pub fn load(mods_dir: &Path) -> io::Result<Vec<LocalMod>> {
        if !mods_dir.exists() {
            warn!("mods directory not found, Everest is not installed");
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

    fn load_single(path: &Path) -> Option<LocalMod> {
        let bytes = zip_finder::extract_file_from_zip(path, b"everest.yaml", Some(b"everest.yml"))
            .inspect_err(|e| error!(?e, "Failed to extract manifest"))
            .ok()?;

        let mut manifests = Manifest::parse(&bytes)
            .inspect_err(|e| error!(?e, "Failed to parse everest.yaml"))
            .ok()?;

        manifests.pop_front().map(|m| LocalMod::new(path, m))
    }

    /// Returns found installed mod names in given directory.
    #[instrument(skip_all, fields(directory = %anonymize(mods_dir)))]
    pub fn extract(mods_dir: &Path) -> io::Result<HashSet<String>> {
        if !mods_dir.exists() {
            warn!("mods directory not found, Everest is not installed");
            return Ok(HashSet::new());
        }

        let paths = Self::scan_directory(mods_dir)?;

        Self::extract_all_names(&paths)
    }

    #[instrument(skip_all)]
    fn extract_all_names(paths: &[PathBuf]) -> io::Result<HashSet<String>> {
        let mods: HashSet<String> = paths
            .par_iter()
            .filter_map(|path| Self::extract_mod_name(path))
            .collect();
        Ok(mods)
    }

    fn extract_mod_name(path: &Path) -> Option<String> {
        let bytes = zip_finder::extract_file_from_zip(path, b"everest.yaml", Some(b"everest.yml"))
            .inspect_err(|e| error!(?e, "Failed to extract manifest"))
            .ok()?;

        let mut manifests = Manifest::parse(&bytes)
            .inspect_err(|e| error!(?e, "Failed to parse everest.yaml"))
            .ok()?;

        manifests.pop_front().map(|m| m.name)
    }

    /// Scans mods directory and returns list of archive paths.
    #[instrument(skip_all, fields(directory = %anonymize(mods_dir)))]
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

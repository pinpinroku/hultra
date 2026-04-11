//! Service for resolving installed mods.
use std::{collections::HashSet, io, path::Path};

use rayon::prelude::*;
use tracing::instrument;

use crate::{
    core::{LocalMod, ModFile},
    manifest::Manifest,
};

pub struct ModResolver;

impl ModResolver {
    fn resolve_manifest(path: &Path) -> anyhow::Result<Manifest> {
        let bytes = zip_finder::extract_file_from_zip(path, b"everest.yaml", Some(b"everest.yml"))?;
        let manifest = Manifest::parse(&bytes)?;
        Ok(manifest)
    }

    /// Resolves a list of installed mods.
    #[instrument(skip_all)]
    pub fn resolve(files: &[ModFile]) -> io::Result<Vec<LocalMod>> {
        let mods = files
            .into_par_iter()
            .filter_map(|file| {
                let manifest = Self::resolve_manifest(file.path()).ok()?;
                Some(LocalMod::new(file.clone(), manifest.name, manifest.version))
            })
            .collect();
        Ok(mods)
    }

    /// Resolves a list of installed mod names.
    #[instrument(skip_all)]
    pub fn resolve_names(files: &[ModFile]) -> io::Result<HashSet<String>> {
        let names = files
            .into_par_iter()
            .filter_map(|file| {
                let manifest = Self::resolve_manifest(file.path()).ok()?;
                Some(manifest.name)
            })
            .collect();
        Ok(names)
    }
}

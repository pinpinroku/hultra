//! Service for resolving installed mods.
use std::{io, marker::Sync, path::Path};

use rayon::prelude::*;
use tracing::instrument;

use crate::{
    core::{
        LocalMod, LocalModFileSource, ModFileSource,
        manifest::{LocalMetadataReader, MetadataReader},
    },
    log::anonymize,
};

/// Scans installed mods.
#[instrument(skip_all, fields(mods_dir = %anonymize(mods_dir)))]
pub fn scan_mods(mods_dir: &Path) -> io::Result<Vec<LocalMod>> {
    let source = LocalModFileSource::new(mods_dir);
    let resolver = ModResolver::new(source, LocalMetadataReader);
    resolver.resolve()
}

/// A service to resolve locally installed mods.
#[derive(Debug)]
struct ModResolver<S: ModFileSource, R: MetadataReader> {
    /// Mods directory scanner.
    source: S,
    /// Manifest reader.
    reader: R,
}

impl<S: ModFileSource, R: MetadataReader + Sync> ModResolver<S, R> {
    fn new(source: S, reader: R) -> Self {
        Self { source, reader }
    }

    /// Resolves a list of installed mods.
    fn resolve(self) -> io::Result<Vec<LocalMod>> {
        let files = self.source.fetch_all()?;
        let mods = files
            .into_par_iter()
            .filter_map(|file| {
                let manifest = self.reader.read_metadata(file.path()).ok()?;
                Some(LocalMod::new(file.clone(), manifest.name, manifest.version))
            })
            .collect();
        Ok(mods)
    }
}

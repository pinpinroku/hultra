//! All about mods. For Everest commands, see src/everest.rs and its submodules.
//!
//! --- Raw Data From YAML File ---
//! * manifest.rs: `everest.yaml`, metadata file in ZIP archive
//! * registry.rs: `everest_update.yaml`, database for check updates, also used for installing mods
//! * dependency.rs: `depencency_graph.yaml`, database for resolving mod dependencies
//!
//! --- Core Domain Logic ---
//! * checksum.rs: xxhash64 of mod file, used for checking updates
//! * cache.rs: cache the file checksum to avoid re-hash
//!
//! --- Networking ---
//! * network.rs: SharedHttpClient
//! * network/api.rs: fetch database from API endpoint
//! * network/downloader.rs: download mods
//!
//! --- Local File ---
//! * local.rs: represents installed mod
pub mod blacklist;
pub mod cache;
pub mod checksum;
pub mod dependency;
pub mod local;
pub mod network;
pub mod registry;
pub mod update;

pub use checksum::{Checksum, ChecksumVerificationError, Checksums, ParseChecksumError};
pub use local::LocalMod;

#[cfg(test)]
pub use local::ModFile;

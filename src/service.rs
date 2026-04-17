//! Handle externals like OS, API, or DB
pub mod archive;
pub mod fs;
pub mod os;

pub use os::LocalFileSystemService;

#[cfg(test)]
pub use os::MockFileSystemService;

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use crate::{
    cdfh::{CdfhError, CentralDirectoryFileHeader},
    eocd::{Eocd, EocdError},
    lfh::{LfhError, LocalFileHeader},
};

mod cdfh;
mod eocd;
mod lfh;
mod utils;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    EocdError(#[from] EocdError),
    #[error(transparent)]
    Cdfh(#[from] CdfhError),
    #[error(transparent)]
    Lfh(#[from] LfhError),
}

/// Extracts the specified file as a byte vector from the given ZIP archive.
///
/// This function attempts to locate the specified file within the ZIP archive and extract it
/// as a byte vector. If the file is not found, a `TargetNotFound` error will be returned.
///
/// # Arguments
///
/// * `path` - A path to the ZIP archive from which the file should be extracted.
/// * `filename: &[u8]` - Target file name in bytes which should be in the ZIP archive. Assume 99% of time it'll be found with this name otherwise the performance may worse.
/// * `alt_name: Option<&[u8]>` - A fallback name in bytes for the file if it does not exist. It can be None if you do not need to find another.
///
/// # Returns
///
/// A `Result<Vec<u8>, Error>` where:
/// - `Ok(Vec<u8>)` contains the byte vector of the extracted file if found.
/// - `Err(Error)` contains a `TargetNotFound` error if the specified file is not found in the archive.
///   It also returns I/O errors and internal errors while parsing the binary.
///
/// # Example
///
/// ```ignore
/// let result = extract_file_from_zip("AchievementHelper.zip", b"everest.yaml", Some(b"everest.yml"));
/// match result {
///     Ok(bytes) => println!("File extracted successfully: {}", &bytes[..20]),
///     Err(e) => println!("Error: {:?}", e),
/// }
/// ```
///
/// # NOTE
///
/// This method focus on low memory usage and high performance.
/// Only parse necessary area of binary instead of reading all entries.
pub fn extract_file_from_zip<P: AsRef<Path>>(
    path: P,
    filename: &[u8],
    alt_name: Option<&[u8]>,
) -> Result<Vec<u8>, Error> {
    let mut file = File::open(path)?;

    let eocd = Eocd::find(&mut file)?;

    // move file pointer to the start of CDFH
    file.seek(SeekFrom::Start(eocd.central_directory_offset() as u64))?;

    // read CDFH to the buffer
    let mut buffer = vec![0u8; eocd.central_directory_size() as usize];
    file.read_exact(&mut buffer)?;

    // trying to find manifest
    let total_records = eocd.total_central_dir_records();
    let cdfh = CentralDirectoryFileHeader::find_record_by_name(&buffer, total_records, filename)
        .or_else(|err| {
            alt_name
                .map(|alt| {
                    CentralDirectoryFileHeader::find_record_by_name(&buffer, total_records, alt)
                })
                .unwrap_or(Err(err))
        })?;

    // extract manifest bytes
    let yaml_slice = LocalFileHeader::extract_local_file(&mut file, cdfh)?;
    Ok(yaml_slice)
}

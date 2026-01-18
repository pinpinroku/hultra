//! Local File Header (LFH)
//!
//! A metadata of the local file.
//! Every local files has this header before actual data starts.
//!
//! <https://en.wikipedia.org/wiki/ZIP_(file_format)#Local_file_header>
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

use flate2::read::DeflateDecoder;

use crate::{cdfh::CentralDirectoryFileHeader, utils::read_u16_le};

const LFH_FIXED_SIZE: usize = 30;

#[derive(thiserror::Error, Debug)]
pub enum LfhError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Unsupported compression method: {0}")]
    UnsupportedCompression(u16),
}

/// Represents the Local File Header (LFH) structure.
#[derive(Debug)]
pub struct LocalFileHeader {
    name_len: u64,
    extra_len: u64,
}

impl LocalFileHeader {
    fn new(buffer: &[u8]) -> Self {
        let n_len = read_u16_le(&buffer[26..]) as u64;
        let m_len = read_u16_le(&buffer[28..]) as u64;
        Self {
            name_len: n_len,
            extra_len: m_len,
        }
    }

    /// Returns file header size before actual contents start.
    fn header_length(&self) -> u64 {
        self.name_len + self.extra_len
    }

    /// Seeks to Local File Header to get the slice of raw local file while decoding its body if needed.
    pub fn extract_local_file(
        file: &mut File,
        cdfh: CentralDirectoryFileHeader,
    ) -> Result<Vec<u8>, LfhError> {
        file.seek(SeekFrom::Start(cdfh.lfh_offset()))?;

        // Fixed LFH slice
        let mut buffer = [0u8; LFH_FIXED_SIZE];
        file.read_exact(&mut buffer)?;

        // Create Local File Header of the target file
        let lfh = LocalFileHeader::new(&buffer);

        // Skipping to the content
        file.seek(SeekFrom::Current(lfh.header_length() as i64))?;

        // Limit the reader to only the compressed/stored size of this file
        let limited_reader = file.take(cdfh.compressed_size() as u64);

        match cdfh.compression_method() {
            0 => {
                let mut c_buf = vec![0u8; cdfh.compressed_size() as usize];
                file.read_exact(&mut c_buf)?;
                Ok(c_buf)
            }
            8 => {
                let mut decoder = DeflateDecoder::new(limited_reader);
                let mut u_buf = vec![0u8; cdfh.uncompressed_size() as usize];
                decoder.read_exact(&mut u_buf)?;
                Ok(u_buf)
            }
            value => Err(LfhError::UnsupportedCompression(value)),
        }
    }
}

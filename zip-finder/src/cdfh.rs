//! Central Directory File Header (CDFH)
//!
//! This entry is an expanded form of the local header.
//!
//! <https://en.wikipedia.org/wiki/ZIP_(file_format)#Central_directory_file_header_(CDFH)>
use crate::utils::{read_u16_le, read_u32_le};

/// The fixed-size portion of the Central Directory File Header (CDFH).
/// Includes signature (4), versions (4), flags (2), method (2),
/// time/date (4), crc (4), sizes (8), lengths (6), and disk/attrs (12).
const CDFH_FIXED_SIZE: usize = 46;

/// Signature of CDFH, the buffer must starts with this value
const CDFH_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];

#[derive(thiserror::Error, Debug)]
pub enum CdfhError {
    #[error("target file not found")]
    TargetNotFound,
    #[error("insufficient data in the buffer as valid CDFH")]
    InsufficientData,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Represents the Central Directory File Header (CDFH) structure.
#[derive(Debug)]
pub struct CentralDirectoryFileHeader {
    compression_method: u16,
    compressed_size: u32,
    uncompressed_size: u32,
    name_len: usize,
    extra_len: usize,
    comment_len: usize,
    lfh_offset: u64,
}

impl CentralDirectoryFileHeader {
    pub fn from_slice(buf: &[u8]) -> Self {
        assert_eq!(&buf[0..4], CDFH_SIGNATURE, "signature should match");
        Self {
            compression_method: read_u16_le(&buf[10..]),
            compressed_size: read_u32_le(&buf[20..]),
            uncompressed_size: read_u32_le(&buf[24..]),
            name_len: read_u16_le(&buf[28..]) as usize,
            extra_len: read_u16_le(&buf[30..]) as usize,
            comment_len: read_u16_le(&buf[32..]) as usize,
            lfh_offset: read_u32_le(&buf[42..]) as u64,
        }
    }

    /// Returns the total size of this header including variable-length fields
    #[inline]
    pub fn total_len(&self) -> usize {
        CDFH_FIXED_SIZE + self.name_len + self.extra_len + self.comment_len
    }

    pub fn compression_method(&self) -> u16 {
        self.compression_method
    }

    pub fn compressed_size(&self) -> u32 {
        self.compressed_size
    }

    pub fn uncompressed_size(&self) -> u32 {
        self.uncompressed_size
    }

    pub fn lfh_offset(&self) -> u64 {
        self.lfh_offset
    }

    pub fn name_len(&self) -> usize {
        self.name_len
    }

    /// Iterates over all records in CDFH, and returns the record matches given filenames.
    pub fn find_record_by_name(
        mut buffer: &[u8],
        total_entries: u16,
        filename: &[u8],
    ) -> Result<Self, CdfhError> {
        for _ in 0..total_entries {
            // Ensure we have at least the fixed-size part of the CDFH
            if buffer.len() < CDFH_FIXED_SIZE || !buffer.starts_with(&CDFH_SIGNATURE) {
                break;
            }

            let cdfh = Self::from_slice(buffer);
            let total_header_len = cdfh.total_len();

            if buffer.len() < total_header_len {
                return Err(CdfhError::InsufficientData);
            }

            // Extract the filename from the current position
            let file_name = &buffer[CDFH_FIXED_SIZE..(CDFH_FIXED_SIZE + cdfh.name_len())];

            if filename == file_name {
                return Ok(cdfh);
            }

            // Advance the buffer slice to the start of the next CDFH
            buffer = &buffer[total_header_len..];
        }

        Err(CdfhError::TargetNotFound)
    }
}

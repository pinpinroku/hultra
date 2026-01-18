//! End Of Central Directory (EOCD)
//!
//! <https://en.wikipedia.org/wiki/ZIP_(file_format)#End_of_central_directory_record_(EOCD)>
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

use crate::utils::{read_u16_le, read_u32_le};

const EOCD_FIXED_SIZE: usize = 22;
const MAX_COMMENT_SIZE: usize = u16::MAX as usize; // 2^16-1 = 65535
/// The maximum number of bytes from the end of the file we need to scan to find the EOCD.
const MAX_EOCD_SEARCH_SIZE: u64 = (EOCD_FIXED_SIZE + MAX_COMMENT_SIZE) as u64;
/// Signature of EOCD, the buffer must starts with this value
const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];

#[derive(thiserror::Error, Debug)]
pub enum EocdError {
    #[error("signature not found in EOCD")]
    SignatureNotFound,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Represents the End Of Central Directory (EOCD) structure.
#[derive(Debug)]
pub struct Eocd {
    total_central_dir_records: u16,
    central_directory_size: u32,
    central_directory_offset: u32,
}

impl Eocd {
    fn new(buf: &[u8]) -> Self {
        assert_eq!(&buf[0..4], EOCD_SIGNATURE, "signature should match");
        Self {
            total_central_dir_records: read_u16_le(&buf[10..]),
            central_directory_size: read_u32_le(&buf[12..]),
            central_directory_offset: read_u32_le(&buf[16..]),
        }
    }

    pub fn total_central_dir_records(&self) -> u16 {
        self.total_central_dir_records
    }

    pub fn central_directory_size(&self) -> u32 {
        self.central_directory_size
    }

    pub fn central_directory_offset(&self) -> u32 {
        self.central_directory_offset
    }

    pub fn find(file: &mut File) -> Result<Self, EocdError> {
        // 1. trying to parse EOCD with minimal size
        file.seek(SeekFrom::End(-(EOCD_FIXED_SIZE as i64)))?;

        let mut buf = [0u8; EOCD_FIXED_SIZE];
        file.read_exact(&mut buf)?;

        if buf.starts_with(&EOCD_SIGNATURE) {
            // return early if signature matches
            return Ok(Self::new(&buf));
        }

        // 2. trying to find EOCD signature backwards with max search size
        let file_size = file.seek(SeekFrom::End(0))?;
        let max_search = std::cmp::min(file_size, MAX_EOCD_SEARCH_SIZE) as usize;

        file.seek(SeekFrom::End(-(max_search as i64)))?;

        let mut buffer = vec![0u8; max_search];
        file.read_exact(&mut buffer)?;

        let eocd_buf = buffer
            .windows(4) // create windows for 4 bytes
            .enumerate() // indexing to get current position in the buffer
            .rev() // search backwards
            .filter(|(_, window)| *window == EOCD_SIGNATURE)
            .find_map(|(pos, _)| {
                // loop each elements to validate comment length
                let comment_len = read_u16_le(&buffer[pos + 20..]) as usize;
                if pos + EOCD_FIXED_SIZE + comment_len == buffer.len() {
                    // if length matches, return the buffer of EOCD
                    Some(&buffer[pos..])
                } else {
                    // if not matches, search next
                    None
                }
            })
            .ok_or(EocdError::SignatureNotFound)?;

        Ok(Self::new(eocd_buf))
    }
}

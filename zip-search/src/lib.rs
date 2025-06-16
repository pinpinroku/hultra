use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZipSearchError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("File too small to be a valid ZIP")]
    FileTooSmall,
    #[error("Valid End of Central Directory record not found")]
    EndOfCentralDirectoryNotFound,
    #[error("Invalid central directory entry signature")]
    InvalidCentralDirectoryEntrySignature,
    #[error("Invalid local file header signature")]
    InvalidLocalFileHeaderSignature,
    #[error("ZIP format error: {0}")]
    Format(String),
    #[error("Unsupported compression method: {0}")]
    UnsupportedCompression(u16),
    #[error("Decompressed size mismatch")]
    DecompressedSizeMismatch,
}

type ZipSearchResult<T> = std::result::Result<T, ZipSearchError>;

/// High-performance ZIP file searcher with zero-copy optimizations
///
/// # Examples
///
/// ```rust
/// use std::path::Path;
///
/// use zip_search::ZipSearcher;
///
/// pub fn example_usage() -> Result<(), Box<dyn std::error::Error>> {
///     let zip_path = Path::new("./test/ChroniaHelper.zip");
///     let mut searcher = ZipSearcher::new(zip_path)?;
///
///     println!("Archive contains {} files", searcher.file_count());
///
///     let target_file = "everest.yaml";
///     match searcher.find_file(target_file)? {
///         Some(entry) => {
///             println!(
///                 "Found: {} ({} bytes, compression: {})",
///                 entry.file_name, entry.uncompressed_size, entry.compression_method
///             );
///
///             let data = searcher.read_file(&entry)?;
///             println!("Read {} bytes successfully", data.len());
///
///             // Convert to string if it's text
///             if let Ok(text) = String::from_utf8(data) {
///                 println!(
///                     "Content preview: {}",
///                     &text[..std::cmp::min(100, text.len())]
///                 );
///             }
///         }
///         None => {
///             println!("File '{}' not found in archive", target_file);
///         }
///     }
///
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct ZipSearcher {
    file: File,
    eocd: EndOfCentralDirectory,
}

/// End of central directory record
#[derive(Debug)]
struct EndOfCentralDirectory {
    total_entries: u16,
    central_directory_offset: u32,
    central_directory_size: u32,
}

/// Central directory file header entry
#[derive(Debug)]
pub struct CentralDirectoryEntry {
    pub file_name: String,
    pub compression_method: u16,
    pub uncompressed_size: u32,
    pub compressed_size: u32,
    pub local_header_offset: u32,
}

/// Fast buffer for chunked reading
struct ReadBuffer {
    data: Vec<u8>,
    valid_len: usize,
    position: usize,
}

impl ReadBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            valid_len: 0,
            position: 0,
        }
    }

    fn remaining(&self) -> usize {
        self.valid_len - self.position
    }

    fn current_slice(&self) -> &[u8] {
        &self.data[self.position..self.valid_len]
    }

    fn advance(&mut self, bytes: usize) {
        self.position = std::cmp::min(self.position + bytes, self.valid_len);
    }

    fn compact_and_fill(&mut self, file: &mut File) -> io::Result<bool> {
        // Move remaining data to start of buffer
        if self.position > 0 {
            let remaining = self.remaining();
            if remaining > 0 {
                self.data.copy_within(self.position..self.valid_len, 0);
            }
            self.valid_len = remaining;
            self.position = 0;
        }

        // Fill rest of buffer
        if self.valid_len < self.data.len() {
            let bytes_read = file.read(&mut self.data[self.valid_len..])?;
            self.valid_len += bytes_read;
            Ok(bytes_read > 0)
        } else {
            Ok(true)
        }
    }
}

// Fast little-endian conversion functions (branchless)
#[inline(always)]
fn read_u16_le(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

#[inline(always)]
fn read_u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

impl ZipSearcher {
    /// Create a new ZIP searcher with minimal initialization overhead
    pub fn new<P: AsRef<Path>>(zip_path: P) -> ZipSearchResult<Self> {
        let mut file = File::open(zip_path)?;
        let eocd = Self::find_end_of_central_directory(&mut file)?;
        Ok(ZipSearcher { file, eocd })
    }

    /// Robust EOCD discovery that handles edge cases properly
    fn find_end_of_central_directory(file: &mut File) -> ZipSearchResult<EndOfCentralDirectory> {
        const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06]; // PK\x05\x06
        const MIN_EOCD_SIZE: usize = 22;

        let file_size = file.metadata()?.len();

        if file_size < MIN_EOCD_SIZE as u64 {
            return Err(ZipSearchError::FileTooSmall);
        }

        // Try different search strategies

        // Strategy 1: Look for EOCD at the very end (no comment)
        if file_size >= MIN_EOCD_SIZE as u64 {
            file.seek(SeekFrom::End(-(MIN_EOCD_SIZE as i64)))?;
            let mut buf = [0u8; MIN_EOCD_SIZE];
            file.read_exact(&mut buf)?;

            if buf[0..4] == EOCD_SIGNATURE {
                if let Some(eocd) = Self::parse_eocd(&buf) {
                    return Ok(eocd);
                }
            }
        }

        // Strategy 2: Search backwards through larger area (with potential comment)
        let max_search = std::cmp::min(file_size, 65557) as usize; // 22 + 65535 max comment
        file.seek(SeekFrom::End(-(max_search as i64)))?;

        let mut buffer = vec![0u8; max_search];
        file.read_exact(&mut buffer)?;

        // Search for signature pattern
        for window in buffer.windows(4).enumerate().rev() {
            let (pos, sig_bytes) = window;

            if sig_bytes == EOCD_SIGNATURE {
                // Check if we have enough space for complete EOCD
                if pos + MIN_EOCD_SIZE <= buffer.len() {
                    if let Some(eocd) = Self::parse_eocd(&buffer[pos..pos + MIN_EOCD_SIZE]) {
                        // Additional validation: check if comment length makes sense
                        let comment_len = read_u16_le(&buffer[pos + 20..]) as usize;
                        if pos + MIN_EOCD_SIZE + comment_len <= buffer.len() {
                            return Ok(eocd);
                        }
                    }
                }
            }
        }

        Err(ZipSearchError::EndOfCentralDirectoryNotFound)
    }

    /// Parse and validate EOCD record
    fn parse_eocd(data: &[u8]) -> Option<EndOfCentralDirectory> {
        if data.len() < 22 {
            return None;
        }

        // Skip signature (already verified)
        let disk_number = read_u16_le(&data[4..]);
        let disk_with_cd = read_u16_le(&data[6..]);
        let entries_on_disk = read_u16_le(&data[8..]);
        let total_entries = read_u16_le(&data[10..]);
        let cd_size = read_u32_le(&data[12..]);
        let cd_offset = read_u32_le(&data[16..]);

        // Basic validation for single-disk ZIP files
        if disk_number == 0
            && disk_with_cd == 0
            && entries_on_disk == total_entries
            && cd_size > 0
            && cd_offset > 0
        {
            Some(EndOfCentralDirectory {
                total_entries,
                central_directory_offset: cd_offset,
                central_directory_size: cd_size,
            })
        } else {
            None
        }
    }

    /// High-performance file search with chunked reading and zero-copy comparisons
    pub fn find_file(
        &mut self,
        target_name: &str,
    ) -> ZipSearchResult<Option<CentralDirectoryEntry>> {
        const CD_ENTRY_SIGNATURE: u32 = 0x02014b50;
        const MIN_CD_ENTRY_SIZE: usize = 46;
        const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks

        let target_bytes = target_name.as_bytes();
        let mut buffer = ReadBuffer::new(CHUNK_SIZE);

        // Seek to central directory
        self.file
            .seek(SeekFrom::Start(self.eocd.central_directory_offset as u64))?;

        let mut entries_found = 0;

        // Fill initial buffer
        buffer.compact_and_fill(&mut self.file)?;

        while entries_found < self.eocd.total_entries && buffer.remaining() >= MIN_CD_ENTRY_SIZE {
            let slice = buffer.current_slice();

            // Check signature
            if read_u32_le(slice) != CD_ENTRY_SIGNATURE {
                return Err(ZipSearchError::InvalidCentralDirectoryEntrySignature);
            }

            // Fast extraction of essential fields
            let filename_len = read_u16_le(&slice[28..]) as usize;
            let extra_len = read_u16_le(&slice[30..]) as usize;
            let comment_len = read_u16_le(&slice[32..]) as usize;

            let entry_size = MIN_CD_ENTRY_SIZE + filename_len + extra_len + comment_len;

            // Check if we have enough data for complete entry
            if buffer.remaining() < entry_size {
                // Need more data
                if !buffer.compact_and_fill(&mut self.file)? {
                    break; // No more data available
                }
                continue; // Retry with more data
            }

            let slice = buffer.current_slice(); // Refresh after potential buffer fill

            // Zero-copy filename comparison
            let filename_start = MIN_CD_ENTRY_SIZE;
            let filename_end = filename_start + filename_len;

            if slice.len() >= filename_end && &slice[filename_start..filename_end] == target_bytes {
                // Found match! Parse complete entry
                let compression_method = read_u16_le(&slice[10..]);
                let compressed_size = read_u32_le(&slice[20..]);
                let uncompressed_size = read_u32_le(&slice[24..]);
                let local_header_offset = read_u32_le(&slice[42..]);

                // Only allocate string when we found the file
                let file_name =
                    String::from_utf8_lossy(&slice[filename_start..filename_end]).into_owned();

                return Ok(Some(CentralDirectoryEntry {
                    file_name,
                    compression_method,
                    uncompressed_size,
                    compressed_size,
                    local_header_offset,
                }));
            }

            // Move to next entry
            buffer.advance(entry_size);
            entries_found += 1;

            // Refill buffer if running low
            if buffer.remaining() < CHUNK_SIZE / 4 {
                buffer.compact_and_fill(&mut self.file)?;
            }
        }

        Ok(None)
    }

    /// Read file data with optimized decompression
    pub fn read_file(&mut self, entry: &CentralDirectoryEntry) -> ZipSearchResult<Vec<u8>> {
        const LOCAL_HEADER_SIGNATURE: u32 = 0x04034b50;
        const MIN_LOCAL_HEADER_SIZE: usize = 30;

        // Seek to local header
        self.file
            .seek(SeekFrom::Start(entry.local_header_offset as u64))?;

        // Read local header
        let mut header_buf = [0u8; MIN_LOCAL_HEADER_SIZE];
        self.file.read_exact(&mut header_buf)?;

        // Verify signature
        if read_u32_le(&header_buf) != LOCAL_HEADER_SIGNATURE {
            return Err(ZipSearchError::InvalidLocalFileHeaderSignature);
        }

        // Extract variable length fields
        let filename_len = read_u16_le(&header_buf[26..]) as u64;
        let extra_len = read_u16_le(&header_buf[28..]) as u64;

        // Skip variable fields to get to file data
        self.file
            .seek(SeekFrom::Current(filename_len as i64 + extra_len as i64))?;

        // Read compressed data
        let mut compressed_data = vec![0u8; entry.compressed_size as usize];
        self.file.read_exact(&mut compressed_data)?;

        // Handle compression
        match entry.compression_method {
            0 => {
                // Stored (no compression)
                Ok(compressed_data)
            }
            8 => {
                // Deflate compression
                self.decompress_deflate(compressed_data, entry.uncompressed_size as usize)
            }
            _ => Err(ZipSearchError::UnsupportedCompression(
                entry.compression_method,
            )),
        }
    }

    /// Fast deflate decompression
    fn decompress_deflate(
        &self,
        compressed_data: Vec<u8>,
        expected_size: usize,
    ) -> ZipSearchResult<Vec<u8>> {
        use flate2::read::DeflateDecoder;
        use std::io::Read;

        let mut decoder = DeflateDecoder::new(compressed_data.as_slice());
        let mut uncompressed_data = Vec::with_capacity(expected_size);

        decoder.read_to_end(&mut uncompressed_data)?;

        if uncompressed_data.len() != expected_size {
            return Err(ZipSearchError::DecompressedSizeMismatch);
        }

        Ok(uncompressed_data)
    }

    /// Get total number of files in the archive
    pub fn file_count(&self) -> u16 {
        self.eocd.total_entries
    }

    /// Get central directory information
    pub fn central_directory_info(&self) -> (u32, u32) {
        (
            self.eocd.central_directory_offset,
            self.eocd.central_directory_size,
        )
    }
}

// Convenience methods for common patterns
impl ZipSearcher {
    /// Check if a file exists without reading it
    pub fn contains_file(&mut self, file_name: &str) -> ZipSearchResult<bool> {
        Ok(self.find_file(file_name)?.is_some())
    }

    /// Get file info without reading the content
    pub fn file_info(&mut self, file_name: &str) -> ZipSearchResult<Option<(u32, u32, u16)>> {
        if let Some(entry) = self.find_file(file_name)? {
            Ok(Some((
                entry.uncompressed_size,
                entry.compressed_size,
                entry.compression_method,
            )))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endian_conversion() {
        let bytes = [0x34, 0x12, 0x78, 0x56];
        assert_eq!(read_u16_le(&bytes), 0x1234);
        assert_eq!(read_u32_le(&bytes), 0x56781234);
    }
}

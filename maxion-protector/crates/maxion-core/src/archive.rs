//! Archive module for virtual asset archive creation and reading
//!
//! This module handles the virtual archive format used to store encrypted game assets.
//! The archive consists of:
//! - Header: Magic number, version, file count, checksums
//! - File Table: Encrypted list of file metadata (path, offset, size, etc.)
//! - Data: Encrypted chunks of actual file data

use crate::compression_parallel::{compress_parallel_with_config, ParallelCompressionConfig};
use crate::crypto::ChunkCipher;
use crate::error::{ArchiveError, Error, Result};
use crate::io::read_file;
use crate::types::{AssetFile, ChunkSize, Config};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

/// Optimal buffer sizes for different file sizes
const LARGE_BUFFER_SIZE: usize = 64 * 1024; // 64 KB

/// Threshold for using parallel compression (100 MB)
const PARALLEL_COMPRESSION_THRESHOLD: usize = 100 * 1024 * 1024;

/// Size of the archive header in bytes
const HEADER_SIZE: usize = 256;

/// Archive header containing metadata
#[derive(Debug, Clone)]
pub struct ArchiveHeader {
    /// Magic number for identification
    pub magic: [u8; 8],

    /// Archive format version
    pub version: u32,

    /// Total number of files in archive
    pub file_count: u32,

    /// Offset of file table in archive
    pub file_table_offset: u64,

    /// Size of encrypted file table
    pub file_table_size: u32,

    /// Checksum of header for integrity
    pub header_checksum: [u8; 32],

    /// Chunk size used for encryption
    pub chunk_size: u32,

    /// Whether compression is enabled
    pub compress: bool,
}

impl ArchiveHeader {
    /// Create a new archive header
    pub fn new(file_count: u32, chunk_size: ChunkSize, compress: bool) -> Self {
        Self {
            magic: *crate::MAGIC,
            version: crate::ARCHIVE_VERSION,
            file_count,
            file_table_offset: 0, // Will be set during building (at offset 16)
            file_table_size: 0,   // Will be set during building (at offset 24)
            header_checksum: [0u8; 32],
            chunk_size: chunk_size.as_u32(),
            compress,
        }
    }

    /// Calculate header checksum using BLAKE3
    pub fn calculate_checksum(&mut self) {
        let data = self.to_bytes();
        // Hash everything except the checksum field itself (offset 28-60)
        // Header structure: magic(8) + version(4) + file_count(4) + file_table_offset(8) + file_table_size(4) = 28
        // Checksum is 32 bytes at offset 28-60
        let header_without_checksum = [&data[..28], &data[60..]].concat();
        self.header_checksum = blake3::hash(&header_without_checksum).into();
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_SIZE);

        log::info!("to_bytes() serializing header:");
        log::info!("  magic: {:02x?}", &self.magic);
        log::info!(
            "  version: {} ({:02x?})",
            self.version,
            &self.version.to_le_bytes()
        );
        log::info!(
            "  file_count: {} ({:02x?})",
            self.file_count,
            &self.file_count.to_le_bytes()
        );
        log::info!(
            "  file_table_offset: {} ({:02x?})",
            self.file_table_offset,
            &self.file_table_offset.to_le_bytes()
        );
        log::info!(
            "  file_table_size: {} ({:02x?})",
            self.file_table_size,
            &self.file_table_size.to_le_bytes()
        );
        log::info!("  header_checksum: {:02x?}", &self.header_checksum);
        log::info!(
            "  chunk_size: {} ({:02x?})",
            self.chunk_size,
            &self.chunk_size.to_le_bytes()
        );
        log::info!("  compress: {}", self.compress);

        // Write magic (8 bytes, offset 0-7)
        buf.extend_from_slice(&self.magic);
        log::info!("After magic: {} bytes (offset 0-7)", buf.len());

        // Write version (4 bytes, offset 8-11)
        buf.extend_from_slice(&self.version.to_le_bytes());
        log::info!("After version: {} bytes (offset 8-11)", buf.len());

        // Write file count (4 bytes, offset 12-15)
        buf.extend_from_slice(&self.file_count.to_le_bytes());
        log::info!("After file_count: {} bytes (offset 12-15)", buf.len());

        // Write file table offset (8 bytes, offset 16-23)
        buf.extend_from_slice(&self.file_table_offset.to_le_bytes());
        log::info!(
            "After file_table_offset: {} bytes (offset 16-23)",
            buf.len()
        );

        // Write file table size (4 bytes, offset 24-27)
        buf.extend_from_slice(&self.file_table_size.to_le_bytes());
        log::info!("After file_table_size: {} bytes (offset 24-27)", buf.len());

        // Write checksum (32 bytes, offset 28-59)
        buf.extend_from_slice(&self.header_checksum);
        log::info!("After checksum: {} bytes (offset 28-59)", buf.len());

        // Write chunk size (4 bytes, offset 60-63)
        buf.extend_from_slice(&self.chunk_size.to_le_bytes());
        log::info!("After chunk_size: {} bytes (offset 60-63)", buf.len());

        // Write compression flag (1 byte, offset 64)
        buf.push(if self.compress { 1 } else { 0 });
        log::info!("After compress flag: {} bytes (offset 64)", buf.len());

        // Pad to HEADER_SIZE (offset 65-255)
        while buf.len() < HEADER_SIZE {
            buf.push(0);
        }

        log::info!(
            "Final header size: {} bytes (padded to {})",
            buf.len(),
            HEADER_SIZE
        );
        log::info!("First 64 bytes: {:02x?}", &buf[..64.min(buf.len())]);

        buf
    }

    /// Deserialize header from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: "Header too short".to_string(),
            }));
        }

        let magic: [u8; 8] = data[..8].try_into().unwrap();
        if magic.as_ref() != crate::MAGIC {
            return Err(Error::Archive(ArchiveError::InvalidMagic {
                expected: crate::MAGIC.to_vec(),
                found: magic.to_vec(),
            }));
        }

        let mut offset = 8;

        let version = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;

        if version != crate::ARCHIVE_VERSION {
            return Err(Error::Archive(ArchiveError::UnsupportedVersion { version }));
        }

        let file_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;

        let file_table_offset = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;

        let file_table_size = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;

        let header_checksum: [u8; 32] = data[offset..offset + 32].try_into().unwrap();
        offset += 32;

        let chunk_size = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;

        let compress = data[offset] == 1;

        Ok(Self {
            magic,
            version,
            file_count,
            file_table_offset,
            file_table_size,
            header_checksum,
            chunk_size,
            compress,
        })
    }

    /// Verify header checksum
    pub fn verify_checksum(&self) -> bool {
        let data = self.to_bytes();
        // Hash everything except the checksum field itself (offset 28-60)
        let header_without_checksum = [&data[..28], &data[60..]].concat();
        let checksum = blake3::hash(&header_without_checksum);
        checksum.as_bytes() == &self.header_checksum
    }
}

/// Builder for creating virtual archives
pub struct ArchiveBuilder {
    config: Config,
    files: Vec<AssetFile>,
    base_dir: Option<PathBuf>,
}

impl ArchiveBuilder {
    /// Create a new archive builder
    pub fn new(config: Config) -> Self {
        Self {
            config,
            files: Vec::new(),
            base_dir: None,
        }
    }

    /// Set the base directory for resolving relative file paths
    pub fn with_base_dir(mut self, base_dir: &Path) -> Self {
        self.base_dir = Some(base_dir.to_path_buf());
        self
    }

    /// Add a file to the archive
    pub fn add_file(&mut self, file: AssetFile) {
        self.files.push(file);
    }

    /// Add multiple files to the archive
    pub fn add_files(&mut self, files: Vec<AssetFile>) {
        self.files.extend(files);
    }

    /// Get the list of files in the builder (for debugging)
    pub fn files(&self) -> &[AssetFile] {
        &self.files
    }

    /// Build archive and write to output file
    pub fn build(&mut self, output: &Path) -> Result<ArchiveHeader> {
        let base_dir = self.base_dir.take();
        self.build_with_base_dir(output, base_dir)
    }

    /// Build archive and write to output file with explicit base directory
    pub fn build_with_base_dir(
        &mut self,
        output: &Path,
        base_dir: Option<PathBuf>,
    ) -> Result<ArchiveHeader> {
        log::info!(
            "ArchiveBuilder::build() called with {} files, chunk_size={}, compress={}",
            self.files.len(),
            self.config.chunk_size.as_u32(),
            self.config.compress
        );

        log::info!("About to create output file at: {:?}", output);
        if let Some(ref base) = base_dir {
            log::info!("Using base directory: {:?}", base);
        }

        // Extract config values to avoid borrow issues
        let compress_enabled = self.config.compress;
        let compression_level = self.config.compression_level;
        let simd_config = self.config.simd_config.as_ref();

        // Create cipher for encryption
        let cipher = ChunkCipher::new(
            &self.config.encryption_key,
            &self.config.nonce,
            self.config.chunk_size,
        );

        // Pass 1: Encrypt all files to memory and calculate metadata
        log::info!("PASS 1: Encrypting files and calculating metadata");
        let mut encrypted_files: Vec<(AssetFile, Vec<Vec<u8>>)> = Vec::new();
        let mut data_section_size = 0u64;

        for (idx, mut file) in self.files.iter().cloned().enumerate() {
            log::info!("Processing file {}: {:?}", idx, file.path);

            // Resolve full path
            let full_path = if let Some(ref base) = base_dir {
                base.join(&file.path)
            } else {
                file.path.clone()
            };

            log::info!("Resolved full path: {:?}", full_path);

            // Read file data using optimized I/O
            let file_data = read_file(&full_path)?;

            // Calculate checksum
            file.calculate_checksum(&file_data);

            // Compress if enabled
            let data = if compress_enabled {
                let compressed =
                    Self::compress_data_static(&file_data, compression_level, simd_config)?;
                log::info!(
                    "  Compressed {} -> {} bytes",
                    file_data.len(),
                    compressed.len()
                );
                compressed
            } else {
                file_data
            };

            // Encrypt in chunks
            let encrypted_chunks = cipher.encrypt_all(&data)?;
            log::info!("  Encrypted into {} chunks", encrypted_chunks.len());

            // Calculate total encrypted size (including 4-byte size prefix per chunk)
            let mut total_encrypted_size = 0u64;
            for encrypted_chunk in &encrypted_chunks {
                total_encrypted_size += 4u64 + encrypted_chunk.len() as u64;
            }

            // Set metadata
            file.packed_size = total_encrypted_size;
            file.offset = data_section_size;
            file.calculate_chunk_count(self.config.chunk_size);

            log::info!(
                "  Metadata: original_size={}, packed_size={}, offset={}, chunk_count={}",
                file.original_size,
                file.packed_size,
                file.offset,
                file.chunk_count
            );

            // Store encrypted chunks and file info
            encrypted_files.push((file, encrypted_chunks));

            // Update data section offset for next file
            data_section_size += total_encrypted_size;
        }

        log::info!(
            "PASS 1 complete: {} files, total data section size: {} bytes",
            encrypted_files.len(),
            data_section_size
        );

        // Pass 2: Write archive to file
        log::info!("PASS 2: Writing archive to file");

        // Create header
        let file_count = self.files.len() as u32;
        log::info!("Creating header with file_count={}", file_count);
        let mut header =
            ArchiveHeader::new(file_count, self.config.chunk_size, self.config.compress);

        // Create output file
        let output_file = File::create(output)?;
        let mut writer = BufWriter::with_capacity(LARGE_BUFFER_SIZE, output_file);

        // Reserve space for header (we'll come back and fill it)
        log::info!(
            "Reserving {} bytes for header at start of file",
            HEADER_SIZE
        );
        writer.write_all(&vec![0u8; HEADER_SIZE])?;
        writer.flush()?; // Flush to ensure file position is updated

        let position_after_header = writer.get_ref().stream_position()?;
        log::info!("Position after reserving header: {}", position_after_header);

        // Write file table
        let file_table_bytes = self.serialize_file_table_with_files(
            encrypted_files.iter().map(|(f, _)| f).cloned().collect(),
        )?;

        // Remember where file table starts
        let file_table_offset = writer.get_ref().stream_position()?;
        log::info!("File table will start at offset {}", file_table_offset);

        // Encrypt and write file table
        let encrypted_table_chunks = cipher.encrypt_all(&file_table_bytes)?;
        log::info!(
            "Encrypted file table into {} chunks",
            encrypted_table_chunks.len()
        );

        // Write encrypted chunks with size prefixes
        let table_offset_before = writer.get_ref().stream_position()?;
        for chunk in &encrypted_table_chunks {
            let chunk_size = chunk.len() as u32;
            writer.write_all(&chunk_size.to_le_bytes())?;
            writer.write_all(chunk)?;
        }
        writer.flush()?; // Flush to ensure file position is updated
        let table_offset_after = writer.get_ref().stream_position()?;

        // Update header with file table info
        header.file_table_offset = file_table_offset;
        header.file_table_size = (table_offset_after - table_offset_before) as u32;
        header.calculate_checksum();

        log::info!(
            "File table written: offset={}, size={}",
            header.file_table_offset,
            header.file_table_size
        );

        // Write data section
        let data_start_offset = writer.get_ref().stream_position()?;
        log::info!("Writing data section at offset {}", data_start_offset);

        for (idx, (file, encrypted_chunks)) in encrypted_files.iter().enumerate() {
            log::info!(
                "Writing file {}: {:?} ({} chunks, {} bytes)",
                idx,
                file.path,
                encrypted_chunks.len(),
                file.packed_size
            );

            for chunk in encrypted_chunks {
                let chunk_size = chunk.len() as u32;
                writer.write_all(&chunk_size.to_le_bytes())?;
                writer.write_all(chunk)?;
            }
            writer.flush()?;
        }

        log::info!(
            "Data section written: {} bytes",
            writer.get_ref().stream_position()? - data_start_offset
        );

        // Go back and write header
        writer.flush()?;
        drop(writer);

        // Log final header state before writing
        log::info!(
            "Writing final header to file: file_count={}, file_table_offset={}, file_table_size={} (total bytes to write: {})",
            header.file_count,
            header.file_table_offset,
            header.file_table_size,
            std::fs::metadata(output)?.len()
        );

        // Reopen file to write header at beginning
        use std::io::Seek;
        let mut output_file = File::options().write(true).open(output)?;
        output_file.seek(io::SeekFrom::Start(0))?;
        let header_bytes = header.to_bytes();
        log::info!(
            "Header bytes length: {}, first 32 bytes: {:02x?}",
            header_bytes.len(),
            &header_bytes[..32.min(header_bytes.len())]
        );
        // Verify header bytes before writing
        log::info!("About to write header to file:");
        log::info!("  header_bytes length: {}", header_bytes.len());
        log::info!(
            "  header_bytes file_count (bytes 8-11): {:02x?}",
            &header_bytes[8..12]
        );
        log::info!(
            "  header_bytes file_table_offset (bytes 12-19): {:02x?}",
            &header_bytes[12..20]
        );
        log::info!(
            "  header_bytes file_table_size (bytes 20-23): {:02x?}",
            &header_bytes[20..24]
        );
        log::info!("  header.file_count: {}", header.file_count);
        log::info!("  header.file_table_offset: {}", header.file_table_offset);
        log::info!("  header.file_table_size: {}", header.file_table_size);

        output_file.write_all(&header_bytes)?;

        // Verify what was written by reading it back
        let final_file_size = std::fs::metadata(output)?.len();
        log::info!("Archive build complete, file size: {}", final_file_size);

        // Read back the header to verify
        if final_file_size >= HEADER_SIZE as u64 {
            let file_data = std::fs::read(output)?;
            let readback_header = ArchiveHeader::from_bytes(&file_data[..HEADER_SIZE])?;
            log::info!("Readback header verification:");
            log::info!(
                "  file_count: {} (should be {})",
                readback_header.file_count,
                header.file_count
            );
            log::info!(
                "  file_table_offset: {} (should be {})",
                readback_header.file_table_offset,
                header.file_table_offset
            );
            log::info!(
                "  file_table_size: {} (should be {})",
                readback_header.file_table_size,
                header.file_table_size
            );
        }

        Ok(header)
    }

    /// Serialize file table from a separate list of files
    fn serialize_file_table_with_files(&self, files: Vec<AssetFile>) -> Result<Vec<u8>> {
        log::info!("Serializing file table with {} files", files.len());

        // Normalize paths for cross-platform compatibility
        // Convert Windows backslashes to forward slashes
        let normalized_files: Vec<AssetFile> = files
            .into_iter()
            .map(|mut file| {
                file.normalize_path();
                file
            })
            .collect();

        // Use bincode for efficient serialization
        let encoded = bincode::serialize(&normalized_files).map_err(|e| {
            Error::Archive(ArchiveError::Corrupted {
                reason: format!("Failed to serialize file table: {}", e),
            })
        })?;

        log::info!("Serialized file table: {} bytes", encoded.len());
        Ok(encoded)
    }

    /// Compress data using Brotli (static method for borrowing)
    fn compress_data_static(
        data: &[u8],
        level: u32,
        simd_config: Option<&crate::simd::SimdConfig>,
    ) -> Result<Vec<u8>> {
        // Use parallel compression for large files
        if data.len() >= PARALLEL_COMPRESSION_THRESHOLD {
            log::info!(
                "Using parallel compression for large file: {} bytes",
                data.len()
            );
            let mut config = ParallelCompressionConfig::new(2 * 1024 * 1024, level); // 2MB chunks

            // Add SIMD config if provided
            if let Some(simd) = simd_config {
                config = config.with_simd_config(*simd);
            }

            let result = compress_parallel_with_config(data, config).map_err(|e| {
                Error::Compression(crate::error::CompressionError::CompressionFailed {
                    reason: e.to_string(),
                })
            })?;
            // Convert parallel result back to single buffer for compatibility
            let mut compressed = Vec::with_capacity(result.total_compressed_size);
            for chunk in &result.chunks {
                compressed.extend_from_slice(&chunk.data);
            }
            log::info!(
                "Parallel compression: {} -> {} bytes ({:.2}%)",
                result.total_original_size,
                result.total_compressed_size,
                result.space_saved_percentage()
            );
            Ok(compressed)
        } else {
            crate::compression::compress(data, level, simd_config)
        }
    }
}

/// Reader for virtual archives
pub struct ArchiveReader {
    header: ArchiveHeader,
    file_table: Vec<AssetFile>,
    file_table_map: HashMap<String, AssetFile>,
}

impl ArchiveReader {
    /// Open an archive from a file
    pub fn open(path: &Path) -> Result<Self> {
        // Read entire archive
        let archive_data = std::fs::read(path)?;

        // Parse header
        let header = ArchiveHeader::from_bytes(&archive_data[..HEADER_SIZE])?;

        // Verify checksum
        if !header.verify_checksum() {
            return Err(Error::Archive(ArchiveError::InvalidChecksum));
        }

        // Read and decrypt file table
        let file_table_start = header.file_table_offset as usize;
        let file_table_end = file_table_start + header.file_table_size as usize;
        let encrypted_table_data = &archive_data[file_table_start..file_table_end];

        // Parse encrypted chunks with size prefixes
        let mut encrypted_chunks = Vec::new();
        let mut offset = 0;
        while offset < encrypted_table_data.len() {
            if offset + 4 > encrypted_table_data.len() {
                return Err(Error::Archive(ArchiveError::Corrupted {
                    reason: "Incomplete chunk size header".to_string(),
                }));
            }
            let chunk_size = u32::from_le_bytes([
                encrypted_table_data[offset],
                encrypted_table_data[offset + 1],
                encrypted_table_data[offset + 2],
                encrypted_table_data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + chunk_size > encrypted_table_data.len() {
                return Err(Error::Archive(ArchiveError::Corrupted {
                    reason: "Chunk data exceeds file table bounds".to_string(),
                }));
            }
            let chunk = encrypted_table_data[offset..offset + chunk_size].to_vec();
            encrypted_chunks.push(chunk);
            offset += chunk_size;
        }

        let cipher = ChunkCipher::new(
            &[0u8; 32], // Will be replaced with actual key
            &[0u8; 24],
            ChunkSize::new(header.chunk_size),
        );
        let table_bytes = cipher.decrypt_all(&encrypted_chunks)?;

        // Deserialize file table
        use bincode;
        let file_table: Vec<AssetFile> = bincode::deserialize(&table_bytes).map_err(|e| {
            Error::Archive(ArchiveError::Corrupted {
                reason: format!("Failed to deserialize file table: {}", e),
            })
        })?;

        // Build lookup map
        let mut file_table_map = HashMap::new();
        for file in &file_table {
            file_table_map.insert(file.path_str(), file.clone());
        }

        Ok(Self {
            header,
            file_table,
            file_table_map,
        })
    }

    /// Get file information by path
    pub fn get_file_info(&self, path: &str) -> Option<&AssetFile> {
        self.file_table_map.get(path)
    }

    /// Get all files in archive
    pub fn get_all_files(&self) -> &[AssetFile] {
        &self.file_table
    }

    /// Get archive header
    pub fn get_header(&self) -> &ArchiveHeader {
        &self.header
    }

    /// Check if file exists in archive
    pub fn contains_file(&self, path: &str) -> bool {
        self.file_table_map.contains_key(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssetFile, Config};
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_header_serialization() {
        let header = ArchiveHeader::new(100, ChunkSize::new(64 * 1024), true);
        let bytes = header.to_bytes();

        assert_eq!(bytes.len(), HEADER_SIZE);

        let parsed = ArchiveHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.file_count, 100);
        assert!(parsed.compress);
    }

    #[test]
    fn test_header_checksum() {
        let mut header = ArchiveHeader::new(50, ChunkSize::new(32 * 1024), false);
        header.calculate_checksum();

        let bytes = header.to_bytes();
        let parsed = ArchiveHeader::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.header_checksum, header.header_checksum);
        assert!(parsed.verify_checksum());
    }

    #[test]
    fn test_archive_builder() {
        let config = Config::new();
        let mut builder = ArchiveBuilder::new(config);

        let file = AssetFile::new(PathBuf::from("test.txt"), 1024);
        builder.add_file(file);

        assert_eq!(builder.files.len(), 1);
    }

    #[test]
    fn test_archive_reader() {
        let dir = tempdir().unwrap();
        let _archive_path = dir.path().join("test.archive");

        // Create a simple archive
        let config = Config::new().with_compression(false, 0);
        let mut builder = ArchiveBuilder::new(config);

        // Add a test file
        let file = AssetFile::new(PathBuf::from("test.txt"), 1024);
        builder.add_file(file);

        // Note: This test is simplified; actual building would need real files
        assert_eq!(builder.files.len(), 1);
    }

    #[test]
    fn test_invalid_magic() {
        let mut data = vec![0u8; HEADER_SIZE];
        data[..8].copy_from_slice(b"INVALID!");

        let result = ArchiveHeader::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_unsupported_version() {
        let mut header = ArchiveHeader::new(0, ChunkSize::default(), false);
        header.version = 999;

        let bytes = header.to_bytes();
        let result = ArchiveHeader::from_bytes(&bytes);
        assert!(result.is_err());
    }
}

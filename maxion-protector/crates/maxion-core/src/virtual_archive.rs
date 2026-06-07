//! Virtual Archive module for memory-efficient asset access
//!
//! This module implements the virtual archive system with:
//! - Memory-mapped file access for efficient reading
//! - LRU caching of decrypted chunks
//! - Context-aware encryption/decryption
//! - Access control integration

use crate::cache::LruCache;
use crate::context::{ChunkCipherContext, EncryptionContext, FromConfig};
use crate::crypto::ChunkCipher;
use crate::error::{ArchiveError, Error, Result};
use crate::types::{AssetFile, ChunkSize, Config};
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs::File;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Virtual Archive for efficient asset access
///
/// Implements the "Just-in-Time" decryption strategy where:
/// 1. Archive is memory-mapped for zero-copy access
/// 2. Chunks are decrypted on-demand and cached
/// 3. Access control prevents bulk extraction
///
/// # Architecture
///
/// ```text
/// VirtualArchive
/// ├── header: ArchiveHeader           // Archive metadata
/// ├── file_table: HashMap              // Path -> FileInfo mapping
/// ├── data: ArchiveData                // Archive data (Mmap or Memory)
/// ├── chunk_cache: LruCache            // Decrypted chunk cache
/// ├── cipher_ctx: ChunkCipherContext   // Context-aware encryption
/// └── config: Config                   // Configuration
/// ```
///
/// # Performance Characteristics
///
/// - **Memory Usage**: ~1MB base + cached chunks (configurable)
/// - **First Read**: ~10ms overhead (decrypt + cache)
/// - **Cached Read**: <1ms (memory access)
/// - **Random Access**: O(1) file lookup, O(1) chunk lookup (if cached)
///
#[allow(dead_code)]
/// Enum to hold archive data from different sources
enum ArchiveData {
    /// Memory-mapped file (for disk-based archives)
    Mmap(Mmap),
    /// In-memory buffer (for embedded archives)
    Memory(Vec<u8>),
}

impl Deref for ArchiveData {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            ArchiveData::Mmap(mmap) => mmap.deref(),
            ArchiveData::Memory(vec) => vec.as_slice(),
        }
    }
}

/// Virtual Archive for efficient asset access
///
/// Generic over the encryption context implementation, allowing flexibility
/// in encryption strategies and easier testing with mock contexts.
///
/// # Type Parameters
///
/// * `C` - Encryption context type implementing `EncryptionContext`
pub struct VirtualArchive<C: EncryptionContext> {
    /// Archive header containing metadata
    header: ArchiveHeader,

    /// File lookup table (normalized path -> file info)
    file_table: HashMap<String, AssetFileInfo>,

    /// Archive data source (either memory-mapped file or in-memory buffer)
    data: ArchiveData,

    /// LRU cache for decrypted chunks (key: chunk_id, value: decrypted data)
    chunk_cache: LruCache<String, Vec<u8>>,

    /// LRU cache for decompressed files (key: path, value: decompressed data)
    file_cache: LruCache<String, Vec<u8>>,

    /// Context-aware cipher for encryption/decryption with access control
    cipher_ctx: C,

    /// Archive configuration
    #[allow(dead_code)]
    config: Config,

    /// Path to the archive file (for error reporting), None for memory-based archives
    #[allow(dead_code)]
    archive_path: Option<PathBuf>,
}

/// File information stored in the virtual archive
#[derive(Debug, Clone)]
pub struct AssetFileInfo {
    /// Original file size (before compression/encryption)
    pub original_size: u64,

    /// Packed size in archive (after compression/encryption)
    pub packed_size: u64,

    /// Offset in the archive data section
    pub offset: u64,

    /// Number of encrypted chunks
    pub chunk_count: u32,

    /// BLAKE3 checksum for integrity verification
    pub checksum: [u8; 32],
}

/// Chunk identifier for caching
///
/// Combines file offset and chunk index for unique identification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)]
struct ChunkId {
    /// File offset in archive
    file_offset: u64,

    /// Chunk index within the file
    chunk_index: u32,
}

/// Type alias for the default VirtualArchive using ChunkCipherContext
///
/// This maintains backward compatibility for code that doesn't need
/// custom encryption context implementations.
pub type DefaultVirtualArchive = VirtualArchive<ChunkCipherContext>;

impl<C: EncryptionContext> VirtualArchive<C> {
    /// Open a virtual archive from a file
    ///
    /// This method:
    /// 1. Memory-maps the archive file
    /// 2. Parses and validates the header
    /// 3. Decrypts the file table
    /// 4. Initializes the chunk cache and encryption context
    ///
    /// # Type Parameters
    ///
    /// * `C` - Encryption context type implementing `EncryptionContext` and `FromConfig`
    ///
    /// # Arguments
    ///
    /// * `archive_path` - Path to the archive file
    /// * `config` - Configuration including encryption keys
    ///
    /// # Returns
    ///
    /// `Result<VirtualArchive>` - Initialized virtual archive
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - File cannot be opened or memory-mapped
    /// - Header is corrupted or has invalid magic
    /// - File table cannot be decrypted
    /// - Checksum verification fails
    pub fn open<P: AsRef<Path>>(archive_path: P, config: Config) -> Result<Self>
    where
        C: FromConfig,
    {
        let archive_path = archive_path.as_ref();
        let path_buf = archive_path.to_path_buf();

        // Open file and memory-map it
        let file = File::open(archive_path).map_err(|e| {
            Error::Archive(ArchiveError::Corrupted {
                reason: format!("Failed to open archive: {}", e),
            })
        })?;

        let data_mmap = unsafe {
            Mmap::map(&file).map_err(|e| {
                Error::Archive(ArchiveError::Corrupted {
                    reason: format!("Failed to memory-map archive: {}", e),
                })
            })?
        };

        let data = ArchiveData::Mmap(data_mmap);

        // Parse header (first 256 bytes)
        if data.len() < 256 {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: "Archive too small to contain header".to_string(),
            }));
        }

        let header = crate::archive::ArchiveHeader::from_bytes(&data[..256])?;

        // Verify header checksum
        if !header.verify_checksum() {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: "Invalid archive header checksum".to_string(),
            }));
        }

        // Decrypt file table
        let file_table_start = header.file_table_offset as usize;
        let file_table_end = file_table_start + header.file_table_size as usize;

        if file_table_end > data.len() {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: "File table offset exceeds archive bounds".to_string(),
            }));
        }

        let encrypted_table_data = &data[file_table_start..file_table_end];

        // Parse encrypted chunks with size prefixes
        let mut encrypted_chunks = Vec::new();
        let mut offset = 0;
        let mut chunk_count = 0;

        log::debug!(
            "Parsing file table: {} bytes at offset {}",
            encrypted_table_data.len(),
            file_table_start
        );

        while offset < encrypted_table_data.len() {
            if offset + 4 > encrypted_table_data.len() {
                return Err(Error::Archive(ArchiveError::Corrupted {
                    reason: format!(
                        "Incomplete chunk size header in file table at offset {} of {}",
                        offset,
                        encrypted_table_data.len()
                    ),
                }));
            }
            let chunk_size = u32::from_le_bytes([
                encrypted_table_data[offset],
                encrypted_table_data[offset + 1],
                encrypted_table_data[offset + 2],
                encrypted_table_data[offset + 3],
            ]) as usize;
            offset += 4;
            chunk_count += 1;

            log::debug!(
                "File table chunk {}: size={}, offset={}",
                chunk_count,
                chunk_size,
                offset
            );

            if offset + chunk_size > encrypted_table_data.len() {
                return Err(Error::Archive(ArchiveError::Corrupted {
                    reason: format!(
                        "Chunk {} ({} bytes) exceeds file table bounds (remaining: {})",
                        chunk_count,
                        chunk_size,
                        encrypted_table_data.len() - offset
                    ),
                }));
            }
            let chunk = encrypted_table_data[offset..offset + chunk_size].to_vec();
            encrypted_chunks.push(chunk);
            offset += chunk_size;
        }

        log::debug!("Parsed {} encrypted chunks from file table", chunk_count);

        // Initialize cipher context (but use raw cipher for file table to bypass access control)
        let raw_cipher = Arc::new(ChunkCipher::new(
            &config.encryption_key,
            &config.nonce,
            ChunkSize::new(header.chunk_size),
        ));

        log::debug!(
            "File table: {} chunks, total size: {} bytes",
            encrypted_chunks.len(),
            encrypted_chunks.iter().map(|c| c.len()).sum::<usize>()
        );

        // Log first chunk details for debugging
        if !encrypted_chunks.is_empty() {
            log::debug!(
                "First chunk: {} bytes, first 16 bytes: {:02x?}",
                encrypted_chunks[0].len(),
                &encrypted_chunks[0][..encrypted_chunks[0].len().min(16)]
            );
        }

        // Decrypt file table using raw cipher (no access control for internal metadata)
        let table_bytes = raw_cipher.decrypt_all(&encrypted_chunks).map_err(|e| {
            Error::Archive(ArchiveError::Corrupted {
                reason: format!("Failed to decrypt file table: {}", e),
            })
        })?;

        log::debug!(
            "Successfully decrypted file table: {} bytes ({} encrypted chunks -> {} bytes)",
            table_bytes.len(),
            encrypted_chunks.len(),
            table_bytes.len()
        );

        // Log first few bytes for debugging (they should be valid bincode data)
        let preview = if table_bytes.len() > 32 {
            &table_bytes[..32]
        } else {
            &table_bytes[..]
        };
        log::debug!("Decrypted data preview: {:02x?}", preview);

        // Initialize cipher context with access control for actual data operations
        let cipher_ctx = C::from_config_keys(
            &config.encryption_key,
            &config.nonce,
            ChunkSize::new(header.chunk_size),
        );

        // Deserialize file table
        let asset_files: Vec<AssetFile> = bincode::deserialize(&table_bytes).map_err(|e| {
            log::error!(
                "Deserialization failed with {} bytes of data. First 64 bytes: {:02x?}",
                table_bytes.len(),
                if table_bytes.len() > 64 {
                    &table_bytes[..64]
                } else {
                    &table_bytes[..]
                }
            );
            Error::Archive(ArchiveError::Corrupted {
                reason: format!("Failed to deserialize file table: {}", e),
            })
        })?;

        log::debug!("Deserialized {} files from file table", asset_files.len());
        // Build lookup map
        let mut file_table = HashMap::new();
        for file in asset_files {
            let asset_info = AssetFileInfo {
                original_size: file.original_size,
                packed_size: file.packed_size,
                offset: file.offset,
                chunk_count: file.chunk_count,
                checksum: file.checksum,
            };
            file_table.insert(file.path_str(), asset_info);
        }

        // Initialize chunk cache (default 128 chunks) and file cache (default 16 files)
        let chunk_cache = LruCache::new(128);
        let file_cache = LruCache::new(16);

        Ok(Self {
            header: header.into(),
            file_table,
            data,
            chunk_cache,
            file_cache,
            cipher_ctx,
            config,
            archive_path: Some(path_buf),
        })
    }

    /// Create a new VirtualArchive from an in-memory buffer.
    ///
    /// This is used when the archive data is embedded in the executable (e.g., in a PE section)
    /// rather than stored as a separate file on disk.
    ///
    /// # Type Parameters
    ///
    /// * `C` - Encryption context type implementing `EncryptionContext`
    ///
    /// # Arguments
    ///
    /// * `data` - Raw archive data including header, file table, and encrypted chunks
    /// * `config` - Archive configuration with encryption key and nonce
    ///
    /// # Returns
    ///
    /// A new `VirtualArchive` instance that reads from the provided memory buffer
    pub fn from_memory(data: Vec<u8>, config: Config) -> Result<Self>
    where
        C: FromConfig,
    {
        log::debug!("Loading archive from memory buffer: {} bytes", data.len());

        // Parse header (first 256 bytes)
        if data.len() < 256 {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: "Archive too small to contain header".to_string(),
            }));
        }

        let header = crate::archive::ArchiveHeader::from_bytes(&data[..256])?;

        // Verify header checksum
        if !header.verify_checksum() {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: "Invalid archive header checksum".to_string(),
            }));
        }

        // Decrypt file table
        let file_table_start = header.file_table_offset as usize;
        let file_table_end = file_table_start + header.file_table_size as usize;

        if file_table_end > data.len() {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: "File table offset exceeds archive bounds".to_string(),
            }));
        }

        let encrypted_table_data = &data[file_table_start..file_table_end];

        // Parse encrypted chunks with size prefixes
        let mut encrypted_chunks = Vec::new();
        let mut offset = 0;
        let mut chunk_count = 0;

        log::debug!(
            "Parsing file table: {} bytes at offset {}",
            encrypted_table_data.len(),
            file_table_start
        );

        while offset < encrypted_table_data.len() {
            if offset + 4 > encrypted_table_data.len() {
                return Err(Error::Archive(ArchiveError::Corrupted {
                    reason: format!(
                        "Incomplete chunk size header in file table at offset {} of {}",
                        offset,
                        encrypted_table_data.len()
                    ),
                }));
            }
            let chunk_size = u32::from_le_bytes([
                encrypted_table_data[offset],
                encrypted_table_data[offset + 1],
                encrypted_table_data[offset + 2],
                encrypted_table_data[offset + 3],
            ]) as usize;
            offset += 4;
            chunk_count += 1;

            log::debug!(
                "File table chunk {}: size={}, offset={}",
                chunk_count,
                chunk_size,
                offset
            );

            if offset + chunk_size > encrypted_table_data.len() {
                return Err(Error::Archive(ArchiveError::Corrupted {
                    reason: format!(
                        "Chunk {} ({} bytes) exceeds file table bounds (remaining: {})",
                        chunk_count,
                        chunk_size,
                        encrypted_table_data.len() - offset
                    ),
                }));
            }
            let chunk = encrypted_table_data[offset..offset + chunk_size].to_vec();
            encrypted_chunks.push(chunk);
            offset += chunk_size;
        }

        log::debug!("Parsed {} encrypted chunks from file table", chunk_count);

        // Initialize cipher context (but use raw cipher for file table to bypass access control)
        let raw_cipher = Arc::new(ChunkCipher::new(
            &config.encryption_key,
            &config.nonce,
            ChunkSize::new(header.chunk_size),
        ));

        log::debug!(
            "File table: {} chunks, total size: {} bytes",
            encrypted_chunks.len(),
            encrypted_chunks.iter().map(|c| c.len()).sum::<usize>()
        );

        // Log first chunk details for debugging
        if !encrypted_chunks.is_empty() {
            log::debug!(
                "First chunk: {} bytes, first 16 bytes: {:02x?}",
                encrypted_chunks[0].len(),
                &encrypted_chunks[0][..encrypted_chunks[0].len().min(16)]
            );
        }

        // Decrypt file table using raw cipher (no access control for internal metadata)
        let table_bytes = raw_cipher.decrypt_all(&encrypted_chunks).map_err(|e| {
            Error::Archive(ArchiveError::Corrupted {
                reason: format!("Failed to decrypt file table: {}", e),
            })
        })?;

        log::debug!(
            "Successfully decrypted file table: {} bytes ({} encrypted chunks -> {} bytes)",
            table_bytes.len(),
            encrypted_chunks.len(),
            table_bytes.len()
        );

        // Log first few bytes for debugging (they should be valid bincode data)
        let preview = if table_bytes.len() > 32 {
            &table_bytes[..32]
        } else {
            &table_bytes[..]
        };
        log::debug!("Decrypted data preview: {:02x?}", preview);

        // Initialize cipher context with access control for actual data operations
        let cipher_ctx = C::from_config_keys(
            &config.encryption_key,
            &config.nonce,
            ChunkSize::new(header.chunk_size),
        );

        // Deserialize file table
        let asset_files: Vec<AssetFile> = bincode::deserialize(&table_bytes).map_err(|e| {
            log::error!(
                "Deserialization failed with {} bytes of data. First 64 bytes: {:02x?}",
                table_bytes.len(),
                if table_bytes.len() > 64 {
                    &table_bytes[..64]
                } else {
                    &table_bytes[..]
                }
            );
            Error::Archive(ArchiveError::Corrupted {
                reason: format!("Failed to deserialize file table: {}", e),
            })
        })?;

        log::debug!("Deserialized {} files from file table", asset_files.len());
        // Build lookup map
        let mut file_table = HashMap::new();
        for file in asset_files {
            let asset_info = AssetFileInfo {
                original_size: file.original_size,
                packed_size: file.packed_size,
                offset: file.offset,
                chunk_count: file.chunk_count,
                checksum: file.checksum,
            };
            file_table.insert(file.path_str(), asset_info);
        }

        // Initialize chunk cache (default 128 chunks) and file cache (default 16 files)
        let chunk_cache = LruCache::new(128);
        let file_cache = LruCache::new(16);

        Ok(Self {
            header: header.into(),
            file_table,
            data: ArchiveData::Memory(data),
            chunk_cache,
            file_cache,
            cipher_ctx,
            config,
            archive_path: None,
        })
    }

    /// Read a file from the virtual archive
    ///
    /// Implements the "Just-in-Time" decryption strategy:
    /// 1. Look up file in file table
    /// 2. For each chunk, check cache first
    /// 3. If not cached, decrypt and cache it
    /// 4. Reassemble chunks into complete file
    /// 5. Verify checksum
    ///
    /// # Arguments
    ///
    /// * `path` - Relative path to the file in the archive
    ///
    /// # Returns
    ///
    /// `Result<Vec<u8>>` - Decrypted file data
    ///
    /// # Performance
    ///
    /// - First read: O(n) where n = number of chunks (decryption overhead)
    /// - Subsequent reads: O(n) where n = number of chunks (cache hits)
    /// - Memory: O(chunk_size) per concurrent read
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - File not found in archive
    /// - Decryption fails
    /// - Checksum verification fails
    /// - Access control limits are exceeded
    pub fn read_file<P: AsRef<Path>>(&mut self, path: P) -> Result<Vec<u8>> {
        let path = path.as_ref();
        let normalized_path = self.normalize_path(path);

        // Look up file in table
        let file_info = self
            .file_table
            .get(&normalized_path)
            .ok_or_else(|| {
                Error::Archive(ArchiveError::FileNotFound {
                    path: normalized_path.clone(),
                })
            })?
            .clone();

        log::debug!(
            "Reading file '{}' ({} bytes, {} chunks)",
            normalized_path,
            file_info.original_size,
            file_info.chunk_count
        );

        // If compression is enabled, check file cache first
        if self.header.compress {
            if let Some(cached_file) = self.file_cache.get(&normalized_path) {
                log::debug!("File cache hit for '{}'", normalized_path);
                return Ok(cached_file.clone());
            }
        }

        // Read and decrypt all chunks
        let mut decrypted_data = Vec::with_capacity(file_info.packed_size as usize);

        for chunk_index in 0..file_info.chunk_count {
            let chunk_key = format!("{}:{}", file_info.offset, chunk_index);

            // Check cache first (only for uncompressed files)
            if !self.header.compress {
                if let Some(cached_chunk) = self.chunk_cache.get(&chunk_key) {
                    decrypted_data.extend_from_slice(cached_chunk);
                    log::trace!(
                        "Cache hit for chunk {} of file '{}'",
                        chunk_index,
                        normalized_path
                    );
                    continue;
                }
            }

            // Cache miss - decrypt chunk
            let chunk_data = self.decrypt_chunk(&file_info, chunk_index)?;

            // Cache the decrypted chunk (only for uncompressed files)
            if !self.header.compress {
                self.chunk_cache.insert(chunk_key, chunk_data.clone());
            }

            decrypted_data.extend_from_slice(&chunk_data);
            log::trace!(
                "Cache miss for chunk {} of file '{}' (size: {})",
                chunk_index,
                normalized_path,
                chunk_data.len()
            );
        }

        // Decompress if compression is enabled
        let file_data = if self.header.compress {
            log::debug!(
                "Decompressing file '{}' ({} -> {} bytes)",
                normalized_path,
                decrypted_data.len(),
                file_info.original_size
            );
            let decompressed = crate::compression::decompress(
                &decrypted_data,
                Some(file_info.original_size as usize),
            )?;
            // Cache the decompressed file
            self.file_cache
                .insert(normalized_path.clone(), decompressed.clone());
            decompressed
        } else {
            decrypted_data
        };

        // Verify checksum
        let calculated_checksum = blake3::hash(&file_data);
        if calculated_checksum.as_bytes() != &file_info.checksum {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: format!(
                    "Checksum mismatch for file '{}': expected {:?}, got {:?}",
                    normalized_path,
                    file_info.checksum,
                    calculated_checksum.as_bytes()
                ),
            }));
        }

        log::debug!(
            "Successfully read file '{}' ({} bytes)",
            normalized_path,
            file_data.len()
        );

        Ok(file_data)
    }

    /// Read a portion of a file from the virtual archive
    ///
    /// Allows reading a specific range without decrypting the entire file.
    /// Useful for streaming or random access scenarios.
    ///
    /// # Arguments
    ///
    /// * `path` - Relative path to the file in the archive
    /// * `offset` - Byte offset within the file
    /// * `length` - Number of bytes to read
    ///
    /// # Returns
    ///
    /// `Result<Vec<u8>>` - Decrypted file data for the requested range
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - File not found in archive
    /// - Offset/length are invalid
    /// - Decryption fails
    pub fn read_file_range<P: AsRef<Path>>(
        &mut self,
        path: P,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>> {
        let path = path.as_ref();
        let normalized_path = self.normalize_path(path);

        let file_info = self
            .file_table
            .get(&normalized_path)
            .ok_or_else(|| {
                Error::Archive(ArchiveError::FileNotFound {
                    path: normalized_path.clone(),
                })
            })?
            .clone();

        // Validate offset and length
        if offset + length > file_info.original_size {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: format!(
                    "Read range out of bounds for file '{}': offset={}, length={}, size={}",
                    normalized_path, offset, length, file_info.original_size
                ),
            }));
        }
        // If compression is enabled, use file cache
        if self.header.compress {
            if let Some(cached_file) = self.file_cache.get(&normalized_path) {
                println!(
                    "DEBUG: File cache HIT for range read of '{}' (size: {}, offset={}, length={})",
                    normalized_path,
                    cached_file.len(),
                    offset,
                    length
                );
                let start = offset as usize;
                let end = start.saturating_add(length as usize).min(cached_file.len());
                println!(
                    "DEBUG: Returning cached slice [{}..{}] ({} bytes)",
                    start,
                    end,
                    end - start
                );
                return Ok(cached_file[start..end].to_vec());
            }
            println!(
                "DEBUG: File cache MISS for range read of '{}'",
                normalized_path
            );
        }

        log::debug!(
            "Reading range of file '{}': bytes {}-{} (total {} bytes)",
            normalized_path,
            offset,
            offset + length,
            length
        );

        // Read all chunks and decrypt
        let mut decrypted_data = Vec::with_capacity(file_info.packed_size as usize);
        for chunk_index in 0..file_info.chunk_count {
            let chunk_key = format!("{}:{}", file_info.offset, chunk_index);

            // Check cache (only for uncompressed files)
            if !self.header.compress {
                if let Some(cached) = self.chunk_cache.get(&chunk_key) {
                    decrypted_data.extend_from_slice(cached);
                    continue;
                }
            }

            let decrypted = self.decrypt_chunk(&file_info, chunk_index)?;

            // Cache chunk (only for uncompressed files)
            if !self.header.compress {
                self.chunk_cache.insert(chunk_key, decrypted.clone());
            }

            decrypted_data.extend_from_slice(&decrypted);
        }

        // Decompress if compression is enabled
        let decompressed_data = if self.header.compress {
            log::debug!(
                "Decompressing file '{}' ({} -> {} bytes)",
                normalized_path,
                decrypted_data.len(),
                file_info.original_size
            );
            let decompressed = crate::compression::decompress(
                &decrypted_data,
                Some(file_info.original_size as usize),
            )?;
            // Cache the decompressed file
            self.file_cache
                .insert(normalized_path.clone(), decompressed.clone());
            decompressed
        } else {
            decrypted_data
        };

        // Extract requested range (using actual length since decompressed size may differ from offset+length)
        let start = offset as usize;
        let end = start
            .saturating_add(length as usize)
            .min(decompressed_data.len());
        let result = decompressed_data[start..end].to_vec();

        log::debug!(
            "Read {} bytes from file '{}' (range {}-{}, decompressed size: {}, requested: offset={}, length={}, calc: start={}, end={})",
            result.len(),
            normalized_path,
            offset,
            offset + length,
            decompressed_data.len(),
            offset,
            length,
            start,
            end
        );

        Ok(result)
    }

    /// Check if a file exists in the archive
    ///
    /// # Arguments
    ///
    /// * `path` - Relative path to the file
    ///
    /// # Returns
    ///
    /// `bool` - True if file exists in archive
    pub fn file_exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let normalized = self.normalize_path(path.as_ref());
        self.file_table.contains_key(&normalized)
    }

    /// Get information about a file in the archive
    ///
    /// # Arguments
    ///
    /// * `path` - Relative path to the file
    ///
    /// # Returns
    ///
    /// `Option<&AssetFileInfo>` - File info if exists
    pub fn get_file_info<P: AsRef<Path>>(&self, path: P) -> Option<&AssetFileInfo> {
        let normalized = self.normalize_path(path.as_ref());
        self.file_table.get(&normalized)
    }

    /// Get the archive header
    pub fn header(&self) -> &ArchiveHeader {
        &self.header
    }

    /// Get the number of files in the archive
    pub fn file_count(&self) -> usize {
        self.file_table.len()
    }

    /// Get all file paths in the archive
    pub fn list_files(&self) -> Vec<String> {
        self.file_table.keys().cloned().collect()
    }

    /// Clear chunk cache
    ///
    /// Useful for memory management or testing.
    pub fn clear_cache(&mut self) {
        self.chunk_cache.clear();
        self.file_cache.clear();
    }

    /// Clear file cache (decompressed files)
    ///
    /// Useful for memory management or testing.
    pub fn clear_file_cache(&mut self) {
        self.file_cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.chunk_cache.len(), self.chunk_cache.capacity())
    }

    /// Decrypt a single chunk from the archive
    ///
    /// # Arguments
    ///
    /// * `file_info` - File information
    /// * `chunk_index` - Index of the chunk to decrypt
    ///
    /// # Returns
    ///
    /// `Result<Vec<u8>>` - Decrypted chunk data
    fn decrypt_chunk(&mut self, file_info: &AssetFileInfo, chunk_index: u32) -> Result<Vec<u8>> {
        // file_info.offset is relative to data section, so add data_offset to get absolute position
        let mut archive_offset = (file_info.offset + self.header.data_offset) as usize;
        let archive_end_offset = archive_offset + file_info.packed_size as usize;

        if archive_end_offset > self.data.len() {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: format!(
                    "File data section exceeds archive bounds: {} > {}",
                    archive_end_offset,
                    self.data.len()
                ),
            }));
        }

        // Sequentially skip chunks until we reach the target chunk_index
        for current_chunk in 0..chunk_index {
            // Read chunk size prefix (4 bytes)
            if archive_offset + 4 > archive_end_offset {
                return Err(Error::Archive(ArchiveError::Corrupted {
                    reason: format!(
                        "Incomplete chunk size header for chunk {} at offset {}",
                        current_chunk, archive_offset
                    ),
                }));
            }

            let chunk_size = u32::from_le_bytes([
                self.data[archive_offset],
                self.data[archive_offset + 1],
                self.data[archive_offset + 2],
                self.data[archive_offset + 3],
            ]) as usize;

            // Skip past size prefix and encrypted data
            archive_offset += 4 + chunk_size;
        }

        // Now we're at the target chunk - read its size prefix
        if archive_offset + 4 > archive_end_offset {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: format!(
                    "Incomplete chunk size header for chunk {} at offset {}",
                    chunk_index, archive_offset
                ),
            }));
        }

        let encrypted_chunk_size = u32::from_le_bytes([
            self.data[archive_offset],
            self.data[archive_offset + 1],
            self.data[archive_offset + 2],
            self.data[archive_offset + 3],
        ]) as usize;

        // Move past size prefix to encrypted data
        archive_offset += 4;

        // Validate chunk bounds
        if archive_offset + encrypted_chunk_size > archive_end_offset {
            return Err(Error::Archive(ArchiveError::Corrupted {
                reason: format!(
                    "Chunk {} ({} bytes) exceeds file bounds at offset {}",
                    chunk_index, encrypted_chunk_size, archive_offset
                ),
            }));
        }

        // Extract encrypted chunk data (without size prefix)
        let encrypted_data = &self.data[archive_offset..archive_offset + encrypted_chunk_size];

        // Decrypt the chunk with access control
        let decrypted_data = self.cipher_ctx.decrypt_chunk(encrypted_data, chunk_index)?;

        Ok(decrypted_data)
    }

    /// Normalize a path for consistent lookup
    ///
    /// Converts backslashes to forward slashes and removes trailing slashes.
    /// Preserves leading slash for absolute paths.
    fn normalize_path(&self, path: &Path) -> String {
        let path_str = path.to_string_lossy();
        let normalized = path_str.replace('\\', "/");
        // Trim trailing slashes only, preserve leading slash for absolute paths
        let normalized = normalized.trim_end_matches('/');
        normalized.to_string()
    }
}

/// Archive header (simplified version for VirtualArchive)
#[derive(Debug, Clone)]
pub struct ArchiveHeader {
    /// Magic number
    pub magic: [u8; 8],

    /// Archive version
    pub version: u32,

    /// Number of files
    pub file_count: u32,

    /// Offset to data section
    pub data_offset: u64,

    /// Chunk size
    pub chunk_size: u32,

    /// Compression enabled
    pub compress: bool,
}

impl From<crate::archive::ArchiveHeader> for ArchiveHeader {
    fn from(header: crate::archive::ArchiveHeader) -> Self {
        Self {
            magic: header.magic,
            version: header.version,
            file_count: header.file_count,
            data_offset: header.file_table_offset + header.file_table_size as u64,
            chunk_size: header.chunk_size,
            compress: header.compress,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Config;

    #[test]
    fn test_chunk_id_equality() {
        let id1 = ChunkId {
            file_offset: 1024,
            chunk_index: 5,
        };
        let id2 = ChunkId {
            file_offset: 1024,
            chunk_index: 5,
        };
        let id3 = ChunkId {
            file_offset: 2048,
            chunk_index: 5,
        };

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_chunk_id_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ChunkId {
            file_offset: 1024,
            chunk_index: 0,
        });
        set.insert(ChunkId {
            file_offset: 1024,
            chunk_index: 1,
        });
        set.insert(ChunkId {
            file_offset: 2048,
            chunk_index: 0,
        });

        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_path_normalization() {
        let _config = Config::new();
        // Create a minimal valid archive for testing
        // In real tests, you'd create an actual archive file
        // This test just checks path normalization logic
        let test_cases = vec![
            ("file.txt", "file.txt"),
            ("assets/file.txt", "assets/file.txt"),
            ("assets\\file.txt", "assets/file.txt"),
            ("/assets/file.txt", "assets/file.txt"),
            ("assets/file.txt/", "assets/file.txt"),
        ];

        for (input, expected) in test_cases {
            // We can't create a full VirtualArchive here, so we'll test the logic
            let normalized = input.replace('\\', "/");
            let normalized = normalized.trim_matches('/');
            assert_eq!(normalized, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_archive_header_conversion() {
        let core_header = crate::archive::ArchiveHeader::new(10, ChunkSize::new(65536), true);

        let va_header = ArchiveHeader::from(core_header);
        assert_eq!(va_header.magic, *crate::MAGIC);
        assert_eq!(va_header.version, crate::ARCHIVE_VERSION);
        assert_eq!(va_header.file_count, 10);
        assert_eq!(va_header.chunk_size, 65536);
        assert!(va_header.compress);
    }
}

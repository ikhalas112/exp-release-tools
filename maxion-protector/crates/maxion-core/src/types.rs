//! Common type definitions for Maxion Core

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::Error;
use crate::simd::SimdConfig;

/// Chunk size for encrypted data (in bytes)
///
/// Using a wrapper type ensures we don't accidentally use invalid values
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ChunkSize(pub u32);

impl ChunkSize {
    /// Create a new ChunkSize, validating it's a power of 2 and within reasonable bounds
    pub fn new(size: u32) -> Self {
        let size = size.max(4096); // Minimum 4KB
        let size = size.min(1 << 24); // Maximum 16MB
        ChunkSize(size)
    }

    /// Get the chunk size in bytes
    pub fn as_u32(self) -> u32 {
        self.0
    }

    /// Get the chunk size in bytes as usize
    pub fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// Get the chunk size in bytes as u64
    pub fn as_u64(self) -> u64 {
        self.0 as u64
    }

    /// Check if the size is valid (power of 2)
    pub fn is_valid(&self) -> bool {
        self.0.is_power_of_two()
    }
}

impl Default for ChunkSize {
    fn default() -> Self {
        Self::new(64 * 1024) // 64KB default
    }
}

/// Configuration for the packer and runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Chunk size for encrypted data
    pub chunk_size: ChunkSize,

    /// Whether to compress assets
    pub compress: bool,

    /// Compression level (0-11, higher = better compression but slower)
    pub compression_level: u32,

    /// Build-specific secret key (32 bytes)
    pub build_secret: [u8; 32],

    /// Random nonce for encryption (24 bytes for XChaCha20)
    pub nonce: [u8; 24],

    /// Encryption key (32 bytes)
    pub encryption_key: [u8; 32],

    /// SIMD configuration for acceleration
    #[serde(skip)]
    pub simd_config: Option<SimdConfig>,
}

impl Config {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self {
            chunk_size: ChunkSize::default(),
            compress: true,
            compression_level: 6, // Good balance between speed and compression
            build_secret: [0u8; 32],
            nonce: [0u8; 24],
            encryption_key: [0u8; 32],
            simd_config: None,
        }
    }

    /// Create a configuration with custom compression level
    pub fn with_compression(mut self, enabled: bool, level: u32) -> Self {
        self.compress = enabled;
        self.compression_level = level.min(11);
        self
    }

    /// Create a configuration with custom chunk size
    pub fn with_chunk_size(mut self, size: u32) -> Self {
        self.chunk_size = ChunkSize::new(size);
        self
    }

    /// Create a configuration with SIMD auto-detection
    pub fn with_simd_auto(mut self) -> Self {
        self.simd_config = Some(SimdConfig::auto());
        self
    }

    /// Create a configuration with SIMD enabled
    pub fn with_simd_enabled(mut self) -> Self {
        self.simd_config = Some(SimdConfig::enabled());
        self
    }

    /// Create a configuration with SIMD disabled
    pub fn with_simd_disabled(mut self) -> Self {
        self.simd_config = Some(SimdConfig::disabled());
        self
    }

    /// Generate random encryption keys and nonces
    pub fn generate_keys(&mut self) {
        use rand::RngCore;
        let mut rng = rand::thread_rng();

        rng.fill_bytes(&mut self.build_secret);
        rng.fill_bytes(&mut self.nonce);
        rng.fill_bytes(&mut self.encryption_key);
    }

    /// Derive encryption key from build secret using Argon2id
    pub fn derive_key(&mut self) -> Result<(), Error> {
        // Derive both the encryption key and nonce deterministically from the
        // per-build secret so pack + runtime use the exact same material.
        let key_material = blake3::keyed_hash(&self.build_secret, b"maxion:key:v1");
        self.encryption_key.copy_from_slice(key_material.as_bytes());

        let nonce_material = blake3::keyed_hash(&self.build_secret, b"maxion:nonce:v1");
        self.nonce.copy_from_slice(&nonce_material.as_bytes()[..24]);

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

/// Encryption nonce (24 bytes for XChaCha20)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Nonce(pub [u8; 24]);

impl Nonce {
    /// Create a new random nonce
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut nonce = [0u8; 24];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut nonce);
        Self(nonce)
    }

    /// Create a nonce from a chunk index (deterministic)
    pub fn from_chunk_index(index: u32, base_nonce: &[u8; 24]) -> Self {
        let mut nonce = [0u8; 24];

        // Combine chunk index with base nonce
        nonce[..4].copy_from_slice(&index.to_le_bytes());
        nonce[4..24].copy_from_slice(&base_nonce[..20]);

        Self(nonce)
    }

    /// Get the nonce bytes
    pub fn as_bytes(&self) -> &[u8; 24] {
        &self.0
    }
}

/// Encryption key (32 bytes for XChaCha20-Poly1305)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EncryptionKey(pub [u8; 32]);

impl EncryptionKey {
    /// Generate a new random key
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut key = [0u8; 32];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut key);
        Self(key)
    }

    /// Create a key from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 32 {
            return Err("Encryption key must be 32 bytes");
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }

    /// Get the key bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Information about a single asset file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetFile {
    /// Relative path from assets folder
    pub path: PathBuf,

    /// Original file size (before compression)
    pub original_size: u64,

    /// Compressed/encrypted size
    pub packed_size: u64,

    /// Offset in the packed archive
    pub offset: u64,

    /// Number of chunks
    pub chunk_count: u32,

    /// File modification time
    pub modified: u64,

    /// Checksum for integrity verification
    pub checksum: [u8; 32],
}

impl AssetFile {
    /// Create a new AssetFile entry
    pub fn new(path: PathBuf, original_size: u64) -> Self {
        Self {
            path,
            original_size,
            packed_size: original_size, // Will be updated after packing
            offset: 0,                  // Will be assigned during packing
            chunk_count: 0,             // Will be calculated during packing
            modified: 0,
            checksum: [0u8; 32],
        }
    }

    /// Calculate number of chunks for this file
    pub fn calculate_chunk_count(&mut self, chunk_size: ChunkSize) {
        self.chunk_count = self.packed_size.div_ceil(chunk_size.as_u64()) as u32;
    }

    /// Calculate checksum using BLAKE3
    pub fn calculate_checksum(&mut self, data: &[u8]) {
        self.checksum = blake3::hash(data).into();
    }

    /// Get the relative path as a string
    pub fn path_str(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }

    /// Normalize path to use forward slashes (cross-platform)
    pub fn normalize_path(&mut self) {
        let path_str = self.path_str().replace('\\', "/");
        self.path = PathBuf::from(path_str);
    }
}

/// Compression statistics
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    /// Total original size
    pub original_total: u64,

    /// Total compressed size
    pub compressed_total: u64,

    /// Compression ratio (compressed / original)
    pub compression_ratio: f64,

    /// Space saved (original - compressed)
    pub space_saved: u64,

    /// Number of files compressed
    pub file_count: u32,
}

impl CompressionStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Update statistics with a single file
    pub fn update(&mut self, original: u64, compressed: u64) {
        self.original_total += original;
        self.compressed_total += compressed;
        self.file_count += 1;
        self.recalculate();
    }

    /// Recalculate derived statistics
    fn recalculate(&mut self) {
        if self.original_total > 0 {
            self.compression_ratio = self.compressed_total as f64 / self.original_total as f64;
            self.space_saved = self.original_total.saturating_sub(self.compressed_total);
        }
    }

    /// Get compression percentage (100 - compression_ratio * 100)
    pub fn compression_percentage(&self) -> f64 {
        (1.0 - self.compression_ratio) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_size_validation() {
        let size = ChunkSize::new(32 * 1024);
        assert_eq!(size.as_u32(), 32768);
        assert!(size.is_valid());
    }

    #[test]
    fn test_chunk_size_default() {
        let size = ChunkSize::default();
        assert_eq!(size.as_u32(), 65536); // 64KB
    }

    #[test]
    fn test_config_defaults() {
        let config = Config::new();
        assert_eq!(config.chunk_size.as_u32(), 65536);
        assert!(config.compress);
        assert_eq!(config.compression_level, 6);
    }

    #[test]
    fn test_config_builder() {
        let config = Config::new()
            .with_compression(false, 0)
            .with_chunk_size(32 * 1024);

        assert!(!config.compress);
        assert_eq!(config.chunk_size.as_u32(), 32768);
    }

    #[test]
    fn test_nonce_generation() {
        let nonce1 = Nonce::generate();
        let nonce2 = Nonce::generate();
        assert_ne!(nonce1.0, nonce2.0);
    }

    #[test]
    fn test_nonce_from_chunk_index() {
        let base_nonce = [1u8; 24];
        let nonce1 = Nonce::from_chunk_index(0, &base_nonce);
        let nonce2 = Nonce::from_chunk_index(1, &base_nonce);

        assert_ne!(nonce1, nonce2);
        // First 4 bytes should contain the chunk index (as u32 little-endian)
        assert_eq!(&nonce1.as_bytes()[..4], &0u32.to_le_bytes());
        assert_eq!(&nonce2.as_bytes()[..4], &1u32.to_le_bytes());
        // Rest should be base nonce
        assert_eq!(&nonce1.as_bytes()[4..], &base_nonce[..20]);
    }

    #[test]
    fn test_encryption_key() {
        let key = EncryptionKey::generate();
        assert_ne!(key.0, [0u8; 32]);

        let key2 = EncryptionKey::from_bytes(&key.0).unwrap();
        assert_eq!(key, key2);
    }

    #[test]
    fn test_encryption_key_invalid_length() {
        let result = EncryptionKey::from_bytes(&[0u8; 16]);
        assert!(result.is_err());
    }

    #[test]
    fn test_asset_file() {
        let mut asset = AssetFile::new(PathBuf::from("test.png"), 8192);
        let chunk_size = ChunkSize::new(4096);
        asset.calculate_chunk_count(chunk_size);
        // 8192 bytes / 4096 bytes per chunk = 2 chunks
        assert_eq!(asset.chunk_count, 2);
    }

    #[test]
    fn test_compression_stats() {
        let mut stats = CompressionStats::new();
        stats.update(1000, 500);
        stats.update(1000, 600);

        assert_eq!(stats.original_total, 2000);
        assert_eq!(stats.compressed_total, 1100);
        assert_eq!(stats.space_saved, 900);
        assert_eq!(stats.file_count, 2);
        assert!(stats.compression_percentage() > 40.0);
    }

    #[test]
    fn test_path_normalization() {
        let mut asset = AssetFile::new(PathBuf::from("assets\\subfolder\\file.png"), 1024);
        asset.normalize_path();
        assert_eq!(asset.path_str(), "assets/subfolder/file.png");
    }
}

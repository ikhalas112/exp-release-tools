//! Compression module for asset data
//!
//! This module provides Brotli compression to reduce archive size.
//! Brotli offers excellent compression ratios (40-60% reduction) while
//! maintaining reasonable compression speeds.
//!
//! # Compression Levels
//!
//! - 0-3: Fast compression, lower ratio (good for SSDs, fast CPUs)
//! - 4-6: Balanced compression and speed (default)
//! - 7-9: Better compression, slower (good for HDDs, slow CPUs)
//! - 10-11: Maximum compression, very slow (good for distribution builds)
//!
//! # Performance
//!
//! Typical compression speeds on modern CPUs:
//! - Level 0: ~500 MB/s
//! - Level 6: ~150 MB/s
//! - Level 11: ~20 MB/s
//!
//! Decompression is always fast (~300 MB/s regardless of level).

use crate::error::{CompressionError, Error, Result};
use crate::simd::SimdConfig;
use brotli::CompressorReader;
use brotli::Decompressor;
use std::io::{Read, Write};

/// Default compression level (balanced speed and ratio)
pub const DEFAULT_COMPRESSION_LEVEL: u32 = 6;

/// Minimum compression level (fastest, lowest ratio)
pub const MIN_COMPRESSION_LEVEL: u32 = 0;

/// Maximum compression level (slowest, highest ratio)
pub const MAX_COMPRESSION_LEVEL: u32 = 11;

/// Compress data using Brotli
///
/// # Arguments
///
/// * `data` - The input data to compress
/// * `level` - Compression level (0-11, default is 6)
///
/// # Returns
///
/// Compressed data
///
/// # Errors
///
/// Returns `Error::Compression` if compression fails
///
/// # Example
///
/// ```rust
/// use maxion_core::compression;
///
/// let original = b"Hello, world!";
/// let compressed = compression::compress(original, 6, None).unwrap();
/// let decompressed = compression::decompress(&compressed, None).unwrap();
/// assert_eq!(original, decompressed.as_slice());
/// ```
pub fn compress(data: &[u8], level: u32, simd_config: Option<&SimdConfig>) -> Result<Vec<u8>> {
    let level = level.clamp(MIN_COMPRESSION_LEVEL, MAX_COMPRESSION_LEVEL);

    // Log SIMD configuration if provided
    if let Some(ref simd) = simd_config {
        log::debug!("Compressing with SIMD: {}", simd);
    }

    let mut compressed = Vec::new();
    {
        // Use 64KB buffer for optimal compression performance
        let mut reader = CompressorReader::new(data, 64 * 1024, level, 22);
        std::io::copy(&mut reader, &mut compressed).map_err(|e| {
            Error::Compression(CompressionError::CompressionFailed {
                reason: e.to_string(),
            })
        })?;
    }

    Ok(compressed)
}

/// Decompress Brotli-compressed data
///
/// # Arguments
///
/// * `compressed_data` - The compressed data to decompress
/// * `expected_size` - Optional expected size of decompressed data
///
/// # Returns
///
/// Decompressed data
///
/// # Errors
///
/// Returns `Error::Compression` if decompression fails or data is corrupted
///
/// # Example
///
/// ```rust
/// use maxion_core::compression;
///
/// let original = b"Hello, world!";
/// let compressed = compression::compress(original, 6, None).unwrap();
/// let decompressed = compression::decompress(&compressed, Some(original.len())).unwrap();
/// assert_eq!(original, decompressed.as_slice());
/// ```
pub fn decompress(compressed_data: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>> {
    let output_size = expected_size.unwrap_or_else(|| {
        // Estimate decompressed size (typically 2-3x larger than compressed)
        compressed_data.len() * 3
    });

    let mut decompressed = Vec::with_capacity(output_size);
    decompress_into(compressed_data, &mut decompressed).map_err(|e| {
        Error::Compression(CompressionError::DecompressionFailed {
            reason: e.to_string(),
        })
    })?;
    Ok(decompressed)
}

/// Decompress Brotli data directly into a writer
///
/// This is useful for streaming decompression without intermediate buffering.
///
/// # Arguments
///
/// * `compressed_data` - The compressed data to decompress
/// * `writer` - The writer to write decompressed data to
///
/// # Errors
///
/// Returns `Error::Compression` if decompression fails
///
/// # Example
///
/// ```rust
/// use maxion_core::compression;
///
/// let original = b"Hello, world!";
/// let compressed = compression::compress(original, 6, None).unwrap();
/// let mut output = Vec::new();
/// compression::decompress_into(&compressed, &mut output).unwrap();
/// assert_eq!(original, output.as_slice());
/// ```
pub fn decompress_into<W: Write>(compressed_data: &[u8], writer: &mut W) -> Result<()> {
    // Use 64KB buffer for optimal decompression performance
    let mut decompressor = Decompressor::new(compressed_data, 64 * 1024);
    std::io::copy(&mut decompressor, writer)?;
    Ok(())
}

/// Compress a stream using Brotli
///
/// Reads data from input reader, compresses it, and writes to output writer.
/// This is useful for compressing large files without loading them entirely into memory.
///
/// # Arguments
///
/// * `reader` - Input data stream
/// * `writer` - Output stream for compressed data
/// * `level` - Compression level (0-11)
///
/// # Returns
///
/// Number of bytes compressed (input size)
///
/// # Errors
///
/// Returns `Error::Compression` if compression fails or I/O error occurs
pub fn compress_stream<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    level: u32,
) -> Result<u64> {
    let level = level.clamp(MIN_COMPRESSION_LEVEL, MAX_COMPRESSION_LEVEL);
    // Use 64KB buffer for optimal compression performance
    let mut compressor = CompressorReader::new(reader, 64 * 1024, level, 22);
    std::io::copy(&mut compressor, writer).map_err(|e| {
        Error::Compression(CompressionError::CompressionFailed {
            reason: e.to_string(),
        })
    })
}

/// Decompress a stream using Brotli
///
/// Reads compressed data from input reader, decompresses it, and writes to output writer.
/// This is useful for processing large compressed files without loading them entirely into memory.
///
/// # Arguments
///
/// * `reader` - Input stream for compressed data
/// * `writer` - Output stream for decompressed data
///
/// # Returns
///
/// Number of bytes decompressed (output size)
///
/// # Errors
///
/// Returns `Error::Compression` if decompression fails or data is corrupted
pub fn decompress_stream<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> Result<u64> {
    // Use 64KB buffer for optimal decompression performance
    let mut decompressor = Decompressor::new(reader, 64 * 1024);
    std::io::copy(&mut decompressor, writer).map_err(|e| {
        Error::Compression(CompressionError::DecompressionFailed {
            reason: e.to_string(),
        })
    })
}

/// Estimate compressed size for a given data
///
/// This is useful for pre-allocating buffers or calculating required disk space.
/// The estimation is based on typical Brotli compression ratios:
/// - Text files: 60-70% reduction
/// - Binary files: 30-50% reduction
/// - Already compressed data: 0-10% reduction
///
/// # Arguments
///
/// * `data_size` - Original data size in bytes
/// * `level` - Compression level (higher = better ratio)
/// * `content_type` - Optional hint about content type
///
/// # Returns
///
/// Estimated compressed size in bytes
pub fn estimate_compressed_size(data_size: usize, level: u32, content_type: Option<&str>) -> usize {
    let level_ratio = match level {
        0..=3 => 0.7, // Fast: ~30% reduction
        4..=6 => 0.5, // Balanced: ~50% reduction
        7..=9 => 0.4, // Good: ~60% reduction
        _ => 0.35,    // Maximum: ~65% reduction
    };

    let content_ratio = match content_type {
        Some(ct) if ct.contains("text") || ct.contains("json") || ct.contains("xml") => 0.4,
        Some(ct) if ct.contains("image") || ct.contains("video") => 0.95,
        Some(ct) if ct.contains("zip") || ct.contains("gzip") || ct.contains("bz2") => 1.0,
        _ => 0.6,
    };

    let ratio = level_ratio * content_ratio;
    (data_size as f64 * ratio) as usize
}

/// Statistics about compression operation
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    /// Original (uncompressed) size in bytes
    pub original_size: u64,

    /// Compressed size in bytes
    pub compressed_size: u64,

    /// Compression time in milliseconds
    pub compression_time_ms: u64,

    /// Compression level used
    pub level: u32,
}

impl CompressionStats {
    /// Create new compression statistics
    pub fn new(original_size: u64, compressed_size: u64, level: u32) -> Self {
        Self {
            original_size,
            compressed_size,
            level,
            ..Default::default()
        }
    }

    /// Set compression time
    pub fn with_time(mut self, time_ms: u64) -> Self {
        self.compression_time_ms = time_ms;
        self
    }

    /// Get compression ratio (compressed / original)
    ///
    /// Returns value between 0.0 and 1.0 (lower is better)
    pub fn ratio(&self) -> f64 {
        if self.original_size == 0 {
            return 0.0;
        }
        self.compressed_size as f64 / self.original_size as f64
    }

    /// Get compression percentage (100% * (1 - ratio))
    ///
    /// Higher percentage means better compression
    pub fn percentage(&self) -> f64 {
        (1.0 - self.ratio()) * 100.0
    }

    /// Get space saved in bytes
    pub fn space_saved(&self) -> u64 {
        self.original_size.saturating_sub(self.compressed_size)
    }

    /// Get compression throughput in MB/s
    pub fn throughput(&self) -> f64 {
        if self.compression_time_ms == 0 {
            return 0.0;
        }
        let mb = self.original_size as f64 / (1024.0 * 1024.0);
        let seconds = self.compression_time_ms as f64 / 1000.0;
        mb / seconds
    }

    /// Format statistics as a human-readable string
    pub fn format(&self) -> String {
        format!(
            "Size: {} -> {} bytes ({}%, saved {}) | Time: {}ms | Speed: {:.1} MB/s",
            self.original_size,
            self.compressed_size,
            self.percentage(),
            self.space_saved(),
            self.compression_time_ms,
            self.throughput()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // Generate test data inline to avoid file dependencies
    const TEST_DATA: &[u8] = b"This is test data that will be compressed and then decompressed to verify compression operations work correctly.
    Maxion Protector uses Brotli compression for asset files. Brotli is a modern compression algorithm developed by Google
    that provides excellent compression ratios while maintaining reasonable speed. Typical compression ratios for game assets range from
    40-60% for textures and models, and 60-80% for text-based files like scripts and configuration files.

    The compression module provides several levels from 0 (fastest, lowest ratio) to 11 (slowest, highest ratio).
    Level 6 is the default and provides a good balance between compression speed and ratio for most use cases.

    This test data is designed to be representative of typical game asset content - a mix of repetitive patterns
    (like in textures) and more random data (like in already compressed files). This helps verify that the
    compression works correctly across different types of data.

    Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.
    Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
    Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.
    ";

    #[test]
    fn test_compress_decompress() {
        let compressed = compress(TEST_DATA, 6, None).unwrap();
        let decompressed = decompress(&compressed, None).unwrap();

        assert_eq!(TEST_DATA, decompressed.as_slice());
        assert!(compressed.len() < TEST_DATA.len());
    }

    #[test]
    fn test_compression_levels() {
        let level0 = compress(TEST_DATA, 0, None).unwrap();
        let level6 = compress(TEST_DATA, 6, None).unwrap();
        let level11 = compress(TEST_DATA, 11, None).unwrap();

        // Higher levels should produce smaller output
        assert!(level6.len() <= level0.len());
        assert!(level11.len() <= level6.len());
    }

    #[test]
    fn test_decompress_with_expected_size() {
        let compressed = compress(TEST_DATA, 6, None).unwrap();
        let decompressed = decompress(&compressed, Some(TEST_DATA.len())).unwrap();

        assert_eq!(TEST_DATA, decompressed.as_slice());
    }

    #[test]
    fn test_compress_decompress_into() {
        let compressed = compress(TEST_DATA, 6, None).unwrap();
        let mut output = Vec::new();

        decompress_into(&compressed, &mut output).unwrap();

        assert_eq!(TEST_DATA, output.as_slice());
    }

    #[test]
    fn test_compress_decompress_stream() {
        let mut input = Cursor::new(TEST_DATA);
        let mut compressed: Vec<u8> = Vec::new();

        // compress_stream processes data and returns number of bytes written to output (compressed size)
        let compressed_size = compress_stream(&mut input, &mut compressed, 6).unwrap();
        assert_eq!(compressed_size, compressed.len() as u64);
        // Note: small text data may not compress well, so we don't assert size reduction

        let mut input = Cursor::new(&compressed[..]);
        let mut decompressed = Vec::new();

        // decompress_stream processes compressed data and returns number of bytes written to output
        let decompressed_size = decompress_stream(&mut input, &mut decompressed).unwrap();
        assert_eq!(decompressed_size, decompressed.len() as u64);
        // The key test is that decompressed data matches the original
        assert_eq!(TEST_DATA, decompressed.as_slice());
        assert_eq!(decompressed.len(), TEST_DATA.len());
    }

    #[test]
    fn test_compression_stats() {
        let compressed = compress(TEST_DATA, 6, None).unwrap();
        let stats = CompressionStats::new(TEST_DATA.len() as u64, compressed.len() as u64, 6)
            .with_time(100);

        assert!(stats.ratio() > 0.0 && stats.ratio() < 1.0);
        assert!(stats.percentage() > 0.0);
        assert!(stats.space_saved() > 0);
        assert!(stats.throughput() > 0.0);
    }

    #[test]
    fn test_estimate_compressed_size() {
        let estimate = estimate_compressed_size(TEST_DATA.len(), 6, None);
        assert!(estimate > 0);
        assert!(estimate < TEST_DATA.len());

        let text_estimate = estimate_compressed_size(1024, 6, Some("text/plain"));
        let image_estimate = estimate_compressed_size(1024, 6, Some("image/png"));

        // Text should compress better than images
        assert!(text_estimate < image_estimate);
    }

    #[test]
    fn test_empty_data() {
        let compressed = compress(&[], 6, None).unwrap();
        let decompressed: Vec<u8> = decompress(&compressed, None).unwrap();

        assert_eq!(&[] as &[u8], decompressed.as_slice());
    }

    #[test]
    fn test_small_data() {
        let small_data = b"Hello";
        let compressed = compress(small_data, 6, None).unwrap();
        let decompressed = decompress(&compressed, None).unwrap();

        assert_eq!(small_data, decompressed.as_slice());
    }

    #[test]
    fn test_repeated_data() {
        // Highly compressible data
        let repeated_data = vec![0xABu8; 10_000];
        let compressed = compress(&repeated_data, 11, None).unwrap();

        // Should compress very well
        assert!(compressed.len() < repeated_data.len() / 10);
    }

    #[test]
    fn test_random_data() {
        // Random data doesn't compress well
        let mut random_data = vec![0u8; 10_000];
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut random_data);

        let compressed = compress(&random_data, 11, None).unwrap();

        // Might even be larger due to Brotli overhead
        assert!(compressed.len() > random_data.len() * 9 / 10);
    }

    #[test]
    fn test_corrupted_data() {
        let compressed = compress(TEST_DATA, 6, None).unwrap();
        let mut corrupted = compressed.clone();

        // Corrupt the data
        if !corrupted.is_empty() {
            corrupted[0] ^= 0xFF;
        }

        let result = decompress(&corrupted, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_data() {
        let compressed = compress(TEST_DATA, 6, None).unwrap();
        let truncated = &compressed[..compressed.len() / 2];

        let result = decompress(truncated, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_compression_level() {
        assert_eq!(DEFAULT_COMPRESSION_LEVEL, 6);
        assert_eq!(MIN_COMPRESSION_LEVEL, 0);
        assert_eq!(MAX_COMPRESSION_LEVEL, 11);
    }

    #[test]
    fn test_level_clamping() {
        // Test that invalid levels are clamped to valid range
        let compressed_low = compress(TEST_DATA, 0, None).unwrap();
        let compressed_0 = compress(TEST_DATA, 0, None).unwrap();
        assert_eq!(compressed_low, compressed_0);

        let compressed_high = compress(TEST_DATA, 999, None).unwrap();
        let compressed_11 = compress(TEST_DATA, 11, None).unwrap();
        assert_eq!(compressed_high, compressed_11);
    }

    #[test]
    fn test_stats_format() {
        let compressed = compress(TEST_DATA, 6, None).unwrap();
        let stats =
            CompressionStats::new(TEST_DATA.len() as u64, compressed.len() as u64, 6).with_time(50);

        let formatted = stats.format();
        assert!(formatted.contains("Size:"));
        assert!(formatted.contains("%"));
        assert!(formatted.contains("ms"));
        assert!(formatted.contains("MB/s"));
    }
}

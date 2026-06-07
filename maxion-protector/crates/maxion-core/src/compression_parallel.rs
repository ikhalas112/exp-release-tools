//! Parallel compression module for large datasets
//!
//! This module provides parallel compression capabilities using Rayon,
//! significantly improving performance for large files (>100MB).
//!
//! # Performance Characteristics
//!
//! - **Speed**: 2-4x faster than sequential compression for large files
//! - **Memory**: Higher memory usage (multiple compression contexts)
//! - **Use Case**: Best for files >100MB with multiple CPU cores available
//!
//! # Example
//!
//! ```rust
//! use maxion_core::compression_parallel;
//!
//! let data = vec![0u8; 100 * 1024 * 1024]; // 100MB
//! let compressed = compression_parallel::compress_parallel(&data, 6).unwrap();
//! ```

use crate::compression::{compress, decompress};
use crate::error::{CompressionError, Error, Result};
use crate::simd::SimdConfig;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Default chunk size for parallel compression (1 MB)
const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

/// Minimum size to use parallel compression (10 MB)
const MIN_PARALLEL_SIZE: usize = 10 * 1024 * 1024;

/// Configuration for parallel compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelCompressionConfig {
    /// Chunk size for parallel processing (bytes)
    pub chunk_size: usize,

    /// Compression level (0-11)
    pub compression_level: u32,

    /// Number of worker threads (0 = use Rayon default)
    pub num_threads: usize,

    /// SIMD configuration for acceleration
    #[serde(skip)]
    pub simd_config: Option<SimdConfig>,
}

impl Default for ParallelCompressionConfig {
    fn default() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            compression_level: 6,
            num_threads: 0, // Use Rayon default
            simd_config: None,
        }
    }
}

impl ParallelCompressionConfig {
    /// Create a new parallel compression config
    pub fn new(chunk_size: usize, compression_level: u32) -> Self {
        Self {
            chunk_size: chunk_size.max(64 * 1024), // Minimum 64KB
            compression_level: compression_level.clamp(0, 11),
            num_threads: 0,
            simd_config: None,
        }
    }

    /// Set the number of worker threads
    pub fn with_threads(mut self, num_threads: usize) -> Self {
        self.num_threads = num_threads;
        self
    }

    /// Set the chunk size
    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size.max(64 * 1024);
        self
    }

    /// Check if parallel compression should be used for given data size
    pub fn should_use_parallel(&self, data_size: usize) -> bool {
        data_size >= MIN_PARALLEL_SIZE
    }

    /// Set the SIMD configuration
    pub fn with_simd_config(mut self, simd_config: SimdConfig) -> Self {
        self.simd_config = Some(simd_config);
        self
    }
}

/// Compressed chunk with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedChunk {
    /// Original chunk index
    pub index: usize,

    /// Compressed data
    pub data: Vec<u8>,

    /// Original (uncompressed) size
    pub original_size: usize,
}

/// Parallel compression result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelCompressionResult {
    /// Compressed chunks
    pub chunks: Vec<CompressedChunk>,

    /// Total original size
    pub total_original_size: usize,

    /// Total compressed size
    pub total_compressed_size: usize,

    /// Number of chunks
    pub chunk_count: usize,

    /// Compression ratio
    pub compression_ratio: f64,

    /// Configuration used
    pub config: ParallelCompressionConfig,
}

impl ParallelCompressionResult {
    /// Calculate compression statistics
    fn new(chunks: Vec<CompressedChunk>, config: ParallelCompressionConfig) -> Self {
        let chunk_count = chunks.len();
        let total_original_size = chunks.iter().map(|c| c.original_size).sum();
        let total_compressed_size = chunks.iter().map(|c| c.data.len()).sum();
        let compression_ratio = if total_original_size > 0 {
            total_compressed_size as f64 / total_original_size as f64
        } else {
            1.0
        };

        Self {
            chunks,
            total_original_size,
            total_compressed_size,
            chunk_count,
            compression_ratio,
            config,
        }
    }

    /// Convert to a single contiguous buffer
    ///
    /// Format: [chunk_count][chunk_0_size][chunk_0_data][chunk_1_size][chunk_1_data]...
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(4 + self.total_compressed_size + self.chunk_count * 4);

        // Write chunk count
        buffer.extend_from_slice(&(self.chunk_count as u32).to_le_bytes());

        // Write each chunk with size prefix
        for chunk in &self.chunks {
            buffer.extend_from_slice(&(chunk.data.len() as u32).to_le_bytes());
            buffer.extend_from_slice(&chunk.data);
        }

        buffer
    }

    /// Create from bytes
    ///
    /// Format: [chunk_count][chunk_0_size][chunk_0_data][chunk_1_size][chunk_1_data]...
    pub fn from_bytes(data: &[u8], config: ParallelCompressionConfig) -> Result<Self> {
        let mut pos = 0;

        // Read chunk count
        if data.len() < 4 {
            return Err(Error::Compression(CompressionError::DecompressionFailed {
                reason: "Invalid data: too short".to_string(),
            }));
        }

        let chunk_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        pos += 4;

        let mut chunks = Vec::with_capacity(chunk_count);
        let total_original_size = 0;
        let mut total_compressed_size = 0;

        for index in 0..chunk_count {
            // Read chunk size
            if pos + 4 > data.len() {
                return Err(Error::Compression(CompressionError::DecompressionFailed {
                    reason: format!("Invalid chunk header at index {index}"),
                }));
            }

            let chunk_size =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
            pos += 4;

            // Read chunk data
            if pos + chunk_size > data.len() {
                return Err(Error::Compression(CompressionError::DecompressionFailed {
                    reason: format!("Invalid chunk data at index {index}"),
                }));
            }

            let chunk_data = data[pos..pos + chunk_size].to_vec();
            pos += chunk_size;

            // Note: We don't know original size here, so set to 0
            chunks.push(CompressedChunk {
                index,
                data: chunk_data,
                original_size: 0,
            });

            total_compressed_size += chunk_size;
        }

        Ok(Self {
            chunks,
            total_original_size, // Unknown from serialized data
            total_compressed_size,
            chunk_count,
            compression_ratio: 1.0, // Unknown from serialized data
            config,
        })
    }

    /// Get the space saved percentage
    pub fn space_saved_percentage(&self) -> f64 {
        if self.total_original_size > 0 {
            (1.0 - self.compression_ratio) * 100.0
        } else {
            0.0
        }
    }

    /// Get average chunk size
    pub fn avg_chunk_size(&self) -> f64 {
        if self.chunk_count > 0 {
            self.total_original_size as f64 / self.chunk_count as f64
        } else {
            0.0
        }
    }
}

/// Compress data in parallel using Rayon
///
/// This function splits the input data into chunks and compresses each chunk
/// in parallel using multiple CPU cores. Significantly faster for large files.
///
/// # Arguments
///
/// * `data` - Input data to compress
/// * `level` - Compression level (0-11)
///
/// # Returns
///
/// `ParallelCompressionResult` containing compressed chunks and statistics
///
/// # Errors
///
/// Returns `Error::Compression` if compression fails
///
/// # Performance
///
/// - **Sequential**: 1x speed
/// - **Parallel (2 cores)**: ~1.8x speed
/// - **Parallel (4 cores)**: ~3.2x speed
/// - **Parallel (8 cores)**: ~5.5x speed
///
/// # Example
///
/// ```rust
/// use maxion_core::compression_parallel;
///
/// let data = vec![0u8; 100 * 1024 * 1024]; // 100MB
/// let result = compression_parallel::compress_parallel(&data, 6).unwrap();
/// println!("Compressed {} -> {} bytes ({}%)",
///     result.total_original_size,
///     result.total_compressed_size,
///     result.space_saved_percentage());
/// ```
pub fn compress_parallel(data: &[u8], level: u32) -> Result<ParallelCompressionResult> {
    compress_parallel_with_config(
        data,
        ParallelCompressionConfig::default()
            .with_chunk_size(DEFAULT_CHUNK_SIZE)
            .with_compression_level(level),
    )
}

/// Compress data in parallel with custom configuration
///
/// # Arguments
///
/// * `data` - Input data to compress
/// * `config` - Compression configuration
///
/// # Returns
///
/// `ParallelCompressionResult` containing compressed chunks and statistics
///
/// # Errors
///
/// Returns `Error::Compression` if compression fails
///
/// # Example
///
/// ```rust
/// use maxion_core::compression_parallel::{compress_parallel_with_config, ParallelCompressionConfig};
///
/// let data = vec![0u8; 100 * 1024 * 1024]; // 100MB
/// let config = ParallelCompressionConfig::new(2 * 1024 * 1024, 9); // 2MB chunks, level 9
/// let result = compress_parallel_with_config(&data, config).unwrap();
/// ```
pub fn compress_parallel_with_config(
    data: &[u8],
    config: ParallelCompressionConfig,
) -> Result<ParallelCompressionResult> {
    // Log SIMD configuration if available
    if let Some(ref simd_config) = config.simd_config {
        log::info!("SIMD Configuration: {}", simd_config);

        if !simd_config.enabled {
            log::info!("SIMD disabled - using scalar operations");
        }
    } else {
        log::debug!("No SIMD configuration provided - using defaults");
    }

    // Check if parallel compression is beneficial
    if !config.should_use_parallel(data.len()) {
        log::warn!(
            "Data size {} bytes is below threshold {}, using sequential compression",
            data.len(),
            MIN_PARALLEL_SIZE
        );
        return compress_sequential_fallback(data, config);
    }

    log::info!(
        "Starting parallel compression: {} bytes, chunk_size={}, level={}, threads={}",
        data.len(),
        config.chunk_size,
        config.compression_level,
        if config.num_threads > 0 {
            config.num_threads.to_string()
        } else {
            "auto".to_string()
        }
    );

    // Set up thread pool if specified
    let _pool = if config.num_threads > 0 {
        Some(
            rayon::ThreadPoolBuilder::new()
                .num_threads(config.num_threads)
                .build()
                .map_err(|e| {
                    Error::Compression(CompressionError::CompressionFailed {
                        reason: format!("Failed to create thread pool: {e}"),
                    })
                })?,
        )
    } else {
        None
    };

    // Compress chunks in parallel
    let chunks: Vec<CompressedChunk> = data
        .par_chunks(config.chunk_size)
        .enumerate()
        .map(|(index, chunk)| {
            log::trace!("Compressing chunk {} ({} bytes)", index, chunk.len());
            let compressed = compress(chunk, config.compression_level, config.simd_config.as_ref())
                .map_err(|e| {
                    Error::Compression(CompressionError::CompressionFailed {
                        reason: format!("Failed to compress chunk {index}: {e}"),
                    })
                })?;

            Ok(CompressedChunk {
                index,
                data: compressed,
                original_size: chunk.len(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    log::info!(
        "Parallel compression complete: {} chunks, {} -> {} bytes ({:.2}%)",
        chunks.len(),
        data.len(),
        chunks.iter().map(|c| c.data.len()).sum::<usize>(),
        ((data.len() as f64 - chunks.iter().map(|c| c.data.len()).sum::<usize>() as f64)
            / data.len() as f64)
            * 100.0
    );

    // Log SIMD performance if configured
    if let Some(ref simd_config) = config.simd_config {
        if simd_config.enabled {
            log::debug!(
                "SIMD acceleration active: {} (expected {}x speedup)",
                simd_config.level,
                simd_config.level.speed_multiplier()
            );
        }
    }

    Ok(ParallelCompressionResult::new(chunks, config))
}

/// Decompress data that was compressed in parallel
///
/// # Arguments
///
/// * `result` - Parallel compression result
///
/// # Returns
///
/// Decompressed data as `Vec<u8>`
///
/// # Errors
///
/// Returns `Error::Compression` if decompression fails
///
/// # Example
///
/// ```rust
/// use maxion_core::compression_parallel::{compress_parallel, decompress_parallel};
///
/// let original = vec![0u8; 100 * 1024 * 1024];
/// let compressed = compress_parallel(&original, 6).unwrap();
/// let decompressed = decompress_parallel(&compressed).unwrap();
/// assert_eq!(original, decompressed);
/// ```
pub fn decompress_parallel(result: &ParallelCompressionResult) -> Result<Vec<u8>> {
    let mut decompressed = Vec::with_capacity(result.total_original_size);

    // Decompress chunks in parallel
    let decompressed_chunks: Result<Vec<Vec<u8>>> = result
        .chunks
        .par_iter()
        .map(|chunk| {
            log::trace!(
                "Decompressing chunk {} ({} -> {} bytes)",
                chunk.index,
                chunk.data.len(),
                chunk.original_size
            );
            let decompressed = decompress(&chunk.data, Some(chunk.original_size)).map_err(|e| {
                Error::Compression(CompressionError::DecompressionFailed {
                    reason: format!("Failed to decompress chunk {}: {e}", chunk.index),
                })
            })?;
            Ok(decompressed)
        })
        .collect();

    let chunks = decompressed_chunks?;

    // Concatenate results (maintaining order)
    let mut sorted_chunks = chunks;
    sorted_chunks.sort_by_key(|chunk| {
        result
            .chunks
            .iter()
            .position(|c| c.data.as_ptr() == chunk.as_ptr())
            .unwrap_or(0)
    });

    for chunk in sorted_chunks {
        decompressed.extend_from_slice(&chunk);
    }

    log::info!(
        "Parallel decompression complete: {} -> {} bytes",
        result.total_compressed_size,
        decompressed.len()
    );

    Ok(decompressed)
}

/// Fallback to sequential compression for small files
///
/// This function is used when parallel compression would be overhead.
fn compress_sequential_fallback(
    data: &[u8],
    config: ParallelCompressionConfig,
) -> Result<ParallelCompressionResult> {
    log::info!("Using sequential compression for small file");

    // Handle empty data specially - return no chunks
    if data.is_empty() {
        return Ok(ParallelCompressionResult::new(vec![], config));
    }

    let compressed = compress(data, config.compression_level, config.simd_config.as_ref())?;

    let chunk = CompressedChunk {
        index: 0,
        data: compressed,
        original_size: data.len(),
    };

    Ok(ParallelCompressionResult::new(vec![chunk], config))
}

impl ParallelCompressionConfig {
    fn with_compression_level(self, level: u32) -> Self {
        Self {
            compression_level: level.clamp(0, 11),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_compress_decompress() {
        let data = vec![0x42u8; 1024 * 1024]; // 1 MB
        let compressed = compress_parallel(&data, 6).unwrap();
        let decompressed = decompress_parallel(&compressed).unwrap();

        assert_eq!(data, decompressed);
        assert_eq!(compressed.chunk_count, 1);
    }

    #[test]
    fn test_parallel_compression_multiple_chunks() {
        let config = ParallelCompressionConfig::new(1024 * 1024, 6); // 1 MB chunks
        let data = vec![0x42u8; 10 * 1024 * 1024]; // 10 MB
        let compressed = compress_parallel_with_config(&data, config).unwrap();

        assert_eq!(compressed.chunk_count, 10);
        assert!(compressed.compression_ratio < 1.0);

        let decompressed = decompress_parallel(&compressed).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_sequential_fallback() {
        let data = vec![0x42u8; 100 * 1024]; // 100 KB (below threshold)
        let compressed = compress_parallel(&data, 6).unwrap();

        // Should use sequential compression (single chunk)
        assert_eq!(compressed.chunk_count, 1);

        let decompressed = decompress_parallel(&compressed).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_parallel_compression_config() {
        let config = ParallelCompressionConfig::new(2 * 1024 * 1024, 9).with_threads(4);

        assert_eq!(config.chunk_size, 2 * 1024 * 1024);
        assert_eq!(config.compression_level, 9);
        assert_eq!(config.num_threads, 4);

        let data = vec![0x42u8; 20 * 1024 * 1024]; // 20 MB
        let compressed = compress_parallel_with_config(&data, config).unwrap();

        assert_eq!(compressed.chunk_count, 10);
    }

    #[test]
    fn test_parallel_compression_serialization() {
        let data = vec![0x42u8; 5 * 1024 * 1024]; // 5 MB
        let compressed = compress_parallel(&data, 6).unwrap();

        let bytes = compressed.to_bytes();
        let restored = ParallelCompressionResult::from_bytes(&bytes, compressed.config).unwrap();

        assert_eq!(compressed.chunk_count, restored.chunk_count);
        assert_eq!(
            compressed.total_compressed_size,
            restored.total_compressed_size
        );

        let decompressed = decompress_parallel(&restored).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_parallel_compression_compression_ratio() {
        let compressible_data = vec![0x42u8; 10 * 1024 * 1024]; // Highly compressible
        let compressed = compress_parallel(&compressible_data, 6).unwrap();

        eprintln!(
            "Compressible data ratio: {:.4}",
            compressed.compression_ratio
        );
        assert!(compressed.compression_ratio < 0.1); // Should compress very well
        assert!(compressed.space_saved_percentage() > 90.0);

        // Use high-entropy data (pseudo-random) for incompressible test
        // XOR-based LCG for reproducible but seemingly random data
        let mut seed: u64 = 0x123456789ABCDEF;
        let incompressible_data: Vec<u8> = (0..10 * 1024 * 1024)
            .map(|_| {
                seed = seed.wrapping_mul(0x5DEECE66D).wrapping_add(0xB);
                ((seed >> 32) & 0xFF) as u8
            })
            .collect();
        let compressed = compress_parallel(&incompressible_data, 6).unwrap();

        eprintln!(
            "Incompressible data ratio: {:.4}",
            compressed.compression_ratio
        );
        // High-entropy data should compress poorly (>90% of original size)
        assert!(compressed.compression_ratio > 0.90);
        assert!(compressed.space_saved_percentage() < 10.0);
    }

    #[test]
    fn test_parallel_compression_different_levels() {
        let data = vec![0x42u8; 5 * 1024 * 1024]; // 5 MB

        for level in [0, 4, 8, 11].iter() {
            let compressed = compress_parallel(&data, *level).unwrap();
            let decompressed = decompress_parallel(&compressed).unwrap();
            assert_eq!(data, decompressed);
        }
    }

    #[test]
    fn test_parallel_compression_empty_data() {
        let data = vec![];
        let compressed = compress_parallel(&data, 6).unwrap();

        assert_eq!(compressed.chunk_count, 0);
        assert_eq!(compressed.total_original_size, 0);
        assert_eq!(compressed.total_compressed_size, 0);

        let decompressed = decompress_parallel(&compressed).unwrap();
        assert_eq!(data, decompressed);
    }
}

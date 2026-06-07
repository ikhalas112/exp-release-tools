//! # Maxion Core Library
//!
//! Shared functionality between the packer tool and runtime stub.
//! Provides encryption, compression, archive format, and error handling.

pub mod access_control;
pub mod archive;
pub mod cache;
pub mod cheat_callback;
pub mod compression;
pub mod compression_parallel;
pub mod context;
pub mod crypto;
pub mod debug;
pub mod error;
pub mod io;
pub mod protected;
pub mod simd;
pub mod types;
pub mod virtual_archive;

// Re-exports for convenience
pub use access_control::{AccessControl, ANTI_SCRAPE_DELAY_MS, MAX_SEQUENTIAL_READS};
pub use archive::{ArchiveBuilder, ArchiveHeader};
pub use cache::LruCache;
pub use cheat_callback::{get_hardware_id, report_cheat_with_callback};
pub use cheat_callback::{CheatCallback, CheatEvent, CheatType};
pub use compression::{compress, decompress};
pub use compression_parallel::{
    compress_parallel, compress_parallel_with_config, decompress_parallel,
    ParallelCompressionConfig, ParallelCompressionResult,
};
pub use context::{ChunkCipherContext, EncryptionContext, FromConfig};
pub use crypto::ChunkCipher;
pub use debug::{ArchiveInspector, DebugLogger, LogLevel, MemoryTracker, PerformanceProfiler};
pub use error::{Error, Result};
pub use io::{get_optimal_buffer_size, read_file, read_file_range, read_zero_copy, write_file};
pub use protected::{is_trap_enabled, reset_trap_state, set_trap_enabled};
pub use protected::{CheatAction, CheatDetector, Protectable, Protected, ProtectedSync};
pub use simd::{detect_simd_level, SimdConfig, SimdLevel};

// Re-export procedural macros
pub use maxion_macros::auto_protected;
pub use types::{AssetFile, ChunkSize, Config, EncryptionKey, Nonce};
pub use virtual_archive::{AssetFileInfo, DefaultVirtualArchive, VirtualArchive};

/// Current version of the archive format
pub const ARCHIVE_VERSION: u32 = 1;

/// Default chunk size for encrypted data (64KB)
/// Balances memory usage, cache efficiency, and encryption overhead
pub fn default_chunk_size() -> ChunkSize {
    ChunkSize::new(64 * 1024)
}

/// Default configuration for the packer
pub fn default_config() -> Config {
    Config::new()
}

/// Magic number to identify Maxion archives
pub const MAGIC: &[u8; 8] = b"MAXION\x01\x00";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(ARCHIVE_VERSION, 1);
        assert_eq!(MAGIC, b"MAXION\x01\x00");
        assert_eq!(default_chunk_size().0, 64 * 1024);
    }
}

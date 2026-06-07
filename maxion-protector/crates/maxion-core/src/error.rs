//! Error types for Maxion Core
//!
//! Provides comprehensive error handling for all Maxion operations.

use std::io;
use thiserror::Error;

/// Result type alias for Maxion operations
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for Maxion operations
#[derive(Error, Debug)]
pub enum Error {
    /// Archive format errors
    #[error("Archive error: {0}")]
    Archive(#[from] ArchiveError),

    /// Cryptographic errors
    #[error("Crypto error: {0}")]
    Crypto(#[from] CryptoError),

    /// Compression errors
    #[error("Compression error: {0}")]
    Compression(#[from] CompressionError),

    /// PE file manipulation errors
    #[error("PE error: {0}")]
    Pe(#[from] PeError),

    /// File I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Generic errors with context
    #[error("{0}")]
    Other(String),

    /// Rate limiting errors
    #[error("Rate limit exceeded: {count} reads exceeds limit of {limit}")]
    RateLimitExceeded { count: u32, limit: u32 },
}

/// Archive-specific errors
#[derive(Error, Debug)]
pub enum ArchiveError {
    #[error("Invalid magic number: expected {expected:?}, found {found:?}")]
    InvalidMagic { expected: Vec<u8>, found: Vec<u8> },

    #[error("Unsupported archive version: {version}")]
    UnsupportedVersion { version: u32 },

    #[error("Corrupted archive: {reason}")]
    Corrupted { reason: String },

    #[error("File not found in archive: {path}")]
    FileNotFound { path: String },

    #[error("Invalid file table checksum")]
    InvalidChecksum,

    #[error("Chunk index out of bounds: {index} (total: {total})")]
    InvalidChunkIndex { index: u32, total: u32 },

    #[error("Invalid file offset: {offset} (file size: {size})")]
    InvalidOffset { offset: u64, size: u64 },
}

/// Cryptographic errors
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Invalid encryption key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    #[error("Invalid nonce length: expected {expected}, got {actual}")]
    InvalidNonceLength { expected: usize, actual: usize },

    #[error("Invalid nonce: {reason}")]
    InvalidNonce { reason: String },

    #[error("Authentication failed: data may be tampered")]
    AuthenticationFailed,

    #[error("Key derivation failed: {reason}")]
    KeyDerivationFailed { reason: String },

    #[error("Encryption error: {reason}")]
    EncryptionFailed { reason: String },

    #[error("Decryption error: {reason}")]
    DecryptionFailed { reason: String },
}

/// PE file manipulation errors
#[derive(Error, Debug)]
pub enum PeError {
    #[error("Invalid PE file: {reason}")]
    InvalidPe { reason: String },

    #[error("Section injection failed: {reason}")]
    SectionInjectionFailed { reason: String },

    #[error("Entry point modification failed: {reason}")]
    EntryPointModificationFailed { reason: String },

    #[error("Relocation processing failed: {reason}")]
    RelocationFailed { reason: String },

    #[error("Import table modification failed: {reason}")]
    ImportTableFailed { reason: String },

    #[error("Insufficient space in PE: needed {needed} bytes, available {available}")]
    InsufficientSpace { needed: u64, available: u64 },
}

/// Compression errors
#[derive(Error, Debug)]
pub enum CompressionError {
    #[error("Compression failed: {reason}")]
    CompressionFailed { reason: String },

    #[error("Decompression failed: {reason}")]
    DecompressionFailed { reason: String },

    #[error("Invalid compression level: {level} (must be 0-11)")]
    InvalidLevel { level: u32 },

    #[error("Compressed data is corrupted")]
    CorruptedData,

    #[error("Expected size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: usize, actual: usize },

    #[error("Buffer too small: needed {needed}, had {actual}")]
    BufferTooSmall { needed: usize, actual: usize },
}

/// Rate limiting errors
#[derive(Error, Debug)]
pub enum RateLimitError {
    #[error("Rate limit exceeded: {count} reads exceeds limit of {limit}")]
    Exceeded { count: u32, limit: u32 },

    #[error("Read timeout exceeded after {timeout_secs} seconds")]
    Timeout { timeout_secs: u64 },
}

impl From<orion::errors::UnknownCryptoError> for CryptoError {
    fn from(err: orion::errors::UnknownCryptoError) -> Self {
        CryptoError::EncryptionFailed {
            reason: err.to_string(),
        }
    }
}

// Note: brotli error handling is done inline where needed
// Note: orion errors are handled via UnknownCryptoError conversion above

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::Archive(ArchiveError::FileNotFound {
            path: "test.png".to_string(),
        });
        assert_eq!(
            err.to_string(),
            "Archive error: File not found in archive: test.png"
        );
    }

    #[test]
    fn test_error_chain() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn test_crypto_error() {
        let err = Error::Crypto(CryptoError::AuthenticationFailed);
        assert_eq!(
            err.to_string(),
            "Crypto error: Authentication failed: data may be tampered"
        );
    }

    #[test]
    fn test_result_type() {
        fn returns_result() -> Result<()> {
            Ok(())
        }
        assert!(returns_result().is_ok());
    }
}

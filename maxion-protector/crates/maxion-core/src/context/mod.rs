//! Encryption context module for stateful encryption operations
//!
//! This module provides context-aware encryption interfaces that allow
//! tracking encryption state, integrating with access control, and
//! providing efficient caching of cryptographic operations.

use crate::access_control::AccessControl;
use crate::crypto::ChunkCipher;
use crate::error::Result;
use crate::types::{ChunkSize, Nonce};
use std::sync::Arc;

/// Trait for context-aware encryption operations
///
/// This trait allows encryption implementations to maintain state
/// and integrate with access control mechanisms.
///
pub trait EncryptionContext: Send + Sync {
    /// Encrypt a chunk of data
    ///
    /// # Arguments
    ///
    /// * `plaintext` - The data to encrypt
    /// * `chunk_index` - Index of the chunk (for nonce derivation)
    ///
    /// # Returns
    ///
    /// Encrypted data with authentication tag
    fn encrypt_chunk(&self, plaintext: &[u8], chunk_index: u32) -> Result<Vec<u8>>;

    /// Decrypt a chunk of data
    ///
    /// # Arguments
    ///
    /// * `ciphertext` - The encrypted data with authentication tag
    /// * `chunk_index` - Index of the chunk (for nonce derivation)
    ///
    /// # Returns
    ///
    /// Decrypted plaintext
    fn decrypt_chunk(&self, ciphertext: &[u8], chunk_index: u32) -> Result<Vec<u8>>;

    /// Get the chunk size
    fn chunk_size(&self) -> ChunkSize;

    /// Get access control reference
    fn access_control(&self) -> &AccessControl;

    /// Get mutable access control reference
    fn access_control_mut(&mut self) -> &mut AccessControl;

    /// Check if a read operation is allowed
    fn check_access(&mut self) -> Result<()> {
        self.access_control_mut().check_rate_limit()
    }

    /// Record a successful read operation
    fn record_access(&mut self) {
        self.access_control_mut().record_read();
    }
}

/// Helper trait for creating encryption contexts from configuration
///
/// This allows `VirtualArchive` to be generic over different encryption
/// context implementations while still being constructible from a `Config`.
pub trait FromConfig: Sized {
    /// Create a new encryption context from configuration
    ///
    /// # Arguments
    ///
    /// * `key` - Encryption key
    /// * `nonce` - Base nonce for chunk derivation
    /// * `chunk_size` - Size of encryption chunks
    ///
    /// # Returns
    ///
    /// A new instance of the encryption context
    fn from_config_keys(key: &[u8; 32], nonce: &[u8; 24], chunk_size: ChunkSize) -> Self;
}

/// Context-aware chunk cipher implementation
///
/// Wraps the stateless `ChunkCipher` and adds:
/// - Access control integration
/// - State tracking for encryption operations
/// - Efficient caching support
pub struct ChunkCipherContext {
    /// The underlying chunk cipher
    cipher: Arc<ChunkCipher>,

    /// Access control for rate limiting
    access_control: AccessControl,

    /// Base nonce for deriving chunk-specific nonces
    base_nonce: [u8; 24],
}

impl ChunkCipherContext {
    /// Create a new chunk cipher context
    ///
    /// # Arguments
    ///
    /// * `cipher` - The underlying chunk cipher
    /// * `access_control` - Access control instance
    /// * `base_nonce` - Base nonce for chunk derivation
    pub fn new(
        cipher: Arc<ChunkCipher>,
        access_control: AccessControl,
        base_nonce: [u8; 24],
    ) -> Self {
        Self {
            cipher,
            access_control,
            base_nonce,
        }
    }

    /// Create a new chunk cipher context with default access control
    ///
    /// # Arguments
    ///
    /// * `key` - 32-byte encryption key
    /// * `nonce` - 24-byte base nonce
    /// * `chunk_size` - Size of each chunk
    pub fn from_keys(key: &[u8; 32], nonce: &[u8; 24], chunk_size: ChunkSize) -> Self {
        let cipher = Arc::new(ChunkCipher::new(key, nonce, chunk_size));
        let access_control = AccessControl::new();
        Self::new(cipher, access_control, *nonce)
    }

    /// Create a new chunk cipher context with custom access control limits
    ///
    /// # Arguments
    ///
    /// * `key` - 32-byte encryption key
    /// * `nonce` - 24-byte base nonce
    /// * `chunk_size` - Size of each chunk
    /// * `max_reads` - Maximum sequential reads allowed
    /// * `delay_ms` - Minimum delay between reads
    pub fn from_keys_with_limits(
        key: &[u8; 32],
        nonce: &[u8; 24],
        chunk_size: ChunkSize,
        max_reads: u32,
        delay_ms: u64,
    ) -> Self {
        let cipher = Arc::new(ChunkCipher::new(key, nonce, chunk_size));
        let access_control = AccessControl::with_limits(max_reads, delay_ms);
        Self::new(cipher, access_control, *nonce)
    }

    /// Get a reference to the underlying chunk cipher
    pub fn cipher(&self) -> &ChunkCipher {
        &self.cipher
    }

    /// Get an Arc reference to the underlying chunk cipher
    pub fn cipher_arc(&self) -> Arc<ChunkCipher> {
        Arc::clone(&self.cipher)
    }

    /// Get the base nonce
    pub fn base_nonce(&self) -> &[u8; 24] {
        &self.base_nonce
    }

    /// Derive a nonce for a specific chunk index
    pub fn derive_nonce(&self, chunk_index: u32) -> Nonce {
        Nonce::from_chunk_index(chunk_index, &self.base_nonce)
    }

    /// Encrypt a data range with access control
    ///
    /// This method checks access limits before encrypting.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to encrypt
    /// * `start_chunk` - Starting chunk index
    ///
    /// # Returns
    ///
    /// Vector of encrypted chunks
    pub fn encrypt_range_with_access(
        &mut self,
        data: &[u8],
        start_chunk: u32,
    ) -> Result<Vec<Vec<u8>>> {
        let mut encrypted_chunks = Vec::new();

        for (chunk_index, chunk) in data.chunks(self.chunk_size().as_usize()).enumerate() {
            self.check_access()?;
            let encrypted = self.encrypt_chunk(chunk, start_chunk + chunk_index as u32)?;
            encrypted_chunks.push(encrypted);
        }

        Ok(encrypted_chunks)
    }

    /// Decrypt a data range with access control
    ///
    /// This method checks access limits before decrypting.
    ///
    /// # Arguments
    ///
    /// * `encrypted_chunks` - Vector of encrypted chunks
    /// * `start_chunk` - Starting chunk index
    ///
    /// # Returns
    ///
    /// Reassembled decrypted data
    pub fn decrypt_range_with_access(
        &mut self,
        encrypted_chunks: &[Vec<u8>],
        start_chunk: u32,
    ) -> Result<Vec<u8>> {
        let mut decrypted_data = Vec::new();

        for (chunk_index, chunk) in encrypted_chunks.iter().enumerate() {
            self.check_access()?;
            let decrypted = self.decrypt_chunk(chunk, start_chunk + chunk_index as u32)?;
            decrypted_data.extend_from_slice(&decrypted);
        }

        Ok(decrypted_data)
    }

    /// Reset access control state
    pub fn reset_access_control(&mut self) {
        self.access_control.reset();
    }

    /// Get access control statistics
    pub fn access_stats(&self) -> (u32, Option<std::time::Duration>) {
        (
            self.access_control.read_count(),
            self.access_control.time_since_last_read(),
        )
    }

    /// Check if currently rate limited
    pub fn is_rate_limited(&self) -> bool {
        self.access_control.is_rate_limited()
    }
}

impl EncryptionContext for ChunkCipherContext {
    fn encrypt_chunk(&self, plaintext: &[u8], chunk_index: u32) -> Result<Vec<u8>> {
        let chunk_nonce = self.derive_nonce(chunk_index);
        self.cipher.encrypt_single(plaintext, &chunk_nonce)
    }

    fn decrypt_chunk(&self, ciphertext: &[u8], chunk_index: u32) -> Result<Vec<u8>> {
        let chunk_nonce = self.derive_nonce(chunk_index);
        self.cipher.decrypt_single(ciphertext, &chunk_nonce)
    }

    fn chunk_size(&self) -> ChunkSize {
        self.cipher.chunk_size()
    }

    fn access_control(&self) -> &AccessControl {
        &self.access_control
    }

    fn access_control_mut(&mut self) -> &mut AccessControl {
        &mut self.access_control
    }
}

impl FromConfig for ChunkCipherContext {
    fn from_config_keys(key: &[u8; 32], nonce: &[u8; 24], chunk_size: ChunkSize) -> Self {
        Self::from_keys(key, nonce, chunk_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EncryptionKey;
    use crate::Error;

    #[test]
    fn test_chunk_cipher_context_creation() {
        let key = EncryptionKey::generate();
        let nonce = Nonce::generate();
        let chunk_size = ChunkSize::new(65536);

        let context = ChunkCipherContext::from_keys(key.as_bytes(), nonce.as_bytes(), chunk_size);

        assert_eq!(context.chunk_size(), chunk_size);
        assert!(!context.is_rate_limited());
    }

    #[test]
    fn test_chunk_cipher_context_with_limits() {
        let key = EncryptionKey::generate();
        let nonce = Nonce::generate();
        let chunk_size = ChunkSize::new(65536);

        let context = ChunkCipherContext::from_keys_with_limits(
            key.as_bytes(),
            nonce.as_bytes(),
            chunk_size,
            5,
            100,
        );

        assert_eq!(context.access_control().max_reads(), 5);
        assert_eq!(context.access_control().delay_ms(), 100);
    }

    #[test]
    fn test_encrypt_decrypt_with_context() {
        let key = EncryptionKey::generate();
        let nonce = Nonce::generate();
        let chunk_size = ChunkSize::new(4096);

        let context = ChunkCipherContext::from_keys(key.as_bytes(), nonce.as_bytes(), chunk_size);

        let plaintext = b"Hello, World! This is a test.";
        let encrypted = context
            .encrypt_chunk(plaintext, 0)
            .expect("Encryption failed");
        let decrypted = context
            .decrypt_chunk(&encrypted, 0)
            .expect("Decryption failed");

        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_range_with_context() {
        let key = EncryptionKey::generate();
        let nonce = Nonce::generate();
        let chunk_size = ChunkSize::new(4096);

        let mut context =
            ChunkCipherContext::from_keys(key.as_bytes(), nonce.as_bytes(), chunk_size);

        // Create data larger than chunk size (minimum chunk size is 4096)
        let data = vec![0u8; 10000];
        println!(
            "Test data size: {}, chunk size: {}",
            data.len(),
            chunk_size.as_usize()
        );

        let encrypted_chunks = context
            .encrypt_range_with_access(&data, 0)
            .expect("Encryption failed");

        println!("Got {} encrypted chunks", encrypted_chunks.len());
        // Should create 3 chunks (10000 / 4096 = 2.44 -> 3)
        assert_eq!(encrypted_chunks.len(), 3);

        let decrypted = context
            .decrypt_range_with_access(&encrypted_chunks, 0)
            .expect("Decryption failed");

        assert_eq!(data, decrypted);
    }

    #[test]
    fn test_access_control_integration() {
        let key = EncryptionKey::generate();
        let nonce = Nonce::generate();
        let chunk_size = ChunkSize::new(1024);

        let mut context = ChunkCipherContext::from_keys_with_limits(
            key.as_bytes(),
            nonce.as_bytes(),
            chunk_size,
            3,
            50,
        );

        let _data = vec![0u8; 500];

        // First 3 accesses should succeed (check_access increments counter)
        for _ in 0..3 {
            context.check_access().expect("Access should be allowed");
        }

        assert!(context.is_rate_limited());

        // 4th access should fail
        let result = context.check_access();
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::RateLimitExceeded { .. })));
    }

    #[test]
    fn test_nonce_derivation() {
        let key = EncryptionKey::generate();
        let nonce_bytes = Nonce::generate();
        let chunk_size = ChunkSize::new(4096);

        let context =
            ChunkCipherContext::from_keys(key.as_bytes(), nonce_bytes.as_bytes(), chunk_size);

        let nonce0 = context.derive_nonce(0);
        let nonce1 = context.derive_nonce(1);

        assert_ne!(nonce0, nonce1);

        // First 4 bytes should contain chunk index (little-endian)
        assert_eq!(&nonce0.as_bytes()[..4], &0u32.to_le_bytes());
        assert_eq!(&nonce1.as_bytes()[..4], &1u32.to_le_bytes());
    }

    #[test]
    fn test_access_stats() {
        let key = EncryptionKey::generate();
        let nonce = Nonce::generate();
        let chunk_size = ChunkSize::new(4096);

        let mut context =
            ChunkCipherContext::from_keys(key.as_bytes(), nonce.as_bytes(), chunk_size);

        let _data = vec![0u8; 500];

        // Perform some operations (check_access increments counter)
        context.check_access().unwrap();
        context.check_access().unwrap();

        let (count, elapsed) = context.access_stats();
        assert_eq!(count, 2);
        assert!(elapsed.is_some());
        assert!(elapsed.unwrap().as_millis() < 100);
    }

    #[test]
    fn test_reset_access_control() {
        let key = EncryptionKey::generate();
        let nonce = Nonce::generate();
        let chunk_size = ChunkSize::new(4096);

        let mut context = ChunkCipherContext::from_keys_with_limits(
            key.as_bytes(),
            nonce.as_bytes(),
            chunk_size,
            2,
            50,
        );

        // Exceed rate limit (check_access increments counter)
        for _ in 0..3 {
            context.check_access().ok();
        }

        assert!(context.is_rate_limited());

        // Reset
        context.reset_access_control();

        assert!(!context.is_rate_limited());
        assert!(context.check_access().is_ok());
    }

    #[test]
    fn test_different_chunks_different_nonces() {
        let key = EncryptionKey::generate();
        let nonce_bytes = Nonce::generate();
        let chunk_size = ChunkSize::new(4096);

        let context =
            ChunkCipherContext::from_keys(key.as_bytes(), nonce_bytes.as_bytes(), chunk_size);

        let data = vec![42u8; 4096];

        let encrypted0 = context.encrypt_chunk(&data, 0).unwrap();
        let encrypted1 = context.encrypt_chunk(&data, 1).unwrap();

        // Same data but different chunks should produce different ciphertext
        assert_ne!(encrypted0, encrypted1);
    }
}

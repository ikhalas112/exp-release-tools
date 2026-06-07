//! Cryptographic operations for asset encryption
//!
//! This module provides XChaCha20-Poly1305 encryption using the `orion` crate.
//! All encryption is authenticated, meaning any tampering with the encrypted data
//! will be detected during decryption.
//!
//! # Design Philosophy
//!
//! - **No custom crypto**: Uses audited `orion` crate
//! - **Authenticated encryption**: XChaCha20-Poly1305 prevents tampering
//! - **Chunk-based**: Encrypts data in chunks for efficient streaming
//! - **Nonce-per-chunk**: Each chunk has a unique nonce to prevent nonce reuse
//!
//! # Performance
//!
//! XChaCha20-Poly1305 is extremely fast:
//! - ~2GB/s on modern CPUs with SIMD
//! - 256-bit nonce allows safe parallel chunk encryption
//! - Minimal memory allocation

use crate::error::{CryptoError, Error, Result};
use crate::types::{ChunkSize, Nonce};
use orion::hazardous::aead::xchacha20poly1305;
use std::io::{Read, Write};

/// Poly1305 authentication tag size
const POLY1305_TAG_SIZE: usize = 16;

/// XChaCha20 nonce size
const XCHACHA20_NONCESIZE: usize = 24;

/// Cipher for encrypting/decrypting data chunks
///
/// Uses XChaCha20-Poly1305 for authenticated encryption with streaming API
/// to support deterministic nonces per chunk.
///
/// # Thread Safety
///
/// `ChunkCipher` can be safely shared across threads for read-only access
/// to the secret key and nonce. Multiple threads can encrypt/decrypt concurrently.
pub struct ChunkCipher {
    /// The secret key
    secret_key: xchacha20poly1305::SecretKey,

    /// Base nonce for deriving chunk-specific nonces
    base_nonce: [u8; 24],

    /// Chunk size in bytes
    chunk_size: ChunkSize,
}

impl ChunkCipher {
    /// Create a new ChunkCipher
    ///
    /// # Arguments
    ///
    /// * `key` - 32-byte encryption key
    /// * `nonce` - 24-byte base nonce
    /// * `chunk_size` - Size of each chunk to encrypt
    pub fn new(key: &[u8; 32], nonce: &[u8; 24], chunk_size: ChunkSize) -> Self {
        Self {
            secret_key: xchacha20poly1305::SecretKey::from_slice(key)
                .expect("Key length validated elsewhere"),
            base_nonce: *nonce,
            chunk_size,
        }
    }

    /// Encrypt a single chunk of data
    ///
    /// # Arguments
    ///
    /// * `plaintext` - The data to encrypt (up to chunk_size)
    /// * `chunk_nonce` - Unique nonce for this chunk (24 bytes)
    ///
    /// # Returns
    ///
    /// Encrypted ciphertext with authentication tag appended
    pub fn encrypt_single(&self, plaintext: &[u8], chunk_nonce: &Nonce) -> Result<Vec<u8>> {
        if chunk_nonce.as_bytes().len() != XCHACHA20_NONCESIZE {
            return Err(Error::Crypto(CryptoError::InvalidNonceLength {
                expected: XCHACHA20_NONCESIZE,
                actual: chunk_nonce.as_bytes().len(),
            }));
        }

        if plaintext.is_empty() {
            return Err(Error::Crypto(CryptoError::EncryptionFailed {
                reason: "Plaintext cannot be empty".to_string(),
            }));
        }

        // Convert our nonce to orion's format
        let orion_nonce =
            xchacha20poly1305::Nonce::from_slice(chunk_nonce.as_bytes()).map_err(|e| {
                Error::Crypto(CryptoError::InvalidNonce {
                    reason: e.to_string(),
                })
            })?;

        // Allocate output buffer: ciphertext + tag
        let mut dst_out = vec![0u8; plaintext.len() + POLY1305_TAG_SIZE];

        // Encrypt using low-level API with our nonce
        xchacha20poly1305::seal(
            &self.secret_key,
            &orion_nonce,
            plaintext,
            None, // no additional data
            &mut dst_out,
        )
        .map_err(|e| {
            Error::Crypto(CryptoError::EncryptionFailed {
                reason: format!("Failed to encrypt chunk: {}", e),
            })
        })?;

        Ok(dst_out)
    }

    /// Decrypt a single chunk of data
    ///
    /// # Arguments
    ///
    /// * `ciphertext` - The encrypted data with authentication tag
    /// * `chunk_nonce` - Unique nonce for this chunk (24 bytes)
    ///
    /// # Returns
    ///
    /// Decrypted plaintext
    ///
    /// # Errors
    ///
    /// Returns `CryptoError::AuthenticationFailed` if the ciphertext has been tampered with
    pub fn decrypt_single(&self, ciphertext: &[u8], chunk_nonce: &Nonce) -> Result<Vec<u8>> {
        if ciphertext.len() <= POLY1305_TAG_SIZE {
            return Err(Error::Crypto(CryptoError::DecryptionFailed {
                reason: "Ciphertext too short (must contain authentication tag)".to_string(),
            }));
        }

        // Convert our nonce to orion's format
        let orion_nonce =
            xchacha20poly1305::Nonce::from_slice(chunk_nonce.as_bytes()).map_err(|e| {
                Error::Crypto(CryptoError::InvalidNonce {
                    reason: e.to_string(),
                })
            })?;

        // Allocate output buffer for plaintext
        let plaintext_len = ciphertext.len() - POLY1305_TAG_SIZE;
        let mut dst_out = vec![0u8; plaintext_len];

        // Decrypt using low-level API with our nonce
        xchacha20poly1305::open(
            &self.secret_key,
            &orion_nonce,
            ciphertext,
            None, // no additional data
            &mut dst_out,
        )
        .map_err(|e| {
            Error::Crypto(CryptoError::DecryptionFailed {
                reason: e.to_string(),
            })
        })?;

        Ok(dst_out)
    }

    /// Encrypt all data in chunks
    ///
    /// Splits the data into chunks and encrypts each chunk with a unique nonce.
    /// The chunk-specific nonce is derived from the chunk index and base nonce.
    ///
    /// # Arguments
    ///
    /// * `data` - The complete data to encrypt
    ///
    /// # Returns
    ///
    /// Vector of encrypted chunks
    ///
    /// # Example
    ///
    /// ```rust
    /// use maxion_core::crypto::ChunkCipher;
    /// use maxion_core::types::{ChunkSize, Nonce};
    ///
    /// let key = [0u8; 32];
    /// let nonce = [1u8; 24];
    /// let chunk_size = ChunkSize::new(4096);
    /// let cipher = ChunkCipher::new(&key, &nonce, chunk_size);
    ///
    /// let data = b"Hello, world!";
    /// let encrypted = cipher.encrypt_all(data).unwrap();
    /// ```
    pub fn encrypt_all(&self, data: &[u8]) -> Result<Vec<Vec<u8>>> {
        let mut encrypted_chunks = Vec::new();

        for (chunk_index, chunk) in data.chunks(self.chunk_size.as_usize()).enumerate() {
            let chunk_nonce = Nonce::from_chunk_index(chunk_index as u32, &self.base_nonce);
            let encrypted_chunk = self.encrypt_single(chunk, &chunk_nonce)?;
            encrypted_chunks.push(encrypted_chunk);
        }

        Ok(encrypted_chunks)
    }

    /// Decrypt all data from chunks
    ///
    /// Decrypts a vector of encrypted chunks using the chunk index to derive
    /// the appropriate nonce for each chunk.
    ///
    /// # Arguments
    ///
    /// * `encrypted_chunks` - Vector of encrypted chunks
    ///
    /// # Returns
    ///
    /// Reassembled decrypted data
    pub fn decrypt_all(&self, encrypted_chunks: &[Vec<u8>]) -> Result<Vec<u8>> {
        let mut decrypted_data = Vec::new();

        for (chunk_index, chunk) in encrypted_chunks.iter().enumerate() {
            let chunk_nonce = Nonce::from_chunk_index(chunk_index as u32, &self.base_nonce);
            let decrypted_chunk = self.decrypt_single(chunk, &chunk_nonce)?;
            decrypted_data.extend_from_slice(&decrypted_chunk);
        }

        Ok(decrypted_data)
    }

    /// Encrypt a stream of data
    ///
    /// Reads data from `reader` in chunks, encrypts each chunk, and writes
    /// the encrypted data to `writer`.
    ///
    /// # Arguments
    ///
    /// * `reader` - Input stream to read plaintext from
    /// * `writer` - Output stream to write encrypted data to
    ///
    /// # Returns
    ///
    /// Number of bytes encrypted
    pub fn encrypt_stream<R: Read, W: Write>(&self, reader: &mut R, writer: &mut W) -> Result<u64> {
        let mut buffer = vec![0u8; self.chunk_size.as_usize()];
        let mut total_bytes = 0u64;
        let mut chunk_index = 0u32;

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let chunk_nonce = Nonce::from_chunk_index(chunk_index, &self.base_nonce);
            let encrypted_chunk = self.encrypt_single(&buffer[..bytes_read], &chunk_nonce)?;

            // Write chunk size followed by encrypted data
            let chunk_size = encrypted_chunk.len() as u32;
            writer.write_all(&chunk_size.to_le_bytes())?;
            writer.write_all(&encrypted_chunk)?;

            total_bytes += bytes_read as u64;
            chunk_index += 1;
        }

        Ok(total_bytes)
    }

    /// Decrypt a stream of data
    ///
    /// Reads encrypted data from `reader` in chunks, decrypts each chunk,
    /// and writes the decrypted data to `writer`.
    ///
    /// # Arguments
    ///
    /// * `reader` - Input stream to read encrypted data from
    /// * `writer` - Output stream to write decrypted data to
    ///
    /// # Returns
    ///
    /// Number of bytes decrypted
    pub fn decrypt_stream<R: Read, W: Write>(&self, reader: &mut R, writer: &mut W) -> Result<u64> {
        let mut chunk_size_bytes = [0u8; 4];
        let mut total_bytes = 0u64;
        let mut chunk_index = 0u32;

        loop {
            // Read chunk size
            let bytes_read = reader.read_exact(&mut chunk_size_bytes);
            if bytes_read.is_err() {
                // End of stream
                break;
            }

            let chunk_size = u32::from_le_bytes(chunk_size_bytes) as usize;

            // Read encrypted chunk
            let mut encrypted_chunk = vec![0u8; chunk_size];
            reader.read_exact(&mut encrypted_chunk)?;

            // Decrypt
            let chunk_nonce = Nonce::from_chunk_index(chunk_index, &self.base_nonce);
            let decrypted_chunk = self.decrypt_single(&encrypted_chunk, &chunk_nonce)?;

            writer.write_all(&decrypted_chunk)?;

            total_bytes += decrypted_chunk.len() as u64;
            chunk_index += 1;
        }

        Ok(total_bytes)
    }

    /// Get the chunk size
    pub fn chunk_size(&self) -> ChunkSize {
        self.chunk_size
    }

    /// Get the base nonce for deriving chunk-specific nonces
    pub fn base_nonce(&self) -> [u8; 24] {
        self.base_nonce
    }
}

/// Cryptographic utilities
pub mod utils {
    use super::*;
    use orion::kdf;

    /// Generate a random encryption key
    ///
    /// # Returns
    ///
    /// 32-byte random key suitable for XChaCha20-Poly1305
    pub fn generate_key() -> [u8; 32] {
        use orion::hazardous::aead::xchacha20poly1305::SecretKey;
        let key = SecretKey::generate();
        let key_bytes = key.unprotected_as_bytes();
        let mut result = [0u8; 32];
        result.copy_from_slice(key_bytes);
        result
    }

    /// Derive a key from a password using Argon2id
    ///
    /// # Arguments
    ///
    /// * `password` - The password to derive from
    /// * `salt` - 16-byte salt for key derivation
    ///
    /// # Returns
    ///
    /// 32-byte derived key
    ///
    /// # Errors
    ///
    /// Returns error if password is empty or key derivation fails
    pub fn derive_key_from_password(password: &[u8], salt: &[u8; 16]) -> Result<[u8; 32]> {
        if password.is_empty() {
            return Err(Error::Crypto(CryptoError::EncryptionFailed {
                reason: "Password cannot be empty".to_string(),
            }));
        }

        let password_obj = kdf::Password::from_slice(password).map_err(|e| {
            Error::Crypto(CryptoError::EncryptionFailed {
                reason: format!("Invalid password: {:?}", e),
            })
        })?;

        let salt_obj = kdf::Salt::from_slice(salt).map_err(|e| {
            Error::Crypto(CryptoError::EncryptionFailed {
                reason: format!("Invalid salt: {:?}", e),
            })
        })?;

        let mut key = [0u8; 32];
        let expected_len = key.len() as u32;

        let derived_key = kdf::derive_key(
            &password_obj,
            &salt_obj,
            3,       // iterations
            1 << 16, // memory
            expected_len,
        )
        .map_err(|e| {
            Error::Crypto(CryptoError::EncryptionFailed {
                reason: format!("Key derivation failed: {:?}", e),
            })
        })?;

        // Extract bytes from SecretKey
        key.copy_from_slice(derived_key.unprotected_as_bytes());

        Ok(key)
    }

    /// Generate a random nonce
    ///
    /// # Returns
    ///
    /// 24-byte random nonce
    pub fn generate_nonce() -> [u8; 24] {
        use rand::RngCore;
        let mut nonce = [0u8; 24];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut nonce);
        nonce
    }

    /// Generate a random salt for key derivation
    ///
    /// # Returns
    ///
    /// 16-byte random salt
    pub fn generate_salt() -> [u8; 16] {
        use rand::RngCore;
        let mut salt = [0u8; 16];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut salt);
        salt
    }

    /// Compute BLAKE3 hash of data
    ///
    /// # Arguments
    ///
    /// * `data` - Data to hash
    ///
    /// # Returns
    ///
    /// 32-byte hash
    pub fn blake3_hash(data: &[u8]) -> blake3::Hash {
        blake3::hash(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA: &[u8] = b"This is test data that will be encrypted and then decrypted to verify crypto operations work correctly.
    Maxion Protector uses XChaCha20-Poly1305 authenticated encryption. This provides both confidentiality and integrity,
    meaning that any tampering with the encrypted data will be detected during decryption.

    The crypto module supports encryption in chunks for efficient processing of large files.
    Each chunk uses a unique nonce derived from the chunk index and a base nonce, preventing nonce reuse.";

    #[test]
    fn test_encrypt_decrypt_single() {
        let key = utils::generate_key();
        let nonce = utils::generate_nonce();
        let chunk_size = ChunkSize::new(4096);
        let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

        let plaintext = b"Hello, world!";
        let encrypted = cipher
            .encrypt_single(plaintext, &Nonce::from_chunk_index(0, &nonce))
            .unwrap();

        assert_ne!(encrypted, plaintext.as_ref());
        assert!(encrypted.len() > plaintext.len()); // Tag adds overhead

        let decrypted = cipher
            .decrypt_single(&encrypted, &Nonce::from_chunk_index(0, &nonce))
            .unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_all() {
        let key = utils::generate_key();
        let nonce = utils::generate_nonce();
        let chunk_size = ChunkSize::new(128);
        let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

        let encrypted = cipher.encrypt_all(TEST_DATA).unwrap();
        let decrypted = cipher.decrypt_all(&encrypted).unwrap();

        assert_eq!(decrypted, TEST_DATA);
    }

    #[test]
    fn test_different_keys_produce_different_ciphertext() {
        let key1 = utils::generate_key();
        let mut key2 = key1;
        key2[0] ^= 0xFF; // Change one byte

        let nonce = utils::generate_nonce();
        let chunk_size = ChunkSize::new(4096);
        let cipher1 = ChunkCipher::new(&key1, &nonce, chunk_size);
        let cipher2 = ChunkCipher::new(&key2, &nonce, chunk_size);

        let plaintext = b"Same plaintext";
        let chunk_nonce = Nonce::from_chunk_index(0, &nonce);

        let encrypted1 = cipher1.encrypt_single(plaintext, &chunk_nonce).unwrap();
        let encrypted2 = cipher2.encrypt_single(plaintext, &chunk_nonce).unwrap();

        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn test_different_nonces_produce_different_ciphertext() {
        let key = utils::generate_key();
        let nonce1 = utils::generate_nonce();
        let mut nonce2 = nonce1;
        nonce2[0] ^= 0xFF; // Change one byte

        let chunk_size = ChunkSize::new(4096);
        let cipher1 = ChunkCipher::new(&key, &nonce1, chunk_size);
        let cipher2 = ChunkCipher::new(&key, &nonce2, chunk_size);

        let plaintext = b"Same plaintext";
        let chunk_nonce1 = Nonce::from_chunk_index(0, &nonce1);
        let chunk_nonce2 = Nonce::from_chunk_index(0, &nonce2);

        let encrypted1 = cipher1.encrypt_single(plaintext, &chunk_nonce1).unwrap();
        let encrypted2 = cipher2.encrypt_single(plaintext, &chunk_nonce2).unwrap();

        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn test_authentication_fails_on_tampering() {
        let key = utils::generate_key();
        let nonce = utils::generate_nonce();
        let chunk_size = ChunkSize::new(4096);
        let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

        let plaintext = b"Important data";
        let chunk_nonce = Nonce::from_chunk_index(0, &nonce);
        let mut encrypted = cipher.encrypt_single(plaintext, &chunk_nonce).unwrap();

        // Tamper with the encrypted data
        encrypted[0] ^= 0xFF;

        let result = cipher.decrypt_single(&encrypted, &chunk_nonce);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = utils::generate_key();
        let mut key2 = key1;
        key2[0] ^= 0xFF;

        let nonce = utils::generate_nonce();
        let chunk_size = ChunkSize::new(4096);
        let cipher1 = ChunkCipher::new(&key1, &nonce, chunk_size);

        let plaintext = b"Secret message";
        let chunk_nonce = Nonce::from_chunk_index(0, &nonce);
        let encrypted = cipher1.encrypt_single(plaintext, &chunk_nonce).unwrap();

        let cipher2 = ChunkCipher::new(&key2, &nonce, chunk_size);
        let result = cipher2.decrypt_single(&encrypted, &chunk_nonce);
        assert!(result.is_err());
    }

    #[test]
    fn test_large_data_encryption() {
        let key = utils::generate_key();
        let nonce = utils::generate_nonce();
        let chunk_size = ChunkSize::new(1024);
        let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

        // 10 KB of data
        let large_data = vec![0xABu8; 10240];

        let encrypted = cipher.encrypt_all(&large_data).unwrap();
        let decrypted = cipher.decrypt_all(&encrypted).unwrap();

        assert_eq!(decrypted, large_data);
    }

    #[test]
    fn test_stream_encryption() {
        let key = utils::generate_key();
        let nonce = utils::generate_nonce();
        let chunk_size = ChunkSize::new(512);
        let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

        let mut reader = std::io::Cursor::new(TEST_DATA);
        let mut encrypted = Vec::new();

        let bytes_written = cipher.encrypt_stream(&mut reader, &mut encrypted).unwrap();
        assert_eq!(bytes_written, TEST_DATA.len() as u64);

        let mut reader = std::io::Cursor::new(&encrypted[..]);
        let mut decrypted = Vec::new();

        let bytes_read = cipher.decrypt_stream(&mut reader, &mut decrypted).unwrap();
        assert_eq!(bytes_read, TEST_DATA.len() as u64);

        assert_eq!(decrypted, TEST_DATA);
    }

    #[test]
    fn test_key_derivation() {
        let password = b"my_secure_password";
        let salt = utils::generate_salt();

        let key1 = utils::derive_key_from_password(password, &salt).unwrap();
        let key2 = utils::derive_key_from_password(password, &salt).unwrap();

        // Same password and salt should produce same key
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_key_derivation_different_salts() {
        let password = b"my_secure_password";
        let salt1 = utils::generate_salt();
        let salt2 = utils::generate_salt();

        let key1 = utils::derive_key_from_password(password, &salt1).unwrap();
        let key2 = utils::derive_key_from_password(password, &salt2).unwrap();

        // Same password but different salts should produce different keys
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_chunk_size_property() {
        let key = utils::generate_key();
        let nonce = utils::generate_nonce();
        let chunk_size = ChunkSize::new(4096);
        let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

        assert_eq!(cipher.chunk_size(), chunk_size);
    }

    #[test]
    fn test_nonce_uniqueness() {
        let nonce = [1u8; 24];

        let nonce1 = Nonce::from_chunk_index(0, &nonce);
        let nonce2 = Nonce::from_chunk_index(1, &nonce);
        let nonce3 = Nonce::from_chunk_index(0, &nonce);

        // Different indices should produce different nonces
        assert_ne!(nonce1, nonce2);

        // Same index should produce same nonce (deterministic)
        assert_eq!(nonce1, nonce3);
    }

    #[test]
    fn test_chunk_nonce_from_index() {
        let base_nonce = [1u8; 24];
        let nonce1 = Nonce::from_chunk_index(0, &base_nonce);
        let nonce2 = Nonce::from_chunk_index(1, &base_nonce);

        assert_ne!(nonce1, nonce2);
        assert_eq!(nonce1.as_bytes()[..4], 0u32.to_le_bytes());
        assert_eq!(nonce2.as_bytes()[..4], 1u32.to_le_bytes());
    }
}

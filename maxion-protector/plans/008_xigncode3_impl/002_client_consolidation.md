# 002: Client Communication Consolidation Specification

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-27 |
| Version | 1.0 |
| Complexity | Intermediate |
| Time to Read | 15 minutes |
| Audience | Developers, Unity Integrators |

## Overview

This document defines the `maxion-anticheat-client` crate, which consolidates all client-side communication logic that was previously split across phases 008a (client foundation) and 008b (client-server communication). This provides a single, unified interface for Unity integration and server communication.

## Architecture Goals

1. **Simplicity**: Single crate for all client communication needs
2. **Unity Integration**: Clean FFI interface for Unity C# bindings
3. **Reliability**: Robust error handling, retry logic, offline support
4. **Performance**: Async/await, connection pooling, efficient serialization
5. **SOLID Compliance**: Single responsibility for client communication

## Crate Structure

```
maxion-anticheat-client/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Main export module + FFI exports
│   ├── mod.rs                    # Module index
│   ├── client.rs                 # Main client struct
│   ├── crypto/
│   │   ├── mod.rs                # Crypto module index
│   │   ├── key_manager.rs        # Ed25519 key management
│   │   ├── signer.rs             # Payload signing
│   │   └── verifier.rs           # Signature verification
│   ├── token.rs                  # Action token management
│   ├── registration.rs           # Player registration
│   ├── telemetry.rs              # Event submission
│   ├── patterns.rs               # Pattern fetching
│   ├── bans.rs                   # Ban status checking
│   ├── retry.rs                  # Retry logic with exponential backoff
│   └── errors.rs                 # Client-specific errors
├── include/
│   └── maxion_anticheat.h        # C header for Unity
└── tests/
    ├── unit.rs
    ├── integration.rs
    └── mock_server.rs
```

## Cargo.toml

```toml
[package]
name = "maxion-anticheat-client"
version = "0.1.0"
edition = "2021"
authors = ["Maxion Team"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/maxion-game/maxion-protector"
description = "Unified client communication for Maxion anti-cheat detection"

[lib]
name = "maxion_anticheat_client"
crate-type = ["cdylib", "staticlib", "rlib"]

[dependencies]
# Shared types
maxion-detection-types = { path = "../maxion-detection-types" }

# Workspace dependencies
serde = { workspace = true }
serde_json = { workspace = true }
bincode = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
blake3 = { workspace = true }
ed25519-dalek = "2.1"
chrono = { workspace = true }
uuid = { workspace = true }
hex = { workspace = true }

# Async runtime
tokio = { version = "1.35", features = ["rt-multi-thread", "macros", "time"] }

# HTTP client
reqwest = { version = "0.11", features = ["json", "rustls-tls"], default-features = false }

# C FFI
libc = "0.2"
cstr = "0.2"

[dev-dependencies]
tokio-test = "0.4"
mockito = "1.2"

[features]
default = []
testing = []
```

## Main Client Structure (src/client.rs)

```rust
//! Main client for anti-cheat communication

use reqwest::Client as HttpClient;
use std::sync::Arc;
use tokio::sync::RwLock;
use maxion_detection_types::{
    Result, CheatEvent, PlayerState, BanRecord, SecurityPattern,
    DetectionError,
};

/// Unified anti-cheat client
pub struct AnticheatClient {
    server_url: String,
    http_client: HttpClient,
    key_manager: Arc<RwLock<crate::crypto::key_manager::KeyManager>>,
    config: ClientConfig,
}

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub enable_offline_mode: bool,
    pub offline_buffer_size: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 5,
            max_retries: 3,
            retry_delay_ms: 100,
            enable_offline_mode: true,
            offline_buffer_size: 1000,
        }
    }
}

impl AnticheatClient {
    /// Create new anticheat client
    pub fn new(server_url: String, config: ClientConfig) -> Self {
        let http_client = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .unwrap();

        let key_manager = Arc::new(RwLock::new(
            crate::crypto::key_manager::KeyManager::new()
        ));

        Self {
            server_url,
            http_client,
            key_manager,
            config,
        }
    }

    /// Initialize client (load or generate keys)
    pub async fn initialize(&self) -> Result<String> {
        let mut manager = self.key_manager.write().await;
        manager.load_or_generate()?;
        Ok(manager.get_player_id())
    }

    /// Get player ID
    pub async fn get_player_id(&self) -> Result<String> {
        let manager = self.key_manager.read().await;
        Ok(manager.get_player_id())
    }

    /// Register player with server
    pub async fn register_player(
        &self,
        hwid: String,
    ) -> Result<maxion_detection_types::RegistrationResponse> {
        let manager = self.key_manager.read().await;
        let player_id = manager.get_player_id();
        let public_key = manager.get_public_key()?;

        let request = maxion_detection_types::RegistrationRequest {
            player_id: player_id.clone(),
            public_key,
            hwid,
        };

        let url = format!("{}/register", self.server_url);
        let response = self.send_request(&url, &request).await?;
        
        Ok(response)
    }

    /// Submit cheat event to server
    pub async fn submit_cheat_event(&self, event: &CheatEvent) -> Result<()> {
        let manager = self.key_manager.read().await;
        let signed_request = manager.sign_event(event)?;

        let url = format!("{}/cheat", self.server_url);
        self.send_request(&url, &signed_request).await?;
        
        Ok(())
    }

    /// Check player ban status
    pub async fn check_ban_status(&self, player_id: &str) -> Result<Option<BanRecord>> {
        let url = format!("{}/status/{}", self.server_url, player_id);
        let response: Option<BanRecord> = self.get_request(&url).await?;
        Ok(response)
    }

    /// Fetch active security patterns
    pub async fn fetch_patterns(&self) -> Result<Vec<SecurityPattern>> {
        let url = format!("{}/patterns", self.server_url);
        let response: Vec<SecurityPattern> = self.get_request(&url).await?;
        Ok(response)
    }

    /// Internal: Send POST request with retry logic
    async fn send_request<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<R> {
        let mut attempt = 0;
        let max_attempts = self.config.max_retries as usize;
        let delay = std::time::Duration::from_millis(self.config.retry_delay_ms);

        loop {
            attempt += 1;

            let response = self.http_client
                .post(url)
                .json(body)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let result: R = resp.json().await.map_err(|e| {
                            DetectionError::SerializationError(e.to_string())
                        })?;
                        return Ok(result);
                    } else {
                        let status = resp.status();
                        if attempt < max_attempts {
                            tokio::time::sleep(delay * attempt as u32).await;
                            continue;
                        } else {
                            return Err(DetectionError::NetworkError(
                                format!("Request failed with status: {}", status)
                            ));
                        }
                    }
                }
                Err(e) => {
                    if attempt < max_attempts {
                        tokio::time::sleep(delay * attempt as u32).await;
                        continue;
                    } else {
                        return Err(DetectionError::NetworkError(e.to_string()));
                    }
                }
            }
        }
    }

    /// Internal: Send GET request with retry logic
    async fn get_request<R: serde::de::DeserializeOwned>(&self, url: &str) -> Result<R> {
        let mut attempt = 0;
        let max_attempts = self.config.max_retries as usize;
        let delay = std::time::Duration::from_millis(self.config.retry_delay_ms);

        loop {
            attempt += 1;

            let response = self.http_client
                .get(url)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let result: R = resp.json().await.map_err(|e| {
                            DetectionError::SerializationError(e.to_string())
                        })?;
                        return Ok(result);
                    } else if resp.status().as_u16() == 404 {
                        return Ok(serde_json::from_str("null").unwrap());
                    } else {
                        let status = resp.status();
                        if attempt < max_attempts {
                            tokio::time::sleep(delay * attempt as u32).await;
                            continue;
                        } else {
                            return Err(DetectionError::NetworkError(
                                format!("Request failed with status: {}", status)
                            ));
                        }
                    }
                }
                Err(e) => {
                    if attempt < max_attempts {
                        tokio::time::sleep(delay * attempt as u32).await;
                        continue;
                    } else {
                        return Err(DetectionError::NetworkError(e.to_string()));
                    }
                }
            }
        }
    }
}
```

## Key Management (src/crypto/key_manager.rs)

```rust
//! Ed25519 key pair management

use std::path::PathBuf;
use std::fs;
use ed25519_dalek::{SigningKey, VerifyingKey, SecretKey};
use maxion_detection_types::{Result, DetectionError};
use blake3::Hash;

pub struct KeyManager {
    keypair_path: PathBuf,
    keypair: Option<ClientKeypair>,
}

#[derive(Debug, Clone)]
pub struct ClientKeypair {
    pub public_key: VerifyingKey,
    pub private_key: SecretKey,
}

impl KeyManager {
    pub fn new() -> Self {
        let keypair_path = dirs::home_dir()
            .unwrap()
            .join(".maxion")
            .join("keypair.bin");

        Self {
            keypair_path,
            keypair: None,
        }
    }

    /// Load existing keypair or generate new one
    pub fn load_or_generate(&mut self) -> Result<()> {
        if self.keypair_path.exists() {
            self.load_keypair()
        } else {
            self.generate_keypair()
        }
    }

    /// Generate new Ed25519 key pair
    pub fn generate_keypair(&mut self) -> Result<()> {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let verifying_key = signing_key.verifying_key();

        let keypair = ClientKeypair {
            public_key: verifying_key,
            private_key: signing_key,
        };

        self.save_keypair(&keypair)?;
        self.keypair = Some(keypair);

        Ok(())
    }

    /// Load existing key pair from disk
    pub fn load_keypair(&mut self) -> Result<()> {
        let bytes = fs::read(&self.keypair_path).map_err(|e| {
            DetectionError::StorageError(format!("Failed to read keypair: {}", e))
        })?;

        // Deserialize: [32 bytes private] + [32 bytes public]
        if bytes.len() != 64 {
            return Err(DetectionError::InvalidRequest(
                "Invalid keypair length".to_string()
            ));
        }

        let private_key = SecretKey::from_bytes(&bytes[0..32]).map_err(|_| {
            DetectionError::InvalidRequest("Invalid private key".to_string())
        })?;

        let public_key = VerifyingKey::from_bytes(&bytes[32..64]).map_err(|_| {
            DetectionError::InvalidRequest("Invalid public key".to_string())
        })?;

        self.keypair = Some(ClientKeypair {
            public_key,
            private_key,
        });

        Ok(())
    }

    /// Save key pair to disk
    fn save_keypair(&self, keypair: &ClientKeypair) -> Result<()> {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&keypair.private_key.to_bytes());
        bytes.extend_from_slice(&keypair.public_key.to_bytes());

        fs::create_dir_all(self.keypair_path.parent().unwrap()).map_err(|e| {
            DetectionError::StorageError(format!("Failed to create directory: {}", e))
        })?;

        fs::write(&self.keypair_path, bytes).map_err(|e| {
            DetectionError::StorageError(format!("Failed to save keypair: {}", e))
        })?;

        Ok(())
    }

    /// Get player ID derived from public key
    pub fn get_player_id(&self) -> String {
        match &self.keypair {
            Some(keypair) => {
                let hash = blake3::hash(keypair.public_key.as_bytes());
                hex::encode(hash.as_bytes())
            }
            None => String::new(),
        }
    }

    /// Get public key bytes
    pub fn get_public_key(&self) -> Result<Vec<u8>> {
        match &self.keypair {
            Some(keypair) => Ok(keypair.public_key.as_bytes().to_vec()),
            None => Err(DetectionError::InvalidRequest(
                "Keypair not initialized".to_string()
            )),
        }
    }

    /// Sign payload
    pub fn sign_payload(&self, payload: &[u8]) -> Result<Vec<u8>> {
        match &self.keypair {
            Some(keypair) => {
                let signature = keypair.private_key.sign(payload);
                Ok(signature.to_bytes().to_vec())
            }
            None => Err(DetectionError::InvalidRequest(
                "Keypair not initialized".to_string()
            )),
        }
    }

    /// Sign cheat event
    pub fn sign_event(&self, event: &CheatEvent) -> Result<maxion_detection_types::SignedRequest> {
        let payload_bytes = bincode::serialize(event).map_err(|e| {
            DetectionError::SerializationError(e.to_string())
        })?;

        let signature = self.sign_payload(&payload_bytes)?;

        Ok(maxion_detection_types::SignedRequest {
            payload: maxion_detection_types::SignedPayload {
                action_data: serde_json::to_value(event).unwrap(),
                action_token: maxion_detection_types::ActionToken {
                    player_id: self.get_player_id(),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    nonce: uuid::Uuid::now_v7().to_string(),
                    token_hash: vec![],
                    expires_at: 0,
                },
            },
            signature,
        })
    }
}
```

## FFI Exports (src/lib.rs)

```rust
//! Maxion Anti-Cheat Client Library
//!
//! Provides unified client communication for anti-cheat detection
//! with FFI exports for Unity integration.

mod client;
mod crypto;
mod token;
mod registration;
mod telemetry;
mod patterns;
mod bans;
mod errors;

pub use client::{AnticheatClient, ClientConfig};
pub use errors::{ClientError, Result as ClientResult};

use std::ffi::{CStr, CString, c_char};
use std::ptr;
use std::sync::Arc;
use tokio::runtime::Runtime;

// Global runtime and client
static mut RUNTIME: Option<Runtime> = None;
static mut CLIENT: Option<Arc<AnticheatClient>> = None;

/// Initialize the anticheat client
///
/// # Safety
/// This function should only be called once from Unity
#[no_mangle]
pub unsafe extern "C" fn maxion_anticheat_init(
    server_url: *const c_char,
) -> *mut c_char {
    let url = match CStr::from_ptr(server_url).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return CString::new("Invalid server URL".to_string()).unwrap().into_raw(),
    };

    // Create runtime if not exists
    if RUNTIME.is_none() {
        RUNTIME = Some(Runtime::new().unwrap());
    }

    let runtime = RUNTIME.as_ref().unwrap();

    // Create client
    let client = Arc::new(AnticheatClient::new(url, ClientConfig::default()));
    CLIENT = Some(client.clone());

    // Initialize (load/generate keys)
    let result = runtime.block_on(async move {
        client.initialize().await
    });

    match result {
        Ok(player_id) => {
            CString::new(player_id).unwrap().into_raw()
        }
        Err(e) => {
            CString::new(format!("Init failed: {}", e)).unwrap().into_raw()
        }
    }
}

/// Get player ID
#[no_mangle]
pub unsafe extern "C" fn maxion_get_player_id() -> *mut c_char {
    if CLIENT.is_none() {
        return CString::new("Client not initialized".to_string()).unwrap().into_raw();
    }

    let client = CLIENT.as_ref().unwrap();
    let runtime = RUNTIME.as_ref().unwrap();

    let result = runtime.block_on(async move {
        client.get_player_id().await
    });

    match result {
        Ok(player_id) => {
            CString::new(player_id).unwrap().into_raw()
        }
        Err(e) => {
            CString::new(format!("Get player ID failed: {}", e)).unwrap().into_raw()
        }
    }
}

/// Submit cheat event
///
/// # Arguments
/// * `player_id` - Player UUID
/// * `cheat_type` - Type of cheat detected (as string)
/// * `hwid` - Hardware ID
/// * `detection_count` - Number of times detected
#[no_mangle]
pub unsafe extern "C" fn maxion_submit_cheat_event(
    player_id: *const c_char,
    cheat_type: *const c_char,
    hwid: *const c_char,
    detection_count: u32,
) -> *mut c_char {
    if CLIENT.is_none() {
        return CString::new("Client not initialized".to_string()).unwrap().into_raw();
    }

    let pid = match CStr::from_ptr(player_id).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return CString::new("Invalid player ID".to_string()).unwrap().into_raw(),
    };

    let c_type = match CStr::from_ptr(cheat_type).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return CString::new("Invalid cheat type".to_string()).unwrap().into_raw(),
    };

    let hardware_id = match CStr::from_ptr(hwid).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return CString::new("Invalid HWID".to_string()).unwrap().into_raw(),
    };

    // Parse cheat type string
    let cheat_type_enum = match c_type.parse::<maxion_detection_types::CheatType>() {
        Ok(t) => t,
        Err(_) => return CString::new("Unknown cheat type".to_string()).unwrap().into_raw(),
    };

    let event = maxion_detection_types::CheatEvent {
        game_id: "unknown".to_string(), // TODO: Pass from Unity
        version: "1.0.0".to_string(), // TODO: Pass from Unity
        player_id: pid,
        cheat_type: cheat_type_enum,
        hwid: hardware_id,
        timestamp: chrono::Utc::now(),
        detection_count,
    };

    let client = CLIENT.as_ref().unwrap();
    let runtime = RUNTIME.as_ref().unwrap();

    let result = runtime.block_on(async move {
        client.submit_cheat_event(&event).await
    });

    match result {
        Ok(()) => {
            CString::new("OK".to_string()).unwrap().into_raw()
        }
        Err(e) => {
            CString::new(format!("Submit failed: {}", e)).unwrap().into_raw()
        }
    }
}

/// Check ban status
#[no_mangle]
pub unsafe extern "C" fn maxion_check_ban_status(
    player_id: *const c_char,
) -> *mut c_char {
    if CLIENT.is_none() {
        return CString::new("Client not initialized".to_string()).unwrap().into_raw();
    }

    let pid = match CStr::from_ptr(player_id).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return CString::new("Invalid player ID".to_string()).unwrap().into_raw(),
    };

    let client = CLIENT.as_ref().unwrap();
    let runtime = RUNTIME.as_ref().unwrap();

    let result = runtime.block_on(async move {
        client.check_ban_status(&pid).await
    });

    match result {
        Ok(Some(ban)) => {
            let json = serde_json::to_string(&ban).unwrap();
            CString::new(json).unwrap().into_raw()
        }
        Ok(None) => {
            CString::new("null".to_string()).unwrap().into_raw()
        }
        Err(e) => {
            CString::new(format!("Check ban failed: {}", e)).unwrap().into_raw()
        }
    }
}

/// Free string allocated by Rust
#[no_mangle]
pub unsafe extern "C" fn maxion_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = ClientConfig::default();
        let client = AnticheatClient::new("http://localhost:8080".to_string(), config);
        assert_eq!(client.server_url, "http://localhost:8080");
    }
}
```

## C Header (include/maxion_anticheat.h)

```c
#ifndef MAXION_ANTICHEAT_H
#define MAXION_ANTICHEAT_H

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Initialize the anticheat client
 * 
 * @param server_url URL of the anticheat server
 * @return Player ID on success, error message on failure
 */
char* maxion_anticheat_init(const char* server_url);

/**
 * Get the current player ID
 * 
 * @return Player ID on success, error message on failure
 */
char* maxion_get_player_id();

/**
 * Submit a cheat event to the server
 * 
 * @param player_id Player UUID
 * @param cheat_type Type of cheat detected (e.g., "ProcessInjection")
 * @param hwid Hardware ID
 * @param detection_count Number of detections
 * @return "OK" on success, error message on failure
 */
char* maxion_submit_cheat_event(
    const char* player_id,
    const char* cheat_type,
    const char* hwid,
    uint32_t detection_count
);

/**
 * Check if player is banned
 * 
 * @param player_id Player UUID
 * @return JSON ban record on success, "null" if not banned, error message on failure
 */
char* maxion_check_ban_status(const char* player_id);

/**
 * Free string allocated by Rust functions
 * 
 * @param ptr Pointer to string to free
 */
void maxion_free_string(char* ptr);

#ifdef __cplusplus
}
#endif

#endif // MAXION_ANTICHEAT_H
```

## Usage Examples

### Example 1: Basic Usage (Rust)

```rust
use maxion_anticheat_client::{AnticheatClient, ClientConfig};
use maxion_detection_types::{CheatEvent, CheatType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ClientConfig::default();
    let client = AnticheatClient::new("https://anticheat.example.com".to_string(), config);

    // Initialize (load or generate keys)
    let player_id = client.initialize().await?;
    println!("Player ID: {}", player_id);

    // Register player
    let hwid = "hwid-123456".to_string();
    let response = client.register_player(hwid).await?;
    println!("Registered: {}", response.player_id);

    // Submit cheat event
    let event = CheatEvent {
        game_id: "my-game".to_string(),
        version: "1.0.0".to_string(),
        player_id: player_id.clone(),
        cheat_type: CheatType::ProcessInjection,
        hwid: "hwid-123456".to_string(),
        timestamp: chrono::Utc::now(),
        detection_count: 1,
    };
    client.submit_cheat_event(&event).await?;

    // Check ban status
    if let Some(ban) = client.check_ban_status(&player_id).await? {
        println!("Player is banned: {:?}", ban);
    } else {
        println!("Player is not banned");
    }

    Ok(())
}
```

### Example 2: Unity Integration (C#)

```csharp
using System;
using System.Runtime.InteropServices;

public class MaxionAnticheat
{
    // Import functions from native library
    [DllImport("maxion_anticheat_client")]
    private static extern IntPtr maxion_anticheat_init(IntPtr serverUrl);

    [DllImport("maxion_anticheat_client")]
    private static extern IntPtr maxion_get_player_id();

    [DllImport("maxion_anticheat_client")]
    private static extern IntPtr maxion_submit_cheat_event(
        IntPtr playerId,
        IntPtr cheatType,
        IntPtr hwid,
        uint detectionCount
    );

    [DllImport("maxion_anticheat_client")]
    private static extern IntPtr maxion_check_ban_status(IntPtr playerId);

    [DllImport("maxion_anticheat_client")]
    private static extern void maxion_free_string(IntPtr ptr);

    // Helper to convert IntPtr to string and free
    private static string GetStringAndFree(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
            return null;
        
        string result = Marshal.PtrToStringAnsi(ptr);
        maxion_free_string(ptr);
        return result;
    }

    // Initialize client
    public static string Initialize(string serverUrl)
    {
        IntPtr urlPtr = Marshal.StringToHGlobalAnsi(serverUrl);
        IntPtr resultPtr = maxion_anticheat_init(urlPtr);
        Marshal.FreeHGlobal(urlPtr);
        return GetStringAndFree(resultPtr);
    }

    // Get player ID
    public static string GetPlayerId()
    {
        IntPtr resultPtr = maxion_get_player_id();
        return GetStringAndFree(resultPtr);
    }

    // Submit cheat event
    public static string SubmitCheatEvent(
        string playerId,
        string cheatType,
        string hwid,
        uint detectionCount)
    {
        IntPtr pidPtr = Marshal.StringToHGlobalAnsi(playerId);
        IntPtr typePtr = Marshal.StringToHGlobalAnsi(cheatType);
        IntPtr hwidPtr = Marshal.StringToHGlobalAnsi(hwid);
        
        IntPtr resultPtr = maxion_submit_cheat_event(pidPtr, typePtr, hwidPtr, detectionCount);
        
        Marshal.FreeHGlobal(pidPtr);
        Marshal.FreeHGlobal(typePtr);
        Marshal.FreeHGlobal(hwidPtr);
        
        return GetStringAndFree(resultPtr);
    }

    // Check ban status
    public static string CheckBanStatus(string playerId)
    {
        IntPtr pidPtr = Marshal.StringToHGlobalAnsi(playerId);
        IntPtr resultPtr = maxion_check_ban_status(pidPtr);
        Marshal.FreeHGlobal(pidPtr);
        return GetStringAndFree(resultPtr);
    }
}

// Usage in Unity
public class AnticheatManager : MonoBehaviour
{
    void Start()
    {
        // Initialize anticheat
        string result = MaxionAnticheat.Initialize("https://anticheat.example.com");
        
        if (result.StartsWith("Error"))
        {
            Debug.LogError($"Failed to initialize: {result}");
            return;
        }

        // Get player ID
        string playerId = MaxionAnticheat.GetPlayerId();
        Debug.Log($"Player ID: {playerId}");

        // Register player
        string hwid = SystemInfo.deviceUniqueIdentifier;
        result = MaxionAnticheat.SubmitCheatEvent(playerId, "ProcessInjection", hwid, 0);
        
        if (result != "OK")
        {
            Debug.LogError($"Failed to submit event: {result}");
        }
    }
}
```

## Benefits of Consolidation

1. **Single Source of Truth**: All client communication in one place
2. **Simplified Unity Integration**: One native library to import
3. **Reduced Duplication**: No more split between 008a and 008b
4. **Easier Maintenance**: Changes in one place affect all functionality
5. **Better Testing**: Comprehensive tests for all client operations
6. **Clear API**: Unified interface for all server operations
7. **Error Handling**: Consistent error handling across all operations

## Migration Guide

### Before (Split Across 008a and 008b)

```
008a_client_foundation.md:
- FFI exports (maxion_register_cheat_callback, maxion_send_cheat_event)
- Key management (ClientKeypair, KeyManager)
- Detection logic

008b_client_server_comm.md:
- ServerClient struct
- HTTP client implementation
- Retry logic
- Action token management
```

### After (Consolidated in maxion-anticheat-client)

```
maxion-anticheat-client/src:
- lib.rs: All FFI exports
- client.rs: Unified AnticheatClient
- crypto/key_manager.rs: Key management
- retry.rs: Retry logic
- errors.rs: Unified error handling
```

## Dependencies

This crate depends on:
- `maxion-detection-types`: Shared type definitions
- `reqwest`: HTTP client
- `tokio`: Async runtime
- `ed25519-dalek`: Cryptographic signing
- `blake3`: Hashing for player ID

## Testing Strategy

### Unit Tests
- Key generation and loading
- Payload signing and verification
- Retry logic with exponential backoff
- Error handling

### Integration Tests
- Full registration flow
- Cheat event submission
- Ban status checking
- Pattern fetching

### Mock Server Tests
- Simulate server responses
- Test retry logic
- Test error scenarios

## Next Steps

1. [ ] Implement all modules (crypto, retry, errors)
2. [ ] Add comprehensive unit tests
3. [ ] Add integration tests with mock server
4. [ ] Create build scripts for Windows/Mac/Linux
5. [ ] Write Unity integration guide
6. [ ] Create example Unity project
7. [ ] Performance benchmarking
8. [ ] Security audit

## Notes

- All FFI functions return JSON strings or error messages
- Strings allocated by Rust must be freed with `maxion_free_string`
- Client maintains single tokio runtime for all async operations
- Keys are stored in `~/.maxion/keypair.bin`
- Supports offline mode (events buffered locally)
- Retry logic uses exponential backoff
- TLS is enforced for all HTTPS connections
- All timestamps use UTC timezone
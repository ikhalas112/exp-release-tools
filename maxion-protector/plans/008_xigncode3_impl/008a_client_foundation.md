# 008a: Client Foundation - Native Windows Detection

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-27 |
| Version | 1.0 |
| Complexity | Advanced |
| Time to Read | 20 minutes |
| Audience | Developers, Windows System Engineers |

## Overview
This plan focuses on implementing client-side anti-hack detection capabilities that operate entirely offline. These components form the foundation for the Maxion Protector's client module, providing immediate protection without requiring server connectivity.

**IMPORTANT CHANGE:** Client runs as native Windows Rust (NOT WASM/Cloudflare Workers). Server-side components (008b-008f) will use Cloudflare Workers + Durable Objects.

## Architecture Notes

### Key Constraints
- **Native Windows Client**: Runs as Windows DLL/exe, full Windows API access
- **No WASM Requirements**: Detection logic can use standard Rust std
- **Unity Integration**: View-only interface, simple FFI calls from Unity to Rust
- **Stateless**: No persistent state on client, all validation happens on demand
- **Server Communication**: Connects to Cloudflare Workers (server-side) for pattern updates and telemetry
- **Dual Verification**: 
  - **Ed25519 Key Pair**: Generated at first launch, public key registered with server linked to HWID
  - **BLAKE3 Action Tokens**: One-time tokens from server for each action, prevents replay attacks
  - **Player ID Derivation**: BLAKE3 hash of Ed25519 public key used as player_id

### Crate Structure
```
maxion-antihack/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Public API, FFI exports
│   ├── mod.rs           # Module organization
│   ├── types.rs         # Decoupled structs/impls
│   └── detection/
│       ├── mod.rs       # Detection module index
│       ├── api.rs       # API monitoring & hooking
│       ├── process.rs   # Process injection detection
│       ├── macro.rs     # Hardware macro detection
│       ├── integrity.rs # OS/Binary integrity validation
│       └── antidebug.rs # Anti-debugging/VM detection
└── build.rs             # Optional build script
```

## Implementation Tasks

### Task 1: Project Setup (Day 1-2)

#### 1.1 Create maxion-antihack Crate
```toml
# Cargo.toml
[package]
name = "maxion-antihack"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
# Core dependencies
anyhow = "1.0"
thiserror = "2.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Windows-specific (required)
windows-sys = { version = "0.52", features = [
    "Win32_Foundation",
    "Win32_System_ProcessStatus",
    "Win32_System_Diagnostics_Debug",
    "Win32_System_Memory",
    "Win32_System_Threading",
    "Win32_Security",
] }
retour = "0.3"

# Cryptography (per project guidelines)
blake3 = "1.8"
orion = "0.17"
ed25519-dalek = "2"      # Ed25519 key generation and signing
bincode = "1.3"            # Serialization for payload signing

# Project dependencies
maxion-core = { path = "../maxion-core" }

[features]
default = []
```

#### 1.2 Implement FFI Interface (lib.rs)
```rust
// src/lib.rs
use std::ffi::{c_char, c_void};
use std::ffi::CString;
use std::ptr;

mod detection;
mod types;

use types::{DetectionEvent, DetectionResult};

// FFI exports for Unity
#[no_mangle]
pub extern "C" fn detect_anomalies() -> *mut c_char {
    match run_detection() {
        Ok(result) => {
            let json = serde_json::to_string(&result).unwrap_or_default();
            CString::new(json)
                .unwrap()
                .into_raw()
        }
        Err(e) => {
            let error_json = format!(r#"{{"error": "{}"}}"#, e);
            CString::new(error_json)
                .unwrap()
                .into_raw()
        }
    }
}

#[no_mangle]
pub extern "C" fn free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

fn run_detection() -> Result<DetectionResult, anyhow::Error> {
    let mut events = Vec::new();
    
    // API monitoring
    events.extend(detection::api::monitor_api_bypasses()?);
    
    // Process injection detection
    events.extend(detection::process::detect_process_injection()?);
    
    // Hardware macro detection
    events.extend(detection::macro::detect_hardware_macros()?);
    
    // OS integrity validation
    events.extend(detection::integrity::validate_os_integrity()?);
    
    // Anti-debugging/VM detection
    events.extend(detection::antidebug::validate_runtime_environment()?);
    
    // Calculate risk score
    let risk_score = calculate_risk_score(&events);
    
    // Get suspicious modules
    let suspicious_modules = detection::process::get_suspicious_modules()?;
    
    Ok(DetectionResult {
        events,
        risk_score,
        suspicious_modules,
    })
}

fn calculate_risk_score(events: &[DetectionEvent]) -> f32 {
    events.iter()
        .map(|e| match e.severity {
            types::DetectionSeverity::Low => 1.0,
            types::DetectionSeverity::Medium => 5.0,
            types::DetectionSeverity::High => 10.0,
            types::DetectionSeverity::Critical => 25.0,
        })
        .sum()
}
```

### Task 2: Core Types (Day 2-3)

#### 2.1 Detection Events (types.rs)
```rust
// src/types.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetectionSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetectionType {
    ApiBypass,
    HardwareMacro,
    ProcessInjection,
    OsIntegrityViolation,
    AntiDebugTriggered,
    VmDetected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionEvent {
    pub event_type: DetectionType,
    pub severity: DetectionSeverity,
    pub timestamp: u64,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub events: Vec<DetectionEvent>,
    pub risk_score: f32,
    pub suspicious_modules: Vec<String>,
}

// === Ed25519 & Action Token Types ===

/// Ed25519 public/private key pair for client-side signing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientKeypair {
    pub public_key: Vec<u8>,  // 32 bytes
    pub private_key: Vec<u8>, // 32 bytes (stored securely)
}

impl ClientKeypair {
    /// Generate new Ed25519 key pair
    pub fn generate() -> Self {
        use rand::rngs::OsRng;
        use ed25519_dalek::{Keypair, Signer};
        
        let mut csprng = OsRng {};
        let keypair = Keypair::generate(&mut csprng);
        
        ClientKeypair {
            public_key: keypair.public.as_bytes().to_vec(),
            private_key: keypair.secret.as_bytes().to_vec(),
        }
    }
    
    /// Derive player_id from public key using BLAKE3
    pub fn derive_player_id(&self) -> String {
        format!("{:x}", blake3::hash(&self.public_key))
    }
}

/// Action token request (client → server)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTokenRequest {
    pub player_id: String,  // BLAKE3 hash of Ed25519 public key
    pub nonce: u64,        // Client-generated timestamp (ms)
}

/// Action token response (server → client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionToken {
    pub player_id: String,
    pub timestamp: u64,        // Server timestamp (seconds)
    pub nonce: u64,            // Echoed back
    pub token_hash: [u8; 32],  // BLAKE3(player_id || timestamp || nonce || server_secret)
    pub expires_at: u64,       // +5 minutes
}

impl ActionToken {
    /// Validate action token freshness
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now().timestamp() as u64;
        self.expires_at > now && self.expires_at - now < 300 // Within 5 minutes
    }
}

/// Signed payload containing action data and action token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPayload {
    pub action_data: CheatEvent,
    pub action_token: ActionToken,
}

/// Cheat event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheatEvent {
    pub game_id: String,
    pub version: String,
    pub player_id: String,      // BLAKE3 hash of Ed25519 public key
    pub cheat_type: i32,
    pub hwid: Option<String>,
    pub timestamp: u64,         // Nanoseconds since epoch
    pub detection_count: u32,
}

/// Full signed request from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedRequest {
    pub payload: SignedPayload,
    pub signature: Vec<u8>,      // 64 bytes Ed25519 signature
}

impl SignedRequest {
    /// Verify the Ed25519 signature
    pub fn verify(&self, public_key: &[u8]) -> Result<bool, Box<dyn std::error::Error>> {
        use ed25519_dalek::{PublicKey, Signature, Verifier};
        
        let pk = PublicKey::from_bytes(public_key)?;
        let payload_bytes = bincode::serialize(&self.payload)?;
        let sig = Signature::from_bytes(&self.signature)?;
        
        Ok(pk.verify(&payload_bytes, &sig).is_ok())
    }
}

/// Player registration request (first launch)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationRequest {
    pub player_id: String,   // Derived from public key
    pub public_key: Vec<u8>, // 32 bytes Ed25519 public key
    pub hwid: Option<String>,
}

/// Registration response from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResponse {
    pub player_id: String,
    pub registered_at: u64,
    pub expires_at: Option<u64>,
}
```

### Task 3: API Monitoring (Day 3-5)

#### 3.1 Hook Critical APIs
Target APIs to monitor:
- `CreateProcess` / `CreateProcessA` / `CreateProcessW`
- `WriteProcessMemory`
- `ReadProcessMemory`
- `VirtualProtect`
- `SetWindowsHookEx`
- `LoadLibrary` / `GetProcAddress`

#### 3.2 Implementation (detection/api.rs)
```rust
// src/detection/api.rs
use anyhow::{Context, Result};
use retour::static_detour;
use std::ptr;
use windows_sys::Win32::Foundation::TRUE;
use crate::types::{DetectionEvent, DetectionSeverity, DetectionType};

static_detour! {
    static CreateProcessHook: unsafe extern "system" fn(
        *const u16, 
        *mut u16, 
        *mut std::ffi::c_void, 
        *mut std::ffi::c_void, 
        i32, 
        u32, 
        *mut std::ffi::c_void, 
        *const u16, 
        *mut std::ffi::c_void, 
        *mut u16
    ) -> i32;
}

type CreateProcessFn = unsafe extern "system" fn(
    *const u16,
    *mut u16,
    *mut std::ffi::c_void,
    *mut std::ffi::c_void,
    i32,
    u32,
    *mut std::ffi::c_void,
    *const u16,
    *mut std::ffi::c_void,
    *mut u16,
) -> i32;

static mut ORIGINAL_CREATE_PROCESS: Option<CreateProcessFn> = None;

pub fn monitor_api_bypasses() -> Result<Vec<DetectionEvent>> {
    let mut events = Vec::new();
    
    // Hook CreateProcess
    unsafe {
        let target = get_create_process_address()?;
        ORIGINAL_CREATE_PROCESS = Some(std::mem::transmute(target));
        
        CreateProcessHook.initialize(
            target,
            create_process_detour
        )?;
        
        CreateProcessHook.enable()?;
    }
    
    Ok(events)
}

unsafe fn get_create_process_address() -> Result<CreateProcessFn> {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    
    let kernel32 = GetModuleHandleW(windows_sys::w!("kernel32.dll\0"));
    if kernel32.is_null() {
        anyhow::bail!("Failed to get kernel32 handle");
    }
    
    let func = GetProcAddress(kernel32, windows_sys::s!("CreateProcessW\0"));
    if func.is_null() {
        anyhow::bail!("Failed to get CreateProcessW address");
    }
    
    Ok(std::mem::transmute(func))
}

unsafe extern "system" fn create_process_detour(
    application_name: *const u16,
    command_line: *mut u16,
    process_attributes: *mut std::ffi::c_void,
    thread_attributes: *mut std::ffi::c_void,
    inherit_handles: i32,
    creation_flags: u32,
    environment: *mut std::ffi::c_void,
    current_directory: *const u16,
    startup_info: *mut std::ffi::c_void,
    process_information: *mut u16,
) -> i32 {
    // Log the process creation attempt
    // Check if it's suspicious (cheat tools, etc.)
    
    // Call original
    if let Some(original) = ORIGINAL_CREATE_PROCESS {
        original(
            application_name,
            command_line,
            process_attributes,
            thread_attributes,
            inherit_handles,
            creation_flags,
            environment,
            current_directory,
            startup_info,
            process_information,
        )
    } else {
        0
    }
}
```

### Task 4: Process Injection Detection (Day 5-7)

#### 4.1 Detection Strategies
- Check for unusual memory permissions
- Monitor for remote thread creation
- Detect suspicious DLL injection patterns
- Scan for known cheat signatures in loaded modules

#### 4.2 Implementation (detection/process.rs)
```rust
// src/detection/process.rs
use anyhow::Result;
use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
use windows_sys::Win32::System::Threading::*;
use crate::types::{DetectionEvent, DetectionSeverity, DetectionType};

pub fn detect_process_injection() -> Result<Vec<DetectionEvent>> {
    let mut events = Vec::new();
    
    // Enumerate processes
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Ok(events);
    }
    
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    
    unsafe {
        if Process32FirstW(snapshot, &mut entry) == TRUE {
            loop {
                let process_name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(0)]
                );
                
                // Check for suspicious processes
                if is_suspicious_process(&process_name) {
                    events.push(DetectionEvent {
                        event_type: DetectionType::ProcessInjection,
                        severity: DetectionSeverity::High,
                        timestamp: get_timestamp(),
                        details: format!("Suspicious process detected: {}", process_name),
                    });
                }
                
                if Process32NextW(snapshot, &mut entry) != TRUE {
                    break;
                }
            }
        }
        
        CloseHandle(snapshot);
    }
    
    Ok(events)
}

pub fn get_suspicious_modules() -> Result<Vec<String>> {
    let mut modules = Vec::new();
    
    // Check loaded modules for known cheat signatures
    // This would scan the current process's modules
    
    Ok(modules)
}

fn is_suspicious_process(name: &str) -> bool {
    let suspicious_names = vec![
        "cheatengine",
        "x64dbg",
        "ida",
        "injector",
        "hack",
    ];
    
    suspicious_names.iter().any(|s| name.to_lowercase().contains(s))
}

fn get_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
```

### Task 5: Hardware Macro Detection (Day 7-10)

#### 5.1 Detection Methodology
Based on research from 007a_research.md:
- Statistical analysis of input timing
- Measure jitter and variance patterns
- Detect automated input signatures
- Analyze input frequency distribution using Kolmogorov-Smirnov test

#### 5.2 Implementation (detection/macro.rs)
```rust
// src/detection/macro.rs
use anyhow::Result;
use crate::types::{DetectionEvent, DetectionSeverity, DetectionType};

pub struct InputAnalysis {
    pub samples: Vec<u64>,  // Timestamps in microseconds
}

impl InputAnalysis {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }
    
    pub fn add_sample(&mut self, timestamp: u64) {
        self.samples.push(timestamp);
    }
    
    pub fn calculate_jitter(&self) -> f32 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        
        let mut deltas = Vec::new();
        for i in 1..self.samples.len() {
            deltas.push(self.samples[i] - self.samples[i - 1]);
        }
        
        // Calculate variance
        let mean: f32 = deltas.iter().map(|&x| x as f32).sum::<f32>() / deltas.len() as f32;
        let variance: f32 = deltas.iter()
            .map(|&x| (x as f32 - mean).powi(2))
            .sum::<f32>() / deltas.len() as f32;
        
        variance.sqrt()
    }
    
    pub fn detect_automation(&self) -> bool {
        if self.samples.len() < 10 {
            return false;
        }
        
        let jitter = self.calculate_jitter();
        
        // Low jitter suggests automation
        // Human input typically has jitter > 1000 microseconds
        // Macros have jitter < 100 microseconds
        jitter < 100.0
    }
}

pub fn detect_hardware_macros() -> Result<Vec<DetectionEvent>> {
    let mut events = Vec::new();
    
    // In a real implementation, this would:
    // 1. Hook RawInput to capture keyboard/mouse events
    // 2. Collect timing samples over time
    // 3. Run statistical analysis
    // 4. Flag if variance is too low
    
    let analysis = InputAnalysis::new();
    
    // Simulated detection
    if analysis.detect_automation() {
        events.push(DetectionEvent {
            event_type: DetectionType::HardwareMacro,
            severity: DetectionSeverity::Medium,
            timestamp: get_timestamp(),
            details: "Hardware macro detected: Abnormally consistent input timing".to_string(),
        });
    }
    
    Ok(events)
}

fn get_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
```

### Task 6: OS Integrity Validation (Day 10-12)

#### 6.1 Integrity Checks
- Verify system DLL hashes using blake3 (per project guidelines)
- Check for hooking in critical system calls
- Detect patched system files
- Validate kernel structures (if accessible)

#### 6.2 Implementation (detection/integrity.rs)
```rust
// src/detection/integrity.rs
use anyhow::{Context, Result};
use blake3::{hash, Hash};
use std::path::PathBuf;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetModuleFileNameW};
use crate::types::{DetectionEvent, DetectionSeverity, DetectionType};

pub fn validate_os_integrity() -> Result<Vec<DetectionEvent>> {
    let mut events = Vec::new();
    
    // Check critical system DLLs
    let system_dlls = vec![
        "kernel32.dll",
        "user32.dll",
        "ntdll.dll",
    ];
    
    for dll_name in system_dlls {
        if let Some(event) = verify_dll_integrity(dll_name)? {
            events.push(event);
        }
    }
    
    Ok(events)
}

fn verify_dll_integrity(dll_name: &str) -> Result<Option<DetectionEvent>> {
    let handle = unsafe { GetModuleHandleW(windows_sys::w!(dll_name)) };
    if handle.is_null() {
        return Ok(None);
    }
    
    let mut path_buffer = [0u16; 260];
    let len = unsafe { GetModuleFileNameW(handle, &mut path_buffer as *mut _ as *mut u16, 260) };
    
    if len == 0 {
        return Ok(None);
    }
    
    let path = String::from_utf16_lossy(&path_buffer[..len as usize]);
    
    // In a real implementation, this would:
    // 1. Calculate blake3 hash of the DLL
    // 2. Compare against known good hash database
    // 3. Flag if mismatched
    
    // Simulated check
    let hash = calculate_file_hash(&path)?;
    let known_good_hash = get_known_good_hash(dll_name);
    
    if hash != known_good_hash {
        return Ok(Some(DetectionEvent {
            event_type: DetectionType::OsIntegrityViolation,
            severity: DetectionSeverity::Critical,
            timestamp: get_timestamp(),
            details: format!("DLL hash mismatch: {} (expected: {}, actual: {})", dll_name, known_good_hash, hash),
        }));
    }
    
    Ok(None)
}

fn calculate_file_hash(path: &str) -> Result<String> {
    // Use blake3 as per project guidelines
    let bytes = std::fs::read(path)?;
    let hash = hash(&bytes);
    Ok(hash.to_hex().to_string())
}

fn get_known_good_hash(dll_name: &str) -> String {
    // In production, this would come from a secure database
    // For now, return a placeholder
    format!("placeholder_hash_for_{}", dll_name)
}

fn get_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
```

### Task 7: Anti-Debugging & VM Detection (Day 12-14)

#### 7.1 Detection Techniques
Based on research from 007a_research.md:
- Check for debugger presence using PEB
- Detect virtualization artifacts
- Monitor timing anomalies (VMs have different timing)
- Check for known hypervisor signatures

#### 7.2 Implementation (detection/antidebug.rs)
```rust
// src/detection/antidebug.rs
use anyhow::Result;
use crate::types::{DetectionEvent, DetectionSeverity, DetectionType};

pub fn detect_debugger() -> bool {
    // Method 1: Check PEB BeingDebugged flag
    if unsafe { check_peb_being_debugged() } {
        return true;
    }
    
    // Method 2: Check for debugger using IsDebuggerPresent
    if unsafe { windows_sys::Win32::System::Diagnostics::Debug::IsDebuggerPresent() != 0 } {
        return true;
    }
    
    false
}

pub fn detect_vm() -> bool {
    // Method 1: Check for common VM registry keys
    if check_vm_registry_keys() {
        return true;
    }
    
    // Method 2: Check for VM-specific processes
    if check_vm_processes() {
        return true;
    }
    
    false
}

unsafe fn check_peb_being_debugged() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::asm;
        let mut peb_address: usize = 0;
        asm!(
            "mov {}, gs:[0x60]",
            out(reg) peb_address
        );
        let being_debugged = *(peb_address as *const u8).add(2);
        being_debugged != 0
    }
    
    #[cfg(target_arch = "x86")]
    {
        use std::arch::asm;
        let mut peb_address: usize = 0;
        asm!(
            "mov {}, fs:[0x30]",
            out(reg) peb_address
        );
        let being_debugged = *(peb_address as *const u8).add(2);
        being_debugged != 0
    }
    
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
    {
        false
    }
}

fn check_vm_registry_keys() -> bool {
    // Check for common VM registry keys
    // This would use windows-sys registry APIs
    false
}

fn check_vm_processes() -> bool {
    // Check for VM-specific processes
    let vm_processes = vec![
        "vmwaretray.exe",
        "vmwareuser.exe",
        "vboxtray.exe",
        "vboxservice.exe",
    ];
    
    // Enumerate processes and check for VM indicators
    false
}

pub fn validate_runtime_environment() -> Result<Vec<DetectionEvent>> {
    let mut events = Vec::new();
    
    if detect_debugger() {
        events.push(DetectionEvent {
            event_type: DetectionType::AntiDebugTriggered,
            severity: DetectionSeverity::Critical,
            timestamp: get_timestamp(),
            details: "Debugger detected".to_string(),
        });
    }
    
    if detect_vm() {
        events.push(DetectionEvent {
            event_type: DetectionType::VmDetected,
            severity: DetectionSeverity::Medium,
            timestamp: get_timestamp(),
            details: "Virtual environment detected".to_string(),
        });
    }
    
    Ok(events)
}

fn get_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
```

### Task 7.5: Ed25519 Key Management & Action Token Client (Day 15-16)

#### 7.5.1 Client-Side Key Management (client/crypto.rs)
```rust
// src/client/crypto.rs
use std::fs;
use std::path::PathBuf;
use ed25519_dalek::{Keypair, Signer, SecretKey, PublicKey};
use blake3;
use crate::types::{ClientKeypair, ActionToken, SignedRequest, CheatEvent};

/// Client key manager for Ed25519 key pair storage and usage
pub struct KeyManager {
    keypair_path: PathBuf,
    keypair: Option<ClientKeypair>,
}

impl KeyManager {
    /// Create new KeyManager with persistent storage path
    pub fn new(base_path: PathBuf) -> Self {
        let keypair_path = base_path.join("client_keypair.bin");
        KeyManager {
            keypair_path,
            keypair: None,
        }
    }
    
    /// Load existing keypair or generate new one
    pub fn load_or_generate(&mut self) -> Result<ClientKeypair, anyhow::Error> {
        if self.keypair.is_some() {
            return Ok(self.keypair.as_ref().unwrap().clone());
        }
        
        // Try to load existing keypair
        if self.keypair_path.exists() {
            let data = fs::read(&self.keypair_path)?;
            let keypair: ClientKeypair = bincode::deserialize(&data)?;
            self.keypair = Some(keypair.clone());
            return Ok(keypair);
        }
        
        // Generate new keypair
        let keypair = ClientKeypair::generate();
        
        // Persist securely
        fs::write(&self.keypair_path, bincode::serialize(&keypair)?)?;
        
        self.keypair = Some(keypair.clone());
        Ok(keypair)
    }
    
    /// Get player_id (derived from public key)
    pub fn get_player_id(&self) -> Result<String, anyhow::Error> {
        let keypair = self.keypair.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Keypair not loaded"))?;
        Ok(keypair.derive_player_id())
    }
    
    /// Sign a payload with Ed25519 private key
    pub fn sign_payload(&self, payload: &SignedPayload) -> Result<Vec<u8>, anyhow::Error> {
        let keypair = self.keypair.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Keypair not loaded"))?;
        
        let secret_key = SecretKey::from_bytes(&keypair.private_key)?;
        let public_key = PublicKey::from_bytes(&keypair.public_key)?;
        let kp = Keypair { secret: secret_key, public: public_key };
        
        let payload_bytes = bincode::serialize(payload)?;
        let signature = kp.sign(&payload_bytes);
        
        Ok(signature.to_bytes().to_vec())
    }
}
```

#### 7.5.2 Action Token Client (client/token.rs)
```rust
// src/client/token.rs
use reqwest::Client;
use chrono::Utc;
use crate::types::{ActionTokenRequest, ActionToken, CheatEvent, SignedRequest, SignedPayload};
use crate::client::crypto::KeyManager;

/// Action token client for requesting tokens from server
pub struct ActionTokenClient {
    server_url: String,
    http_client: Client,
}

impl ActionTokenClient {
    pub fn new(server_url: String) -> Self {
        ActionTokenClient {
            server_url,
            http_client: Client::new(),
        }
    }
    
    /// Request one-time action token from server
    pub async fn request_token(&self, player_id: &str) -> Result<ActionToken, anyhow::Error> {
        let nonce = Utc::now().timestamp_millis() as u64;
        
        let request = ActionTokenRequest {
            player_id: player_id.to_string(),
            nonce,
        };
        
        let url = format!("{}/action-token", self.server_url);
        let response = self.http_client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .error_for_status()?;
        
        let token: ActionToken = response.json().await?;
        
        // Validate token freshness
        if !token.is_valid() {
            return Err(anyhow::anyhow!("Received expired or invalid action token"));
        }
        
        Ok(token)
    }
    
    /// Sign and send cheat event to server
    pub async fn send_cheat_event(
        &self,
        key_manager: &KeyManager,
        event: CheatEvent,
    ) -> Result<(), anyhow::Error> {
        // Request action token
        let action_token = self.request_token(&event.player_id).await?;
        
        // Create signed payload
        let payload = SignedPayload {
            action_data: event,
            action_token,
        };
        
        // Sign payload
        let signature = key_manager.sign_payload(&payload)?;
        
        // Create signed request
        let request = SignedRequest {
            payload,
            signature,
        };
        
        // Send to server
        let url = format!("{}/cheat", self.server_url);
        self.http_client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .error_for_status()?;
        
        Ok(())
    }
}
```

#### 7.5.3 Player Registration (client/registration.rs)
```rust
// src/client/registration.rs
use reqwest::Client;
use crate::types::{RegistrationRequest, RegistrationResponse};
use crate::client::crypto::KeyManager;

/// Client registration for first-time player setup
pub struct RegistrationClient {
    server_url: String,
    http_client: Client,
}

impl RegistrationClient {
    pub fn new(server_url: String) -> Self {
        RegistrationClient {
            server_url,
            http_client: Client::new(),
        }
    }
    
    /// Register player's Ed25519 public key with server
    pub async fn register_player(
        &self,
        key_manager: &KeyManager,
        hwid: Option<String>,
    ) -> Result<RegistrationResponse, anyhow::Error> {
        let keypair = key_manager.load_or_generate()?;
        let player_id = keypair.derive_player_id();
        
        let request = RegistrationRequest {
            player_id: player_id.clone(),
            public_key: keypair.public_key,
            hwid,
        };
        
        let url = format!("{}/register", self.server_url);
        let response = self.http_client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .error_for_status()?;
        
        let result: RegistrationResponse = response.json().await?;
        
        Ok(result)
    }
}
```

#### 7.5.4 FFI Exports for Key Management (lib.rs additions)
```rust
// Add to src/lib.rs
mod client;

use client::crypto::KeyManager;
use client::token::ActionTokenClient;
use client::registration::RegistrationClient;

static KEY_MANAGER: Lazy<Mutex<KeyManager>> = Lazy::new(|| {
    let base_dir = std::env::var("MAXION_DATA_DIR")
        .unwrap_or_else(|_| ".".to_string());
    Mutex::new(KeyManager::new(PathBuf::from(base_dir)))
});

/// Get or generate player_id from Ed25519 public key
#[no_mangle]
pub extern "C" fn get_player_id(ptr: *mut *const u8, len: *mut usize) {
    let mut manager = KEY_MANAGER.lock().unwrap();
    
    match manager.load_or_generate() {
        Ok(keypair) => {
            let player_id = keypair.derive_player_id();
            let c_str = CString::new(player_id).unwrap();
            unsafe {
                *ptr = c_str.into_raw();
                *len = c_str.as_bytes().len();
            }
        }
        Err(_) => {
            unsafe {
                *ptr = std::ptr::null();
                *len = 0;
            }
        }
    }
}

/// Request action token from server (async)
#[no_mangle]
pub extern "C" fn request_action_token(
    player_id: *const c_char,
    player_id_len: usize,
    server_url: *const c_char,
    server_url_len: usize,
    result_ptr: *mut *const u8,
    result_len: *mut usize,
) -> i32 {
    // Convert C strings to Rust strings
    let player_id_str = unsafe {
        std::slice::from_raw_parts(player_id as *const u8, player_id_len)
    };
    let player_id = match std::str::from_utf8(player_id_str) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    
    let server_url_str = unsafe {
        std::slice::from_raw_parts(server_url as *const u8, server_url_len)
    };
    let server_url = match std::str::from_utf8(server_url_str) {
        Ok(s) => s,
        Err(_) => return -2,
    };
    
    // Request token (blocking for simplicity, should use async in practice)
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async {
        let client = ActionTokenClient::new(server_url.to_string());
        client.request_token(player_id).await
    });
    
    match result {
        Ok(token) => {
            let json = serde_json::to_string(&token).unwrap();
            let c_str = CString::new(json).unwrap();
            unsafe {
                *result_ptr = c_str.into_raw();
                *result_len = c_str.as_bytes().len();
            }
            0 // Success
        }
        Err(_) => -3,
    }
}

/// Send signed cheat event to server (async)
#[no_mangle]
pub extern "C" fn send_cheat_event_signed(
    game_id: *const c_char,
    game_id_len: usize,
    version: *const c_char,
    version_len: usize,
    cheat_type: i32,
    hwid: *const c_char,
    hwid_len: usize,
    timestamp: u64,
    detection_count: u32,
    server_url: *const c_char,
    server_url_len: usize,
) -> i32 {
    let mut manager = KEY_MANAGER.lock().unwrap();
    let keypair = match manager.load_or_generate() {
        Ok(k) => k,
        Err(_) => return -1,
    };
    
    // Convert C strings
    let game_id = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(game_id as *const u8, game_id_len)) };
    let version = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(version as *const u8, version_len)) };
    let hwid = if !hwid.is_null() && hwid_len > 0 {
        Some(unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(hwid as *const u8, hwid_len)).to_string() })
    } else {
        None
    };
    let server_url = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(server_url as *const u8, server_url_len)) };
    
    // Create cheat event
    let player_id = keypair.derive_player_id();
    let event = CheatEvent {
        game_id: game_id.to_string(),
        version: version.to_string(),
        player_id,
        cheat_type,
        hwid,
        timestamp,
        detection_count,
    };
    
    // Send signed event (blocking for simplicity)
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = ActionTokenClient::new(server_url.to_string());
    let result = rt.block_on(async {
        client.send_cheat_event(&manager, event).await
    });
    
    match result {
        Ok(_) => 0,   // Success
        Err(_) => -2, // Error
    }
}

/// Register player with server (first launch)
#[no_mangle]
pub extern "C" fn register_player(
    server_url: *const c_char,
    server_url_len: usize,
    hwid: *const c_char,
    hwid_len: usize,
    result_ptr: *mut *const u8,
    result_len: *mut usize,
) -> i32 {
    let mut manager = KEY_MANAGER.lock().unwrap();
    
    // Convert C strings
    let server_url = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(server_url as *const u8, server_url_len)) };
    let hwid = if !hwid.is_null() && hwid_len > 0 {
        Some(unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(hwid as *const u8, hwid_len)).to_string() })
    } else {
        None
    };
    
    // Register (blocking for simplicity)
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = RegistrationClient::new(server_url.to_string());
    let result = rt.block_on(async {
        client.register_player(&manager, hwid).await
    });
    
    match result {
        Ok(response) => {
            let json = serde_json::to_string(&response).unwrap();
            let c_str = CString::new(json).unwrap();
            unsafe {
                *result_ptr = c_str.into_raw();
                *result_len = c_str.as_bytes().len();
            }
            0 // Success
        }
        Err(_) => -1, // Error
    }
}
```

### Task 8: Unity FFI Integration (Day 14-15)

#### 8.1 C# Interface
```csharp
// Assets/Scripts/AntiHack/AntiHackManager.cs
using UnityEngine;
using System.Runtime.InteropServices;
using System.Text.Json;

public class AntiHackManager : MonoBehaviour
{
    [DllImport("maxion_antihack")]
    private static extern IntPtr detect_anomalies();

    [DllImport("maxion_antihack")]
    private static extern void free_string(IntPtr ptr);

    public void RunDetection()
    {
        // Call Rust detection
        IntPtr resultPtr = detect_anomalies();
        
        try
        {
            string resultJson = Marshal.PtrToStringUTF8(resultPtr);
            var result = JsonSerializer.Deserialize<DetectionResult>(resultJson);
            
            // Handle detection results
            if (result.risk_score > 10.0f)
            {
                Debug.LogWarning($"High risk detected: {result.risk_score}");
                // Take appropriate action
            }
            
            foreach (var evt in result.events)
            {
                Debug.Log($"Detection: {evt.event_type} - {evt.details}");
            }
        }
        finally
        {
            free_string(resultPtr);
        }
    }

    [System.Serializable]
    public class DetectionEvent
    {
        public string event_type;
        public string severity;
        public long timestamp;
        public string details;
    }

    [System.Serializable]
    public class DetectionResult
    {
        public DetectionEvent[] events;
        public float risk_score;
        public string[] suspicious_modules;
    }
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_bypass_detection() {
        // Test API monitoring
    }

    #[test]
    fn test_macro_timing_analysis() {
        // Test input pattern recognition
    }

    #[test]
    fn test_integrity_validation() {
        // Test OS integrity checks
    }
    
    #[test]
    fn test_jitter_calculation() {
        let mut analysis = InputAnalysis::new();
        for i in 0..10 {
            analysis.add_sample(1000 + i * 1050); // 1050 microsecond intervals
        }
        
        let jitter = analysis.calculate_jitter();
        // Should be 0 for perfect intervals
        assert_eq!(jitter, 0.0);
    }
}
```

### Integration Tests
- Test FFI interface with Unity mock
- Verify detection accuracy with known cheat tools
- Performance benchmarking (must stay under 5ms overhead)

## Performance Requirements

- **Detection Latency**: < 5ms per scan
- **Memory Overhead**: < 10MB
- **CPU Impact**: < 2% during gameplay
- **False Positive Rate**: < 0.1%

## Security Considerations

### Anti-Tampering
- Code obfuscation (using rust-native-obf or goldberg)
- Binary integrity verification (using blake3)
- Memory protection for detection logic

### Bypass Prevention
- Randomized detection intervals
- Multiple validation layers
- Silent operation (no visible indicators)

## Dependencies

### Required Crates
- `anyhow` - Error handling
- `thiserror` - Error types
- `serde` - Serialization
- `blake3` - Hashing (per project guidelines)
- `orion` - Cryptography (per project guidelines)
- `retour` - API hooking (Windows)
- `windows-sys` - Windows API access
- `maxion-core` - Core integration

### Optional/Conditional
- `detour` - Alternative hooking library
- `goldberg` - Code obfuscation
- `rust-native-obf` - Obfuscation

## Deliverables

1. ✅ Complete `maxion-antihack` crate (native Windows)
2. ✅ FFI interface for Unity
3. ✅ Unit and integration tests
4. ✅ Performance benchmarks
5. ✅ Documentation for Unity integration

## Server Communication (Preview)

The client will communicate with Cloudflare Workers (implemented in 008b) for:
- Pattern updates (download new detection signatures)
- Telemetry upload (send detection events)
- Status checks (verify client version, ban status)

Communication will use:
- HTTP/HTTPS for pattern updates
- WebSocket for real-time telemetry (if needed)
- JWT tokens for authentication

## Next Steps

After completing this phase, proceed to:
- **008b**: Client-Server Communication Layer (Cloudflare Workers + Axum)
- **008c**: Server-Side Detection Service (Durable Objects + SQLite)
- **008d**: Pattern Management System

## Notes

- All detection logic is stateless (per project guidelines)
- No persistent storage on client
- Results are aggregated by server (008b+)
- Follow project coding style: snake_case, match over if, early returns
- Use `blake3` for all hashing (per project guidelines)
- Use `Uuid::now_v7()` for any ID generation (if needed)
- Client is native Windows Rust - NO WASM requirements
- Server-side uses Cloudflare Workers + Durable Objects + SQLite

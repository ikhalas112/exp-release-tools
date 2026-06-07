//! # Cheat Callback System
//!
//! Provides FFI callbacks to Unity when cheats are detected, along with hardware identification.
//!
//! ## Architecture
//! - **Rust**: Handles detection, HWID generation, callback invocation
//! - **Unity**: VIEW-ONLY - receives notifications and decides what to display
//!
//! ## Features
//! - Thread-safe callback registration
//! - Cached hardware ID generation
//! - Multiple cheat type support
//! - Graceful degradation if callback not registered

use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use machineid_rs::{Encryption, HWIDComponent, IdBuilder};
use once_cell::sync::Lazy;
use std::ffi::c_void;
use std::time::{SystemTime, UNIX_EPOCH};

/// Types of cheats that can be detected
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheatType {
    /// Memory scanning and value modification detected
    MemoryTampering = 0,
    /// Value freezing (god mode, unlimited ammo) detected
    ValueFreeze = 1,
    /// Code or memory integrity violation detected
    IntegrityViolation = 2,
    /// Unknown cheat type
    Unknown = 99,
}

impl CheatType {
    /// Convert from integer (for FFI compatibility)
    #[inline]
    pub fn from_int(value: i32) -> Self {
        match value {
            0 => Self::MemoryTampering,
            1 => Self::ValueFreeze,
            2 => Self::IntegrityViolation,
            _ => Self::Unknown,
        }
    }

    /// Convert to integer (for FFI compatibility)
    #[inline]
    pub fn to_int(self) -> i32 {
        self as i32
    }
}

/// Information about a cheat detection event
#[derive(Debug, Clone)]
pub struct CheatEvent {
    /// Unix timestamp in milliseconds
    pub timestamp: u64,
    /// Type of cheat detected
    pub cheat_type: CheatType,
    /// Number of times this cheat type has been detected
    pub detection_count: u32,
}

impl CheatEvent {
    /// Create a new cheat event
    #[inline]
    pub fn new(cheat_type: CheatType, detection_count: u32) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            timestamp,
            cheat_type,
            detection_count,
        }
    }
}

/// Type of callback function for Unity
///
/// # Arguments
/// * `cheat_type` - Type of cheat (as i32 enum value)
/// * `hwid_ptr` - Pointer to hardware ID string (UTF-8)
/// * `hwid_len` - Length of hardware ID string
/// * `timestamp` - Unix timestamp in milliseconds
/// * `detection_count` - Number of detections for this cheat type
pub type CheatCallback = extern "C" fn(
    cheat_type: i32,
    hwid_ptr: *const u8,
    hwid_len: usize,
    timestamp: u64,
    detection_count: u32,
);

/// Thread-safe callback registry
///
/// Uses AtomicPtr to store the callback function pointer safely.
struct CheatCallbackRegistry {
    /// Registered callback function pointer
    callback: AtomicPtr<c_void>,
    /// Flag indicating if callback is registered
    registered: AtomicBool,
}

impl CheatCallbackRegistry {
    /// Create a new empty registry
    #[inline]
    const fn new() -> Self {
        Self {
            callback: AtomicPtr::new(std::ptr::null_mut()),
            registered: AtomicBool::new(false),
        }
    }

    /// Register a callback function
    ///
    /// # Safety
    /// The callback must be a valid function pointer that follows the CheatCallback signature.
    #[inline]
    unsafe fn register(&self, callback: Option<CheatCallback>) {
        match callback {
            Some(cb) => {
                self.callback.store(cb as *mut c_void, Ordering::Release);
                self.registered.store(true, Ordering::Release);
                log::info!("Cheat callback registered");
            }
            None => {
                self.unregister();
            }
        }
    }

    /// Unregister the current callback
    #[inline]
    pub(crate) fn unregister(&self) {
        self.callback.store(std::ptr::null_mut(), Ordering::Release);
        self.registered.store(false, Ordering::Release);
        log::info!("Cheat callback unregistered");
    }

    /// Check if a callback is registered
    #[inline]
    fn has_callback(&self) -> bool {
        self.registered.load(Ordering::Acquire)
    }

    /// Invoke the registered callback if it exists
    ///
    /// # Safety
    /// This function is thread-safe and handles invalid callbacks gracefully.
    #[inline]
    unsafe fn invoke(&self, event: &CheatEvent) {
        if !self.has_callback() {
            log::debug!("No cheat callback registered, skipping notification");
            return;
        }

        let callback_ptr = self.callback.load(Ordering::Acquire);
        if callback_ptr.is_null() {
            log::warn!("Callback registered but pointer is null");
            return;
        }

        let callback: CheatCallback = std::mem::transmute(callback_ptr);

        // Get HWID as UTF-8 bytes
        let hwid = HARDWARE_ID.as_bytes();

        // Invoke callback
        callback(
            event.cheat_type.to_int(),
            hwid.as_ptr(),
            hwid.len(),
            event.timestamp,
            event.detection_count,
        );

        log::debug!(
            "Cheat callback invoked: type={:?}, count={}",
            event.cheat_type,
            event.detection_count
        );
    }
}

/// Global instance of the callback registry
///
/// Static to allow cheat detection from anywhere in the code.
static CALLBACK_REGISTRY: CheatCallbackRegistry = CheatCallbackRegistry::new();

/// Hardware ID generator with caching
///
/// Uses machineid-rs to generate a unique hardware identifier.
/// The ID is generated once and cached for subsequent calls.
static HARDWARE_ID: Lazy<String> = Lazy::new(|| {
    log::info!("Generating hardware ID...");

    let result = generate_hardware_id();

    match result {
        Ok(hwid) => {
            log::info!("Hardware ID generated successfully: {}", hwid);
            hwid
        }
        Err(e) => {
            log::error!(
                "Failed to generate hardware ID: {}. Using fallback UUID.",
                e
            );
            // Fallback to UUID if HWID generation fails
            uuid::Uuid::now_v7().to_string()
        }
    }
});

/// Generate hardware ID using machineid-rs
///
/// Combines system ID, CPU ID, and drive serial for unique identification.
/// Uses MD5 encryption (128-bit) for UUID compatibility.
fn generate_hardware_id() -> Result<String, Box<dyn std::error::Error>> {
    let mut builder = IdBuilder::new(Encryption::MD5);
    builder
        .add_component(HWIDComponent::SystemID)
        .add_component(HWIDComponent::CPUID)
        .add_component(HWIDComponent::DriveSerial);

    // In production, this should use a secret key from config
    // For now, using a static key (should be made configurable)
    let hwid = builder.build("maxion-secret-key")?;

    Ok(hwid)
}

// ============================================================================
// FFI Functions for Unity
// ============================================================================

/// Register a cheat callback function from Unity
///
/// # Arguments
/// * `callback` - Optional callback function pointer, null to unregister
///
/// # Safety
/// This function modifies global state and should be called from Unity during initialization.
#[no_mangle]
pub extern "C" fn maxion_register_cheat_callback(callback: Option<CheatCallback>) {
    unsafe {
        CALLBACK_REGISTRY.register(callback);
    }
}

/// Get hardware ID as UTF-8 string
///
/// # Arguments
/// * `ptr` - Output parameter for pointer to HWID bytes (null to skip)
/// * `len` - Output parameter for HWID length (null to skip)
///
/// # Safety
/// The returned pointer is valid for the lifetime of the program.
/// Do not free the memory. The function dereferences raw pointers passed by the caller.
#[no_mangle]
pub unsafe extern "C" fn maxion_get_hardware_id(ptr: *mut *const u8, len: *mut usize) {
    let hwid = HARDWARE_ID.as_bytes();
    if !ptr.is_null() {
        *ptr = hwid.as_ptr();
    }
    if !len.is_null() {
        *len = hwid.len();
    }
}

/// Check if a cheat callback is registered
///
/// # Returns
/// * `true` if callback is registered, `false` otherwise
#[no_mangle]
pub extern "C" fn maxion_has_cheat_callback() -> bool {
    CALLBACK_REGISTRY.has_callback()
}

/// Get cheat type as integer
///
/// # Arguments
/// * `cheat_type` - Cheat type enum
///
/// # Returns
/// * Integer representation of cheat type
#[no_mangle]
pub extern "C" fn maxion_cheat_type_to_int(cheat_type: CheatType) -> i32 {
    cheat_type.to_int()
}

// ============================================================================
// Internal Functions
// ============================================================================

/// Report a cheat detection and invoke callback if registered
///
/// This is the main entry point for cheat detection notifications.
///
/// # Arguments
/// * `cheat_type` - Type of cheat detected
/// * `detection_count` - Number of times this cheat has been detected
pub fn report_cheat_with_callback(cheat_type: CheatType, detection_count: u32) {
    let event = CheatEvent::new(cheat_type, detection_count);

    log::warn!(
        "Cheat detected: type={:?}, count={}, timestamp={}",
        event.cheat_type,
        event.detection_count,
        event.timestamp
    );

    unsafe {
        CALLBACK_REGISTRY.invoke(&event);
    }
}

/// Get cached hardware ID as string
///
/// # Returns
/// * Reference to the cached hardware ID string
#[inline]
pub fn get_hardware_id() -> &'static str {
    HARDWARE_ID.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cheat_type_conversion() {
        assert_eq!(CheatType::MemoryTampering, CheatType::from_int(0));
        assert_eq!(CheatType::ValueFreeze, CheatType::from_int(1));
        assert_eq!(CheatType::IntegrityViolation, CheatType::from_int(2));
        assert_eq!(CheatType::Unknown, CheatType::from_int(99));
        assert_eq!(CheatType::Unknown, CheatType::from_int(999));

        assert_eq!(0, CheatType::MemoryTampering.to_int());
        assert_eq!(1, CheatType::ValueFreeze.to_int());
        assert_eq!(2, CheatType::IntegrityViolation.to_int());
        assert_eq!(99, CheatType::Unknown.to_int());
    }

    #[test]
    fn test_cheat_event_creation() {
        let event = CheatEvent::new(CheatType::MemoryTampering, 5);
        assert_eq!(event.cheat_type, CheatType::MemoryTampering);
        assert_eq!(event.detection_count, 5);
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_callback_registry_registration() {
        unsafe {
            // Initially no callback
            assert!(!CALLBACK_REGISTRY.has_callback());

            // Register a dummy callback
            extern "C" fn dummy_callback(_: i32, _: *const u8, _: usize, _: u64, _: u32) {}

            CALLBACK_REGISTRY.register(Some(dummy_callback));
            assert!(CALLBACK_REGISTRY.has_callback());

            // Unregister
            CALLBACK_REGISTRY.unregister();
            assert!(!CALLBACK_REGISTRY.has_callback());
        }
    }

    #[test]
    fn test_hardware_id_generation() {
        let hwid = get_hardware_id();

        // HWID should be a valid MD5 hash (32 hex characters)
        assert_eq!(hwid.len(), 32);
        assert!(hwid.chars().all(|c| c.is_ascii_hexdigit()));

        // Should be idempotent (cached)
        let hwid2 = get_hardware_id();
        assert_eq!(hwid, hwid2);
    }

    #[test]
    fn test_report_cheat_with_callback() {
        unsafe {
            extern "C" fn test_callback(
                cheat_type: i32,
                hwid_ptr: *const u8,
                hwid_len: usize,
                timestamp: u64,
                detection_count: u32,
            ) {
                // Verify callback parameters
                assert_eq!(cheat_type, 0); // MemoryTampering
                assert!(!hwid_ptr.is_null());
                assert_eq!(hwid_len, 32); // MD5 hash length
                assert!(timestamp > 0);
                assert_eq!(detection_count, 1);
            }

            // Register test callback
            CALLBACK_REGISTRY.register(Some(test_callback));

            // Report cheat (should invoke callback)
            report_cheat_with_callback(CheatType::MemoryTampering, 1);

            // Cleanup
            CALLBACK_REGISTRY.unregister();
        }
    }

    #[test]
    fn test_callback_without_registration() {
        // Ensure no callback is registered (test isolation)
        CALLBACK_REGISTRY.unregister();

        // Should not crash when no callback is registered
        report_cheat_with_callback(CheatType::Unknown, 0);
    }

    #[test]
    fn test_maxion_get_hardware_id() {
        let mut ptr: *const u8 = std::ptr::null();
        let mut len: usize = 0;

        // Call FFI function
        unsafe {
            maxion_get_hardware_id(&mut ptr, &mut len);
        }

        // Verify results
        assert!(!ptr.is_null(), "HWID pointer should not be null");
        assert_eq!(len, 32, "HWID length should be 32 (MD5 hash)");

        // Verify the data matches expected HWID
        let hwid_slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        let hwid_str = std::str::from_utf8(hwid_slice).unwrap();
        let expected_hwid = get_hardware_id();
        assert_eq!(hwid_str, expected_hwid, "HWID should match cached value");

        // Test with null pointer (should not crash)
        unsafe {
            maxion_get_hardware_id(std::ptr::null_mut(), &mut len);
            maxion_get_hardware_id(&mut ptr, std::ptr::null_mut());
            maxion_get_hardware_id(std::ptr::null_mut(), std::ptr::null_mut());
        }
    }

    #[test]
    fn test_notify_unity_action() {
        use crate::protected::{CheatAction, CheatDetector};

        static CALLBACK_INVOKED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);

        // Only run this test in debug builds since take_action is only called then
        #[cfg(debug_assertions)]
        unsafe {
            extern "C" fn notify_callback(
                cheat_type: i32,
                hwid_ptr: *const u8,
                hwid_len: usize,
                timestamp: u64,
                detection_count: u32,
            ) {
                // Verify callback parameters
                assert_eq!(cheat_type, CheatType::MemoryTampering.to_int());
                assert!(!hwid_ptr.is_null());
                assert_eq!(hwid_len, 32);
                assert!(timestamp > 0);
                assert_eq!(detection_count, 1);

                // Mark callback as invoked
                CALLBACK_INVOKED.store(true, std::sync::atomic::Ordering::Release);
            }

            // Ensure clean state (test isolation)
            CALLBACK_REGISTRY.unregister();
            CALLBACK_INVOKED.store(false, std::sync::atomic::Ordering::Release);

            // Register callback
            CALLBACK_REGISTRY.register(Some(notify_callback));

            // Create detector with NotifyUnity action
            let mut detector = CheatDetector::new();
            detector.action = CheatAction::NotifyUnity;

            // Report cheat (should invoke callback via take_action)
            detector.report_cheat();

            // Verify callback was invoked
            assert!(
                CALLBACK_INVOKED.load(std::sync::atomic::Ordering::Acquire),
                "Callback should have been invoked with NotifyUnity action"
            );

            // Cleanup
            CALLBACK_REGISTRY.unregister();
        }

        #[cfg(not(debug_assertions))]
        {
            // In release builds, take_action is not called, so skip this test
            println!("Skipping test_notify_unity_action in release build (take_action not called)");
        }
    }
}

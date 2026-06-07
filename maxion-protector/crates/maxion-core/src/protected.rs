//! # Honeypot Anti-Cheat Protection Module
//!
//! This module provides protected value types with honeypot detection to defend
//! against memory tampering by cheat engines (Cheat Engine, ArtMoney, etc.).
//!
//! ## How It Works
//!
//! Each `Protected<T>` value contains:
//! - A **trap value**: Plain text, easily searchable by Cheat Engine
//! - A **real value**: Encrypted/obfuscated, hard to find
//! - An **encryption key**: Rotated on writes to prevent freezing
//!
//! When Cheat Engine modifies the trap value, the next read detects the mismatch
//! between trap and real values, triggering a cheat detection response.
//!
//! ## Usage Example
//!
//! ```rust
//! use maxion_core::Protected;
//!
//! // Create a protected health value
//! let health = Protected::new(100i32);
//!
//! // Read the value (automatically checks for tampering)
//! let current = health.get();
//!
//! // Update the value (rotates encryption key)
//! health.set(75);
//! ```
//!
//! ## Thread Safety
//!
//! Use `ProtectedSync<T>` for thread-safe access across multiple threads.

use once_cell::sync::OnceCell;
use rand::Rng;
use std::cell::UnsafeCell;
use std::ptr::{read_volatile, write_volatile};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

// =============================================================================
// Cheat Detection Configuration
// =============================================================================

/// Actions to take when cheat detection is triggered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheatAction {
    /// Panic immediately (development/testing)
    Panic,
    /// Log the detection (production)
    #[default]
    Log,
    /// Crash randomly to confuse cheaters
    RandomCrash,
    /// Flag account for banning
    FlagAccount,
    /// Notify Unity via callback (allows game to decide what to do)
    NotifyUnity,
}

/// Global configuration for trap checking
pub struct TrapConfig {
    enabled: AtomicBool,
}

impl TrapConfig {
    const fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
        }
    }

    /// Enable or disable trap checking
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    /// Check if trap checking is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Reset trap checking to default enabled state
    /// Used for test isolation
    pub fn reset(&self) {
        self.enabled.store(true, Ordering::Release);
    }
}

/// Global trap configuration instance
static TRAP_CONFIG: OnceCell<TrapConfig> = OnceCell::new();

/// Global silent cheat flag for production-safe detection
/// This flag is set when tampering is detected but does NOT crash
static CHEAT_FLAG: AtomicBool = AtomicBool::new(false);

/// Cheat detection details for delayed ban processing
static CHEAT_DETECTION_LOG: Mutex<Vec<CheatDetectionEvent>> = Mutex::new(Vec::new());

/// Track when the first cheat was detected (for delayed banning)
static FIRST_CHEAT_TIME: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// Cheat detection event for logging
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields are used in logging but compiler can't detect usage via mutex
struct CheatDetectionEvent {
    timestamp: std::time::Instant,
    value_address: usize,
    detection_count: u32,
}

/// Initialize trap configuration (called automatically)
fn get_trap_config() -> &'static TrapConfig {
    TRAP_CONFIG.get_or_init(TrapConfig::new)
}

/// Enable or disable trap checking globally
///
/// # Arguments
///
/// * `enabled` - If true, trap checking is enabled (default). If false, trap checking is disabled.
///
/// # Example
///
/// ```rust
/// use maxion_core::protected;
///
/// // Disable trap checking for performance
/// protected::set_trap_enabled(false);
///
/// // Re-enable trap checking for security
/// protected::set_trap_enabled(true);
/// ```
pub fn set_trap_enabled(enabled: bool) {
    get_trap_config().set_enabled(enabled);
}

/// Check if trap checking is currently enabled
///
/// # Returns
///
/// true if trap checking is enabled, false otherwise
pub fn is_trap_enabled() -> bool {
    get_trap_config().is_enabled()
}

/// Reset trap checking to default enabled state
///
/// This is primarily used for test isolation to ensure each test
/// starts with a clean trap configuration state.
///
/// # Example
///
/// ```rust
/// use maxion_core::protected;
///
/// // Reset to default state at the start of a test
/// protected::reset_trap_state();
/// ```
pub fn reset_trap_state() {
    get_trap_config().reset();
}

/// Cheat detection handler
///
/// This handles cheat detection with configurable actions.
/// For now, we use a simple logging-based approach.
#[allow(dead_code)] // `action` is only used in debug builds via `take_action`
pub struct CheatDetector {
    pub(crate) action: CheatAction,
    detection_count: AtomicU32,
    max_detections: u32,
}

impl CheatDetector {
    /// Create a new cheat detector
    pub const fn new() -> Self {
        Self {
            action: CheatAction::Log,
            detection_count: AtomicU32::new(0),
            max_detections: 10,
        }
    }

    /// Initialize cheat detection with custom settings
    pub fn init(action: CheatAction, max_detections: u32) {
        log::warn!(
            "Cheat detection initialized: action={:?}, max_detections={}",
            action,
            max_detections
        );
    }

    /// Report a detected cheat attempt
    pub fn report_cheat(&self) {
        let count = self.detection_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Set global silent flag (production-safe)
        CHEAT_FLAG.store(true, Ordering::Relaxed);

        log::warn!(
            "⚠️ CHEAT DETECTED! Silent flag set. Detection #{}/{}",
            count,
            self.max_detections
        );

        // Only take immediate action in debug builds or if configured
        #[cfg(debug_assertions)]
        {
            self.take_action(count);
        }

        #[cfg(not(debug_assertions))]
        {
            // In production, only log - let delayed ban handle it
            if count == 1 {
                let mut first_time = FIRST_CHEAT_TIME.lock().unwrap();
                if first_time.is_none() {
                    *first_time = Some(std::time::Instant::now());
                }
            }
        }
    }

    /// Take action based on detection configuration
    #[allow(dead_code)] // Only called in debug builds from `report_cheat`
    fn take_action(&self, detection_count: u32) {
        match self.action {
            CheatAction::Panic => {
                panic!("Cheat detected! Memory tampering detected at protected value.");
            }
            CheatAction::Log => {
                // Already logged above
            }
            CheatAction::RandomCrash => {
                // Random delay to confuse cheater
                let delay = if rand::thread_rng().gen_bool(0.5) {
                    Some(10)
                } else {
                    None
                };

                if let Some(ms) = delay {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                }

                // Random panic to confuse cheater
                if rand::thread_rng().gen_bool(0.3) {
                    panic!("Memory corruption detected");
                }
            }
            CheatAction::FlagAccount => {
                log::error!(
                    "🚨 ACCOUNT FLAGGED FOR CHEATING! Detection #{}",
                    detection_count
                );
                // In production, this would send to server
            }
            CheatAction::NotifyUnity => {
                // Invoke callback to let Unity decide what to do
                crate::cheat_callback::report_cheat_with_callback(
                    crate::cheat_callback::CheatType::MemoryTampering,
                    detection_count,
                );
            }
        }
    }

    /// Get current detection count
    pub fn detection_count(&self) -> u32 {
        self.detection_count.load(Ordering::Relaxed)
    }

    /// Reset detection count (for testing)
    #[cfg(test)]
    pub fn reset(&mut self) {
        self.detection_count.store(0, Ordering::SeqCst);
    }
}

impl Default for CheatDetector {
    fn default() -> Self {
        // Production-safe default: Log action instead of panic
        #[cfg(debug_assertions)]
        {
            Self::new()
        }

        #[cfg(not(debug_assertions))]
        {
            Self {
                action: CheatAction::Log, // Safe default for production
                detection_count: AtomicU32::new(0),
                max_detections: 5,
            }
        }
    }
}

/// Report a detected cheat attempt (convenience function)
///
/// # Safety
///
/// This function uses silent flagging for production builds to avoid revealing
/// detection location to hackers. In debug builds, it may panic for easier
/// debugging.
pub fn report_cheat() {
    // Set global silent flag (production-safe)
    CHEAT_FLAG.store(true, Ordering::Relaxed);

    // Log() detection (doesn't crash)
    log::warn!("⚠️ CHEAT DETECTED! Silent flag set. Memory tampering detected.");

    // Record detection time for delayed ban
    let mut first_time = FIRST_CHEAT_TIME.lock().unwrap_or_else(|e| e.into_inner());
    if first_time.is_none() {
        *first_time = Some(std::time::Instant::now());
        log::warn!(
            "First cheat detected at {:?}. Will trigger action after grace period.",
            first_time.unwrap()
        );
    }

    // Only panic in debug builds for easier testing
    // Check if trap checking is enabled to allow tests to disable panics
    #[cfg(debug_assertions)]
    {
        if get_trap_config().is_enabled() {
            panic!("⚠️ CHEAT DETECTED! (Debug build - would silently flag in production)");
        }
    }
}

/// Check if any cheats have been detected
pub fn has_cheat_detected() -> bool {
    CHEAT_FLAG.load(Ordering::Relaxed)
}

/// Get's time elapsed since first cheat detection
pub fn time_since_first_cheat() -> Option<std::time::Duration> {
    match FIRST_CHEAT_TIME.lock() {
        Ok(time) => time.map(|t| t.elapsed()),
        Err(e) => e.into_inner().map(|t| t.elapsed()),
    }
}

/// Clear cheat flag (for testing or after processing ban)
#[cfg(test)]
pub fn clear_cheat_flag() {
    CHEAT_FLAG.store(false, Ordering::Relaxed);
    // Handle poisoned locks gracefully for test isolation
    match FIRST_CHEAT_TIME.lock() {
        Ok(mut time) => *time = None,
        Err(e) => *e.into_inner() = None,
    }
    match CHEAT_DETECTION_LOG.lock() {
        Ok(mut log) => log.clear(),
        Err(e) => e.into_inner().clear(),
    }
}

/// Check if delayed ban should be triggered (e.g., after 5 minutes)
///
/// # Arguments
///
/// * `delay_ms` - Delay in milliseconds before triggering ban (default: 300000 = 5 minutes)
///
/// # Returns
///
/// true if ban should be triggered, false otherwise
pub fn should_trigger_ban(delay_ms: u64) -> bool {
    if !CHEAT_FLAG.load(Ordering::Relaxed) {
        return false;
    }

    if let Some(first_time) = *FIRST_CHEAT_TIME.lock().unwrap_or_else(|e| e.into_inner()) {
        let elapsed = first_time.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;
        if elapsed_ms >= delay_ms {
            log::warn!("Triggering ban after {:?} delay", elapsed);
            return true;
        }
    }

    false
}

/// Log a specific cheat detection event
pub fn log_cheat_detection(value_address: usize, detection_count: u32) {
    let event = CheatDetectionEvent {
        timestamp: std::time::Instant::now(),
        value_address,
        detection_count,
    };

    // Use match to avoid potential deadlock with if_let_mutex
    match CHEAT_DETECTION_LOG.lock() {
        Ok(mut log) => {
            log.push(event);
            log::warn!(
                "Cheat detection logged: address=0x{:x}, count={}",
                value_address,
                detection_count
            );
        }
        Err(e) => {
            // Lock was poisoned, recover and log
            let mut log = e.into_inner();
            log.push(event);
            log::warn!(
                "Cheat detection logged (recovered from poisoned lock): address=0x{:x}, count={}",
                value_address,
                detection_count
            );
        }
    }
}

// =============================================================================
// Protectable Trait
// =============================================================================

/// Trait for types that can be protected with honeypot encoding
pub trait Protectable: Copy + PartialEq + std::fmt::Debug {
    /// Encode value to u64 using XOR encryption
    fn encode(&self, key: u64) -> u64;

    /// Decode u64 to value using XOR decryption
    fn decode(encoded: u64, key: u64) -> Self;
}

// Implement Protectable for common types
impl Protectable for i32 {
    fn encode(&self, key: u64) -> u64 {
        (*self as u64) ^ key
    }

    fn decode(encoded: u64, key: u64) -> i32 {
        (encoded ^ key) as i32
    }
}

impl Protectable for f32 {
    fn encode(&self, key: u64) -> u64 {
        // Convert float to bits, then XOR encode
        (self.to_bits() as u64) ^ key
    }

    fn decode(encoded: u64, key: u64) -> f32 {
        // XOR decode, then convert bits back to float
        f32::from_bits((encoded ^ key) as u32)
    }
}

impl Protectable for u32 {
    fn encode(&self, key: u64) -> u64 {
        (*self as u64) ^ key
    }

    fn decode(encoded: u64, key: u64) -> u32 {
        (encoded ^ key) as u32
    }
}

impl Protectable for i64 {
    fn encode(&self, key: u64) -> u64 {
        (*self as u64) ^ key
    }

    fn decode(encoded: u64, key: u64) -> i64 {
        (encoded ^ key) as i64
    }
}

impl Protectable for u64 {
    fn encode(&self, key: u64) -> u64 {
        *self ^ key
    }

    fn decode(encoded: u64, key: u64) -> u64 {
        encoded ^ key
    }
}

// Implement Protectable for tuples (e.g., position coordinates)
// Note: This implementation uses reduced precision (16 bits per float) to fit in u64
// Suitable for game positions where extreme precision isn't required
impl Protectable for (f32, f32, f32) {
    fn encode(&self, key: u64) -> u64 {
        // Convert floats to integers with reduced precision
        // Scale by 100 to preserve 2 decimal places, fit in i16 range
        let x_scaled = (self.0 * 100.0) as i32 as i16 as u16;
        let y_scaled = (self.1 * 100.0) as i32 as i16 as u16;
        let z_scaled = (self.2 * 100.0) as i32 as i16 as u16;

        // Encode each component
        let x_enc = ((x_scaled as u64) ^ key) & 0xFFFF;
        let y_enc = ((y_scaled as u64) ^ (key >> 16)) & 0xFFFF;
        let z_enc = ((z_scaled as u64) ^ (key >> 32)) & 0xFFFF;

        // Combine: z (16 bits) | y (16 bits) | x (16 bits) | padding (16 bits)
        (x_enc) | (y_enc << 16) | (z_enc << 32)
    }

    fn decode(encoded: u64, key: u64) -> (f32, f32, f32) {
        // Extract components
        let x_enc = encoded & 0xFFFF;
        let y_enc = (encoded >> 16) & 0xFFFF;
        let z_enc = (encoded >> 32) & 0xFFFF;

        // Decode each component
        let x_scaled = ((x_enc ^ key) as u16) as i16 as i32;
        let y_scaled = ((y_enc ^ (key >> 16)) as u16) as i16 as i32;
        let z_scaled = ((z_enc ^ (key >> 32)) as u16) as i16 as i32;

        // Convert back to floats
        let x = x_scaled as f32 / 100.0;
        let y = y_scaled as f32 / 100.0;
        let z = z_scaled as f32 / 100.0;

        (x, y, z)
    }
}

// =============================================================================
// Protected<T> - Main Honeypot Implementation
// =============================================================================

/// Protected value with honeypot anti-cheat detection
///
/// This struct stores a value in both plain text (trap) and encrypted (real) forms.
/// Cheat Engine typically finds the plain text trap value and modifies it.
/// The next read detects the mismatch and triggers cheat detection.
///
/// # Type Parameters
///
/// * `T` - The type of value to protect (must implement `Protectable`)
///
/// # Thread Safety
///
/// This type is NOT thread-safe by default. Use `ProtectedSync<T>` for
/// multi-threaded scenarios.
///
/// # Example
///
/// ```rust
/// use maxion_core::Protected;
///
/// let health = Protected::new(100i32);
/// assert_eq!(health.get(), 100);
///
/// health.set(75);
/// assert_eq!(health.get(), 75);
/// ```
pub struct Protected<T: Protectable> {
    /// Honeypot value - easily searchable by Cheat Engine
    /// Uses UnsafeCell to prevent compiler optimization
    pub trap_value: UnsafeCell<T>,

    /// Real value - obfuscated (XOR-encoded with random key)
    /// Uses UnsafeCell for interior mutability
    pub real_value_obfuscated: UnsafeCell<u64>,

    /// Encryption key - rotated on writes to prevent freezing
    /// Uses UnsafeCell for interior mutability
    pub key: UnsafeCell<u64>,
}

impl<T: Protectable> Protected<T> {
    /// Create new protected value with initial value
    ///
    /// # Arguments
    ///
    /// * `val` - The initial value to protect
    ///
    /// # Example
    ///
    /// ```rust
    /// use maxion_core::Protected;
    ///
    /// let health = Protected::new(100i32);
    /// ```
    pub fn new(val: T) -> Self {
        let mut rng = rand::thread_rng();
        let key: u64 = rng.gen();

        let real_encoded = val.encode(key);

        Self {
            trap_value: UnsafeCell::new(val), // Plain text honeypot
            real_value_obfuscated: UnsafeCell::new(real_encoded), // Encrypted real value
            key: UnsafeCell::new(key),        // Encryption key
        }
    }

    /// Get the protected value, checking for tampering
    ///
    /// This method:
    /// 1. Reads the encrypted real value and decrypts it
    /// 2. Reads the plain text trap value (volatile read)
    /// 3. Compares them - if mismatch, cheat detected!
    /// 4. Returns the real value
    ///
    /// # Panics
    ///
    /// Panics if cheat detection is configured to panic and tampering is detected
    ///
    /// # Example
    ///
    /// ```rust
    /// use maxion_core::Protected;
    ///
    /// let health = Protected::new(100i32);
    /// assert_eq!(health.get(), 100);
    /// ```
    pub fn get(&self) -> T {
        // Decrypt real value
        let real_val = unsafe {
            let key = read_volatile(self.key.get());
            let real_enc = read_volatile(self.real_value_obfuscated.get());
            T::decode(real_enc, key)
        };

        // Check for tampering (only if trap checking is enabled)
        if get_trap_config().is_enabled() {
            // Volatile read of trap value (prevents optimization) - only when enabled
            let trap_val = unsafe { read_volatile(self.trap_value.get()) };

            if real_val != trap_val {
                report_cheat();
            }
        }

        real_val
    }

    /// Set a new value (rotates encryption key)
    ///
    /// This method:
    /// 1. Generates a new random key
    /// 2. Encrypts the new value with the new key
    /// 3. Updates the encrypted real value
    /// 4. Updates the plain text trap value (volatile write)
    ///
    /// Key rotation prevents cheaters from "freezing" the encrypted value.
    ///
    /// # Arguments
    ///
    /// * `val` - The new value to set
    ///
    /// # Example
    ///
    /// ```rust
    /// use maxion_core::Protected;
    ///
    /// let health = Protected::new(100i32);
    /// health.set(75);
    /// assert_eq!(health.get(), 75);
    /// ```
    pub fn set(&self, val: T) {
        // Generate new random key (rotation prevents freezing)
        let new_key = rand::thread_rng().gen::<u64>();

        // Encrypt with new key
        let real_encoded = val.encode(new_key);

        // Update values (volatile writes prevent optimization)
        unsafe {
            write_volatile(self.real_value_obfuscated.get(), real_encoded);
            write_volatile(self.key.get(), new_key);
            write_volatile(self.trap_value.get(), val);
        }
    }

    /// Get the protected value without checking for tampering
    ///
    /// # Safety
    ///
    /// This bypasses cheat detection. Only use for testing or when you
    /// intentionally want to ignore tampering.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maxion_core::Protected;
    ///
    /// let health = Protected::new(100i32);
    /// let val = unsafe { health.get_unchecked() };
    /// ```
    pub unsafe fn get_unchecked(&self) -> T {
        let key = read_volatile(self.key.get());
        let real_enc = read_volatile(self.real_value_obfuscated.get());
        T::decode(real_enc, key)
    }

    /// Set only the real value (not the trap)
    ///
    /// # Safety
    ///
    /// This bypasses the trap value update. Only use for testing.
    ///
    /// # Arguments
    ///
    /// * `val` - The new value to set
    pub unsafe fn set_real_only(&self, val: T) {
        let key = read_volatile(self.key.get());
        let real_encoded = val.encode(key);
        write_volatile(self.real_value_obfuscated.get(), real_encoded);
    }
}

// Implement Debug for Protected<T>
impl<T: Protectable> std::fmt::Debug for Protected<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Get value without triggering cheat detection in debug mode
        let val = unsafe { self.get_unchecked() };
        let key = unsafe { read_volatile(self.key.get()) };
        f.debug_struct("Protected")
            .field("value", &val)
            .field("key", &key)
            .finish()
    }
}

// =============================================================================
// ProtectedSync<T> - Thread-Safe Implementation
// =============================================================================

/// Thread-safe protected value with honeypot detection
///
/// This is a thread-safe wrapper around `Protected<T>` using `std::sync::Mutex`.
/// Use this when the protected value needs to be accessed from multiple threads.
///
/// # Type Parameters
///
/// * `T` - The type of value to protect (must implement `Protectable`)
///
/// # Example
///
/// ```rust
/// use maxion_core::ProtectedSync;
/// use std::thread;
/// use std::sync::Arc;
///
/// let health = Arc::new(ProtectedSync::new(100i32));
///
/// let health_clone = Arc::clone(&health);
/// thread::spawn(move || {
///     health_clone.set(75);
/// }).join().unwrap();
///
/// assert_eq!(health.get(), 75);
/// ```
pub struct ProtectedSync<T: Protectable> {
    inner: std::sync::Mutex<Protected<T>>,
}

impl<T: Protectable> ProtectedSync<T> {
    /// Create new thread-safe protected value
    ///
    /// # Arguments
    ///
    /// * `val` - The initial value to protect
    pub fn new(val: T) -> Self {
        Self {
            inner: std::sync::Mutex::new(Protected::new(val)),
        }
    }

    /// Get the protected value (thread-safe)
    pub fn get(&self) -> T {
        let guard = self.inner.lock().unwrap();
        guard.get()
    }

    /// Set a new value (thread-safe)
    pub fn set(&self, val: T) {
        let guard = self.inner.lock().unwrap();
        guard.set(val);
    }
}

// Implement Clone for ProtectedSync<T>
impl<T: Protectable> Clone for ProtectedSync<T> {
    fn clone(&self) -> Self {
        let val = self.get();
        Self::new(val)
    }
}

// Implement Debug for ProtectedSync<T>
impl<T: Protectable> std::fmt::Debug for ProtectedSync<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let guard = self.inner.lock().unwrap();
        write!(f, "ProtectedSync({:?})", *guard)
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::thread;

    #[test]
    fn test_protected_get_set() {
        let val = Protected::new(100i32);

        assert_eq!(val.get(), 100);

        val.set(200);
        assert_eq!(val.get(), 200);

        val.set(-50);
        assert_eq!(val.get(), -50);
    }

    #[test]
    fn test_float_encoding() {
        let val = Protected::new(100.5f32);

        assert!((val.get() - 100.5).abs() < 0.0001);

        val.set(200.75);
        assert!((val.get() - 200.75).abs() < 0.0001);
    }

    #[test]
    fn test_tuple_encoding() {
        let val = Protected::new((1.0f32, 2.0f32, 3.0f32));

        let result = val.get();
        // Note: Reduced precision means we check within 0.01 tolerance (2 decimal places)
        assert!(
            (result.0 - 1.0).abs() < 0.01,
            "x component mismatch: got {}",
            result.0
        );
        assert!(
            (result.1 - 2.0).abs() < 0.01,
            "y component mismatch: got {}",
            result.1
        );
        assert!(
            (result.2 - 3.0).abs() < 0.01,
            "z component mismatch: got {}",
            result.2
        );

        // Test with different values (with 2 decimal precision)
        val.set((10.5f32, -20.75f32, 30.25f32));
        let result2 = val.get();
        assert!((result2.0 - 10.5).abs() < 0.01);
        assert!((result2.1 - (-20.75)).abs() < 0.01);
        assert!((result2.2 - 30.25).abs() < 0.01);
    }

    #[test]
    fn test_honeypot_detection() {
        let detector = CheatDetector::new();
        CheatDetector::init(CheatAction::Log, 10);

        let val = Protected::new(100i32);

        // Verify initial state
        assert_eq!(val.get(), 100);
        assert_eq!(detector.detection_count(), 0);

        // Simulate Cheat Engine modifying trap value
        unsafe {
            write_volatile(val.trap_value.get(), 999);
        }

        // In debug builds, this will panic (expected behavior for testing)
        // In release builds, this should detect tampering and log (but not panic with Log action)
        #[cfg(debug_assertions)]
        {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| val.get()));
            assert!(
                result.is_err(),
                "Should panic in debug mode when cheat detected"
            );
        }

        #[cfg(not(debug_assertions))]
        {
            let result = val.get();

            // Should return to real value (100), not to modified trap value (999)
            assert_eq!(result, 100);

            // Note: Detection is logged but we can't easily test to count without global state
            // The important thing is that get() doesn't panic and returns to real value
        }
    }

    #[test]
    fn test_freeze_detection() {
        let detector = CheatDetector::new();
        CheatDetector::init(CheatAction::Log, 10);

        let val = Protected::new(100i32);

        // Step 1: Cheat Engine reads to trap value (100)
        let frozen_trap_value = unsafe { read_volatile(val.trap_value.get()) };
        assert_eq!(frozen_trap_value, 100);

        // Step 2: Game updates to value (both trap and real change, key rotates)
        val.set(200);

        // Verify no detections yet
        assert_eq!(detector.detection_count(), 0);

        // Step 3: Cheat Engine writes back to frozen trap value
        // This simulates of "freeze" feature of Cheat Engine
        unsafe {
            write_volatile(val.trap_value.get(), frozen_trap_value);
        }

        // Step 4: Game reads to value
        // Now trap (100) != real (200 when decrypted)
        // In debug builds, this will panic (expected behavior for testing)
        #[cfg(debug_assertions)]
        {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| val.get()));
            assert!(
                result.is_err(),
                "Should panic in debug mode when freeze detected"
            );
        }

        #[cfg(not(debug_assertions))]
        {
            // The get() should detect this mismatch
            let result = val.get();

            // Should return to real value (200), not to frozen trap value (100)
            assert_eq!(result, 200);

            // Note: Detection is logged but we can't easily test to count without global state
            // The important thing is that get() doesn't panic and returns to real value
        }
    }

    #[test]
    fn test_thread_safe() {
        use std::sync::Arc;

        let val = Arc::new(ProtectedSync::new(0i32));

        // Each thread increments the value multiple times
        let handles: Vec<_> = (0..10)
            .map(|_i| {
                let val_clone = Arc::clone(&val);
                thread::spawn(move || {
                    for _ in 0..100 {
                        // Read current, increment, write back
                        let current = val_clone.get();
                        val_clone.set(current + 1);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Due to race conditions (read-modify-write without atomic operations),
        // we won't get exactly 1000, but we should get something > 0
        let final_value = val.get();
        assert!(
            final_value > 0,
            "Value should be incremented through concurrent access, got {}",
            final_value
        );
        // In practice, due to lost updates from non-atomic increments,
        // this will be much less than 1000, but demonstrates thread safety
    }

    #[test]
    fn test_get_unchecked() {
        let val = Protected::new(100i32);

        // get_unchecked should not trigger detection or check trap value
        let result = unsafe { val.get_unchecked() };
        assert_eq!(result, 100);

        // Corrupt trap value (simulate Cheat Engine modification)
        unsafe {
            write_volatile(val.trap_value.get(), 999);
        }

        // get_unchecked still returns real value without checking trap
        let result = unsafe { val.get_unchecked() };
        assert_eq!(result, 100);

        // Verify trap value is indeed corrupted
        let trap_val = unsafe { read_volatile(val.trap_value.get()) };
        assert_eq!(trap_val, 999);

        // get() would detect this mismatch and log, but we can't easily test that
        // The key point is get_unchecked() bypasses trap check entirely
    }

    #[test]
    #[serial]
    fn test_silent_cheat_flag() {
        clear_cheat_flag();

        // Initially, no cheat should be detected
        assert!(!has_cheat_detected());
        assert!(time_since_first_cheat().is_none());
        assert!(!should_trigger_ban(300000));

        // Manually set cheat flag and time to test the flag mechanism
        // (avoiding panic in debug mode)
        CHEAT_FLAG.store(true, Ordering::Relaxed);
        *FIRST_CHEAT_TIME.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(std::time::Instant::now());

        // Cheat flag should be set
        assert!(has_cheat_detected());

        // Time should be recorded
        assert!(time_since_first_cheat().is_some());

        // Ban should NOT trigger immediately (5 minute delay)
        assert!(!should_trigger_ban(300000));
    }

    #[test]
    #[serial]
    fn test_delayed_ban_timing() {
        clear_cheat_flag();

        // Manually set cheat flag and time to test timing
        // (avoiding panic in debug mode)
        CHEAT_FLAG.store(true, Ordering::Relaxed);
        match FIRST_CHEAT_TIME.lock() {
            Ok(mut time) => *time = Some(std::time::Instant::now()),
            Err(e) => *e.into_inner() = Some(std::time::Instant::now()),
        }

        assert!(has_cheat_detected());

        // Should not trigger immediately (100ms delay threshold)
        assert!(!should_trigger_ban(100));

        // Sleep for 120ms
        std::thread::sleep(std::time::Duration::from_millis(120));

        // Should trigger after 120ms (exceeds 100ms threshold)
        assert!(should_trigger_ban(100));
    }

    #[test]
    #[serial]
    fn test_multiple_cheat_detections() {
        clear_cheat_flag();

        // Manually set cheat flag and time to test multiple detections
        // (avoiding panic in debug mode)
        CHEAT_FLAG.store(true, Ordering::Relaxed);
        match FIRST_CHEAT_TIME.lock() {
            Ok(mut time) => *time = Some(std::time::Instant::now()),
            Err(e) => *e.into_inner() = Some(std::time::Instant::now()),
        }

        assert!(has_cheat_detected());

        // Record first time
        let first_time = time_since_first_cheat();

        // Simulate second detection (should not change first_cheat_time)
        CHEAT_FLAG.store(true, Ordering::Relaxed); // Set flag again

        assert!(has_cheat_detected());

        // Time should be equal or slightly greater (due to elapsed time), but never less
        let second_time = time_since_first_cheat();
        assert!(
            second_time >= first_time,
            "Second detection time should not be less than first: {:?} >= {:?}",
            second_time,
            first_time
        );
    }

    #[test]
    #[serial]
    fn test_cheat_detection_logging() {
        // Ensure clean state at start
        clear_cheat_flag();

        // Verify clean state
        assert!(!has_cheat_detected(), "Flag should start cleared");
        assert!(
            time_since_first_cheat().is_none(),
            "Time should start empty"
        );

        let val = Protected::new(100i32);

        // Log cheat detection (this just adds to the log, doesn't set global flag)
        log_cheat_detection(val.trap_value.get() as usize, 1);

        // Cheat flag should NOT be set by log_cheat_detection alone
        // (only set by report_cheat() which is called from get())
        assert!(
            !has_cheat_detected(),
            "log_cheat_detection should not set global flag"
        );

        // Verify log was populated
        assert!(
            CHEAT_DETECTION_LOG
                .try_lock()
                .map(|log| !log.is_empty())
                .unwrap_or(false),
            "Log should contain detection event"
        );
        assert!(
            time_since_first_cheat().is_none(),
            "log_cheat_detection should not set cheat time"
        );

        // Now manually set flag and time to test clearing
        CHEAT_FLAG.store(true, Ordering::Relaxed);
        match FIRST_CHEAT_TIME.lock() {
            Ok(mut time) => *time = Some(std::time::Instant::now()),
            Err(e) => *e.into_inner() = Some(std::time::Instant::now()),
        }
        assert!(
            has_cheat_detected(),
            "Flag should be set after manual assignment"
        );

        // Clear and verify
        clear_cheat_flag();
        assert!(!has_cheat_detected(), "Flag should be cleared");
        assert!(time_since_first_cheat().is_none(), "Time should be cleared");
        match CHEAT_DETECTION_LOG.lock() {
            Ok(log) => assert!(log.is_empty(), "Log should be cleared"),
            Err(e) => assert!(e.into_inner().is_empty(), "Log should be cleared"),
        }
    }

    #[test]
    #[serial]
    fn test_production_safe_default() {
        clear_cheat_flag();
        // Test that CheatDetector defaults are production-safe
        let _detector = CheatDetector::default();

        // In release builds, test that no panic occurs
        #[cfg(not(debug_assertions))]
        {
            // Release builds should use Log action (safe default)
            let val = Protected::new(100i32);

            // Simulate cheat - should NOT panic in release
            unsafe {
                write_volatile(val.trap_value.get(), 999);
            }

            // This should NOT panic in production
            let result = val.get();
            assert_eq!(result, 100);

            // Cheat flag should be set silently
            assert!(has_cheat_detected());
        }

        // In debug builds, we verify that panic occurs (expected behavior)
        #[cfg(debug_assertions)]
        {
            let val = Protected::new(100i32);

            // Simulate cheat - SHOULD panic in debug mode
            unsafe {
                write_volatile(val.trap_value.get(), 999);
            }

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| val.get()));
            assert!(
                result.is_err(),
                "Should panic in debug mode for easier testing"
            );
        }
    }
}

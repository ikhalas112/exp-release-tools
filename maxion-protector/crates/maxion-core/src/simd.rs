//! SIMD (Single Instruction, Multiple Data) detection and configuration
//!
//! This module provides runtime CPU feature detection to enable SIMD-accelerated
//! operations for compression, hashing, and encryption. It supports cross-platform
//! detection for x86_64 (SSE, AVX) and ARM64 (NEON) architectures.

use log::{debug, info, warn};
use std::fmt;

/// SIMD capability levels supported by the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SimdLevel {
    /// SIMD disabled
    Disabled,
    /// Scalar (no SIMD) operations
    Scalar,
    /// SSE4.1 (Intel/AMD, 2006+)
    Sse41,
    /// AVX2 (Intel/AMD, 2013+, Haswell+)
    Avx2,
    /// AVX-512 (Intel only, 2016+, Skylake-X+)
    Avx512,
    /// NEON (ARM64, Apple Silicon, AWS Graviton)
    Neon,
}

impl fmt::Display for SimdLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimdLevel::Disabled => write!(f, "Disabled"),
            SimdLevel::Scalar => write!(f, "Scalar (No SIMD)"),
            SimdLevel::Sse41 => write!(f, "SSE4.1"),
            SimdLevel::Avx2 => write!(f, "AVX2"),
            SimdLevel::Avx512 => write!(f, "AVX-512"),
            SimdLevel::Neon => write!(f, "NEON"),
        }
    }
}

impl SimdLevel {
    /// Returns the speed improvement multiplier expected for this SIMD level
    pub fn speed_multiplier(&self) -> f32 {
        match self {
            SimdLevel::Disabled => 1.0,
            SimdLevel::Scalar => 1.0,
            SimdLevel::Sse41 => 1.5,  // 50% faster
            SimdLevel::Avx2 => 2.5,   // 150% faster
            SimdLevel::Avx512 => 3.5, // 250% faster
            SimdLevel::Neon => 2.0,   // 100% faster
        }
    }
}

/// SIMD configuration for compression and hashing operations
#[derive(Debug, Clone, Copy)]
pub struct SimdConfig {
    /// Whether SIMD is enabled
    pub enabled: bool,
    /// Detected or forced SIMD level
    pub level: SimdLevel,
    /// Whether SIMD was explicitly forced (not auto-detected)
    pub force_enabled: bool,
}

impl SimdConfig {
    /// Create config with auto-detection (default)
    pub fn auto() -> Self {
        let level = detect_simd_level();
        let enabled = level != SimdLevel::Disabled && level != SimdLevel::Scalar;

        info!("Auto-detected SIMD level: {level}");

        Self {
            enabled,
            level,
            force_enabled: false,
        }
    }

    /// Create config with SIMD forced enabled
    pub fn enabled() -> Self {
        let level = detect_simd_level();

        if level == SimdLevel::Disabled || level == SimdLevel::Scalar {
            warn!("SIMD forced enabled but no SIMD support detected. May fail or use scalar fallback.");
        } else {
            info!("SIMD forced enabled: {level}");
        }

        Self {
            enabled: true,
            level,
            force_enabled: true,
        }
    }

    /// Create config with SIMD disabled
    pub fn disabled() -> Self {
        info!("SIMD disabled: using scalar operations only");

        Self {
            enabled: false,
            level: SimdLevel::Disabled,
            force_enabled: true,
        }
    }

    /// Check if AVX2 or better is available
    pub fn has_avx2(&self) -> bool {
        self.enabled && (self.level == SimdLevel::Avx2 || self.level == SimdLevel::Avx512)
    }

    /// Check if AVX-512 is available
    pub fn has_avx512(&self) -> bool {
        self.enabled && self.level == SimdLevel::Avx512
    }

    /// Check if SSE4.1 or better is available
    pub fn has_sse41(&self) -> bool {
        self.enabled && self.level != SimdLevel::Disabled && self.level != SimdLevel::Scalar
    }

    /// Check if NEON is available (ARM64)
    pub fn has_neon(&self) -> bool {
        self.enabled && self.level == SimdLevel::Neon
    }
}

impl fmt::Display for SimdConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.enabled {
            write!(
                f,
                "SIMD Enabled: {} ({}x speed)",
                self.level,
                self.level.speed_multiplier()
            )
        } else {
            write!(f, "SIMD Disabled")
        }
    }
}

/// Detect the highest available SIMD level on the current CPU
///
/// This performs runtime CPU feature detection using target-specific intrinsics.
/// It's safe to call and will gracefully handle unsupported architectures.
///
/// # Returns
///
/// The highest SIMD level supported by the current CPU
///
/// # Examples
///
/// ```rust
/// use maxion_core::simd;
///
/// let level = simd::detect_simd_level();
/// println!("Detected SIMD: {}", level);
/// ```
pub fn detect_simd_level() -> SimdLevel {
    #[cfg(target_arch = "x86_64")]
    {
        detect_x86_simd()
    }

    #[cfg(target_arch = "aarch64")]
    {
        detect_aarch64_simd()
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        debug!("Unsupported architecture for SIMD detection, using scalar");
        SimdLevel::Scalar
    }
}

/// Detect SIMD features on x86_64 architecture
#[cfg(target_arch = "x86_64")]
fn detect_x86_simd() -> SimdLevel {
    // Check for AVX-512 first (highest level)
    // This is a runtime check using the CPUID instruction
    if is_x86_feature_detected!("avx512f") {
        debug!("Detected AVX-512 support");
        return SimdLevel::Avx512;
    }

    // Check for AVX2
    if is_x86_feature_detected!("avx2") {
        debug!("Detected AVX2 support");
        return SimdLevel::Avx2;
    }

    // Check for SSE4.1
    if is_x86_feature_detected!("sse4.1") {
        debug!("Detected SSE4.1 support");
        return SimdLevel::Sse41;
    }

    debug!("No SIMD support detected on x86_64, using scalar");
    SimdLevel::Scalar
}

/// Detect SIMD features on ARM64 architecture
#[cfg(target_arch = "aarch64")]
fn detect_aarch64_simd() -> SimdLevel {
    // NEON is mandatory on ARM64, always available
    debug!("Detected NEON support");
    SimdLevel::Neon
}

/// Validate SIMD mode string and return appropriate config
///
/// # Arguments
///
/// * `mode` - String specifying SIMD mode: "auto", "on", or "off"
///
/// # Returns
///
/// Result containing SimdConfig or error string
///
/// # Examples
///
/// ```rust
/// use maxion_core::simd;
///
/// let config = simd::validate_simd_mode("auto").unwrap();
/// assert!(config.enabled);
/// ```
pub fn validate_simd_mode(mode: &str) -> Result<SimdConfig, String> {
    match mode.to_lowercase().as_str() {
        "auto" => Ok(SimdConfig::auto()),
        "on" | "enabled" | "true" => Ok(SimdConfig::enabled()),
        "off" | "disabled" | "false" => Ok(SimdConfig::disabled()),
        _ => Err(format!(
            "Invalid SIMD mode: '{}'. Expected 'auto', 'on', or 'off'",
            mode
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_level_ordering() {
        assert!(SimdLevel::Avx512 > SimdLevel::Avx2);
        assert!(SimdLevel::Avx2 > SimdLevel::Sse41);
        assert!(SimdLevel::Sse41 > SimdLevel::Scalar);
        assert!(SimdLevel::Scalar > SimdLevel::Disabled);
    }

    #[test]
    fn test_simd_level_speed_multiplier() {
        assert_eq!(SimdLevel::Scalar.speed_multiplier(), 1.0);
        assert_eq!(SimdLevel::Sse41.speed_multiplier(), 1.5);
        assert_eq!(SimdLevel::Avx2.speed_multiplier(), 2.5);
        assert_eq!(SimdLevel::Avx512.speed_multiplier(), 3.5);
        assert_eq!(SimdLevel::Neon.speed_multiplier(), 2.0);
    }

    #[test]
    fn test_simd_level_display() {
        assert_eq!(format!("{}", SimdLevel::Disabled), "Disabled");
        assert_eq!(format!("{}", SimdLevel::Scalar), "Scalar (No SIMD)");
        assert_eq!(format!("{}", SimdLevel::Sse41), "SSE4.1");
        assert_eq!(format!("{}", SimdLevel::Avx2), "AVX2");
        assert_eq!(format!("{}", SimdLevel::Avx512), "AVX-512");
        assert_eq!(format!("{}", SimdLevel::Neon), "NEON");
    }

    #[test]
    fn test_validate_simd_mode_valid() {
        // Valid modes
        assert!(validate_simd_mode("auto").is_ok());
        assert!(validate_simd_mode("on").is_ok());
        assert!(validate_simd_mode("off").is_ok());
        assert!(validate_simd_mode("enabled").is_ok());
        assert!(validate_simd_mode("disabled").is_ok());
        assert!(validate_simd_mode("true").is_ok());
        assert!(validate_simd_mode("false").is_ok());

        // Case insensitive
        assert!(validate_simd_mode("AUTO").is_ok());
        assert!(validate_simd_mode("ON").is_ok());
        assert!(validate_simd_mode("OFF").is_ok());
    }

    #[test]
    fn test_validate_simd_mode_invalid() {
        assert!(validate_simd_mode("invalid").is_err());
        assert!(validate_simd_mode("maybe").is_err());
        assert!(validate_simd_mode("1").is_err());
        assert!(validate_simd_mode("").is_err());
    }

    #[test]
    fn test_simd_config_disabled() {
        let config = SimdConfig::disabled();
        assert!(!config.enabled);
        assert_eq!(config.level, SimdLevel::Disabled);
        assert!(config.force_enabled);
    }

    #[test]
    fn test_simd_config_checks() {
        let config = SimdConfig::disabled();
        assert!(!config.has_avx2());
        assert!(!config.has_avx512());
        assert!(!config.has_sse41());
        assert!(!config.has_neon());
    }

    #[test]
    fn test_simd_config_display() {
        let config = SimdConfig::disabled();
        let display = format!("{}", config);
        assert!(display.contains("SIMD Disabled"));
    }

    #[test]
    fn test_detect_simd_level_runs() {
        // Just ensure it doesn't panic
        let level = detect_simd_level();
        // It should return something valid
        match level {
            SimdLevel::Disabled
            | SimdLevel::Scalar
            | SimdLevel::Sse41
            | SimdLevel::Avx2
            | SimdLevel::Avx512
            | SimdLevel::Neon => {
                // Valid value
            }
        }
    }
}

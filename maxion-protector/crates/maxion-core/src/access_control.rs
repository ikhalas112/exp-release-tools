//! Access Control Module
//!
//! Provides rate limiting and anti-scraping mechanisms to protect
//! against unauthorized bulk extraction of assets.

use crate::error::{Error, Result};
use std::time::{Duration, Instant};

/// Maximum number of sequential reads allowed within the delay window
/// This prevents rapid-fire access to assets that could indicate scraping
pub const MAX_SEQUENTIAL_READS: u32 = 100;

/// Minimum delay between reads to slow down scraping attempts
/// 50ms delay makes bulk extraction significantly slower
pub const ANTI_SCRAPE_DELAY_MS: u64 = 50;

/// Default timeout for read operations in seconds
pub const DEFAULT_READ_TIMEOUT_SECS: u64 = 30;

/// Access Control for preventing asset scraping and abuse
///
/// Implements a simple rate limiting mechanism that:
/// 1. Tracks sequential read operations
/// 2. Enforces minimum delays between reads
/// 3. Resets counters when sufficient time has passed
pub struct AccessControl {
    /// Number of consecutive reads within the delay window
    read_count: u32,

    /// Timestamp of the last read operation
    last_read_time: Option<Instant>,

    /// Maximum reads allowed before triggering a rate limit
    max_reads: u32,

    /// Minimum delay between reads in milliseconds
    delay_ms: u64,
}

impl AccessControl {
    /// Create a new access controller with default limits
    pub fn new() -> Self {
        Self {
            read_count: 0,
            last_read_time: None,
            max_reads: MAX_SEQUENTIAL_READS,
            delay_ms: ANTI_SCRAPE_DELAY_MS,
        }
    }

    /// Create a new access controller with custom limits
    pub fn with_limits(max_reads: u32, delay_ms: u64) -> Self {
        Self {
            read_count: 0,
            last_read_time: None,
            max_reads,
            delay_ms,
        }
    }

    /// Check if a read operation is allowed and update counters
    ///
    /// This method implements the rate limiting logic:
    /// 1. If enough time has passed since the last read, reset the counter
    /// 2. If too many reads occur within the delay window, deny access
    /// 3. Otherwise, increment the counter and allow access
    pub fn check_rate_limit(&mut self) -> Result<()> {
        let now = Instant::now();

        match self.last_read_time {
            Some(last_time) => {
                let elapsed = now.duration_since(last_time);

                if elapsed.as_millis() as u64 >= self.delay_ms {
                    // Enough time passed, reset counter
                    self.read_count = 0;
                }

                // Count this read
                self.read_count += 1;
                self.last_read_time = Some(now);

                if self.read_count > self.max_reads {
                    Err(Error::RateLimitExceeded {
                        count: self.read_count,
                        limit: self.max_reads,
                    })
                } else {
                    Ok(())
                }
            }
            None => {
                // First read, initialize timer
                self.last_read_time = Some(now);
                self.read_count = 1;
                Ok(())
            }
        }
    }

    /// Record a successful read operation
    ///
    /// This should be called after a read completes successfully
    /// to update the access control state.
    pub fn record_read(&mut self) {
        self.read_count += 1;
        self.last_read_time = Some(Instant::now());
    }

    /// Reset the access control state
    ///
    /// Useful for testing or when implementing a cooldown period.
    pub fn reset(&mut self) {
        self.read_count = 0;
        self.last_read_time = None;
    }

    /// Get the current read count
    pub fn read_count(&self) -> u32 {
        self.read_count
    }

    /// Get the time since the last read
    pub fn time_since_last_read(&self) -> Option<Duration> {
        self.last_read_time
            .map(|t| Instant::now().duration_since(t))
    }

    /// Check if currently rate limited
    pub fn is_rate_limited(&self) -> bool {
        self.read_count >= self.max_reads
    }

    /// Get the maximum allowed reads
    pub fn max_reads(&self) -> u32 {
        self.max_reads
    }

    /// Get the delay between reads
    pub fn delay_ms(&self) -> u64 {
        self.delay_ms
    }
}

impl Default for AccessControl {
    fn default() -> Self {
        Self::new()
    }
}

/// Access control statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct AccessStats {
    /// Total number of access attempts
    pub total_attempts: u64,

    /// Number of successful accesses
    pub successful_reads: u64,

    /// Number of rate limit violations
    pub rate_limit_violations: u64,
}

impl AccessStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful access
    pub fn record_success(&mut self) {
        self.total_attempts += 1;
        self.successful_reads += 1;
    }

    /// Record a rate limit violation
    pub fn record_violation(&mut self) {
        self.total_attempts += 1;
        self.rate_limit_violations += 1;
    }

    /// Get the success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_attempts == 0 {
            0.0
        } else {
            (self.successful_reads as f64 / self.total_attempts as f64) * 100.0
        }
    }

    /// Get the violation rate as a percentage
    pub fn violation_rate(&self) -> f64 {
        if self.total_attempts == 0 {
            0.0
        } else {
            (self.rate_limit_violations as f64 / self.total_attempts as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_control_new() {
        let control = AccessControl::new();
        assert_eq!(control.read_count(), 0);
        assert_eq!(control.max_reads(), MAX_SEQUENTIAL_READS);
        assert_eq!(control.delay_ms(), ANTI_SCRAPE_DELAY_MS);
    }

    #[test]
    fn test_access_control_with_limits() {
        let control = AccessControl::with_limits(50, 100);
        assert_eq!(control.max_reads(), 50);
        assert_eq!(control.delay_ms(), 100);
    }

    #[test]
    fn test_access_control_default() {
        let control = AccessControl::default();
        assert_eq!(control.read_count(), 0);
    }

    #[test]
    fn test_first_read_always_allowed() {
        let mut control = AccessControl::new();
        assert!(control.check_rate_limit().is_ok());
        // First read doesn't increment counter
        assert_eq!(control.read_count(), 1);
    }

    #[test]
    fn test_sequential_reads() {
        let mut control = AccessControl::with_limits(3, 10);

        // First 3 reads should succeed
        assert!(control.check_rate_limit().is_ok());
        assert!(control.check_rate_limit().is_ok());
        assert!(control.check_rate_limit().is_ok());

        // Wait for delay to pass
        std::thread::sleep(Duration::from_millis(15));

        // Should reset and allow more reads
        assert!(control.check_rate_limit().is_ok());
    }

    #[test]
    fn test_rate_limit_exceeded() {
        let mut control = AccessControl::with_limits(2, 100);

        // First 2 reads should succeed
        assert!(control.check_rate_limit().is_ok());
        assert!(control.check_rate_limit().is_ok());

        // Third read within delay window should fail
        let result = control.check_rate_limit();
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::RateLimitExceeded { .. })));
    }

    #[test]
    fn test_reset() {
        let mut control = AccessControl::with_limits(1, 100);

        // Read until rate limited
        assert!(control.check_rate_limit().is_ok());
        assert!(control.check_rate_limit().is_err());

        // Reset
        control.reset();

        // Should allow reads again
        assert!(control.check_rate_limit().is_ok());
    }

    #[test]
    fn test_record_read() {
        let mut control = AccessControl::new();
        assert_eq!(control.read_count(), 0);

        control.record_read();
        assert_eq!(control.read_count(), 1);

        control.record_read();
        assert_eq!(control.read_count(), 2);
    }

    #[test]
    fn test_is_rate_limited() {
        let mut control = AccessControl::with_limits(1, 100);

        assert!(!control.is_rate_limited());

        control.record_read();
        assert!(control.is_rate_limited());

        control.reset();
        assert!(!control.is_rate_limited());
    }

    #[test]
    fn test_access_stats() {
        let mut stats = AccessStats::new();

        stats.record_success();
        stats.record_success();
        stats.record_violation();

        assert_eq!(stats.total_attempts, 3);
        assert_eq!(stats.successful_reads, 2);
        assert_eq!(stats.rate_limit_violations, 1);
        assert_eq!(stats.success_rate(), 66.66666666666666);
        assert_eq!(stats.violation_rate(), 33.33333333333333);
    }

    #[test]
    fn test_access_stats_empty() {
        let stats = AccessStats::new();
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.violation_rate(), 0.0);
    }

    #[test]
    fn test_time_since_last_read() {
        let mut control = AccessControl::new();

        assert!(control.time_since_last_read().is_none());

        control.record_read();
        let elapsed = control.time_since_last_read();
        assert!(elapsed.is_some());
        assert!(elapsed.unwrap().as_millis() < 100);
    }
}

//! Debugging tools module for Maxion Protector
//!
//! Provides utilities for:
//! - Archive inspection and validation
//! - Performance profiling and benchmarking
//! - Memory usage tracking
//! - Debug logging and diagnostics

use crate::archive::ArchiveHeader;
use crate::Config;
use crate::Result;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Archive inspector for examining archive contents and structure
pub struct ArchiveInspector {
    archive_path: PathBuf,
    cached_header: OnceLock<ArchiveHeader>,
    cached_file_table: OnceLock<Vec<(String, u64, u64)>>,
}

impl ArchiveInspector {
    /// Create a new archive inspector
    ///
    /// # Arguments
    ///
    /// * `archive_path` - Path to the archive file
    pub fn new(archive_path: impl AsRef<Path>) -> Self {
        Self {
            archive_path: archive_path.as_ref().to_path_buf(),
            cached_header: OnceLock::new(),
            cached_file_table: OnceLock::new(),
        }
    }

    /// Clear cached data
    ///
    /// Use this to force re-reading the archive file
    pub fn clear_cache(&mut self) {
        let _ = self.cached_header.take();
        let _ = self.cached_file_table.take();
    }

    /// Load and parse the archive header
    ///
    /// # Returns
    ///
    /// Archive header information
    pub fn load_header(&self) -> Result<ArchiveHeader> {
        // Return cached header if available
        if let Some(header) = self.cached_header.get() {
            return Ok(header.clone());
        }

        // Load header from file
        let mut file = File::open(&self.archive_path).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Archive not found: {e}"),
            )
        })?;

        let mut header_bytes = vec![0u8; 256];
        file.read_exact(&mut header_bytes)?;

        let header = ArchiveHeader::from_bytes(&header_bytes)?;

        // Cache the header for future use
        let _ = self.cached_header.set(header.clone());

        Ok(header)
    }

    /// Get archive information as a formatted string
    ///
    /// # Returns
    ///
    /// Detailed archive information
    pub fn info(&self) -> Result<String> {
        let archive_path = self.archive_path.clone();
        let path_str = archive_path.display().to_string();

        let file_size = fs::metadata(&archive_path)
            .map_err(|e| std::io::Error::other(format!("Failed to get file size: {e}")))?
            .len();

        let header = self.load_header()?;

        let data_offset = header.file_table_offset + header.file_table_size as u64;

        let mut info = String::new();
        info.push_str("=== Archive Information ===\n");
        info.push_str(&format!("Path: {}\n", path_str));
        info.push_str(&format!(
            "File Size: {} bytes ({:.2} MB)\n",
            file_size,
            file_size as f64 / 1024.0 / 1024.0
        ));
        info.push_str(&format!("Version: {}\n", header.version));
        info.push_str(&format!("File Count: {}\n", header.file_count));
        info.push_str(&format!(
            "File Table Offset: 0x{:X} ({} bytes)\n",
            header.file_table_offset, header.file_table_offset
        ));
        info.push_str(&format!(
            "File Table Size: {} bytes\n",
            header.file_table_size
        ));
        info.push_str(&format!(
            "Data Offset: 0x{:X} ({} bytes)\n",
            data_offset, data_offset
        ));
        info.push_str(&format!(
            "Chunk Size: {} bytes ({:.2} KB)\n",
            header.chunk_size,
            header.chunk_size as f64 / 1024.0
        ));
        info.push_str(&format!(
            "Compression: {}\n",
            if header.compress {
                "Enabled"
            } else {
                "Disabled"
            }
        ));
        info.push_str(&format!(
            "Header Checksum: 0x{}\n",
            header
                .header_checksum
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        ));
        info.push_str(&format!("Checksum Valid: {}\n", header.verify_checksum()));

        Ok(info)
    }

    /// List all files in the archive with detailed information
    ///
    /// # Returns
    ///
    /// Formatted list of all files
    pub fn list_files(&self) -> Result<String> {
        let archive_path = self.archive_path.clone();
        let header = self.load_header()?;
        let file_count = header.file_count;

        let mut list = String::new();
        list.push_str("=== Archive File List ===\n");
        list.push_str(&format!("Total Files: {}\n", file_count));
        list.push_str("(Detailed file listing requires VirtualArchive file_table access which is currently private)\n\n");

        // Get archive file size for total calculation
        let file_size = fs::metadata(&archive_path)
            .map_err(|e| std::io::Error::other(format!("Failed to get file size: {e}")))?
            .len();

        let _total_packed = file_size - header.file_table_offset;

        list.push_str("=== Archive Size ===\n");
        list.push_str(&format!(
            "Archive Size: {} bytes ({:.2} MB)\n",
            file_size,
            file_size as f64 / 1024.0 / 1024.0
        ));
        list.push_str(&format!(
            "File Table Size: {} bytes ({:.2} KB)\n",
            header.file_table_size,
            header.file_table_size as f64 / 1024.0
        ));

        Ok(list)
    }

    /// Validate archive integrity
    ///
    /// # Returns
    ///
    /// Validation result with details
    pub fn validate(&self) -> Result<ValidationResult> {
        let archive_path = self.archive_path.clone();
        let path_str = archive_path.display().to_string();

        let file_size = fs::metadata(&archive_path)
            .map_err(|e| std::io::Error::other(format!("Failed to get file size: {e}")))?
            .len();

        let header = self.load_header()?;
        let mut result = ValidationResult::new();

        // Check header checksum
        result.checks.push(ValidationCheck {
            name: "Header Checksum".to_string(),
            passed: header.verify_checksum(),
            message: if header.verify_checksum() {
                "Header checksum is valid".to_string()
            } else {
                format!(
                    "Header checksum mismatch: expected 0x{}",
                    header
                        .header_checksum
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<String>()
                )
            },
        });

        // Check file size
        let expected_min_size = header.file_table_offset + header.file_table_size as u64;
        result.checks.push(ValidationCheck {
            name: "Archive Size".to_string(),
            passed: file_size >= expected_min_size,
            message: format!(
                "Archive size: {} bytes (expected minimum: {} bytes), path: {}",
                file_size, expected_min_size, path_str
            ),
        });

        // Check file table presence
        result.checks.push(ValidationCheck {
            name: "File Table".to_string(),
            passed: header.file_table_size > 0 && header.file_table_offset > 0,
            message: format!(
                "File table offset: 0x{:X}, size: {} bytes",
                header.file_table_offset, header.file_table_size
            ),
        });

        // Check chunk size
        result.checks.push(ValidationCheck {
            name: "Chunk Size".to_string(),
            passed: header.chunk_size >= 4096 && header.chunk_size <= 1024 * 1024,
            message: format!("Chunk size: {} bytes", header.chunk_size),
        });

        // Check version
        result.checks.push(ValidationCheck {
            name: "Archive Version".to_string(),
            passed: header.version == crate::ARCHIVE_VERSION,
            message: format!(
                "Archive version: {} (current: {})",
                header.version,
                crate::ARCHIVE_VERSION
            ),
        });

        // Try to open archive
        let open_result = crate::DefaultVirtualArchive::open(&archive_path, Config::new());
        match open_result {
            Ok(_archive) => {
                result.checks.push(ValidationCheck {
                    name: "Archive Open".to_string(),
                    passed: true,
                    message: "Archive can be opened".to_string(),
                });
            }
            Err(e) => {
                result.checks.push(ValidationCheck {
                    name: "Archive Open".to_string(),
                    passed: false,
                    message: format!("Failed to open archive: {e}"),
                });
            }
        }

        Ok(result)
    }
}

/// Validation result for archive inspection
#[derive(Debug, Clone)]
pub struct ValidationResult {
    checks: Vec<ValidationCheck>,
}

impl ValidationResult {
    fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Check if all validation checks passed
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }

    /// Get number of passed checks
    pub fn passed_count(&self) -> usize {
        self.checks.iter().filter(|check| check.passed).count()
    }

    /// Get number of failed checks
    pub fn failed_count(&self) -> usize {
        self.checks.iter().filter(|check| !check.passed).count()
    }

    /// Get total number of checks performed
    pub fn total_checks(&self) -> usize {
        self.checks.len()
    }

    /// Format validation result as string
    pub fn format(&self) -> String {
        let mut result = String::new();
        result.push_str("=== Archive Validation ===\n");
        result.push_str(&format!(
            "Passed: {}/{}\n\n",
            self.passed_count(),
            self.checks.len()
        ));

        for check in &self.checks {
            let status = if check.passed { "✓" } else { "✗" };
            result.push_str(&format!("{} {}: {}\n", status, check.name, check.message));
        }

        if self.all_passed() {
            result.push_str("\nAll checks passed! Archive is valid.\n");
        } else {
            result.push_str(&format!(
                "\n{} check(s) failed. Archive may be corrupted.\n",
                self.failed_count()
            ));
        }

        result
    }

    /// Get a brief summary of validation results
    pub fn summary(&self) -> String {
        format!(
            "Validation: {}/{} passed, {}/{} failed",
            self.passed_count(),
            self.total_checks(),
            self.failed_count(),
            self.total_checks()
        )
    }
}

/// Individual validation check
#[derive(Debug, Clone)]
struct ValidationCheck {
    name: String,
    passed: bool,
    message: String,
}

/// Performance profiler for measuring operation timings
pub struct PerformanceProfiler {
    measurements: HashMap<String, Vec<Duration>>,
}

impl PerformanceProfiler {
    /// Create a new performance profiler
    pub fn new() -> Self {
        Self {
            measurements: HashMap::new(),
        }
    }

    /// Start measuring a named operation
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the operation being measured
    ///
    /// # Returns
    ///
    /// A timer that will record the duration when dropped
    pub fn start_timer(&mut self, name: impl AsRef<str>) -> ProfilerTimer<'_> {
        ProfilerTimer {
            profiler: self,
            name: name.as_ref().to_string(),
            start: Instant::now(),
        }
    }

    /// Record a measurement manually
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the operation
    /// * `duration` - Duration of the operation
    pub fn record(&mut self, name: impl AsRef<str>, duration: Duration) {
        self.measurements
            .entry(name.as_ref().to_string())
            .or_default()
            .push(duration);
    }

    /// Benchmark a function and record its execution time
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the operation
    /// * `f` - Function to benchmark
    ///
    /// # Returns
    ///
    /// The result of the function
    pub fn benchmark<F, R>(&mut self, name: impl AsRef<str>, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        self.record(name, duration);
        result
    }

    /// Get statistics for a specific operation
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the operation
    ///
    /// # Returns
    ///
    /// Statistics if the operation exists, None otherwise
    pub fn get_stats(&self, name: &str) -> Option<OperationStats> {
        let durations = self.measurements.get(name)?;
        if durations.is_empty() {
            return None;
        }

        let count = durations.len();
        let total: Duration = durations.iter().sum();
        let min = *durations.iter().min().unwrap();
        let max = *durations.iter().max().unwrap();
        let avg = total / count as u32;

        // Calculate median
        let mut sorted = durations.clone();
        sorted.sort();
        let median = if count % 2 == 0 {
            (sorted[count / 2 - 1] + sorted[count / 2]) / 2
        } else {
            sorted[count / 2]
        };

        Some(OperationStats {
            count,
            min,
            max,
            avg,
            median,
            total,
        })
    }

    /// Format all measurements as a report
    ///
    /// # Returns
    ///
    /// Formatted performance report
    pub fn format_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Performance Report ===\n\n");

        let mut names: Vec<_> = self.measurements.keys().collect();
        names.sort();

        for name in names {
            if let Some(stats) = self.get_stats(name) {
                report.push_str(&format!("{}\n", name));
                report.push_str(&format!("  Count: {}\n", stats.count));
                report.push_str(&format!("  Total: {:.3}s\n", stats.total.as_secs_f64()));
                report.push_str(&format!(
                    "  Average: {:.3}ms\n",
                    stats.avg.as_secs_f64() * 1000.0
                ));
                report.push_str(&format!(
                    "  Median: {:.3}ms\n",
                    stats.median.as_secs_f64() * 1000.0
                ));
                report.push_str(&format!(
                    "  Min: {:.3}ms\n",
                    stats.min.as_secs_f64() * 1000.0
                ));
                report.push_str(&format!(
                    "  Max: {:.3}ms\n\n",
                    stats.max.as_secs_f64() * 1000.0
                ));
            }
        }

        report
    }

    /// Clear all measurements
    pub fn clear(&mut self) {
        self.measurements.clear();
    }
}

impl Default for PerformanceProfiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer that records duration when dropped
pub struct ProfilerTimer<'a> {
    profiler: &'a mut PerformanceProfiler,
    name: String,
    start: Instant,
}

impl<'a> Drop for ProfilerTimer<'a> {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        self.profiler.record(&self.name, duration);
    }
}

/// Statistics for a specific operation
#[derive(Debug, Clone, Copy)]
pub struct OperationStats {
    pub count: usize,
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
    pub median: Duration,
    pub total: Duration,
}

/// Memory usage tracker
pub struct MemoryTracker {
    allocations: HashMap<String, usize>,
}

impl MemoryTracker {
    /// Create a new memory tracker
    pub fn new() -> Self {
        Self {
            allocations: HashMap::new(),
        }
    }

    /// Record an allocation
    ///
    /// # Arguments
    ///
    /// * `name` - Name or description of the allocation
    /// * `size` - Size of the allocation in bytes
    pub fn record_allocation(&mut self, name: impl AsRef<str>, size: usize) {
        *self
            .allocations
            .entry(name.as_ref().to_string())
            .or_insert(0) += size;
    }

    /// Get total memory tracked
    pub fn total(&self) -> usize {
        self.allocations.values().sum()
    }

    /// Get memory for a specific allocation
    pub fn get(&self, name: &str) -> Option<usize> {
        self.allocations.get(name).copied()
    }

    /// Format memory usage as a report
    ///
    /// # Returns
    ///
    /// Formatted memory report
    pub fn format_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Memory Usage Report ===\n\n");

        let total = self.total();
        report.push_str(&format!(
            "Total: {} bytes ({:.2} MB)\n\n",
            total,
            total as f64 / 1024.0 / 1024.0
        ));

        let mut entries: Vec<_> = self.allocations.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1)); // Sort by size descending

        for (name, size) in entries {
            let percentage = if total > 0 {
                *size as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            report.push_str(&format!(
                "{}: {} bytes ({:.2} KB, {:.2}%)\n",
                name,
                size,
                *size as f64 / 1024.0,
                percentage
            ));
        }

        report
    }

    /// Clear all tracked allocations
    pub fn clear(&mut self) {
        self.allocations.clear();
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Debug log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// No logging
    Off = 0,
    /// Error messages only
    Error = 1,
    /// Warning and error messages
    Warn = 2,
    /// Info, warning, and error messages
    Info = 3,
    /// Debug, info, warning, and error messages
    Debug = 4,
    /// All messages
    Trace = 5,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Off => write!(f, "off"),
            LogLevel::Error => write!(f, "error"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Trace => write!(f, "trace"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| {
            format!("Invalid log level: '{s}'. Valid values: off, error, warn, info, debug, trace")
        })
    }
}

impl LogLevel {
    /// Parse log level from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" => Some(LogLevel::Off),
            "error" => Some(LogLevel::Error),
            "warn" | "warning" => Some(LogLevel::Warn),
            "info" => Some(LogLevel::Info),
            "debug" => Some(LogLevel::Debug),
            "trace" => Some(LogLevel::Trace),
            _ => None,
        }
    }
}

/// Debug logger with configurable log levels
pub struct DebugLogger {
    level: LogLevel,
}

/// Builder for creating configured DebugLogger instances
pub struct DebugLoggerBuilder {
    level: Option<LogLevel>,
}

impl Default for DebugLoggerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugLoggerBuilder {
    /// Create a new debug logger builder
    pub fn new() -> Self {
        Self { level: None }
    }

    /// Set the log level
    ///
    /// # Arguments
    ///
    /// * `level` - Minimum log level to output
    pub fn level(mut self, level: LogLevel) -> Self {
        self.level = Some(level);
        self
    }

    /// Set the log level from environment variable
    ///
    /// # Arguments
    ///
    /// * `env_var` - Environment variable name (e.g., "RUST_LOG")
    pub fn level_from_env(mut self, env_var: &str) -> Self {
        if self.level.is_none() {
            self.level = std::env::var(env_var)
                .ok()
                .and_then(|s| LogLevel::parse(&s));
        }
        self
    }

    /// Build the DebugLogger
    ///
    /// Uses the configured level or defaults to Info
    pub fn build(self) -> DebugLogger {
        DebugLogger {
            level: self.level.unwrap_or(LogLevel::Info),
        }
    }
}

impl DebugLogger {
    /// Create a new debug logger with specified level
    ///
    /// # Arguments
    ///
    /// * `level` - Minimum log level to output
    pub fn new(level: LogLevel) -> Self {
        Self { level }
    }

    /// Create a new debug logger from environment variable
    ///
    /// # Arguments
    ///
    /// * `env_var` - Environment variable name (e.g., "RUST_LOG")
    ///
    /// # Returns
    ///
    /// Logger with level from environment, or Info if not set
    pub fn from_env(env_var: &str) -> Self {
        let level = std::env::var(env_var)
            .ok()
            .and_then(|s| LogLevel::parse(&s))
            .unwrap_or(LogLevel::Info);

        Self::new(level)
    }

    /// Set the log level
    pub fn set_level(&mut self, level: LogLevel) {
        self.level = level;
    }

    /// Get the current log level
    pub fn level(&self) -> LogLevel {
        self.level
    }

    /// Log an error message
    pub fn error(&self, message: &str) {
        if self.level >= LogLevel::Error {
            eprintln!("[{}] {}", LogLevel::Error, message);
        }
    }

    /// Log a warning message
    pub fn warn(&self, message: &str) {
        if self.level >= LogLevel::Warn {
            eprintln!("[{}] {}", LogLevel::Warn, message);
        }
    }

    /// Log an info message
    pub fn info(&self, message: &str) {
        if self.level >= LogLevel::Info {
            println!("[{}] {}", LogLevel::Info, message);
        }
    }

    /// Log a debug message
    pub fn debug(&self, message: &str) {
        if self.level >= LogLevel::Debug {
            println!("[{}] {}", LogLevel::Debug, message);
        }
    }

    /// Log a trace message
    pub fn trace(&self, message: &str) {
        if self.level >= LogLevel::Trace {
            println!("[{}] {}", LogLevel::Trace, message);
        }
    }
}

impl Default for DebugLogger {
    fn default() -> Self {
        Self::from_env("RUST_LOG")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_profiler() {
        let mut profiler = PerformanceProfiler::new();

        // Simulate some operations
        {
            let _timer = profiler.start_timer("operation1");
            std::thread::sleep(Duration::from_millis(10));
        }

        {
            let _timer = profiler.start_timer("operation1");
            std::thread::sleep(Duration::from_millis(20));
        }

        {
            let _timer = profiler.start_timer("operation2");
            std::thread::sleep(Duration::from_millis(5));
        }

        let stats1 = profiler.get_stats("operation1").unwrap();
        assert_eq!(stats1.count, 2);
        assert!(stats1.avg.as_millis() >= 10);

        let stats2 = profiler.get_stats("operation2").unwrap();
        assert_eq!(stats2.count, 1);
        assert!(stats2.avg.as_millis() >= 5);
        assert!(stats2.avg.as_millis() < stats1.avg.as_millis());
    }

    #[test]
    fn test_memory_tracker() {
        let mut tracker = MemoryTracker::new();

        tracker.record_allocation("allocation1", 1024);
        tracker.record_allocation("allocation1", 2048);
        tracker.record_allocation("allocation2", 512);

        assert_eq!(tracker.total(), 3584);
        assert_eq!(tracker.get("allocation1"), Some(3072));
        assert_eq!(tracker.get("allocation2"), Some(512));
    }

    #[test]
    fn test_debug_logger() {
        let logger = DebugLogger::new(LogLevel::Info);

        assert_eq!(logger.level(), LogLevel::Info);

        logger.error("error message"); // Should print
        logger.warn("warn message"); // Should print
        logger.info("info message"); // Should print
        logger.debug("debug message"); // Should not print
        logger.trace("trace message"); // Should not print

        let trace_logger = DebugLogger::new(LogLevel::Trace);
        assert_eq!(trace_logger.level(), LogLevel::Trace);
    }

    #[test]
    fn test_log_level_parse() {
        assert_eq!(LogLevel::parse("off"), Some(LogLevel::Off));
        assert_eq!(LogLevel::parse("error"), Some(LogLevel::Error));
        assert_eq!(LogLevel::parse("warn"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::parse("warning"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::parse("info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::parse("debug"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::parse("trace"), Some(LogLevel::Trace));
        assert_eq!(LogLevel::parse("invalid"), None);
    }

    #[test]
    fn test_log_level_from_str_trait() {
        use std::str::FromStr;

        assert_eq!(LogLevel::from_str("off"), Ok(LogLevel::Off));
        assert_eq!(LogLevel::from_str("error"), Ok(LogLevel::Error));
        assert_eq!(LogLevel::from_str("warn"), Ok(LogLevel::Warn));
        assert_eq!(LogLevel::from_str("info"), Ok(LogLevel::Info));
        assert_eq!(
            LogLevel::from_str("invalid"),
            Err(
                "Invalid log level: 'invalid'. Valid values: off, error, warn, info, debug, trace"
                    .to_string()
            )
        );
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(format!("{}", LogLevel::Off), "off");
        assert_eq!(format!("{}", LogLevel::Error), "error");
        assert_eq!(format!("{}", LogLevel::Warn), "warn");
        assert_eq!(format!("{}", LogLevel::Info), "info");
        assert_eq!(format!("{}", LogLevel::Debug), "debug");
        assert_eq!(format!("{}", LogLevel::Trace), "trace");
    }

    #[test]
    fn test_validation_result_summary() {
        let mut result = ValidationResult::new();

        result.checks.push(ValidationCheck {
            name: "Test 1".to_string(),
            passed: true,
            message: "Passed".to_string(),
        });

        result.checks.push(ValidationCheck {
            name: "Test 2".to_string(),
            passed: false,
            message: "Failed".to_string(),
        });

        let summary = result.summary();
        assert!(summary.contains("1/2 passed"));
        assert!(summary.contains("1/2 failed"));
    }

    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::new();

        result.checks.push(ValidationCheck {
            name: "Test 1".to_string(),
            passed: true,
            message: "Passed".to_string(),
        });

        result.checks.push(ValidationCheck {
            name: "Test 2".to_string(),
            passed: false,
            message: "Failed".to_string(),
        });

        assert!(!result.all_passed());
        assert_eq!(result.passed_count(), 1);
        assert_eq!(result.failed_count(), 1);
    }

    #[test]
    fn test_operation_stats() {
        let durations = [
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(30),
        ];

        let count = durations.len();
        let total: Duration = durations.iter().sum();
        let avg = total / count as u32;

        assert_eq!(count, 3);
        assert_eq!(total, Duration::from_millis(60));
        assert_eq!(avg, Duration::from_millis(20));
    }

    #[test]
    fn test_memory_tracker_report() {
        let mut tracker = MemoryTracker::new();

        tracker.record_allocation("large", 1024 * 1024);
        tracker.record_allocation("small", 512);

        let report = tracker.format_report();
        // Total: 1,048,576 + 512 = 1,049,088 bytes
        assert!(report.contains("1049088")); // Total bytes
                                             // 1,049,088 / 1024 / 1024 = 1.0005 MB, formatted to ~1.00
        assert!(report.contains("1.00")); // MB
    }

    #[test]
    fn test_performance_profiler_benchmark() {
        let mut profiler = PerformanceProfiler::new();

        // Benchmark a simple function
        let result = profiler.benchmark("test_operation", || {
            std::thread::sleep(Duration::from_millis(10));
            42
        });

        assert_eq!(result, 42);

        // Check that the measurement was recorded
        let stats = profiler.get_stats("test_operation").unwrap();
        assert_eq!(stats.count, 1);
        assert!(stats.total >= Duration::from_millis(10));
    }

    #[test]
    fn test_debug_logger_builder() {
        // Test builder with explicit level
        let logger1 = DebugLoggerBuilder::new().level(LogLevel::Error).build();
        assert_eq!(logger1.level(), LogLevel::Error);

        // Test builder with environment level
        let logger2 = DebugLoggerBuilder::new()
            .level_from_env("TEST_LOG_LEVEL")
            .build();
        // Should default to Info if env var not set
        assert_eq!(logger2.level(), LogLevel::Info);

        // Test builder with both explicit and env (explicit takes precedence)
        let logger3 = DebugLoggerBuilder::new()
            .level_from_env("TEST_LOG_LEVEL")
            .level(LogLevel::Debug)
            .build();
        assert_eq!(logger3.level(), LogLevel::Debug);
    }

    #[test]
    fn test_performance_profiler_multiple_benchmarks() {
        let mut profiler = PerformanceProfiler::new();

        // Run multiple benchmarks
        profiler.benchmark("fast_op", || {
            std::thread::sleep(Duration::from_millis(1));
        });

        profiler.benchmark("slow_op", || {
            std::thread::sleep(Duration::from_millis(20));
        });

        profiler.benchmark("fast_op", || {
            std::thread::sleep(Duration::from_millis(2));
        });

        // Check fast_op stats
        let fast_stats = profiler.get_stats("fast_op").unwrap();
        assert_eq!(fast_stats.count, 2);
        assert!(fast_stats.total >= Duration::from_millis(3));
        assert!(fast_stats.total < Duration::from_millis(10));

        // Check slow_op stats
        let slow_stats = profiler.get_stats("slow_op").unwrap();
        assert_eq!(slow_stats.count, 1);
        assert!(slow_stats.total >= Duration::from_millis(20));
    }
}

//! # Maxion Profiler
//!
//! A timing and performance measurement library for Maxion Protector.
//!
//! Provides utilities for measuring asset loading latency, recording metrics,
//! and generating performance reports.

use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

/// Global metrics storage
static METRICS: OnceLock<Mutex<MetricsCollector>> = OnceLock::new();

/// Initialize the global metrics collector
pub fn init_metrics(output_path: &str) {
    let collector = MetricsCollector::new(output_path);
    METRICS
        .set(Mutex::new(collector))
        .expect("Metrics collector already initialized");
}

/// Get a reference to the global metrics collector
fn get_metrics() -> &'static Mutex<MetricsCollector> {
    METRICS
        .get()
        .expect("Metrics collector not initialized. Call init_metrics() first.")
}

/// Flush all metrics to the output file
pub fn flush_metrics() -> anyhow::Result<()> {
    let metrics = get_metrics();
    let collector = metrics
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to lock metrics: {}", e))?;
    collector.flush()
}

/// A timing measurement with automatic drop handling
#[derive(Debug, Clone)]
pub struct Timer {
    start: Option<Instant>,
    label: String,
}

impl Timer {
    /// Start a new timer with a label
    pub fn start(label: &str) -> Self {
        Self {
            start: Some(Instant::now()),
            label: label.to_string(),
        }
    }

    /// Stop the timer and record the duration
    pub fn stop(self) -> Duration {
        let duration = self.duration();
        if let Ok(mut metrics) = get_metrics().lock() {
            metrics.record_timing(&self.label, duration);
        }
        duration
    }

    /// Stop the timer with a custom label
    pub fn stop_with_label(mut self, label: &str) -> Duration {
        self.label = label.to_string();
        self.stop()
    }

    /// Get the elapsed duration without stopping
    pub fn duration(&self) -> Duration {
        self.start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// Get the elapsed duration in milliseconds
    pub fn elapsed_ms(&self) -> u128 {
        self.duration().as_millis()
    }

    /// Get the elapsed duration in microseconds
    pub fn elapsed_us(&self) -> u128 {
        self.duration().as_micros()
    }

    /// Get the elapsed duration in nanoseconds
    pub fn elapsed_ns(&self) -> u128 {
        self.duration().as_nanos()
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(start) = self.start.take() {
            let duration = start.elapsed();
            if let Ok(mut metrics) = get_metrics().lock() {
                metrics.record_timing(&self.label, duration);
            }
        }
    }
}

/// Metrics collector for recording timing and counter metrics
#[derive(Debug, Serialize)]
pub struct MetricsCollector {
    output_path: String,
    timings: HashMap<String, Vec<Duration>>,
    counters: HashMap<String, u64>,
    file_loads: Vec<FileLoadMetric>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(output_path: &str) -> Self {
        Self {
            output_path: output_path.to_string(),
            timings: HashMap::new(),
            counters: HashMap::new(),
            file_loads: Vec::new(),
        }
    }

    /// Record a timing measurement
    pub fn record_timing(&mut self, label: &str, duration: Duration) {
        self.timings
            .entry(label.to_string())
            .or_default()
            .push(duration);
    }

    /// Record a counter value
    pub fn record_counter(&mut self, label: &str, value: u64) {
        *self.counters.entry(label.to_string()).or_insert(0) += value;
    }

    /// Record a file load metric
    pub fn record_file_load(&mut self, metric: FileLoadMetric) {
        self.file_loads.push(metric);
    }

    /// Get all timings as milliseconds
    pub fn get_timings_ms(&self, label: &str) -> Vec<u128> {
        self.timings
            .get(label)
            .map(|durations| durations.iter().map(|d| d.as_millis()).collect())
            .unwrap_or_default()
    }

    /// Get average timing for a label
    pub fn get_average_ms(&self, label: &str) -> Option<f64> {
        self.timings.get(label).map(|durations| {
            let count = durations.len();
            if count == 0 {
                return 0.0;
            }
            let total: Duration = durations.iter().sum();
            let avg = total / count as u32;
            avg.as_secs_f64() * 1000.0
        })
    }

    /// Get minimum timing for a label
    pub fn get_min_ms(&self, label: &str) -> Option<u128> {
        self.timings
            .get(label)
            .and_then(|durations| durations.iter().map(|d| d.as_millis()).min())
    }

    /// Get maximum timing for a label
    pub fn get_max_ms(&self, label: &str) -> Option<u128> {
        self.timings
            .get(label)
            .and_then(|durations| durations.iter().map(|d| d.as_millis()).max())
    }

    /// Flush all metrics to JSON file
    pub fn flush(&self) -> anyhow::Result<()> {
        let output = &self.output_path;

        // Create parent directory if it doesn't exist
        if let Some(parent) = Path::new(output).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let report = BenchmarkReport {
            timings: self
                .timings
                .iter()
                .map(|(label, durations)| {
                    let durations_ms: Vec<u128> = durations.iter().map(|d| d.as_millis()).collect();
                    (label.clone(), durations_ms)
                })
                .collect(),
            counters: self.counters.clone(),
            file_loads: self.file_loads.clone(),
            summary: self.generate_summary(),
        };

        let json = serde_json::to_string_pretty(&report)?;
        std::fs::write(output, json)?;

        println!("Metrics flushed to {}", output);
        Ok(())
    }

    /// Generate a summary of all metrics
    fn generate_summary(&self) -> MetricsSummary {
        let mut timing_summary = HashMap::new();

        for (label, durations) in &self.timings {
            let count = durations.len();
            let total: Duration = durations.iter().sum();
            let avg = total / count as u32;
            let min = durations.iter().min().copied().unwrap_or(Duration::ZERO);
            let max = durations.iter().max().copied().unwrap_or(Duration::ZERO);

            timing_summary.insert(
                label.clone(),
                TimingStats {
                    count,
                    avg_ms: avg.as_millis(),
                    min_ms: min.as_millis(),
                    max_ms: max.as_millis(),
                    total_ms: total.as_millis(),
                },
            );
        }

        MetricsSummary {
            timings: timing_summary,
            counters: self.counters.clone(),
        }
    }
}

/// File load metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLoadMetric {
    pub file_path: String,
    pub file_size: u64,
    pub load_time_ms: u128,
    pub method: LoadMethod,
}

/// Method used to load the file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadMethod {
    /// Direct file system read
    Direct,
    /// VFS read from packed executable
    Vfs,
    /// Streaming read
    Stream,
}

/// Complete benchmark report
#[derive(Debug, Serialize)]
struct BenchmarkReport {
    timings: HashMap<String, Vec<u128>>,
    counters: HashMap<String, u64>,
    file_loads: Vec<FileLoadMetric>,
    summary: MetricsSummary,
}

/// Metrics summary
#[derive(Debug, Serialize)]
struct MetricsSummary {
    timings: HashMap<String, TimingStats>,
    counters: HashMap<String, u64>,
}

/// Timing statistics for a label
#[derive(Debug, Serialize)]
struct TimingStats {
    count: usize,
    avg_ms: u128,
    min_ms: u128,
    max_ms: u128,
    total_ms: u128,
}

/// Record a simple timing metric
pub fn record_metric(label: &str, duration: Duration) {
    if let Ok(mut metrics) = get_metrics().lock() {
        metrics.record_timing(label, duration);
    }
}

/// Record a counter value
pub fn record_counter(label: &str, value: u64) {
    if let Ok(mut metrics) = get_metrics().lock() {
        metrics.record_counter(label, value);
    }
}

/// Track a file load operation
pub fn track_file_load<P: AsRef<Path>>(
    file_path: P,
    file_size: u64,
    load_time_ms: u128,
    method: LoadMethod,
) {
    let metric = FileLoadMetric {
        file_path: file_path.as_ref().to_string_lossy().to_string(),
        file_size,
        load_time_ms,
        method,
    };

    if let Ok(mut metrics) = get_metrics().lock() {
        metrics.record_file_load(metric);
    }
}

/// Run a benchmark with multiple iterations
pub fn benchmark<F, R>(label: &str, iterations: usize, func: F) -> anyhow::Result<Duration>
where
    F: Fn() -> anyhow::Result<R>,
{
    let mut total_duration = Duration::ZERO;
    let mut results = Vec::with_capacity(iterations);

    // Warmup run
    let _ = func();

    for i in 0..iterations {
        let start = Instant::now();
        let result = func()?;
        total_duration += start.elapsed();
        results.push(result);

        if (i + 1) % 10 == 0 {
            println!("  Iteration {}/{}", i + 1, iterations);
        }
    }

    let avg_duration = total_duration / iterations as u32;
    println!(
        "  {} average: {:.2}ms",
        label,
        avg_duration.as_secs_f64() * 1000.0
    );

    if let Ok(mut metrics) = get_metrics().lock() {
        for _ in 0..iterations {
            metrics.record_timing(label, avg_duration);
        }
    }

    Ok(avg_duration)
}

/// Run a benchmark with automatic timing
pub fn benchmark_auto<F, R>(label: &str, func: F) -> anyhow::Result<R>
where
    F: FnOnce() -> anyhow::Result<R>,
{
    let _timer = Timer::start(label);
    func()
}

/// Compare two operations
pub fn benchmark_compare<F1, R1, F2, R2>(
    label1: &str,
    func1: F1,
    label2: &str,
    func2: F2,
) -> anyhow::Result<(Duration, Duration)>
where
    F1: FnOnce() -> anyhow::Result<R1>,
    F2: FnOnce() -> anyhow::Result<R2>,
{
    println!("Comparing {} vs {}", label1, label2);

    let start1 = Instant::now();
    let _result1 = func1()?;
    let duration1 = start1.elapsed();

    let start2 = Instant::now();
    let _result2 = func2()?;
    let duration2 = start2.elapsed();

    let ratio = if duration1 > Duration::ZERO {
        duration2.as_secs_f64() / duration1.as_secs_f64()
    } else {
        1.0
    };

    println!("  {}: {:.2}ms", label1, duration1.as_secs_f64() * 1000.0);
    println!("  {}: {:.2}ms", label2, duration2.as_secs_f64() * 1000.0);
    println!("  Ratio: {:.2}x", ratio);

    if let Ok(mut metrics) = get_metrics().lock() {
        metrics.record_timing(label1, duration1);
        metrics.record_timing(label2, duration2);
    }

    Ok((duration1, duration2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_basic() {
        let timer = Timer::start("test");
        std::thread::sleep(Duration::from_millis(10));
        let duration = timer.stop();
        assert!(duration >= Duration::from_millis(10));
    }

    #[test]
    fn test_timer_drop() {
        init_metrics("/tmp/test_metrics.json");
        {
            let _timer = Timer::start("drop_test");
            std::thread::sleep(Duration::from_millis(5));
        }
        // Timer should have recorded when dropped
        let metrics = get_metrics().lock().unwrap();
        assert!(metrics.timings.contains_key("drop_test"));
    }

    #[test]
    fn test_metrics_collector() {
        let mut collector = MetricsCollector::new("/tmp/test.json");
        collector.record_timing("test", Duration::from_millis(100));
        collector.record_timing("test", Duration::from_millis(200));

        let timings = collector.get_timings_ms("test");
        assert_eq!(timings.len(), 2);
        assert_eq!(timings[0], 100);
        assert_eq!(timings[1], 200);
    }

    #[test]
    fn test_counters() {
        let mut collector = MetricsCollector::new("/tmp/test.json");
        collector.record_counter("operations", 10);
        collector.record_counter("operations", 5);

        assert_eq!(collector.counters.get("operations"), Some(&15));
    }

    #[test]
    fn test_stats() {
        let mut collector = MetricsCollector::new("/tmp/test.json");
        collector.record_timing("test", Duration::from_millis(100));
        collector.record_timing("test", Duration::from_millis(200));
        collector.record_timing("test", Duration::from_millis(300));

        let avg = collector.get_average_ms("test").unwrap();
        assert_eq!(avg, 200.0);

        let min = collector.get_min_ms("test").unwrap();
        assert_eq!(min, 100);

        let max = collector.get_max_ms("test").unwrap();
        assert_eq!(max, 300);
    }
}

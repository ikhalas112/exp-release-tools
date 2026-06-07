//! Unprotected Benchmark - Raw File I/O Performance
//!
//! This benchmark measures raw file I/O performance without any protection.
//! Used as baseline to compare against Maxion Protector's overhead.
//!
//! Run with: cargo run --release --example unprotected_bench

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct BenchmarkResult {
    name: String,
    duration: Duration,
    throughput_mbps: f64,
    data_size_kb: u64,
    operation_count: usize,
}

fn main() -> anyhow::Result<()> {
    println!("===========================================================================");
    println!("  Unprotected File I/O Performance Benchmark (Baseline)");
    println!("===========================================================================");
    println!();
    println!("This benchmark measures RAW file I/O performance WITHOUT protection.");
    println!("Use this as baseline to compare Maxion Protector's overhead.");
    println!();

    let temp_dir = tempfile::tempdir()?;
    let mut results: Vec<BenchmarkResult> = Vec::new();

    // Benchmark small files (1KB)
    results.push(benchmark_small_files(&temp_dir)?);
    println!();

    // Benchmark medium files (100KB)
    results.push(benchmark_medium_files(&temp_dir)?);
    println!();

    // Benchmark large files (1MB)
    results.push(benchmark_large_files(&temp_dir)?);
    println!();

    // Benchmark mixed workload (realistic game startup)
    results.push(benchmark_mixed_workload(&temp_dir)?);
    println!();

    // Print summary
    print_summary(&results);

    Ok(())
}

fn benchmark_small_files(temp_dir: &tempfile::TempDir) -> anyhow::Result<BenchmarkResult> {
    println!("===========================================================================");
    println!("  Small Files Benchmark (1KB x 100 files)");
    println!("===========================================================================");

    const FILE_COUNT: usize = 100;
    const FILE_SIZE: usize = 1024; // 1KB

    let files_dir = temp_dir.path().join("small_files");
    fs::create_dir_all(&files_dir)?;

    // Create test files
    let mut total_size = 0u64;
    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = vec![0xABu8; FILE_SIZE];
        fs::write(&file_path, &data)?;
        total_size += data.len() as u64;
    }

    println!(
        "Created {} files (total: {} KB)",
        FILE_COUNT,
        total_size / 1024
    );
    println!();

    // Warm-up run
    for i in 0..10 {
        let file_path = files_dir.join(format!("file_{:04}.dat", i % FILE_COUNT));
        let _ = fs::read(&file_path);
    }

    // Measure read performance
    let start = Instant::now();
    let mut bytes_read = 0usize;

    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = fs::read(&file_path)?;
        bytes_read += data.len();
    }

    let read_duration = start.elapsed();

    // Measure write performance
    let start = Instant::now();
    let mut bytes_written = 0usize;

    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = vec![0xCDu8; FILE_SIZE];
        fs::write(&file_path, &data)?;
        bytes_written += data.len();
    }

    let write_duration = start.elapsed();

    let total_duration = read_duration + write_duration;
    let throughput =
        (bytes_read + bytes_written) as f64 / (total_duration.as_secs_f64() * 1_000_000.0);

    println!(
        "✅ Read {} files: {:.3}ms",
        FILE_COUNT,
        read_duration.as_secs_f64() * 1000.0
    );
    println!(
        "✅ Write {} files: {:.3}ms",
        FILE_COUNT,
        write_duration.as_secs_f64() * 1000.0
    );
    println!("📊 Total Throughput: {:.2} MB/s", throughput);
    println!(
        "📊 Average Read Latency: {:.3}ms",
        read_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );
    println!(
        "📊 Average Write Latency: {:.3}ms",
        write_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );

    Ok(BenchmarkResult {
        name: "Small Files (1KB)".to_string(),
        duration: total_duration,
        throughput_mbps: throughput,
        data_size_kb: (bytes_read + bytes_written) as u64 / 1024,
        operation_count: FILE_COUNT * 2, // read + write
    })
}

fn benchmark_medium_files(temp_dir: &tempfile::TempDir) -> anyhow::Result<BenchmarkResult> {
    println!("===========================================================================");
    println!("  Medium Files Benchmark (100KB x 50 files)");
    println!("===========================================================================");

    const FILE_COUNT: usize = 50;
    const FILE_SIZE: usize = 100 * 1024; // 100KB

    let files_dir = temp_dir.path().join("medium_files");
    fs::create_dir_all(&files_dir)?;

    // Create test files
    let mut total_size = 0u64;
    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = vec![(i % 256) as u8; FILE_SIZE];
        fs::write(&file_path, &data)?;
        total_size += data.len() as u64;
    }

    println!(
        "Created {} files (total: {} KB)",
        FILE_COUNT,
        total_size / 1024
    );
    println!();

    // Warm-up runs (multiple for medium files to populate cache)
    for i in 0..5 {
        let file_path = files_dir.join(format!("file_{:04}.dat", i % FILE_COUNT));
        let _ = fs::read(&file_path);
    }

    // Measure read performance with BufReader (realistic)
    let start = Instant::now();
    let mut bytes_read = 0usize;

    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let file = File::open(&file_path)?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut buffer = Vec::with_capacity(FILE_SIZE);
        reader.read_to_end(&mut buffer)?;
        bytes_read += buffer.len();
    }

    let read_duration = start.elapsed();

    // Measure write performance with BufWriter (realistic)
    let start = Instant::now();
    let mut bytes_written = 0usize;

    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = vec![0xEFu8; FILE_SIZE];
        let file = File::create(&file_path)?;
        let mut writer = BufWriter::with_capacity(64 * 1024, file);
        writer.write_all(&data)?;
        writer.flush()?;
        bytes_written += data.len();
    }

    let write_duration = start.elapsed();

    let total_duration = read_duration + write_duration;
    let throughput =
        (bytes_read + bytes_written) as f64 / (total_duration.as_secs_f64() * 1_000_000.0);

    println!(
        "✅ Read {} files: {:.3}ms",
        FILE_COUNT,
        read_duration.as_secs_f64() * 1000.0
    );
    println!(
        "✅ Write {} files: {:.3}ms",
        FILE_COUNT,
        write_duration.as_secs_f64() * 1000.0
    );
    println!("📊 Total Throughput: {:.2} MB/s", throughput);
    println!(
        "📊 Average Read Latency: {:.3}ms",
        read_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );
    println!(
        "📊 Average Write Latency: {:.3}ms",
        write_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );

    Ok(BenchmarkResult {
        name: "Medium Files (100KB)".to_string(),
        duration: total_duration,
        throughput_mbps: throughput,
        data_size_kb: (bytes_read + bytes_written) as u64 / 1024,
        operation_count: FILE_COUNT * 2,
    })
}

fn benchmark_large_files(temp_dir: &tempfile::TempDir) -> anyhow::Result<BenchmarkResult> {
    println!("===========================================================================");
    println!("  Large Files Benchmark (1MB x 10 files)");
    println!("===========================================================================");

    const FILE_COUNT: usize = 10;
    const FILE_SIZE: usize = 1024 * 1024; // 1MB

    let files_dir = temp_dir.path().join("large_files");
    fs::create_dir_all(&files_dir)?;

    // Create test files
    let mut total_size = 0u64;
    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = vec![((i * 17) % 256) as u8; FILE_SIZE];
        fs::write(&file_path, &data)?;
        total_size += data.len() as u64;
    }

    println!(
        "Created {} files (total: {} MB)",
        FILE_COUNT,
        total_size / (1024 * 1024)
    );
    println!();

    // Warm-up run
    for i in 0..2 {
        let file_path = files_dir.join(format!("file_{:04}.dat", i % FILE_COUNT));
        let _ = fs::read(&file_path);
    }

    // Measure read performance with direct File::read (optimal for large files)
    let start = Instant::now();
    let mut bytes_read = 0usize;

    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = fs::read(&file_path)?;
        bytes_read += data.len();
    }

    let read_duration = start.elapsed();

    // Measure write performance with BufWriter
    let start = Instant::now();
    let mut bytes_written = 0usize;

    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = vec![0xDEu8; FILE_SIZE];
        let file = File::create(&file_path)?;
        let mut writer = BufWriter::with_capacity(1024 * 1024, file);
        writer.write_all(&data)?;
        writer.flush()?;
        bytes_written += data.len();
    }

    let write_duration = start.elapsed();

    let total_duration = read_duration + write_duration;
    let throughput =
        (bytes_read + bytes_written) as f64 / (total_duration.as_secs_f64() * 1_000_000.0);

    println!(
        "✅ Read {} files: {:.3}ms",
        FILE_COUNT,
        read_duration.as_secs_f64() * 1000.0
    );
    println!(
        "✅ Write {} files: {:.3}ms",
        FILE_COUNT,
        write_duration.as_secs_f64() * 1000.0
    );
    println!("📊 Total Throughput: {:.2} MB/s", throughput);
    println!(
        "📊 Average Read Latency: {:.3}ms",
        read_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );
    println!(
        "📊 Average Write Latency: {:.3}ms",
        write_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );

    Ok(BenchmarkResult {
        name: "Large Files (1MB)".to_string(),
        duration: total_duration,
        throughput_mbps: throughput,
        data_size_kb: (bytes_read + bytes_written) as u64 / 1024,
        operation_count: FILE_COUNT * 2,
    })
}

fn benchmark_mixed_workload(temp_dir: &tempfile::TempDir) -> anyhow::Result<BenchmarkResult> {
    println!("===========================================================================");
    println!("  Mixed Workload Benchmark (Realistic Game Startup)");
    println!("===========================================================================");
    println!("Simulates typical game startup with various file sizes:");
    println!("  - 20 small config files (1KB each)");
    println!("  - 10 medium assets (100KB each)");
    println!("  - 5 large resources (1MB each)");
    println!();

    let files_dir = temp_dir.path().join("mixed");
    fs::create_dir_all(&files_dir)?;

    let mut file_list: Vec<(PathBuf, usize)> = Vec::new();

    // Small files (configs, scripts)
    for i in 0..20 {
        let file_path = files_dir.join(format!("config_{:02}.cfg", i));
        let data = vec![0x55u8; 1024];
        fs::write(&file_path, &data)?;
        file_list.push((file_path, 1024));
    }

    // Medium files (textures, sounds)
    for i in 0..10 {
        let file_path = files_dir.join(format!("asset_{:02}.dat", i));
        let data = vec![0xAAu8; 100 * 1024];
        fs::write(&file_path, &data)?;
        file_list.push((file_path, 100 * 1024));
    }

    // Large files (models, music)
    for i in 0..5 {
        let file_path = files_dir.join(format!("resource_{:02}.bin", i));
        let data = vec![0xBBu8; 1024 * 1024];
        fs::write(&file_path, &data)?;
        file_list.push((file_path, 1024 * 1024));
    }

    println!(
        "Created {} files (total: {:.2} MB)",
        file_list.len(),
        file_list.iter().map(|(_, size)| *size).sum::<usize>() as f64 / (1024.0 * 1024.0)
    );
    println!();

    // Warm-up: Load files in realistic order (configs first, then assets, then resources)
    let mut warmup_order: Vec<usize> = (0..20).collect(); // configs
    warmup_order.extend(20..30); // assets
    warmup_order.extend(30..35); // resources

    for idx in warmup_order.iter().take(10) {
        if let Some((path, _)) = file_list.get(*idx) {
            let _ = fs::read(path);
        }
    }

    // Measure realistic workload: sequential load in game order
    let start = Instant::now();
    let mut bytes_loaded = 0usize;

    // Load configs
    println!("Loading 20 config files...");
    for i in 0..20 {
        if let Some((path, _size)) = file_list.get(i) {
            let data = fs::read(path)?;
            bytes_loaded += data.len();
        }
    }

    // Load assets
    println!("Loading 10 medium assets...");
    for i in 20..30 {
        if let Some((path, size)) = file_list.get(i) {
            let file = File::open(path)?;
            let mut reader = BufReader::with_capacity(64 * 1024, file);
            let mut buffer = Vec::with_capacity(*size);
            reader.read_to_end(&mut buffer)?;
            bytes_loaded += buffer.len();
        }
    }

    // Load resources
    println!("Loading 5 large resources...");
    for i in 30..35 {
        if let Some((path, _)) = file_list.get(i) {
            let data = fs::read(path)?;
            bytes_loaded += data.len();
        }
    }

    let load_duration = start.elapsed();

    println!(
        "✅ Loaded {} files in {:.3}ms",
        file_list.len(),
        load_duration.as_secs_f64() * 1000.0
    );
    println!(
        "📊 Total Data: {:.2} MB",
        bytes_loaded as f64 / (1024.0 * 1024.0)
    );
    println!(
        "📊 Throughput: {:.2} MB/s",
        bytes_loaded as f64 / (load_duration.as_secs_f64() * 1_000_000.0)
    );
    println!(
        "📊 Average Load Time: {:.3}ms per file",
        load_duration.as_secs_f64() * 1000.0 / file_list.len() as f64
    );

    Ok(BenchmarkResult {
        name: "Mixed Workload".to_string(),
        duration: load_duration,
        throughput_mbps: bytes_loaded as f64 / (load_duration.as_secs_f64() * 1_000_000.0),
        data_size_kb: bytes_loaded as u64 / 1024,
        operation_count: file_list.len(),
    })
}

fn print_summary(results: &[BenchmarkResult]) {
    println!();
    println!("===========================================================================");
    println!("  Benchmark Summary - Unprotected (Raw File I/O)");
    println!("===========================================================================");
    println!();

    let total_data: u64 = results.iter().map(|r| r.data_size_kb).sum();
    let total_time: Duration = results.iter().map(|r| r.duration).sum();
    let total_ops: usize = results.iter().map(|r| r.operation_count).sum();

    for result in results {
        println!("{}:", result.name);
        println!(
            "  Duration: {:.3}ms",
            result.duration.as_secs_f64() * 1000.0
        );
        println!("  Data: {} KB", result.data_size_kb);
        println!("  Throughput: {:.2} MB/s", result.throughput_mbps);
        println!("  Operations: {}", result.operation_count);
        println!();
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TOTALS:");
    println!(
        "  Total Duration: {:.3}ms",
        total_time.as_secs_f64() * 1000.0
    );
    println!("  Total Data: {:.2} MB", total_data as f64 / 1024.0);
    println!("  Total Operations: {}", total_ops);
    println!(
        "  Overall Throughput: {:.2} MB/s",
        total_data as f64 / (total_time.as_secs_f64() * 1.024)
    );
    println!(
        "  Average Latency: {:.3}ms per operation",
        total_time.as_secs_f64() * 1000.0 / total_ops as f64
    );
    println!("===========================================================================");
    println!();
    println!("💡 This is the BASELINE for unprotected (raw) file I/O.");
    println!("💡 Compare with protected benchmark to measure Maxion Protector's overhead.");
    println!();
}

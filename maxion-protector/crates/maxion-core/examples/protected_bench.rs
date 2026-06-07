//! Protected Benchmark - Maxion Protector Performance
//!
//! This benchmark measures file I/O performance WITH protection using
//! Maxion Protector's VirtualArchive with encryption and compression.
//!
//! Compare this with unprotected_bench.rs to see the protection overhead.
//!
//! Run with: cargo run --release --example protected_bench

use maxion_core::{
    archive::ArchiveBuilder,
    types::{ChunkSize, Config},
    virtual_archive::DefaultVirtualArchive as VirtualArchive,
};
use std::fs;
use std::time::{Duration, Instant};

struct BenchmarkResult {
    name: String,
    duration: Duration,
    throughput_mbps: f64,
    data_size_kb: u64,
    operation_count: usize,
    compression_ratio: f64,
    encryption_overhead_ns: f64,
}

fn main() -> anyhow::Result<()> {
    println!("===========================================================================");
    println!("  Protected File I/O Performance Benchmark (Maxion Protector)");
    println!("===========================================================================");
    println!();
    println!("This benchmark measures PROTECTED file I/O using:");
    println!("  - XChaCha20-Poly1305 authenticated encryption");
    println!("  - Brotli compression (level 6)");
    println!("  - 64KB chunk size");
    println!("  - VirtualArchive with LRU caching");
    println!();
    println!("Compare with unprotected_bench.rs to measure protection overhead.");
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
    println!("  Small Files Benchmark (1KB x 100 files) - PROTECTED");
    println!("===========================================================================");

    const FILE_COUNT: usize = 100;
    const FILE_SIZE: usize = 1024; // 1KB

    let files_dir = temp_dir.path().join("small_files");
    fs::create_dir_all(&files_dir)?;

    // Create test files
    let mut total_size = 0u64;
    let mut file_paths: Vec<std::path::PathBuf> = Vec::new();

    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = vec![0xABu8; FILE_SIZE];
        fs::write(&file_path, &data)?;
        total_size += data.len() as u64;
        file_paths.push(file_path);
    }

    println!(
        "Created {} files (total: {} KB)",
        FILE_COUNT,
        total_size / 1024
    );

    // Create encrypted archive
    let archive_path = temp_dir.path().join("small_files.archive");
    let mut config = Config::new();
    config.generate_keys();
    config.compress = true;
    config.chunk_size = ChunkSize(64 * 1024);

    println!();
    println!("Creating encrypted archive...");

    let pack_start = Instant::now();
    let mut builder = ArchiveBuilder::new(config.clone()).with_base_dir(&files_dir);

    for file_path in &file_paths {
        // Use just the filename (relative path) for archive storage
        let file_name = file_path
            .file_name()
            .expect("Failed to get file name")
            .to_str()
            .expect("Failed to convert file name to string");
        let asset_file = maxion_core::types::AssetFile::new(
            std::path::PathBuf::from(file_name),
            FILE_SIZE as u64,
        );
        builder.add_file(asset_file);
    }

    let _header = builder.build(&archive_path)?;
    let pack_duration = pack_start.elapsed();

    let archive_size = fs::metadata(&archive_path)?.len();
    let compression_ratio = archive_size as f64 / total_size as f64;

    println!(
        "✅ Archive created in {:.3}ms",
        pack_duration.as_secs_f64() * 1000.0
    );
    println!("✅ Archive size: {} KB", archive_size / 1024);
    println!(
        "✅ Compression ratio: {:.2}x ({:.1}% space saved)",
        compression_ratio,
        (1.0 - compression_ratio) * 100.0
    );
    println!();

    // Warm-up runs
    for _ in 0..5 {
        let mut archive = VirtualArchive::open(&archive_path, config.clone())?;
        let _ = archive.read_file("file_0000.dat")?;
    }

    // Measure archive loading (startup overhead)
    let load_times = (0..10)
        .map(|_| {
            let start = Instant::now();
            let _ = VirtualArchive::open(&archive_path, config.clone());
            start.elapsed()
        })
        .collect::<Vec<_>>();

    let avg_load_time = load_times.iter().sum::<Duration>() / load_times.len() as u32;

    println!("📊 Archive Loading (one-time):");
    println!("  Average: {:.3}ms", avg_load_time.as_secs_f64() * 1000.0);
    println!();

    // Measure read performance
    let mut archive = VirtualArchive::open(&archive_path, config.clone())?;

    let read_start = Instant::now();
    let mut bytes_read = 0usize;

    for (i, _file_path) in file_paths.iter().enumerate() {
        let file_name = format!("file_{:04}.dat", i);
        let data = archive.read_file(&file_name)?;
        bytes_read += data.len();
    }

    let read_duration = read_start.elapsed();

    let total_duration = read_duration; // Don't include pack time in runtime overhead
    let throughput = bytes_read as f64 / (read_duration.as_secs_f64() * 1_000_000.0);

    println!(
        "✅ Read {} files: {:.3}ms",
        FILE_COUNT,
        read_duration.as_secs_f64() * 1000.0
    );
    println!("📊 Total Throughput: {:.2} MB/s", throughput);
    println!(
        "📊 Average Read Latency: {:.3}ms",
        read_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );
    println!(
        "📊 Per-File Overhead: {:.3}ms (includes encryption + decompression)",
        read_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );

    Ok(BenchmarkResult {
        name: "Small Files (1KB)".to_string(),
        duration: total_duration,
        throughput_mbps: throughput,
        data_size_kb: bytes_read as u64 / 1024,
        operation_count: FILE_COUNT,
        compression_ratio,
        encryption_overhead_ns: read_duration.as_nanos() as f64 / FILE_COUNT as f64,
    })
}

fn benchmark_medium_files(temp_dir: &tempfile::TempDir) -> anyhow::Result<BenchmarkResult> {
    println!("===========================================================================");
    println!("  Medium Files Benchmark (100KB x 50 files) - PROTECTED");
    println!("===========================================================================");

    const FILE_COUNT: usize = 50;
    const FILE_SIZE: usize = 100 * 1024; // 100KB

    let files_dir = temp_dir.path().join("medium_files");
    fs::create_dir_all(&files_dir)?;

    // Create test files
    let mut total_size = 0u64;
    let mut file_paths: Vec<std::path::PathBuf> = Vec::new();

    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = vec![(i % 256) as u8; FILE_SIZE];
        fs::write(&file_path, &data)?;
        total_size += data.len() as u64;
        file_paths.push(file_path);
    }

    println!(
        "Created {} files (total: {} KB)",
        FILE_COUNT,
        total_size / 1024
    );

    // Create encrypted archive
    let archive_path = temp_dir.path().join("medium_files.archive");
    let mut config = Config::new();
    config.generate_keys();
    config.compress = true;
    config.chunk_size = ChunkSize(64 * 1024);

    println!();
    println!("Creating encrypted archive...");

    let pack_start = Instant::now();
    let mut builder = ArchiveBuilder::new(config.clone()).with_base_dir(&files_dir);

    for file_path in &file_paths {
        // Use just the filename (relative path) for archive storage
        let file_name = file_path
            .file_name()
            .expect("Failed to get file name")
            .to_str()
            .expect("Failed to convert file name to string");
        let asset_file = maxion_core::types::AssetFile::new(
            std::path::PathBuf::from(file_name),
            FILE_SIZE as u64,
        );
        builder.add_file(asset_file);
    }

    let _header = builder.build(&archive_path)?;
    let pack_duration = pack_start.elapsed();

    let archive_size = fs::metadata(&archive_path)?.len();
    let compression_ratio = archive_size as f64 / total_size as f64;

    println!(
        "✅ Archive created in {:.3}ms",
        pack_duration.as_secs_f64() * 1000.0
    );
    println!("✅ Archive size: {} KB", archive_size / 1024);
    println!(
        "✅ Compression ratio: {:.2}x ({:.1}% space saved)",
        compression_ratio,
        (1.0 - compression_ratio) * 100.0
    );
    println!();

    // Warm-up runs (multiple to populate LRU cache)
    for _ in 0..3 {
        let mut archive = VirtualArchive::open(&archive_path, config.clone())?;
        let _ = archive.read_file("file_0000.dat")?;
    }

    // Measure archive loading
    let load_times = (0..10)
        .map(|_| {
            let start = Instant::now();
            let _ = VirtualArchive::open(&archive_path, config.clone());
            start.elapsed()
        })
        .collect::<Vec<_>>();

    let avg_load_time = load_times.iter().sum::<Duration>() / load_times.len() as u32;

    println!("📊 Archive Loading (one-time):");
    println!("  Average: {:.3}ms", avg_load_time.as_secs_f64() * 1000.0);
    println!();

    // Measure read performance
    let mut archive = VirtualArchive::open(&archive_path, config.clone())?;

    let read_start = Instant::now();
    let mut bytes_read = 0usize;

    for (i, _) in file_paths.iter().enumerate() {
        let file_name = format!("file_{:04}.dat", i);
        let data = archive.read_file(&file_name)?;
        bytes_read += data.len();
    }

    let read_duration = read_start.elapsed();

    let total_duration = read_duration;
    let throughput = bytes_read as f64 / (read_duration.as_secs_f64() * 1_000_000.0);

    println!(
        "✅ Read {} files: {:.3}ms",
        FILE_COUNT,
        read_duration.as_secs_f64() * 1000.0
    );
    println!("📊 Total Throughput: {:.2} MB/s", throughput);
    println!(
        "📊 Average Read Latency: {:.3}ms",
        read_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );
    println!(
        "📊 Per-File Overhead: {:.3}ms (includes encryption + decompression)",
        read_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );

    Ok(BenchmarkResult {
        name: "Medium Files (100KB)".to_string(),
        duration: total_duration,
        throughput_mbps: throughput,
        data_size_kb: bytes_read as u64 / 1024,
        operation_count: FILE_COUNT,
        compression_ratio,
        encryption_overhead_ns: read_duration.as_nanos() as f64 / FILE_COUNT as f64,
    })
}

fn benchmark_large_files(temp_dir: &tempfile::TempDir) -> anyhow::Result<BenchmarkResult> {
    println!("===========================================================================");
    println!("  Large Files Benchmark (1MB x 10 files) - PROTECTED");
    println!("===========================================================================");

    const FILE_COUNT: usize = 10;
    const FILE_SIZE: usize = 1024 * 1024; // 1MB

    let files_dir = temp_dir.path().join("large_files");
    fs::create_dir_all(&files_dir)?;

    // Create test files
    let mut total_size = 0u64;
    let mut file_paths: Vec<std::path::PathBuf> = Vec::new();

    for i in 0..FILE_COUNT {
        let file_path = files_dir.join(format!("file_{:04}.dat", i));
        let data = vec![((i * 17) % 256) as u8; FILE_SIZE];
        fs::write(&file_path, &data)?;
        total_size += data.len() as u64;
        file_paths.push(file_path);
    }

    println!(
        "Created {} files (total: {} MB)",
        FILE_COUNT,
        total_size / (1024 * 1024)
    );

    // Create encrypted archive
    let archive_path = temp_dir.path().join("large_files.archive");
    let mut config = Config::new();
    config.generate_keys();
    config.compress = true;
    config.chunk_size = ChunkSize(64 * 1024);

    println!();
    println!("Creating encrypted archive...");

    let pack_start = Instant::now();
    let mut builder = ArchiveBuilder::new(config.clone()).with_base_dir(&files_dir);

    for file_path in &file_paths {
        // Use just the filename (relative path) for archive storage
        let file_name = file_path
            .file_name()
            .expect("Failed to get file name")
            .to_str()
            .expect("Failed to convert file name to string");
        let asset_file = maxion_core::types::AssetFile::new(
            std::path::PathBuf::from(file_name),
            FILE_SIZE as u64,
        );
        builder.add_file(asset_file);
    }

    let _header = builder.build(&archive_path)?;
    let pack_duration = pack_start.elapsed();

    let archive_size = fs::metadata(&archive_path)?.len();
    let compression_ratio = archive_size as f64 / total_size as f64;

    println!(
        "✅ Archive created in {:.3}ms",
        pack_duration.as_secs_f64() * 1000.0
    );
    println!("✅ Archive size: {} KB", archive_size / 1024);
    println!(
        "✅ Compression ratio: {:.2}x ({:.1}% space saved)",
        compression_ratio,
        (1.0 - compression_ratio) * 100.0
    );
    println!();

    // Warm-up run
    for _ in 0..2 {
        let mut archive = VirtualArchive::open(&archive_path, config.clone())?;
        let _ = archive.read_file("file_0000.dat")?;
    }

    // Measure archive loading
    let load_times = (0..10)
        .map(|_| {
            let start = Instant::now();
            let _ = VirtualArchive::open(&archive_path, config.clone());
            start.elapsed()
        })
        .collect::<Vec<_>>();

    let avg_load_time = load_times.iter().sum::<Duration>() / load_times.len() as u32;

    println!("📊 Archive Loading (one-time):");
    println!("  Average: {:.3}ms", avg_load_time.as_secs_f64() * 1000.0);
    println!();

    // Measure read performance
    let mut archive = VirtualArchive::open(&archive_path, config.clone())?;

    let read_start = Instant::now();
    let mut bytes_read = 0usize;

    for (i, _) in file_paths.iter().enumerate() {
        let file_name = format!("file_{:04}.dat", i);
        let data = archive.read_file(&file_name)?;
        bytes_read += data.len();
    }

    let read_duration = read_start.elapsed();

    let total_duration = read_duration;
    let throughput = bytes_read as f64 / (read_duration.as_secs_f64() * 1_000_000.0);

    println!(
        "✅ Read {} files: {:.3}ms",
        FILE_COUNT,
        read_duration.as_secs_f64() * 1000.0
    );
    println!("📊 Total Throughput: {:.2} MB/s", throughput);
    println!(
        "📊 Average Read Latency: {:.3}ms",
        read_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );
    println!(
        "📊 Per-File Overhead: {:.3}ms (includes encryption + decompression)",
        read_duration.as_secs_f64() * 1000.0 / FILE_COUNT as f64
    );

    Ok(BenchmarkResult {
        name: "Large Files (1MB)".to_string(),
        duration: total_duration,
        throughput_mbps: throughput,
        data_size_kb: bytes_read as u64 / 1024,
        operation_count: FILE_COUNT,
        compression_ratio,
        encryption_overhead_ns: read_duration.as_nanos() as f64 / FILE_COUNT as f64,
    })
}

fn benchmark_mixed_workload(temp_dir: &tempfile::TempDir) -> anyhow::Result<BenchmarkResult> {
    println!("===========================================================================");
    println!("  Mixed Workload Benchmark (Realistic Game Startup) - PROTECTED");
    println!("===========================================================================");
    println!("Simulates typical game startup with various file sizes:");
    println!("  - 20 small config files (1KB each)");
    println!("  - 10 medium assets (100KB each)");
    println!("  - 5 large resources (1MB each)");
    println!();

    let files_dir = temp_dir.path().join("mixed");
    fs::create_dir_all(&files_dir)?;

    let mut file_list: Vec<(String, usize)> = Vec::new();

    // Small files (configs, scripts)
    for i in 0..20 {
        let file_name = format!("config_{:02}.cfg", i);
        let file_path = files_dir.join(&file_name);
        let data = vec![0x55u8; 1024];
        fs::write(&file_path, &data)?;
        file_list.push((file_name, 1024));
    }

    // Medium files (textures, sounds)
    for i in 0..10 {
        let file_name = format!("asset_{:02}.dat", i);
        let file_path = files_dir.join(&file_name);
        let data = vec![0xAAu8; 100 * 1024];
        fs::write(&file_path, &data)?;
        file_list.push((file_name, 100 * 1024));
    }

    // Large files (models, music)
    for i in 0..5 {
        let file_name = format!("resource_{:02}.bin", i);
        let file_path = files_dir.join(&file_name);
        let data = vec![0xBBu8; 1024 * 1024];
        fs::write(&file_path, &data)?;
        file_list.push((file_name, 1024 * 1024));
    }

    println!(
        "Created {} files (total: {:.2} MB)",
        file_list.len(),
        file_list.iter().map(|(_, size)| *size).sum::<usize>() as f64 / (1024.0 * 1024.0)
    );

    // Create encrypted archive
    let archive_path = temp_dir.path().join("mixed.archive");
    let mut config = Config::new();
    config.generate_keys();
    config.compress = true;
    config.chunk_size = ChunkSize(64 * 1024);

    println!();
    println!("Creating encrypted archive...");

    let pack_start = Instant::now();
    let mut builder = ArchiveBuilder::new(config.clone()).with_base_dir(&files_dir);

    for (file_name, size) in &file_list {
        // Use relative path (file_name) for archive storage
        let asset_file =
            maxion_core::types::AssetFile::new(std::path::PathBuf::from(file_name), *size as u64);
        builder.add_file(asset_file);
    }

    let _header = builder.build(&archive_path)?;
    let pack_duration = pack_start.elapsed();

    let total_size: u64 = file_list.iter().map(|(_, size)| *size as u64).sum();
    let archive_size = fs::metadata(&archive_path)?.len();
    let compression_ratio = archive_size as f64 / total_size as f64;

    println!(
        "✅ Archive created in {:.3}ms",
        pack_duration.as_secs_f64() * 1000.0
    );
    println!("✅ Archive size: {} KB", archive_size / 1024);
    println!(
        "✅ Compression ratio: {:.2}x ({:.1}% space saved)",
        compression_ratio,
        (1.0 - compression_ratio) * 100.0
    );
    println!();

    // Warm-up: Load files in realistic order
    let warmup_order: Vec<usize> = (0..20).chain(20..30).chain(30..35).collect();
    for idx in warmup_order.iter().take(10) {
        if let Some((file_name, _)) = file_list.get(*idx) {
            let mut archive = VirtualArchive::open(&archive_path, config.clone())?;
            let _ = archive.read_file(file_name)?;
        }
    }

    // Measure archive loading (startup overhead)
    let load_times = (0..10)
        .map(|_| {
            let start = Instant::now();
            let _ = VirtualArchive::open(&archive_path, config.clone());
            start.elapsed()
        })
        .collect::<Vec<_>>();

    let avg_load_time = load_times.iter().sum::<Duration>() / load_times.len() as u32;

    println!("📊 Archive Loading (one-time):");
    println!("  Average: {:.3}ms", avg_load_time.as_secs_f64() * 1000.0);
    println!();

    // Measure realistic workload: sequential load in game order
    let mut archive = VirtualArchive::open(&archive_path, config.clone())?;

    let load_start = Instant::now();
    let mut bytes_loaded = 0usize;

    // Load configs
    println!("Loading 20 config files...");
    for i in 0..20 {
        if let Some((file_name, _size)) = file_list.get(i) {
            let data = archive.read_file(file_name)?;
            bytes_loaded += data.len();
        }
    }

    // Load assets
    println!("Loading 10 medium assets...");
    for i in 20..30 {
        if let Some((file_name, _size)) = file_list.get(i) {
            let data = archive.read_file(file_name)?;
            bytes_loaded += data.len();
        }
    }

    // Load resources
    println!("Loading 5 large resources...");
    for i in 30..35 {
        if let Some((file_name, _size)) = file_list.get(i) {
            let data = archive.read_file(file_name)?;
            bytes_loaded += data.len();
        }
    }

    let load_duration = load_start.elapsed();

    let total_duration = load_duration;
    let throughput = bytes_loaded as f64 / (load_duration.as_secs_f64() * 1_000_000.0);

    println!(
        "✅ Loaded {} files in {:.3}ms",
        file_list.len(),
        load_duration.as_secs_f64() * 1000.0
    );
    println!(
        "📊 Total Data: {:.2} MB",
        bytes_loaded as f64 / (1024.0 * 1024.0)
    );
    println!("📊 Throughput: {:.2} MB/s", throughput);
    println!(
        "📊 Average Load Time: {:.3}ms per file",
        load_duration.as_secs_f64() * 1000.0 / file_list.len() as f64
    );

    Ok(BenchmarkResult {
        name: "Mixed Workload".to_string(),
        duration: total_duration,
        throughput_mbps: throughput,
        data_size_kb: bytes_loaded as u64 / 1024,
        operation_count: file_list.len(),
        compression_ratio,
        encryption_overhead_ns: load_duration.as_nanos() as f64 / file_list.len() as f64,
    })
}

fn print_summary(results: &[BenchmarkResult]) {
    println!();
    println!("===========================================================================");
    println!("  Benchmark Summary - Protected (Maxion Protector)");
    println!("===========================================================================");
    println!();
    println!("Protection: XChaCha20-Poly1305 encryption + Brotli compression (level 6)");
    println!("Chunk Size: 64KB");
    println!("Caching: LRU (128 chunks, 16 files)");
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
        println!(
            "  Compression Ratio: {:.2}x ({:.1}% space saved)",
            result.compression_ratio,
            (1.0 - result.compression_ratio) * 100.0
        );
        println!(
            "  Per-File Overhead: {:.3}ms",
            result.encryption_overhead_ns / 1_000_000.0
        );
        println!();
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TOTALS (Runtime - excludes packing time):");
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
    println!("💡 This is PROTECTED performance with encryption and compression.");
    println!("💡 Compare with unprotected_bench.rs to see protection overhead.");
    println!("💡 The overhead includes: decryption, decompression, cache lookups.");
    println!("💡 Compression provides huge I/O benefits that offset overhead.");
    println!();
}

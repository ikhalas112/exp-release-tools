// maxion-protector/tests/benchmarks/benchmark.rs
//! Comprehensive performance benchmarks for Maxion Protector
//!
//! This suite benchmarks all critical operations to ensure performance meets targets.

use maxion_core::{
    archive::ArchiveBuilder,
    config::{Compression, Config},
    debug::{MemoryTracker, PerformanceProfiler},
    error::Result,
    types::ArchiveHeader,
    virtual_archive::DefaultVirtualArchive as VirtualArchive,
};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Benchmark configuration constants
const SMALL_FILE_SIZE: usize = 1_024; // 1 KB
const MEDIUM_FILE_SIZE: usize = 102_400; // 100 KB
const LARGE_FILE_SIZE: usize = 1_048_576; // 1 MB
const CHUNK_SIZE: usize = 16_384; // 16 KB

/// Number of iterations for each benchmark
const ITERATIONS: usize = 10;

/// Create test data with specific pattern
fn create_test_data(size: usize, pattern: u8) -> Vec<u8> {
    vec![pattern; size]
}

/// Create test data with sequential pattern
fn create_sequential_data(size: usize) -> Vec<u8> {
    (0..size as u8).cycle().take(size).collect()
}

/// Create random-like data (non-compressible)
fn create_random_data(size: usize) -> Vec<u8> {
    let mut data = vec![0u8; size];
    let seed: u64 = 12345;
    let mut state = seed;
    for byte in &mut data {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        *byte = (state >> 24) as u8;
    }
    data
}

/// Helper function to measure execution time
fn measure_time<F>(f: F) -> Duration
where
    F: FnOnce(),
{
    let start = Instant::now();
    f();
    start.elapsed()
}

/// Helper function to measure average execution time
fn measure_average<F>(iterations: usize, f: F) -> Duration
where
    F: Fn(),
{
    let mut total = Duration::from_nanos(0);
    for _ in 0..iterations {
        total += measure_time(&f);
    }
    total / iterations as u32
}

/// Print benchmark header
fn print_header(title: &str) {
    println!("\n{}", "=".repeat(80));
    println!("  {}", title);
    println!("{}", "=".repeat(80));
}

/// Print benchmark result
fn print_result(name: &str, duration: Duration, ops_per_sec: Option<f64>) {
    println!("{:<50} {:>12.3?}", name, duration);
    if let Some(ops) = ops_per_sec {
        println!("{:50} {:>12.1} ops/sec", "", ops);
    }
}

/// Print comparison with target
fn print_comparison(actual: Duration, target: Duration, metric: &str) {
    let ratio = actual.as_secs_f64() / target.as_secs_f64();
    let status = if ratio <= 1.0 {
        "✅ PASS"
    } else if ratio <= 1.5 {
        "⚠️  WARN"
    } else {
        "❌ FAIL"
    };
    println!(
        "  {} Target: {:>12.3?} | Actual: {:>12.3?} | Ratio: {:.2}x",
        status, target, actual, ratio
    );
}

fn main() -> Result<()> {
    // Initialize debug tools
    let profiler = PerformanceProfiler::new();
    let memory_tracker = MemoryTracker::new();

    print_header("Maxion Protector Performance Benchmarks");

    // Run all benchmark suites
    benchmark_encryption_decryption()?;
    benchmark_compression_decompression()?;
    benchmark_archive_operations()?;
    benchmark_virtual_file_system()?;
    benchmark_cache_performance()?;
    benchmark_concurrent_operations()?;
    benchmark_realistic_scenarios()?;

    print_summary(&profiler, &memory_tracker);

    Ok(())
}

/// Benchmark encryption and decryption operations
fn benchmark_encryption_decryption() -> Result<()> {
    print_header("Encryption & Decryption Benchmarks");

    let profiler = PerformanceProfiler::new();
    let key = [0u8; 32]; // Test key
    let nonce = [0u8; 12]; // Test nonce

    // Test different data sizes
    let sizes = [
        ("Small (1 KB)", SMALL_FILE_SIZE),
        ("Medium (100 KB)", MEDIUM_FILE_SIZE),
        ("Large (1 MB)", LARGE_FILE_SIZE),
    ];

    for (name, size) in sizes.iter() {
        let data = create_test_data(*size, 0x42);

        // Benchmark encryption
        let encrypt_duration = measure_average(ITERATIONS, || {
            let _timer = profiler.start_timer(format!("encrypt_{}", name));
            let _encrypted = maxion_core::crypto::encrypt(&data, &key, &nonce);
        });

        print_result(&format!("Encryption - {}", name), encrypt_duration, None);

        // Benchmark decryption
        let encrypted = maxion_core::crypto::encrypt(&data, &key, &nonce)?;
        let decrypt_duration = measure_average(ITERATIONS, || {
            let _timer = profiler.start_timer(format!("decrypt_{}", name));
            let _decrypted = maxion_core::crypto::decrypt(&encrypted, &key, &nonce);
        });

        print_result(&format!("Decryption - {}", name), decrypt_duration, None);

        // Compare with targets
        let encrypt_target = Duration::from_micros(*size as u64 / 10);
        let decrypt_target = Duration::from_micros(*size as u64 / 10);

        print_comparison(encrypt_duration, encrypt_target, "Encryption");
        print_comparison(decrypt_duration, decrypt_target, "Decryption");
    }

    Ok(())
}

/// Benchmark compression and decompression operations
fn benchmark_compression_decompression() -> Result<()> {
    print_header("Compression & Decompression Benchmarks");

    let profiler = PerformanceProfiler::new();

    // Test different data patterns
    let patterns = [
        (
            "Highly compressible (repeated)",
            Compression::Brotli(4),
            true,
        ),
        (
            "Less compressible (sequential)",
            Compression::Brotli(4),
            false,
        ),
        ("Incompressible (random)", Compression::Brotli(4), false),
    ];

    for (name, compression, compressible) in patterns.iter() {
        let data = if *compressible {
            create_test_data(LARGE_FILE_SIZE, 0x42)
        } else if name.contains("sequential") {
            create_sequential_data(LARGE_FILE_SIZE)
        } else {
            create_random_data(LARGE_FILE_SIZE)
        };

        // Benchmark compression
        let compress_duration = measure_average(ITERATIONS, || {
            let _timer = profiler.start_timer(format!("compress_{}", name));
            let _compressed = maxion_core::compression::compress(&data, compression);
        });

        print_result(&format!("Compression - {}", name), compress_duration, None);

        // Compress once for decompression test
        let compressed = maxion_core::compression::compress(&data, compression)?;

        // Benchmark decompression
        let decompress_duration = measure_average(ITERATIONS, || {
            let _timer = profiler.start_timer(format!("decompress_{}", name));
            let _decompressed = maxion_core::compression::decompress(&compressed);
        });

        print_result(
            &format!("Decompression - {}", name),
            decompress_duration,
            None,
        );

        // Report compression ratio
        let ratio = compressed.len() as f64 / data.len() as f64;
        println!("{:50} {:>12.2}x compression", "", ratio);
    }

    Ok(())
}

/// Benchmark archive operations
fn benchmark_archive_operations() -> Result<()> {
    print_header("Archive Operations Benchmarks");

    let profiler = PerformanceProfiler::new();
    let memory_tracker = MemoryTracker::new();
    let temp_dir = tempfile::tempdir()?;

    // Test different archive sizes
    let archive_sizes = [
        ("Small (10 files, 1 KB each)", 10, SMALL_FILE_SIZE),
        ("Medium (100 files, 10 KB each)", 100, 10 * SMALL_FILE_SIZE),
        ("Large (1000 files, 10 KB each)", 1000, 10 * SMALL_FILE_SIZE),
    ];

    for (name, file_count, file_size) in archive_sizes.iter() {
        let archive_path = temp_dir.path().join(format!("archive_{}.bin", name));

        // Create test files
        memory_tracker.record_allocation("test_files", file_count * file_size);

        // Benchmark archive creation
        let create_duration = measure_average(3, || {
            let _timer = profiler.start_timer(format!("create_archive_{}", name));

            let config = Config {
                compression: Compression::Brotli(4),
                chunk_size: CHUNK_SIZE,
                ..Default::default()
            };

            let mut builder = ArchiveBuilder::new(&archive_path, config)
                .expect("Failed to create archive builder");

            for i in 0..*file_count {
                let data = create_sequential_data(*file_size);
                let file_path = format!("file_{:05}.dat", i);
                builder
                    .add_file(&file_path, &data)
                    .expect("Failed to add file");
            }

            builder.build().expect("Failed to build archive");
        });

        print_result(&format!("Create Archive - {}", name), create_duration, None);

        // Calculate creation throughput
        let total_size = file_count * file_size;
        let throughput_mb = (total_size as f64 / 1_048_576.0) / create_duration.as_secs_f64();
        println!("{:50} {:>12.2} MB/s", "", throughput_mb);

        // Benchmark archive loading
        let load_duration = measure_average(10, || {
            let _timer = profiler.start_timer(format!("load_archive_{}", name));
            let _archive = VirtualArchive::open(&archive_path).expect("Failed to open archive");
        });

        print_result(&format!("Load Archive - {}", name), load_duration, None);

        // Benchmark file reading
        let read_duration = measure_average(100, || {
            let _timer = profiler.start_timer(format!("read_file_{}", name));
            let archive = VirtualArchive::open(&archive_path).expect("Failed to open archive");
            let _data = archive
                .read_file("file_00000.dat")
                .expect("Failed to read file");
        });

        print_result(&format!("Read File - {}", name), read_duration, None);

        // Compare with targets
        let create_target = Duration::from_millis((file_count * file_size / 10_000) as u64);
        let load_target = Duration::from_millis(100);
        let read_target = Duration::from_micros(500);

        print_comparison(create_duration, create_target, "Archive Creation");
        print_comparison(load_duration, load_target, "Archive Loading");
        print_comparison(read_duration, read_target, "File Reading");
    }

    Ok(())
}

/// Benchmark virtual file system operations
fn benchmark_virtual_file_system() -> Result<()> {
    print_header("Virtual File System Benchmarks");

    let profiler = PerformanceProfiler::new();
    let temp_dir = tempfile::tempdir()?;

    // Create test archive
    let archive_path = temp_dir.path().join("vfs_test.bin");
    let config = Config {
        compression: Compression::Brotli(4),
        chunk_size: CHUNK_SIZE,
        ..Default::default()
    };

    let mut builder = ArchiveBuilder::new(&archive_path, config)?;
    for i in 0..100 {
        let data = create_test_data(MEDIUM_FILE_SIZE, (i % 256) as u8);
        builder.add_file(&format!("file_{:03}.dat", i), &data)?;
    }
    builder.build()?;

    let archive = VirtualArchive::open(&archive_path)?;

    // Benchmark file lookup
    let lookup_duration = measure_average(1000, || {
        let _timer = profiler.start_timer("vfs_lookup");
        let _file_info = archive.get_file_info("file_042.dat");
    });

    print_result("VFS File Lookup", lookup_duration, None);

    // Benchmark sequential reads
    let sequential_read_duration = measure_average(10, || {
        let _timer = profiler.start_timer("vfs_sequential_read");
        for i in 0..100 {
            let _data = archive.read_file(&format!("file_{:03}.dat", i))?;
        }
        Ok::<(), maxion_core::error::Error>(())
    })?;

    print_result(
        "VFS Sequential Read (100 files)",
        sequential_read_duration,
        None,
    );

    // Benchmark random reads
    let random_read_duration = measure_average(10, || {
        let _timer = profiler.start_timer("vfs_random_read");
        for i in (0..100).rev() {
            let _data = archive.read_file(&format!("file_{:03}.dat", i))?;
        }
        Ok::<(), maxion_core::error::Error>(())
    })?;

    print_result("VFS Random Read (100 files)", random_read_duration, None);

    // Compare with targets
    let lookup_target = Duration::from_micros(10);
    let sequential_target = Duration::from_millis(100);
    let random_target = Duration::from_millis(150);

    print_comparison(lookup_duration, lookup_target, "File Lookup");
    print_comparison(
        sequential_read_duration,
        sequential_target,
        "Sequential Read",
    );
    print_comparison(random_read_duration, random_target, "Random Read");

    Ok(())
}

/// Benchmark cache performance
fn benchmark_cache_performance() -> Result<()> {
    print_header("Cache Performance Benchmarks");

    let profiler = PerformanceProfiler::new();
    let temp_dir = tempfile::tempdir()?;

    // Create test archive
    let archive_path = temp_dir.path().join("cache_test.bin");
    let config = Config {
        compression: Compression::Brotli(4),
        chunk_size: CHUNK_SIZE,
        ..Default::default()
    };

    let mut builder = ArchiveBuilder::new(&archive_path, config)?;
    for i in 0..50 {
        let data = create_test_data(MEDIUM_FILE_SIZE, (i % 256) as u8);
        builder.add_file(&format!("file_{:03}.dat", i), &data)?;
    }
    builder.build()?;

    // Benchmark cold cache (first read)
    let cold_cache_duration = measure_average(10, || {
        let _timer = profiler.start_timer("cache_cold");
        let archive = VirtualArchive::open(&archive_path)?;
        let _data = archive.read_file("file_025.dat")?;
        Ok::<(), maxion_core::error::Error>(())
    })?;

    print_result("Cache - Cold Read", cold_cache_duration, None);

    // Benchmark warm cache (repeated reads)
    let warm_cache_duration = measure_average(100, || {
        let _timer = profiler.start_timer("cache_warm");
        let archive = VirtualArchive::open(&archive_path)?;
        // First read to populate cache
        let _ = archive.read_file("file_025.dat")?;
        // Second read from cache
        let _data = archive.read_file("file_025.dat")?;
        Ok::<(), maxion_core::error::Error>(())
    })?;

    print_result("Cache - Warm Read", warm_cache_duration, None);

    // Calculate cache speedup
    let speedup = cold_cache_duration.as_secs_f64() / warm_cache_duration.as_secs_f64();
    println!("{:50} {:>12.2}x speedup", "", speedup);

    // Compare with targets
    let cold_target = Duration::from_millis(10);
    let warm_target = Duration::from_micros(100);

    print_comparison(cold_cache_duration, cold_target, "Cold Cache");
    print_comparison(warm_cache_duration, warm_target, "Warm Cache");

    Ok(())
}

/// Benchmark concurrent operations
fn benchmark_concurrent_operations() -> Result<()> {
    print_header("Concurrent Operations Benchmarks");

    let profiler = PerformanceProfiler::new();
    let temp_dir = tempfile::tempdir()?;

    // Create test archive
    let archive_path = temp_dir.path().join("concurrent_test.bin");
    let config = Config {
        compression: Compression::Brotli(4),
        chunk_size: CHUNK_SIZE,
        ..Default::default()
    };

    let mut builder = ArchiveBuilder::new(&archive_path, config)?;
    for i in 0..50 {
        let data = create_test_data(MEDIUM_FILE_SIZE, (i % 256) as u8);
        builder.add_file(&format!("file_{:03}.dat", i), &data)?;
    }
    builder.build()?;

    // Benchmark single-threaded reads
    let single_thread_duration = measure_average(5, || {
        let _timer = profiler.start_timer("single_thread");
        let archive = VirtualArchive::open(&archive_path)?;
        for i in 0..50 {
            let _data = archive.read_file(&format!("file_{:03}.dat", i))?;
        }
        Ok::<(), maxion_core::error::Error>(())
    })?;

    print_result(
        "Concurrent - Single Thread (50 files)",
        single_thread_duration,
        None,
    );

    // Benchmark multi-threaded reads (simulate with separate archives)
    let multi_thread_duration = measure_average(5, || {
        let _timer = profiler.start_timer("multi_thread");
        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                let archive_path = archive_path.clone();
                std::thread::spawn(move || {
                    let archive = VirtualArchive::open(&archive_path)?;
                    for i in (thread_id * 12)..((thread_id + 1) * 12).min(50) {
                        let _data = archive.read_file(&format!("file_{:03}.dat", i))?;
                    }
                    Ok::<(), maxion_core::error::Error>(())
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap()?;
        }
        Ok::<(), maxion_core::error::Error>(())
    })?;

    print_result(
        "Concurrent - Multi Thread (4 threads, 50 files)",
        multi_thread_duration,
        None,
    );

    // Calculate parallel efficiency
    let efficiency =
        single_thread_duration.as_secs_f64() / (multi_thread_duration.as_secs_f64() * 4.0);
    println!("{:50} {:>12.2}x efficiency", "", efficiency);

    Ok(())
}

/// Benchmark realistic scenarios
fn benchmark_realistic_scenarios() -> Result<()> {
    print_header("Realistic Scenario Benchmarks");

    let profiler = PerformanceProfiler::new();
    let temp_dir = tempfile::tempdir()?;

    // Scenario 1: Game asset loading
    println!("\nScenario 1: Game Asset Loading (50 files, mixed sizes)");
    let archive_path = temp_dir.path().join("game_assets.bin");
    let config = Config {
        compression: Compression::Brotli(4),
        chunk_size: CHUNK_SIZE,
        ..Default::default()
    };

    let mut builder = ArchiveBuilder::new(&archive_path, config)?;

    // Add various file sizes (textures, models, sounds, etc.)
    for i in 0..20 {
        let size = match i % 4 {
            0 => 2_097_152, // 2 MB textures
            1 => 524_288,   // 512 KB models
            2 => 131_072,   // 128 KB sounds
            _ => 4_096,     // 4 KB scripts
        };
        let data = create_sequential_data(size);
        builder.add_file(&format!("asset_{:02}_{:03}.dat", i % 4, i), &data)?;
    }
    builder.build()?;

    let load_duration = measure_average(5, || {
        let _timer = profiler.start_timer("game_load");
        let archive = VirtualArchive::open(&archive_path)?;
        // Load assets in typical game order (textures first, then models, etc.)
        for i in 0..20 {
            let _data = archive.read_file(&format!("asset_{:02}_{:03}.dat", i % 4, i))?;
        }
        Ok::<(), maxion_core::error::Error>(())
    })?;

    print_result("Game Asset Loading", load_duration, None);

    // Scenario 2: UI theme loading
    println!("\nScenario 2: UI Theme Loading (100 small files)");
    let archive_path = temp_dir.path().join("ui_theme.bin");
    let config = Config {
        compression: Compression::Brotli(4),
        chunk_size: CHUNK_SIZE,
        ..Default::default()
    };

    let mut builder = ArchiveBuilder::new(&archive_path, config)?;
    for i in 0..100 {
        let data = create_test_data(1_024, (i % 256) as u8);
        builder.add_file(&format!("ui_{:03}.xml", i), &data)?;
    }
    builder.build()?;

    let ui_load_duration = measure_average(10, || {
        let _timer = profiler.start_timer("ui_load");
        let archive = VirtualArchive::open(&archive_path)?;
        for i in 0..100 {
            let _data = archive.read_file(&format!("ui_{:03}.xml", i))?;
        }
        Ok::<(), maxion_core::error::Error>(())
    })?;

    print_result("UI Theme Loading", ui_load_duration, None);

    // Scenario 3: Level streaming
    println!("\nScenario 3: Level Streaming (10 large files)");
    let archive_path = temp_dir.path().join("level_data.bin");
    let config = Config {
        compression: Compression::Brotli(4),
        chunk_size: CHUNK_SIZE,
        ..Default::default()
    };

    let mut builder = ArchiveBuilder::new(&archive_path, config)?;
    for i in 0..10 {
        let data = create_random_data(5_242_880); // 5 MB each
        builder.add_file(&format!("level_{:02}.bin", i), &data)?;
    }
    builder.build()?;

    let stream_duration = measure_average(5, || {
        let _timer = profiler.start_timer("level_stream");
        let archive = VirtualArchive::open(&archive_path)?;
        // Stream levels sequentially
        for i in 0..10 {
            let _data = archive.read_file(&format!("level_{:02}.bin", i))?;
        }
        Ok::<(), maxion_core::error::Error>(())
    })?;

    print_result("Level Streaming", stream_duration, None);

    // Compare with realistic targets
    let game_load_target = Duration::from_millis(5000); // 5 seconds
    let ui_load_target = Duration::from_millis(1000); // 1 second
    let stream_target = Duration::from_millis(2000); // 2 seconds

    print_comparison(load_duration, game_load_target, "Game Load");
    print_comparison(ui_load_duration, ui_load_target, "UI Load");
    print_comparison(stream_duration, stream_target, "Level Stream");

    Ok(())
}

/// Print summary of all benchmarks
fn print_summary(profiler: &PerformanceProfiler, memory_tracker: &MemoryTracker) {
    print_header("Benchmark Summary");

    let stats = profiler.get_stats();
    println!("\nPerformance Statistics:");
    println!("  Total measurements: {}", stats.count);
    println!("  Total time: {:.3?}", stats.total);
    println!("  Average time: {:.3?}", stats.avg);
    println!("  Median time: {:.3?}", stats.median);
    println!("  Min time: {:.3?}", stats.min);
    println!("  Max time: {:.3?}", stats.max);

    let memory = memory_tracker.total();
    println!("\nMemory Tracking:");
    println!(
        "  Total allocated: {} bytes ({:.2} MB)",
        memory,
        memory as f64 / 1_048_576.0
    );

    println!("\n{}", "=".repeat(80));
    println!("  Benchmarking Complete");
    println!("{}", "=".repeat(80));
    println!("\nTips for improving performance:");
    println!("  - Use appropriate chunk sizes (16KB is a good default)");
    println!("  - Enable compression for compressible data");
    println!("  - Leverage cache for frequently accessed files");
    println!("  - Consider pre-loading critical assets");
    println!("  - Use async I/O for large file operations");
}

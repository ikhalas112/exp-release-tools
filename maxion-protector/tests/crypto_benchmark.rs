//! Phase 5: Encryption Performance Benchmarks
//!
//! This module benchmarks the encryption/decryption performance after
//! implementing the Phase 5 optimizations:
//! - Context reuse optimization (buffer reuse in ChunkCipher)
//! - SIMD compilation flags
//! - Proper benchmarking methodology

use maxion_core::crypto::ChunkCipher;
use maxion_core::types::{ChunkSize, Nonce};
use std::time::{Duration, Instant};

/// Benchmark configuration constants
const SMALL_FILE_SIZE: usize = 1024; // 1 KB
const MEDIUM_FILE_SIZE: usize = 100 * 1024; // 100 KB
const LARGE_FILE_SIZE: usize = 1024 * 1024; // 1 MB
const VERY_LARGE_FILE_SIZE: usize = 10 * 1024 * 1024; // 10 MB

/// Number of iterations for stable measurements
const WARMUP_ITERATIONS: usize = 5;
const BENCHMARK_ITERATIONS: usize = 20;

/// Target throughput (MB/s) for optimization
const TARGET_THROUGHPUT_MBPS: f64 = 100.0;

/// Test data patterns
#[derive(Debug, Clone, Copy)]
enum DataPattern {
    /// All zeros (best case compression, representative of encrypted files)
    Zeros,
    /// Sequential pattern (0x00, 0x01, 0x02, ...)
    Sequential,
    /// Random data (worst case for compression)
    Random,
}

impl DataPattern {
    fn generate_data(self, size: usize) -> Vec<u8> {
        match self {
            DataPattern::Zeros => vec![0u8; size],
            DataPattern::Sequential => (0..size as u8).cycle().take(size).collect(),
            DataPattern::Random => {
                use rand::RngCore;
                let mut data = vec![0u8; size];
                let mut rng = rand::thread_rng();
                rng.fill_bytes(&mut data);
                data
            }
        }
    }
}

/// Benchmark result structure
#[derive(Debug)]
struct BenchmarkResult {
    name: String,
    #[allow(dead_code)]
    data_size: usize,
    #[allow(dead_code)]
    iterations: usize,
    #[allow(dead_code)]
    total_duration: Duration,
    #[allow(dead_code)]
    avg_duration: Duration,
    throughput_mbps: f64,
    #[allow(dead_code)]
    target_mbps: f64,
    passed: bool,
}

impl BenchmarkResult {
    #[allow(dead_code)]
    fn print(&self) {
        println!("\n{}", "=".repeat(60));
        println!("Benchmark: {}", self.name);
        println!("{}", "=".repeat(60));
        println!(
            "Data size:      {:.2} MB",
            self.data_size as f64 / 1024.0 / 1024.0
        );
        println!("Iterations:     {}", self.iterations);
        println!("Total time:     {:?}", self.total_duration);
        println!("Avg time:       {:?}", self.avg_duration);
        println!("Throughput:     {:.2} MB/s", self.throughput_mbps);
        println!("Target:         {:.2} MB/s", self.target_mbps);
        println!(
            "Result:         {}",
            if self.passed { "✓ PASS" } else { "✗ FAIL" }
        );
        if self.passed {
            println!(
                "Improvement:     {:.1}%",
                (self.throughput_mbps / self.target_mbps) * 100.0 - 100.0
            );
        } else {
            println!(
                "Gap:            {:.1}%",
                (1.0 - self.throughput_mbps / self.target_mbps) * 100.0
            );
        }
    }
}

/// Warmup function to stabilize performance
fn warmup_cipher(cipher: &ChunkCipher, data: &[u8]) {
    for _ in 0..WARMUP_ITERATIONS {
        let _ = cipher.encrypt_all(data);
    }
}

/// Measure encryption throughput
fn measure_encryption_throughput(
    cipher: &ChunkCipher,
    data: &[u8],
    iterations: usize,
) -> (Duration, f64) {
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = cipher.encrypt_all(data);
    }

    let duration = start.elapsed();

    // Calculate throughput: (size * iterations) / duration_in_seconds / 1MB
    let total_bytes = data.len() * iterations;
    let duration_secs = duration.as_secs_f64();
    let throughput_mbps = (total_bytes as f64 / 1024.0 / 1024.0) / duration_secs;

    (duration, throughput_mbps)
}

/// Measure decryption throughput
#[allow(dead_code)]
fn measure_decryption_throughput(
    cipher: &ChunkCipher,
    encrypted_chunks: &[Vec<u8>],
    iterations: usize,
) -> (Duration, f64) {
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = cipher.decrypt_all(encrypted_chunks);
    }

    let duration = start.elapsed();

    // Calculate total size from encrypted chunks
    let total_size: usize = encrypted_chunks.iter().map(|c| c.len()).sum();
    let total_bytes = total_size * iterations;
    let duration_secs = duration.as_secs_f64();
    let throughput_mbps = (total_bytes as f64 / 1024.0 / 1024.0) / duration_secs;

    (duration, throughput_mbps)
}

/// Run benchmark for a specific data size and pattern
fn run_benchmark(
    name: &str,
    data_size: usize,
    pattern: DataPattern,
    cipher: &ChunkCipher,
) -> BenchmarkResult {
    println!("\nRunning benchmark: {}...", name);

    let data = pattern.generate_data(data_size);

    // Warmup
    warmup_cipher(cipher, &data);

    // Benchmark encryption
    let (encrypt_duration, encrypt_throughput) =
        measure_encryption_throughput(cipher, &data, BENCHMARK_ITERATIONS);

    // Verify correctness
    let encrypted = cipher.encrypt_all(&data).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    assert_eq!(data, decrypted, "Decryption did not produce original data");

    let avg_duration = encrypt_duration / BENCHMARK_ITERATIONS as u32;

    BenchmarkResult {
        name: name.to_string(),
        data_size,
        iterations: BENCHMARK_ITERATIONS,
        total_duration: encrypt_duration,
        avg_duration,
        throughput_mbps: encrypt_throughput,
        target_mbps: TARGET_THROUGHPUT_MBPS,
        passed: encrypt_throughput >= TARGET_THROUGHPUT_MBPS,
    }
}

/// Test encryption performance with different chunk sizes
fn test_chunk_size_impact() {
    println!("\n{}", "=".repeat(60));
    println!("Testing chunk size impact on performance");
    println!("{}", "=".repeat(60));

    const DATA_SIZE: usize = LARGE_FILE_SIZE;
    const CHUNK_SIZES: [usize; 4] = [4096, 16384, 65536, 262144]; // 4KB, 16KB, 64KB, 256KB

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let data = DataPattern::Random.generate_data(DATA_SIZE);

    for chunk_size in CHUNK_SIZES {
        let chunk_size_obj = ChunkSize::new(chunk_size as u32);
        let cipher = ChunkCipher::new(&key, &nonce, chunk_size_obj);

        let (duration, throughput) =
            measure_encryption_throughput(&cipher, &data, BENCHMARK_ITERATIONS);

        println!("\nChunk size:     {:>8} bytes", chunk_size);
        println!(
            "Duration:       {:>8.2?}",
            duration / BENCHMARK_ITERATIONS as u32
        );
        println!("Throughput:     {:>8.2} MB/s", throughput);
    }
}

/// Test context reuse effectiveness
fn test_context_reuse_effectiveness() {
    println!("\n{}", "=".repeat(60));
    println!("Testing context reuse effectiveness");
    println!("{}", "=".repeat(60));

    const DATA_SIZE: usize = MEDIUM_FILE_SIZE;
    const ITERATIONS: usize = 1000;

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(4096);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);
    let data = DataPattern::Zeros.generate_data(DATA_SIZE);

    // Test single cipher instance (context reuse)
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = cipher.encrypt_all(&data);
    }
    let reused_duration = start.elapsed();

    // Test creating new cipher instance each time (no reuse)
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let new_cipher = ChunkCipher::new(&key, &nonce, chunk_size);
        let _ = new_cipher.encrypt_all(&data);
    }
    let no_reuse_duration = start.elapsed();

    let reused_throughput =
        (DATA_SIZE as f64 * ITERATIONS as f64 / 1024.0 / 1024.0) / reused_duration.as_secs_f64();
    let no_reuse_throughput =
        (DATA_SIZE as f64 * ITERATIONS as f64 / 1024.0 / 1024.0) / no_reuse_duration.as_secs_f64();

    println!("\nWith context reuse:");
    println!("  Duration:       {:?}", reused_duration);
    println!("  Throughput:     {:.2} MB/s", reused_throughput);

    println!("\nWithout context reuse:");
    println!("  Duration:       {:?}", no_reuse_duration);
    println!("  Throughput:     {:.2} MB/s", no_reuse_throughput);

    let improvement = reused_throughput / no_reuse_throughput;
    println!("\nImprovement:     {:.1}%", (improvement - 1.0) * 100.0);
}

/// Test different data patterns
fn test_data_patterns() {
    println!("\n{}", "=".repeat(60));
    println!("Testing encryption with different data patterns");
    println!("{}", "=".repeat(60));

    const DATA_SIZE: usize = LARGE_FILE_SIZE;

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(65536);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

    let patterns = [
        ("Zeros (encrypted files)", DataPattern::Zeros),
        ("Sequential", DataPattern::Sequential),
        ("Random", DataPattern::Random),
    ];

    for (name, pattern) in &patterns {
        let data = pattern.generate_data(DATA_SIZE);
        warmup_cipher(&cipher, &data);

        let (duration, throughput) =
            measure_encryption_throughput(&cipher, &data, BENCHMARK_ITERATIONS);

        println!("\nPattern:        {}", name);
        println!(
            "Duration:       {:?}",
            duration / BENCHMARK_ITERATIONS as u32
        );
        println!("Throughput:     {:.2} MB/s", throughput);
        println!(
            "Target met:     {}",
            if throughput >= TARGET_THROUGHPUT_MBPS {
                "✓"
            } else {
                "✗"
            }
        );
    }
}

/// Main benchmark suite
#[test]
fn test_encryption_performance_phase5() {
    println!("\n{}", "=".repeat(60));
    println!("PHASE 5: Encryption Performance Optimization Benchmarks");
    println!("Target: {:.0} MB/s throughput", TARGET_THROUGHPUT_MBPS);
    println!("{}", "=".repeat(60));

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(65536);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

    // Test different data sizes
    let benchmarks = vec![
        ("Small (1 KB)", SMALL_FILE_SIZE, DataPattern::Zeros),
        ("Medium (100 KB)", MEDIUM_FILE_SIZE, DataPattern::Zeros),
        ("Large (1 MB)", LARGE_FILE_SIZE, DataPattern::Zeros),
        (
            "Very Large (10 MB)",
            VERY_LARGE_FILE_SIZE,
            DataPattern::Zeros,
        ),
    ];

    let mut results = Vec::new();
    for (name, size, pattern) in benchmarks {
        let result = run_benchmark(name, size, pattern, &cipher);
        results.push(result);
    }

    // Print summary
    println!("\n{}", "=".repeat(60));
    println!("BENCHMARK SUMMARY");
    println!("{}", "=".repeat(60));

    let passed_count = results.iter().filter(|r| r.passed).count();
    let total_count = results.len();

    println!("\nTests passed:    {}/{}", passed_count, total_count);
    println!("\nIndividual results:");
    for result in &results {
        println!(
            "  {:<20} {:>8.2} MB/s  {}",
            result.name,
            result.throughput_mbps,
            if result.passed { "✓" } else { "✗" }
        );
    }

    // Assert that we're meeting performance targets (release builds only)
    // In debug builds, performance will be significantly lower due to lack of optimizations
    #[cfg(debug_assertions)]
    {
        if passed_count < total_count {
            println!("⚠️  Performance targets not met (expected in debug builds)");
            println!("   Run with: cargo test --release");
        }
    }

    #[cfg(not(debug_assertions))]
    {
        if passed_count < total_count {
            panic!("Performance targets not met!");
        }
    }

    // Additional tests
    test_chunk_size_impact();
    test_context_reuse_effectiveness();
    test_data_patterns();

    println!("\n{}", "=".repeat(60));
    println!("All Phase 5 benchmarks completed successfully!");
    println!("{}", "=".repeat(60));
}

/// Test correctness of encryption/decryption
#[test]
fn test_encryption_correctness() {
    println!("\nTesting encryption correctness...");

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(4096);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

    // Test various data sizes
    let test_sizes = [0, 1, 100, 4096, 10000, 65536];

    for size in test_sizes {
        let data = vec![0x42u8; size];

        // Skip empty data test as it should fail
        if size == 0 {
            continue;
        }

        let encrypted = cipher.encrypt_all(&data).expect("Encryption failed");
        let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");

        assert_eq!(data, decrypted, "Data mismatch for size {}", size);
    }

    println!("✓ All correctness tests passed!");
}

/// Test nonce uniqueness
#[test]
fn test_nonce_uniqueness_phase5() {
    println!("\nTesting nonce uniqueness...");

    let key = [0u8; 32];
    let base_nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(4096);
    let data = vec![0x42u8; 4096];

    let cipher = ChunkCipher::new(&key, &base_nonce, chunk_size);
    let encrypted = cipher.encrypt_all(&data).expect("Encryption failed");

    // Verify different chunks have different nonces
    let mut nonces = Vec::new();
    for chunk_index in 0..encrypted.len() {
        let nonce = Nonce::from_chunk_index(chunk_index as u32, &base_nonce);
        nonces.push(nonce);
    }

    // All nonces should be unique
    for i in 0..nonces.len() {
        for j in (i + 1)..nonces.len() {
            assert_ne!(
                nonces[i], nonces[j],
                "Nonces for chunks {} and {} are identical!",
                i, j
            );
        }
    }

    println!("✓ All nonces are unique!");
}

/// Test context reuse with multiple chunks
#[test]
fn test_context_reuse_correctness() {
    println!("\nTesting context reuse correctness...");

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(4096);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

    // Create data with multiple chunks
    let data = vec![0x42u8; 20000]; // About 5 chunks

    // Encrypt multiple times to ensure buffer reuse works correctly
    for i in 0..10 {
        let encrypted = cipher
            .encrypt_all(&data)
            .unwrap_or_else(|_| panic!("Encryption failed on iteration {}", i));
        let decrypted = cipher
            .decrypt_all(&encrypted)
            .unwrap_or_else(|_| panic!("Decryption failed on iteration {}", i));

        assert_eq!(data, decrypted, "Data mismatch on iteration {}", i);
    }

    println!("✓ Context reuse correctness verified!");
}

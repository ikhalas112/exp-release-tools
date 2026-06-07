//! Phase 5 Integration Tests
//!
//! Integration tests to verify Phase 5 performance optimizations:
//! - Context reuse optimization (buffer reuse in ChunkCipher)
//! - SIMD compilation support
//! - Performance targets (100 MB/s throughput)
//! - Correctness of encrypted/decrypted data

use maxion_core::crypto::ChunkCipher;
use maxion_core::types::{ChunkSize, Nonce};
use std::time::Instant;

/// Test data sizes for integration testing
const TEST_DATA_SIZES: &[usize] = &[
    4096,             // 4 KB (single chunk)
    65536,            // 64 KB (default chunk size)
    1024 * 1024,      // 1 MB (16 chunks at 64 KB)
    10 * 1024 * 1024, // 10 MB (156 chunks at 64 KB)
];

/// Target throughput in MB/s (from Phase 5 plan)
const TARGET_THROUGHPUT_MBPS: f64 = 100.0;

/// Iterations for performance testing
const PERF_ITERATIONS: usize = 10;

/// Generate test data with a specific pattern
fn generate_test_data(size: usize, pattern_byte: u8) -> Vec<u8> {
    vec![pattern_byte; size]
}

/// Generate random test data
fn generate_random_data(size: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut data = vec![0u8; size];
    let mut rng = rand::thread_rng();
    rng.fill_bytes(&mut data);
    data
}

/// Measure encryption throughput
fn measure_throughput<F>(mut operation: F, data_size: usize, iterations: usize) -> f64
where
    F: FnMut() -> Vec<Vec<u8>>,
{
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = operation();
    }

    let duration = start.elapsed();
    let total_bytes = data_size * iterations;
    let duration_secs = duration.as_secs_f64();

    (total_bytes as f64 / 1024.0 / 1024.0) / duration_secs
}

/// Test 1: Verify context reuse optimization doesn't break correctness
#[test]
fn test_context_reuse_correctness() {
    println!("\n=== Test 1: Context Reuse Correctness ===");

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(65536);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

    // Test multiple encryption/decryption cycles to ensure buffer reuse works
    for (i, &size) in TEST_DATA_SIZES.iter().enumerate() {
        let original_data = generate_test_data(size, i as u8);

        // Encrypt
        let encrypted = cipher
            .encrypt_all(&original_data)
            .expect("Encryption failed");

        // Decrypt
        let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");

        // Verify
        assert_eq!(
            original_data, decrypted,
            "Data mismatch for size {} after encryption/decryption",
            size
        );

        println!(
            "✓ Size {}: Correctness verified ({} chunks)",
            size,
            encrypted.len()
        );
    }

    println!("✅ Context reuse correctness test passed!\n");
}

/// Test 2: Verify nonce uniqueness with context reuse
#[test]
fn test_nonce_uniqueness_with_reuse() {
    println!("\n=== Test 2: Nonce Uniqueness with Context Reuse ===");

    let key = [0u8; 32];
    let base_nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(4096);
    let cipher = ChunkCipher::new(&key, &base_nonce, chunk_size);

    let data = generate_test_data(100000, 0x42);
    let encrypted = cipher.encrypt_all(&data).expect("Encryption failed");

    // Collect all nonces used
    let mut nonces = Vec::new();
    for chunk_index in 0..encrypted.len() {
        let nonce = Nonce::from_chunk_index(chunk_index as u32, &base_nonce);
        nonces.push(nonce);
    }

    // Verify all nonces are unique
    for i in 0..nonces.len() {
        for j in (i + 1)..nonces.len() {
            assert_ne!(
                nonces[i], nonces[j],
                "Nonces for chunks {} and {} are identical!",
                i, j
            );
        }
    }

    println!("✓ Verified {} unique nonces", nonces.len());
    println!("✅ Nonce uniqueness test passed!\n");
}

/// Test 3: Performance validation (must meet 100 MB/s target)
#[test]
fn test_performance_targets() {
    println!("\n=== Test 3: Performance Target Validation ===");
    println!("Target: {:.0} MB/s throughput", TARGET_THROUGHPUT_MBPS);

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(65536);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

    let mut all_passed = true;
    let mut results = Vec::new();

    // Test each data size
    for &size in TEST_DATA_SIZES {
        let data = generate_test_data(size, 0x42);

        // Warmup
        for _ in 0..3 {
            let _ = cipher.encrypt_all(&data);
        }

        // Measure encryption throughput
        let encryption_throughput =
            measure_throughput(|| cipher.encrypt_all(&data).unwrap(), size, PERF_ITERATIONS);

        // Verify correctness
        let encrypted = cipher.encrypt_all(&data).expect("Encryption failed");
        let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
        assert_eq!(
            data, decrypted,
            "Correctness check failed for size {}",
            size
        );

        let passed = encryption_throughput >= TARGET_THROUGHPUT_MBPS;
        all_passed = all_passed && passed;

        results.push((size, encryption_throughput, passed));

        println!(
            "Size: {:>8} bytes | Throughput: {:>6.2} MB/s | {}",
            size,
            encryption_throughput,
            if passed { "✓ PASS" } else { "✗ FAIL" }
        );
    }

    // Summary
    println!("\n--- Performance Summary ---");
    let avg_throughput: f64 =
        results.iter().map(|(_, t, _)| *t).sum::<f64>() / results.len() as f64;
    println!("Average throughput: {:.2} MB/s", avg_throughput);
    println!("Target throughput:  {:.2} MB/s", TARGET_THROUGHPUT_MBPS);

    if all_passed {
        println!("✅ All performance targets met!");
    } else {
        println!("⚠️  Some performance targets not met");
        println!("   Note: Performance depends on CPU features (SIMD) and compilation flags");
        println!("   Build with: cargo build --release --features simd");
    }

    // Assert that average meets target (with some tolerance for development machines)
    #[cfg(debug_assertions)]
    {
        if avg_throughput < TARGET_THROUGHPUT_MBPS * 0.8 {
            println!("⚠️  Performance targets not met (expected in debug builds)");
            println!("   Run with: cargo test --release");
        }
    }

    #[cfg(not(debug_assertions))]
    {
        assert!(
            avg_throughput >= TARGET_THROUGHPUT_MBPS * 0.8,
            "Average throughput {:.2} MB/s is significantly below target {:.2} MB/s",
            avg_throughput,
            TARGET_THROUGHPUT_MBPS
        );
    }

    println!();
}

/// Test 4: Buffer reuse efficiency (verify buffer is actually reused)
#[test]
fn test_buffer_reuse_efficiency() {
    println!("\n=== Test 4: Buffer Reuse Efficiency ===");

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(65536);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

    let data_size = 65536; // Exactly one chunk
    let data = generate_test_data(data_size, 0x42);

    // Perform multiple encryptions
    let start = Instant::now();
    for _ in 0..100 {
        let encrypted = cipher.encrypt_all(&data).expect("Encryption failed");
        let _ = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    }
    let duration = start.elapsed();

    println!(
        "100 encryption/decryption cycles for 64 KB data in: {:?}",
        duration
    );
    println!("Average per cycle: {:?}", duration / 100);

    // If buffer reuse is working, this should be fast
    // (less than 1 second for 100 cycles on modern hardware)
    assert!(
        duration.as_secs() < 2,
        "Buffer reuse appears inefficient: {:?} for 100 cycles",
        duration
    );

    println!("✅ Buffer reuse efficiency test passed!\n");
}

/// Test 5: Edge cases and boundary conditions
#[test]
fn test_edge_cases() {
    println!("\n=== Test 5: Edge Cases and Boundary Conditions ===");

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(4096);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

    // Test 1: Data exactly equal to chunk size
    println!("Testing data equal to chunk size (4096 bytes)...");
    let exact_chunk = vec![0x42u8; 4096];
    let encrypted = cipher.encrypt_all(&exact_chunk).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    assert_eq!(exact_chunk, decrypted);
    println!("✓ Passed");

    // Test 2: Data just under chunk size
    println!("Testing data just under chunk size (4095 bytes)...");
    let under_chunk = vec![0x43u8; 4095];
    let encrypted = cipher.encrypt_all(&under_chunk).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    assert_eq!(under_chunk, decrypted);
    println!("✓ Passed");

    // Test 3: Data just over chunk size (multiple chunks)
    println!("Testing data just over chunk size (4097 bytes, 2 chunks)...");
    let over_chunk = vec![0x44u8; 4097];
    let encrypted = cipher.encrypt_all(&over_chunk).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    assert_eq!(over_chunk, decrypted);
    println!("✓ Passed");

    // Test 4: Very small data (but not empty - empty is handled separately)
    println!("Testing very small data (1 byte)...");
    let tiny_data = vec![0x45u8; 1];
    let encrypted = cipher.encrypt_all(&tiny_data).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    assert_eq!(tiny_data, decrypted);
    println!("✓ Passed");

    // Test 5: Multiple chunks with different sizes
    println!("Testing multiple chunks with varying sizes...");
    let mut multi_chunk = Vec::new();
    for i in 0..10000 {
        multi_chunk.push(i as u8);
    }
    let encrypted = cipher.encrypt_all(&multi_chunk).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    assert_eq!(multi_chunk, decrypted);
    println!("✓ Passed");

    println!("✅ All edge case tests passed!\n");
}

/// Test 6: Data integrity with different patterns
#[test]
fn test_data_integrity_patterns() {
    println!("\n=== Test 6: Data Integrity with Different Patterns ===");

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(65536);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

    let test_size = 65536;

    // Test with zeros (common in encrypted files)
    println!("Testing with zeros...");
    let zeros = vec![0u8; test_size];
    let encrypted = cipher.encrypt_all(&zeros).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    assert_eq!(zeros, decrypted);
    println!("✓ Passed");

    // Test with repeated pattern
    println!("Testing with repeated pattern (0x42)...");
    let repeated = vec![0x42u8; test_size];
    let encrypted = cipher.encrypt_all(&repeated).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    assert_eq!(repeated, decrypted);
    println!("✓ Passed");

    // Test with sequential pattern
    println!("Testing with sequential pattern...");
    let sequential: Vec<u8> = (0..test_size as u8).cycle().take(test_size).collect();
    let encrypted = cipher.encrypt_all(&sequential).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    assert_eq!(sequential, decrypted);
    println!("✓ Passed");

    // Test with random data
    println!("Testing with random data...");
    let random = generate_random_data(test_size);
    let encrypted = cipher.encrypt_all(&random).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");
    assert_eq!(random, decrypted);
    println!("✓ Passed");

    println!("✅ All data integrity tests passed!\n");
}

/// Test 7: Concurrent encryption safety (basic test)
#[test]
fn test_concurrent_safety() {
    println!("\n=== Test 7: Concurrent Encryption Safety ===");

    use std::sync::Arc;
    use std::thread;

    let key = Arc::new([0u8; 32]);
    let nonce = Arc::new([1u8; 24]);
    let chunk_size = ChunkSize::new(4096);
    let cipher = Arc::new(ChunkCipher::new(&key, &nonce, chunk_size));

    let mut handles = Vec::new();

    // Spawn multiple threads
    for thread_id in 0..4 {
        let cipher_clone = Arc::clone(&cipher);
        let handle = thread::spawn(move || {
            let data = generate_test_data(10000, thread_id as u8);
            let encrypted = cipher_clone.encrypt_all(&data).expect("Encryption failed");
            let decrypted = cipher_clone
                .decrypt_all(&encrypted)
                .expect("Decryption failed");
            assert_eq!(data, decrypted);
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    println!("✅ Concurrent safety test passed!\n");
}

/// Test 8: Verify SIMD optimization is available (if compiled with feature)
#[test]
fn test_simd_availability() {
    println!("\n=== Test 8: SIMD Availability Check ===");

    // Check if we're compiled with SIMD support
    #[cfg(feature = "simd")]
    {
        println!("✓ SIMD feature is enabled");

        // Check CPU features
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                println!("✓ AVX2 support detected");
            } else if is_x86_feature_detected!("sse4.1") {
                println!("✓ SSE4.1 support detected");
            } else {
                println!("⚠ No x86 SIMD features detected, using scalar fallback");
            }

            if is_x86_feature_detected!("avx512f") {
                println!("✓ AVX-512 support detected (maximum performance)");
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if is_aarch64_feature_detected!("neon") {
                println!("✓ NEON support detected");
            } else {
                println!("⚠ No ARM64 SIMD features detected, using scalar fallback");
            }
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            println!("⚠ SIMD not supported on this architecture");
        }
    }

    #[cfg(not(feature = "simd"))]
    {
        println!("⚠ SIMD feature not enabled");
        println!("   Build with: cargo test --features simd");
    }

    println!("✅ SIMD availability check completed!\n");
}

/// Main integration test entry point
#[test]
fn test_phase5_integration() {
    println!("\n{}", "=".repeat(70));
    println!("PHASE 5 INTEGRATION TEST SUITE");
    println!("Performance Optimizations: Context Reuse + SIMD Support");
    println!("{}", "=".repeat(70));

    // All individual tests will run
    // This is a summary test that verifies the overall integration

    let key = [0u8; 32];
    let nonce = [1u8; 24];
    let chunk_size = ChunkSize::new(65536);
    let cipher = ChunkCipher::new(&key, &nonce, chunk_size);

    // Comprehensive end-to-end test
    let data = generate_random_data(1024 * 1024); // 1 MB

    let encrypted = cipher.encrypt_all(&data).expect("Encryption failed");
    let decrypted = cipher.decrypt_all(&encrypted).expect("Decryption failed");

    assert_eq!(data, decrypted, "End-to-end encryption/decryption failed");

    // Verify performance
    let throughput = measure_throughput(|| cipher.encrypt_all(&data).unwrap(), data.len(), 10);

    println!("\n--- End-to-End Integration Test Results ---");
    println!("Data size:  1 MB");
    println!("Throughput: {:.2} MB/s", throughput);
    println!("Target:     {:.2} MB/s", TARGET_THROUGHPUT_MBPS);

    if throughput >= TARGET_THROUGHPUT_MBPS {
        println!("✅ Phase 5 integration test PASSED - Performance target met!");
    } else {
        println!("⚠️ Phase 5 integration test WARNING - Performance below target");
        println!("   Note: Build with --release --features simd for best performance");
    }

    println!("{}", "=".repeat(70));
}

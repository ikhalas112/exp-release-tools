// Benchmark program for trap vs no-trap performance comparison
// This program measures the overhead of trap checking in Protected<T>
//
// Usage:
//   cargo run --bin trap_benchmark --release
//
// The program will output performance metrics comparing:
// - Regular values (baseline)
// - Protected values with trap checking ENABLED
// - Protected values with trap checking DISABLED

use std::hint::black_box;
use std::time::{Duration, Instant};

fn main() {
    println!("Trap vs No-Trap Performance Benchmark");
    println!("======================================");
    println!();

    // Test configuration
    let iterations = 1_000_000;
    let num_values = 100;

    println!("Benchmark Configuration:");
    println!("  Iterations per test: {}", iterations);
    println!("  Number of values: {}", num_values);
    println!("  Total operations: {}", iterations * num_values * 2);
    println!();

    // Run benchmarks
    benchmark_regular_i32(iterations, num_values);
    benchmark_protected_i32_trap_enabled(iterations, num_values);
    benchmark_protected_i32_trap_disabled(iterations, num_values);

    println!();
    benchmark_regular_f32(iterations, num_values);
    benchmark_protected_f32_trap_enabled(iterations, num_values);
    benchmark_protected_f32_trap_disabled(iterations, num_values);

    println!();
    benchmark_regular_tuple(iterations, num_values);
    benchmark_protected_tuple_trap_enabled(iterations, num_values);
    benchmark_protected_tuple_trap_disabled(iterations, num_values);

    println!();
    println!("Summary:");
    println!("--------");
    println!("Trap checking adds overhead for:");
    println!("  - Extra volatile read operation");
    println!("  - Comparison between trap and real values");
    println!("  - Potential cheat detection reporting");
    println!();
    println!("Recommendations:");
    println!("  - Use trap checking for critical game values (health, ammo, currency)");
    println!(
        "  - Consider disabling trap checking for less critical values to improve performance"
    );
    println!("  - Balance security needs with performance requirements");
}

fn benchmark_regular_i32(iterations: usize, num_values: usize) {
    print!("Regular i32 (baseline).................... ");

    // Create regular values
    let mut values: Vec<i32> = (0..num_values).map(|i| i as i32 * 10).collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for val in values.iter_mut() {
            // Read
            let v = black_box(*val);
            // Write
            *val = black_box(v + 1);
        }
    }
    let duration = start.elapsed();

    print_timing(duration, iterations, num_values);
}

fn benchmark_protected_i32_trap_enabled(iterations: usize, num_values: usize) {
    print!("Protected<i32> with trap enabled........... ");

    // Import here to ensure trap is enabled
    use maxion_core::{set_trap_enabled, Protected};

    // Enable trap checking
    set_trap_enabled(true);

    // Create protected values
    let mut values: Vec<Protected<i32>> = (0..num_values)
        .map(|i| Protected::new(i as i32 * 10))
        .collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for val in values.iter_mut() {
            // Read (includes trap check)
            let v = black_box(val.get());
            // Write (rotates key, updates trap)
            val.set(black_box(v + 1));
        }
    }
    let duration = start.elapsed();

    print_timing(duration, iterations, num_values);
}

fn benchmark_protected_i32_trap_disabled(iterations: usize, num_values: usize) {
    print!("Protected<i32> with trap DISABLED.......... ");

    use maxion_core::{set_trap_enabled, Protected};

    // Disable trap checking
    set_trap_enabled(false);

    // Create protected values
    let mut values: Vec<Protected<i32>> = (0..num_values)
        .map(|i| Protected::new(i as i32 * 10))
        .collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for val in values.iter_mut() {
            // Read (no trap check)
            let v = black_box(val.get());
            // Write (rotates key, updates trap but no comparison)
            val.set(black_box(v + 1));
        }
    }
    let duration = start.elapsed();

    print_timing(duration, iterations, num_values);
}

fn benchmark_regular_f32(iterations: usize, num_values: usize) {
    print!("Regular f32 (baseline).................... ");

    let mut values: Vec<f32> = (0..num_values).map(|i| i as f32 * 0.5).collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for val in values.iter_mut() {
            let v = black_box(*val);
            *val = black_box(v + 1.0);
        }
    }
    let duration = start.elapsed();

    print_timing(duration, iterations, num_values);
}

fn benchmark_protected_f32_trap_enabled(iterations: usize, num_values: usize) {
    print!("Protected<f32> with trap enabled........... ");

    use maxion_core::{set_trap_enabled, Protected};

    set_trap_enabled(true);

    let mut values: Vec<Protected<f32>> = (0..num_values)
        .map(|i| Protected::new(i as f32 * 0.5))
        .collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for val in values.iter_mut() {
            let v = black_box(val.get());
            val.set(black_box(v + 1.0));
        }
    }
    let duration = start.elapsed();

    print_timing(duration, iterations, num_values);
}

fn benchmark_protected_f32_trap_disabled(iterations: usize, num_values: usize) {
    print!("Protected<f32> with trap DISABLED.......... ");

    use maxion_core::{set_trap_enabled, Protected};

    set_trap_enabled(false);

    let mut values: Vec<Protected<f32>> = (0..num_values)
        .map(|i| Protected::new(i as f32 * 0.5))
        .collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for val in values.iter_mut() {
            let v = black_box(val.get());
            val.set(black_box(v + 1.0));
        }
    }
    let duration = start.elapsed();

    print_timing(duration, iterations, num_values);
}

fn benchmark_regular_tuple(iterations: usize, num_values: usize) {
    print!("Regular (f32,f32,f32) (baseline)........... ");

    let mut values: Vec<(f32, f32, f32)> = (0..num_values)
        .map(|i| (i as f32, i as f32 * 2.0, i as f32 * 3.0))
        .collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for val in values.iter_mut() {
            let v = black_box(*val);
            *val = black_box((v.0 + 1.0, v.1 + 1.0, v.2 + 1.0));
        }
    }
    let duration = start.elapsed();

    print_timing(duration, iterations, num_values);
}

fn benchmark_protected_tuple_trap_enabled(iterations: usize, num_values: usize) {
    print!("Protected<(f32,f32,f32)> trap enabled....... ");

    use maxion_core::{set_trap_enabled, Protected};

    set_trap_enabled(true);

    let mut values: Vec<Protected<(f32, f32, f32)>> = (0..num_values)
        .map(|i| Protected::new((i as f32, i as f32 * 2.0, i as f32 * 3.0)))
        .collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for val in values.iter_mut() {
            let v = black_box(val.get());
            val.set(black_box((v.0 + 1.0, v.1 + 1.0, v.2 + 1.0)));
        }
    }
    let duration = start.elapsed();

    print_timing(duration, iterations, num_values);
}

fn benchmark_protected_tuple_trap_disabled(iterations: usize, num_values: usize) {
    print!("Protected<(f32,f32,f32)> trap DISABLED...... ");

    use maxion_core::{set_trap_enabled, Protected};

    set_trap_enabled(false);

    let mut values: Vec<Protected<(f32, f32, f32)>> = (0..num_values)
        .map(|i| Protected::new((i as f32, i as f32 * 2.0, i as f32 * 3.0)))
        .collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for val in values.iter_mut() {
            let v = black_box(val.get());
            val.set(black_box((v.0 + 1.0, v.1 + 1.0, v.2 + 1.0)));
        }
    }
    let duration = start.elapsed();

    print_timing(duration, iterations, num_values);
}

fn print_timing(duration: Duration, iterations: usize, num_values: usize) {
    let total_ops = iterations * num_values * 2; // read + write
    let micros = duration.as_micros() as f64;
    let ops_per_us = total_ops as f64 / micros;
    let ops_per_sec = ops_per_us * 1_000_000.0;
    let avg_ns_per_op = (micros * 1_000.0) / total_ops as f64;

    println!(
        "{:>8.2} ms | {:>10.0} ops/s | {:>8.2} ns/op",
        micros / 1_000.0,
        ops_per_sec,
        avg_ns_per_op
    );
}

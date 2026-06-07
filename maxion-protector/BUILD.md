# Build Instructions

This document provides comprehensive build instructions for the Maxion Protector project, including optimized builds for different use cases and performance targets.

## Table of Contents

- [Quick Start](#quick-start)
- [Build Profiles](#build-profiles)
- [Standard Builds](#standard-builds)
- [Optimized Builds](#optimized-builds)
- [Maximum Optimization](#maximum-optimization)
- [Target-Specific Builds](#target-specific-builds)
- [Testing Builds](#testing-builds)
- [Troubleshooting](#troubleshooting)
- [Performance Tips](#performance-tips)

## Quick Start

```bash
# Standard build (good for development)
cargo build --release

# Optimized build with SIMD (recommended for production)
cargo build --release --features simd

# Run tests
cargo test --release

# Run benchmarks
cargo test -p maxion-core --test crypto_benchmark -- --nocapture
```

## Build Profiles

The project provides multiple build profiles optimized for different scenarios:

| Profile | Description | Use Case | Opt Level | LTO |
|---------|-------------|----------|-----------|-----|
| `dev` | Fast compilation, no optimization | Development/debugging | 0 | No |
| `release` | Standard release build | Production (portable) | `z` (size) | No |
| `opt` | Performance optimized | High-performance systems | 3 (max) | `fat` |
| `max-opt` | Maximum optimization | Benchmarks/profiling | 3 (max) | `fat` |
| `stub` | Minimal binary size | Loader stub | `z` (size) | No |

## Standard Builds

### Portable Release Build

**Best for:** Production deployment on unknown target hardware

```bash
# Standard release build (no SIMD, maximum compatibility)
cargo build --release

# Output location
# Windows: target/release/
# Linux/Mac: target/release/
```

**Characteristics:**
- ✅ Maximum portability (works on any x86_64 or ARM64 CPU)
- ✅ Small binary size
- ⚠️ No SIMD acceleration (2-5x slower than optimized builds)
- ⚠️ No link-time optimization

**Performance:** ~50-80 MB/s encryption throughput

### Debug Build

**Best for:** Development and debugging

```bash
# Standard debug build
cargo build

# Debug with extra checks
RUSTFLAGS="-Z macro-backtrace" cargo build
```

## Optimized Builds

### Standard Optimized Build (Portable + SIMD)

**Best for:** Production on modern CPUs with automatic SIMD detection

```bash
# Build with SIMD support (auto-detects CPU features at runtime)
cargo build --release --features simd

# With verbose output
cargo build --release --features simd --verbose
```

**Characteristics:**
- ✅ Portable (auto-detects SSE4.1, AVX2, AVX-512, NEON at runtime)
- ✅ 1.5x-3.5x performance improvement with SIMD
- ✅ Automatic fallback to scalar operations
- ✅ Small binary size

**Performance:** 
- Scalar (no SIMD): ~80 MB/s
- SSE4.1: ~120 MB/s (+50%)
- AVX2: ~200 MB/s (+150%)
- AVX-512: ~280 MB/s (+250%)
- NEON (ARM64): ~160 MB/s (+100%)

**When to use:**
- Production builds on modern hardware
- Systems with varied CPU capabilities
- When you don't know the target CPU in advance

### Optimized Build Profile

**Best for:** High-performance production systems

```bash
# Build with optimized profile
cargo build --profile opt --features simd

# Clean build to ensure fresh compilation
cargo clean
cargo build --profile opt --features simd
```

**Characteristics:**
- ✅ Maximum optimization (opt-level = 3)
- ✅ Link-time optimization (LTO = fat)
- ✅ Single codegen unit for better optimization
- ✅ SIMD runtime detection
- ⚠️ Longer compile time (2-3x slower)

**Performance:** ~250-350 MB/s on AVX2 CPUs

**When to use:**
- Production servers with known CPU capabilities
- Systems where compile time is not a concern
- When maximum performance is required

## Maximum Optimization

### Maximum Performance Build

**Best for:** Benchmarking and performance testing

```bash
# Build with maximum optimization
cargo build --profile max-opt --features simd

# With native CPU targeting (CPU-specific optimizations)
RUSTFLAGS="-C target-cpu=native" cargo build --profile max-opt --features simd
```

**Characteristics:**
- ✅ Maximum optimization (opt-level = 3)
- ✅ Full link-time optimization
- ✅ Native CPU targeting (AVX-512, etc.)
- ✅ Debug symbols retained for profiling
- ⚠️ Longest compile time (3-5x slower)
- ⚠️ Binary not portable across different CPU generations

**Performance:** ~500-800 MB/s on AVX-512 CPUs

**When to use:**
- Performance benchmarking
- Profiling and optimization analysis
- Systems with known high-end CPUs (Intel Xeon, AMD EPYC)

### CPU-Specific Targeting

For even better performance on specific CPU architectures:

```bash
# Intel Haswell+ (AVX2)
RUSTFLAGS="-C target-cpu=haswell" cargo build --release --features simd

# Intel Skylake+ (AVX-512F)
RUSTFLAGS="-C target-cpu=skylake-avx512" cargo build --release --features simd

# AMD Zen 2+ (AVX2)
RUSTFLAGS="-C target-cpu=znver2" cargo build --release --features simd

# Apple Silicon (M1/M2/M3 with NEON)
RUSTFLAGS="-C target-cpu=apple-m1" cargo build --release --features simd
```

**Warning:** CPU-specific builds may not run on older CPUs.

## Target-Specific Builds

### Loader Stub Build

**Best for:** Minimal loader/injector binary

```bash
# Build stub with minimal size
cargo build --release --profile stub -p maxion-stub

# Output: target/release/maxion-stub.exe (Windows) or maxion-stub (Unix)
```

**Characteristics:**
- ✅ Smallest possible binary size
- ✅ All symbols stripped
- ✅ Panic = abort (no unwinding)
- ⚠️ No debug symbols
- ⚠️ Minimal error messages

**Binary Size:** ~100-200 KB

### Packer Build

**Best for:** Asset packing and encryption

```bash
# Standard packer build
cargo build --release -p maxion-packer

# Optimized packer with SIMD (faster encryption)
cargo build --release --features simd -p maxion-packer

# Maximum performance for large asset sets
cargo build --profile opt --features simd -p maxion-packer
```

### Profiler Build

**Best for:** Performance analysis and debugging

```bash
# Build with debug symbols for profiling
cargo build --release -p maxion-profiler

# With extra profiling info
RUSTFLAGS="-g" cargo build --release -p maxion-profiler
```

## Testing Builds

### Standard Test Build

```bash
# Run all tests (debug build)
cargo test

# Run tests in release mode (faster)
cargo test --release

# Run tests with SIMD support
cargo test --release --features simd

# Run tests with maximum optimization
cargo test --profile max-opt --features simd

# Run specific test
cargo test --release test_encryption_correctness

# Run tests for specific package
cargo test --release -p maxion-core

# Run tests with output
cargo test --release -- --nocapture
```

### Benchmark Builds

```bash
# Run Phase 5 encryption benchmarks
cargo test -p maxion-core --test crypto_benchmark -- --nocapture

# Run standalone benchmark binary
cargo run --bin phase5_benchmark_main

# Run with maximum optimization
cargo run --bin phase5_benchmark_main --profile max-opt --features simd

# Run with verbose output
cargo run --bin phase5_benchmark_main -- --verbose
```

### Integration Tests

```bash
# Run all integration tests
cargo test --release --test '*'

# Run Phase 5 integration tests
cargo test --release --test phase5_integration_test

# Run integration tests with logging
RUST_LOG=info cargo test --release --test '*'
```

## Troubleshooting

### Build Fails with SIMD Features

**Problem:** Build fails when using `--features simd`

**Solution:**

1. Check Rust version (minimum 1.75):
   ```bash
   rustc --version
   # If too old: rustup update
   ```

2. Update dependencies:
   ```bash
   cargo update -p orion
   cargo build --release --features simd
   ```

3. Check target architecture:
   ```bash
   # SIMD only works on x86_64 and aarch64
   rustc --print target-arch
   ```

4. Try without native targeting:
   ```bash
   # Don't use RUSTFLAGS="-C target-cpu=native"
   cargo build --release --features simd
   ```

### Binary Too Large

**Problem:** Release binary is larger than expected

**Solution:**

```bash
# Use stub profile for minimal size
cargo build --release --profile stub -p maxion-stub

# Strip symbols from release build
strip target/release/maxion-protector.exe  # Windows
strip target/release/maxion-protector       # Linux/Mac

# Use UPX compression (optional)
upx --best target/release/maxion-protector
```

### Performance Below Expected

**Problem:** Encryption throughput is below 100 MB/s target

**Solution:**

1. Verify you're using optimized build:
   ```bash
   # Must use --release or optimized profile
   cargo build --release --features simd
   ```

2. Check SIMD is enabled:
   ```bash
   # Build with SIMD feature
   cargo build --release --features simd
   ```

3. Verify CPU supports SIMD:
   ```bash
   # Check CPU features on Linux
   lscpu | grep -i sse
   lscpu | grep -i avx
   
   # Check CPU features on Windows
   systeminfo | findstr /C:"Processor"
   
   # Check CPU features on Mac
   sysctl -a | grep machdep.cpu.features
   ```

4. Run benchmarks to diagnose:
   ```bash
   cargo run --bin phase5_benchmark_main -- --verbose
   ```

5. Try native CPU targeting:
   ```bash
   RUSTFLAGS="-C target-cpu=native" cargo build --release --features simd
   ```

### Compilation Too Slow

**Problem:** Build takes too long with optimization

**Solution:**

```bash
# Use standard release (faster compile, good performance)
cargo build --release --features simd

# Skip LTO for faster compile
cargo build --release --features simd --no-default-features

# Use cargo check for quick validation
cargo check --features simd

# Parallel compilation (use all cores)
CARGO_BUILD_JOBS=8 cargo build --release --features simd
```

### Linker Errors

**Problem:** Linker fails with LTO or optimization

**Solution:**

```bash
# Use system linker (lld is faster)
# On Linux:
cargo install lld
RUSTFLAGS="-C linker=clang -C link-arg=-fuse-ld=lld" cargo build --release

# On Windows (Visual Studio):
cargo build --release

# Try without LTO
cargo build --release --features simd
```

### Cross-Compilation Issues

**Problem:** Building for different target platform

**Solution:**

```bash
# Add target
rustup target add x86_64-pc-windows-gnu      # Windows GNU
rustup target add aarch64-unknown-linux-gnu   # ARM64 Linux

# Cross-compile
cargo build --release --target aarch64-unknown-linux-gnu --features simd

# With proper linker
cargo build --release --target x86_64-pc-windows-gnu --features simd
```

## Performance Tips

### 1. Use SIMD Features

Always build with SIMD for production:
```bash
cargo build --release --features simd
```

Performance improvement: **+50% to +250%** depending on CPU.

### 2. Choose Right Profile

| Use Case | Recommended Profile |
|----------|-------------------|
| Development | `dev` or `release` |
| Production (portable) | `release --features simd` |
| Production (known CPU) | `opt --features simd` |
| Benchmarking | `max-opt --features simd` |
| Loader stub | `stub` |

### 3. Profile-Specific Optimization

For critical performance paths, use `max-opt`:
```bash
cargo build --profile max-opt --features simd
```

### 4. Enable Link-Time Optimization

The `opt` and `max-opt` profiles already enable LTO, which provides:
- 5-10% additional performance
- Better cross-crate optimization
- Smaller code size in some cases

### 5. Use Native CPU Targeting (when safe)

If you control the target hardware:
```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release --features simd
```

Provides 5-10% additional performance on compatible CPUs.

### 6. Parallel Builds

Speed up compilation on multi-core systems:
```bash
# Use all available cores (default)
cargo build --release --features simd

# Specify number of jobs
CARGO_BUILD_JOBS=8 cargo build --release --features simd
```

### 7. Cache Dependencies

Use cargo build cache for faster rebuilds:
```bash
# First build is slow
cargo build --release --features simd

# Subsequent builds are fast (only recompile changed code)
# Edit code...
cargo build --release --features simd
```

### 8. Disable Debug Info in Release

Already configured in release profiles, but verify:
```toml
[profile.release]
strip = true
```

## Build Verification

After building, verify the binary:

```bash
# Check binary size
ls -lh target/release/maxion-protector

# Check binary type
file target/release/maxion-protector

# Check for stripped symbols
nm target/release/maxion-protector | wc -l  # Should be minimal

# Run quick smoke test
./target/release/maxion-protector --help

# Check for SIMD (on Linux)
objdump -d target/release/maxion-protector | grep -i avx2
objdump -d target/release/maxion-protector | grep -i sse4
```

## Continuous Integration

For CI/CD pipelines, use optimized builds:

```yaml
# GitHub Actions example
- name: Build optimized
  run: |
    rustup update
    cargo build --release --features simd

- name: Run benchmarks
  run: |
    cargo test --release --features simd --test crypto_benchmark -- --nocapture

- name: Check performance
  run: |
    cargo run --bin phase5_benchmark_main --profile max-opt --features simd
```

## Additional Resources

- **Rust Compilation Guide:** https://doc.rust-lang.org/cargo/reference/profiles.html
- **SIMD Optimizations:** See `docs/05_benchmark/phase5_optimization.md`
- **Performance Benchmarks:** See `tests/phase5_benchmarks/`
- **Project Plans:** See `plans/005_perf.md`

## Summary

| Build Type | Command | Performance | Portability | Compile Time |
|------------|---------|------------|-------------|--------------|
| Debug | `cargo build` | 10x slower | 100% | Fastest |
| Release | `cargo build --release` | 50-80 MB/s | 100% | Fast |
| Release+SIMD | `cargo build --release --features simd` | 80-280 MB/s | 100% | Medium |
| Optimized | `cargo build --profile opt --features simd` | 250-350 MB/s | 100% | Slow |
| Max Optimized | `cargo build --profile max-opt --features simd` | 500-800 MB/s | CPU-specific | Slowest |
| Stub | `cargo build --profile stub` | ~50 MB/s | 100% | Fast |

**Recommendation:** Use `cargo build --release --features simd` for production. Use `max-opt` only for benchmarking on known hardware.
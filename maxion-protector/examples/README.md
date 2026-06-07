# Examples

This directory contains examples demonstrating various features of Maxion Protector.

## Available Examples

| Example | Description | Language | Focus |
|---------|-------------|----------|--------|
| [hello-world](./hello-world/) | E2E test with asset loading | Rust | End-to-end workflow, performance benchmarking |
| [protected-cpp-example](./protected-cpp-example/) | AutoProtected demonstration & encryption verification | C++ | Proof of protection, encryption verification, auto-protected variables |
| [AutoProtected Demo](./protected-cpp-example/) | Variable-level protection demo | C++ | AutoProtected<T> usage, best practices, performance analysis |
| [cheat-cpp-callback-demo](./cheat-cpp-callback-demo/) | Cheat detection callback system | C++ | Callback registration, cheat types, protected vs unprotected comparison |

---

# E2E Test: Hello World Asset Loader

This example demonstrates complete Maxion Protector workflow using a minimal Windows application that loads an external asset (PNG image).

## Purpose

The Hello World E2E test serves as a comprehensive demonstration and validation of the Maxion Protector system:

- ✅ **Validate the protection workflow end-to-end** - From source to protected executable
- ✅ **Compare file sizes and overhead** - Understand the cost of protection
- ✅ **Demonstrate asset encryption and packing** - See encryption in action
- ✅ **Provide a baseline for performance testing** - Establish metrics for optimization
- ✅ **Serve as integration test** - Verify all components work together

## Quick Start

Get up and running with just a few commands:

### 1. Build the Maxion Packer

First, ensure the packer tool is built:

```bash
cargo build --release -p maxion-packer
```

### 2. Build the Hello World Application

Build for Windows from macOS (default):

```bash
./scripts/build_hello_world.sh
```

Or build for native platform:

```bash
# Linux
TARGET=x86_64-unknown-linux-gnu ./scripts/build_hello_world.sh

# macOS (for testing purposes)
TARGET=aarch64-apple-darwin ./scripts/build_hello_world.sh
```

### 3. Protect the Application

Use the packer to embed and encrypt the assets:

```bash
./scripts/protect_hello_world.sh
```

### 4. Benchmark and Compare

Analyze the results:

```bash
./scripts/benchmark_hello_world.sh
```

## Project Structure

```
examples/
├── assets/
│   └── sirref.png           # Test asset (PNG image)
├── hello-world/
│   ├── Cargo.toml           # Application manifest
│   ├── build.rs             # Build configuration
│   └── src/
│       └── main.rs          # Application entry point
├── protected-cpp-example/    # C++ example for AutoProtected<T>
│   ├── README.md            # Detailed documentation
│   ├── AUTO_PROTECTED_CPP.md # AutoProtected<T> guide
│   ├── main.cpp             # C++ demonstration code
│   ├── auto_protected.h      # AutoProtected<T> header
│   ├── tests/
│   │   └── test_protection.cpp  # Unit tests (15 tests)
│   ├── assets/              # Test assets
│   │   ├── public_config.json
│   │   ├── secret_key.bin
│   │   └── secret_config.json
│   └── scripts/
│       ├── pack_assets.sh    # Pack assets
│       └── run_example.sh   # Compile and run C++ example
└── README.md                # This file (examples overview)

scripts/
├── build_hello_world.sh     # Build script with cross-compilation
├── protect_hello_world.sh   # Protection script using packer
└── benchmark_hello_world.sh # Benchmark and comparison script
```

## Application Design

### Hello World Application (`hello-world`)

A minimal Windows application that:

1. **Loads External Asset**: Reads `assets/sirref.png` from disk
2. **Validates File**: Checks file existence and reads metadata
3. **Decodes Image**: Uses the `image` crate to decode the PNG
4. **Reports Information**: Displays file size, dimensions, color type, etc.
5. **Exits Cleanly**: Returns success if all operations complete

This simple application is perfect for testing because:

- It has a clear dependency on an external file
- It performs multiple I/O operations that can be hooked
- It's easy to verify success/failure
- It's representative of real-world asset loading

### Test Asset (`sirref.png`)

A PNG image used as test data:

- Acts as a realistic game asset
- Has a known file size for comparison
- Can be verified to load correctly
- Demonstrates encryption of binary data

## Files Generated

After running the build and protection scripts, you'll find:

```
target/e2e/
├── hello.exe              # Original unprotected executable
├── hello_packed.exe       # Protected executable with embedded assets
├── assets/                # Original external assets (for comparison)
│   └── sirref.png
└── benchmark_report.md    # Detailed benchmark analysis
```

## Expected Behavior

### Unpacked Version (`hello.exe`)

When run with the assets directory present:

```
Hello Asset Loader Test
========================

✓ Found asset at: assets/sirref.png
✓ File size: <size> bytes
✓ Modified: <timestamp>

Loading image...
✓ Image loaded successfully
  Dimensions: <width>x<height>
  Color type: <type>
  Pixel format: <format>
  Memory size: <bytes>

✓ Test completed successfully!
```

### Packed Version (`hello_packed.exe`)

Should produce **identical output** when run on Windows:

- Same output text
- Same file information
- Same image dimensions and properties
- Same successful completion

**Key Difference**: The packed version reads from the embedded encrypted archive instead of external files.

## Testing on Windows

Since we're building from macOS, execution testing requires a Windows environment:

### Prerequisites

1. A Windows machine (physical or virtual)
2. Ability to run executables
3. (Optional) PowerShell or Command Prompt

### Testing Procedure

#### Test 1: Unpacked Version

```powershell
# Copy to Windows machine
target/e2e/hello.exe
target/e2e/assets/ (entire directory)

# Run with assets in the same directory
.\hello.exe
```

**Expected**: Success (assets loaded from disk)

#### Test 2: Packed Version

```powershell
# Copy only the executable
target/e2e/hello_packed.exe

# Run standalone (no assets needed)
.\hello_packed.exe
```

**Expected**: Success (assets loaded from embedded archive)

#### Test 3: Comparison

```powershell
# Run both and compare output
.\hello.exe > output_unpacked.txt
.\hello_packed.exe > output_packed.txt

# Compare (PowerShell)
Compare-Object (Get-Content output_unpacked.txt) (Get-Content output_packed.txt)
```

**Expected**: No differences (output should be identical)

## Performance Metrics

The benchmark script provides several key metrics:

### File Size Comparison

| Metric | Description |
|--------|-------------|
| Base Executable Size | Size of the unprotected executable |
| Protected Executable Size | Size after embedding assets |
| Protection Overhead | Additional bytes added by protection |
| Overhead Percentage | Overhead as percentage of base size |

### Storage Metrics

| Metric | Description |
|--------|-------------|
| Total Unpacked Size | Executable + external assets |
| Total Packed Size | Single protected executable |
| Space Saved | Bytes saved by embedding |
| Space Savings Percentage | Efficiency of the approach |

### Interpreting Results

**Protection Overhead**: Typical range 50KB - 500KB
- Includes runtime stub code
- Includes encryption keys and metadata
- Includes archive header structure
- Affected by PE file alignment

**Space Savings**: Typically 10-30% for small examples, higher for large asset sets
- Eliminates separate asset files
- May include compression benefits
- Reduces file system overhead

## Troubleshooting

### Build Errors

#### Error: `rustup target list --installed` fails

```bash
# Install Windows target
rustup target add x86_64-pc-windows-gnu

# Alternative target (if using MSVC toolchain)
rustup target add x86_64-pc-windows-msvc
```

#### Error: `hello.exe not found`

```bash
# Check build output
ls -lh examples/hello-world/target/x86_64-pc-windows-gnu/release/

# Rebuild explicitly
cd examples/hello-world
cargo build --release --target x86_64-pc-windows-gnu
```

#### Error: Image crate not found

```bash
# Ensure dependencies are available
cd examples/hello-world
cargo build --release
```

### Protection Errors

#### Error: `maxion-packer not found`

```bash
# Build the packer
cargo build --release -p maxion-packer

# Verify it exists
```

#### Error: Asset not found

```bash
# Verify assets are copied
ls -lh target/e2e/assets/

# Manually copy if needed
cp examples/assets/sirref.png target/e2e/assets/
```

#### Error: PE injection fails

This is typically a limitation of cross-compilation. Solutions:

1. Use a Windows machine for the protection step
2. Use Docker with Windows containers
3. Use a Windows CI/CD pipeline

### Execution Errors (on Windows)

#### Error: Asset not found in unpacked version

Ensure the `assets/` directory is in the same location as `hello.exe`.

#### Error: Packed version fails to load

Check the packer output for any warnings or errors. Common issues:

- Incorrect PE file format
- Missing encryption keys
- Archive corruption

### Missing Output

```bash
# Check output directory
ls -lh target/e2e/

# Verify all files exist
ls -lh target/e2e/hello.exe
ls -lh target/e2e/hello_packed.exe
ls -lh target/e2e/assets/
```

## Advanced Usage

### Custom Build Configuration

Modify `examples/hello-world/Cargo.toml` to add features or dependencies:

```toml
[dependencies]
image = "0.24"
serde = { version = "1.0", features = ["derive"] }
```

### Different Compression Levels

Edit the protection script:

```bash
# Lower compression (faster packing, larger output)
--compression-level 1

# Higher compression (slower packing, smaller output)
--compression-level 11
```

### Different Chunk Sizes

```bash
# Smaller chunks (more granular encryption, larger header)
--chunk-size 16384

# Larger chunks (faster decryption, less granular)
--chunk-size 131072
```

### Custom Build Secret

For reproducible builds, use a fixed build secret:

```bash
# Generate a random secret
openssl rand -hex 32

# Use it in protection
--build-secret <your-64-char-hex-string>
```

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: E2E Test

on: [push, pull_request]

jobs:
  build-and-protect:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          
      - name: Build Maxion Packer
        run: cargo build --release --bin maxion-packer
        
      - name: Build Hello World
        run: ./scripts/build_hello_world.sh
        
      - name: Protect Hello World
        run: ./scripts/protect_hello_world.sh
        
      - name: Test Unpacked
        run: ./target/e2e/hello.exe
        
      - name: Test Packed
        run: ./target/e2e/hello_packed.exe
```

## Extending the Example

Ideas for extending this E2E test:

1. **Multiple Asset Types**: Add audio files, 3D models, etc.
2. **Loading Benchmark**: Measure actual loading times
3. **Asset Streaming**: Test large file streaming
4. **Encryption Strength**: Test different encryption parameters
5. **Compression Testing**: Compare different compression levels
6. **Memory Usage**: Measure runtime memory overhead
7. **Multiple Executables**: Test with different game types

## Success Criteria

The E2E test is considered successful when:

- ✅ `hello-world` application builds without errors
- ✅ `hello-world` loads `sirref.png` successfully (when run with assets)
- ✅ `maxion-packer protect` runs without errors
- ✅ `hello_packed.exe` is generated
- ✅ File sizes are reasonable (overhead < 10MB for this minimal example)
- ✅ Space savings are demonstrated (packed < unpacked + assets)
- ⚠️  Execution verified on Windows machine (manual step)
- ✅ Benchmark report generated

## Known Limitations

1. **Cross-platform execution**: Cannot test Windows binaries on macOS
2. **Runtime performance**: Can only measure file size, not runtime performance from macOS
3. **Integration testing**: Full integration requires Windows environment for execution
4. **PE manipulation**: Some PE operations may not work correctly with cross-compiled executables

## Next Steps

1. **Windows Execution Testing** - Complete the testing on a Windows machine
2. **Performance Profiling** - Measure actual loading times and memory usage
3. **Automated Testing** - Set up CI/CD for automated E2E testing
4. **Larger Examples** - Create more complex test cases with realistic game assets
5. **Documentation** - Document findings and lessons learned

## Related Documentation

- [AutoProtected<T> Demo](./protected-cpp-example/) - C++ example with proof of protection
- [002_e2e_test.md](../plans/002_e2e_test.md) - Detailed E2E test plan
- [001_plan.md](../plans/001_plan.md) - Overall project architecture
- [ISSUES.md](../ISSUES.md) - Current issues and development status
- [README.md](../README.md) - Project overview

## Support

For issues or questions:

1. Check the troubleshooting section above
2. Review the ISSUES.md file for known problems
3. Check the implementation status in IMPLEMENTATION_STATUS.md
4. Open an issue on GitHub

## See Also

### For Variable-Level Protection

Looking to protect your game state variables from memory tampering? Both Rust and C++ provide automatic protection features!

#### Rust: `#[auto_protected]` Attribute

Zero-boilerplate syntax for automatic variable protection:

```rust
use maxion_core::auto_protected;

#[auto_protected]
struct Player {
    health: i32,
    ammo: i32,
    score: i32,
}

// Constructor generated automatically!
let player = Player::new(100, 30, 0);
player.health.set(75);
```

**Benefits:**
- ✅ Zero boilerplate
- ✅ Compile-time transformation
- ✅ Automatic constructor generation
- ✅ Type-safe

**Documentation:** `docs/06_security/008_auto_protected.md`

#### C++: `AutoProtected<T>` Template

Clean, flexible wrapper for automatic protection:

```cpp
#include "auto_protected.h"

struct Player {
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;
    AutoProtected<int32_t> score;

    Player(int32_t h, int32_t a, int32_t s)
        : health(h), ammo(a), score(s) {}
};

Player player(100, 30, 0);
player.health.set(75);
```

**Benefits:**
- ✅ Clear, explicit code
- ✅ Full control over implementation
- ✅ Can mix protected/unprotected fields
- ✅ Works with any C++ compiler

**Documentation:** `examples/protected-cpp-example/AUTO_PROTECTED_CPP.md`

#### Quick Start

**Rust:**
```bash
# Run demo
cargo run --example auto_protected_demo --package maxion-core
```

**C++:**
```bash
cd examples/protected-cpp-example
./scripts/run_auto_protected_demo.sh
```

### For Cheat Detection with Callbacks

Want to understand how protected values trigger callbacks when tampering is detected? The callback demo showcases the anti-cheat notification system!

Check out the [C++ Callback Demo](./cheat-cpp-callback-demo/):
- 🎮 **6 comprehensive demos** covering callback scenarios
- ⚠️ **Protected vs Unprotected comparison** showing the critical difference
- 🔔 **Multiple callback modes** (warning, silent, none)
- 🏷️ **Hardware ID integration** for player identification
- 🧵 **Thread-safe callback system** demonstration
- 🚨 **Different cheat types** with type-specific responses

Quick start:
```bash
cd cheat-cpp-callback-demo
mkdir build && cd build
cmake .. && make
./callback_demo
```

**Key Demo**: Protected vs Unprotected Comparison
- Shows exactly what happens when a player attempts to modify memory
- Protected values: Detects tampering → Shows warning ✅
- Unprotected values: Silent failure → No warning ❌

### For Variable-Level Protection (AutoProtected<T>)

Looking to demonstrate and verify that `AutoProtected<T>` provides genuine encryption and protection of sensitive variables?

Check out the [C++ Protected Example](./protected-cpp-example/):
- 📊 **15 unit tests** verifying encryption is working
- 🔒 **Multi-layer proof** that files are genuinely protected
- 📈 **Performance benchmarks** comparing protection modes
- 🎯 **Hex dumps and analysis** showing gibberish in encrypted data
- ✅ **Round-trip verification** ensuring no data loss

Quick start:
```bash
cd protected-cpp-example
./scripts/pack_assets.sh
./scripts/run_example.sh
```

## License

This example is part of the Maxion Protector project, licensed under MIT OR Apache-2.0.
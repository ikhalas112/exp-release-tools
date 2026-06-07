# E2E Test Plan: Hello World Example

## Overview

This plan outlines the creation and execution of a comprehensive end-to-end (E2E) test for the Maxion Protector system. The test will create a minimal "Hello World" application that loads an external asset, then protect it using the packer to demonstrate the complete workflow.

## Objectives

1. Create a minimal example application that loads an external asset (PNG image)
2. Build the application for Windows platform from macOS
3. Pack the application with encrypted assets into a single executable
4. Benchmark and compare performance between unpacked and packed versions
5. Validate the complete protection workflow

## Test Application Design

### Application: `hello_asset`

A minimal Windows application that:
- Loads `examples/assets/sirref.png` from disk
- Displays basic information about the loaded asset
- Serves as a baseline for performance comparison

### Asset: `sirref.png`

Already exists at `examples/assets/sirref.png`
- Acts as test data for asset loading
- Will be packed into the encrypted archive

## Implementation Steps

### Phase 1: Create Hello World Application (Cross-Platform)

**Location:** `examples/hello_asset/`

#### File Structure
```
examples/
├── assets/
│   └── sirref.png           # Test asset
└── hello_asset/
    ├── Cargo.toml           # Application manifest
    ├── src/
    │   └── main.rs          # Application entry point
    └── build.rs             # Windows cross-compilation script
```

#### `examples/hello_asset/Cargo.toml`

```toml
[package]
name = "hello_asset"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "hello"
path = "src/main.rs"

[dependencies]
image = "0.24"
```

#### `examples/hello_asset/src/main.rs`

```rust
use std::path::Path;

fn main() {
    println!("Hello Asset Loader Test");
    println!("========================");
    
    let asset_path = Path::new("assets/sirref.png");
    
    // Check if asset exists
    if !asset_path.exists() {
        eprintln!("Error: Asset not found at {}", asset_path.display());
        std::process::exit(1);
    }
    
    println!("✓ Found asset at: {}", asset_path.display());
    
    // Get asset metadata
    if let Ok(metadata) = std::fs::metadata(asset_path) {
        println!("✓ File size: {} bytes", metadata.len());
    }
    
    // Try to load and validate the image
    match image::open(asset_path) {
        Ok(img) => {
            println!("✓ Image loaded successfully");
            println!("  Dimensions: {}x{}", img.width(), img.height());
            println!("  Color type: {:?}", img.color());
        }
        Err(e) => {
            eprintln!("Error: Failed to load image: {}", e);
            std::process::exit(1);
        }
    }
    
    println!("\n✓ Test completed successfully!");
}
```

#### `examples/hello_asset/build.rs`

```rust
fn main() {
    // Set default output directory for Windows cross-compilation
    if std::env::var("CARGO_CFG_TARGET").unwrap_or_default().contains("windows") {
        println!("cargo:rerun-if-changed=build.rs");
    }
}
```

### Phase 2: Build Script Infrastructure

**Location:** `scripts/build_hello_asset.sh`

```bash
#!/bin/bash

# Build script for Hello World E2E test
# Supports building for Windows from macOS

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
EXAMPLES_DIR="$PROJECT_ROOT/examples"
HELLO_DIR="$EXAMPLES_DIR/hello_asset"
ASSETS_DIR="$EXAMPLES_DIR/assets"
OUTPUT_DIR="$PROJECT_ROOT/target/e2e"

echo "=== Hello World E2E Build Script ==="
echo "Project root: $PROJECT_ROOT"

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Determine target
if [ -z "$TARGET" ]; then
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS - cross-compile for Windows by default
        TARGET="x86_64-pc-windows-gnu"
        echo "Cross-compiling for Windows from macOS"
    else
        TARGET=$(rustc -vV | grep "^host:" | awk '{print $2}')
        echo "Building for native platform: $TARGET"
    fi
else
    echo "Building for target: $TARGET"
fi

# Install cross-compilation toolchain if needed
if [[ "$TARGET" == *"windows"* ]] && [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Checking for Windows cross-compilation toolchain..."
    if ! rustup target list --installed | grep -q "$TARGET"; then
        echo "Installing $TARGET target..."
        rustup target add "$TARGET"
    fi
fi

# Build hello asset application
echo ""
echo "Building hello_asset..."
cd "$HELLO_DIR"

if [ "$TARGET" = "x86_64-pc-windows-gnu" ]; then
    cargo build --release --target "$TARGET"
    cp "target/$TARGET/release/hello.exe" "$OUTPUT_DIR/hello.exe" || {
        echo "Warning: hello.exe not found, looking for alternative name..."
        find "target" -name "hello.exe" -exec cp {} "$OUTPUT_DIR/" \; || true
    }
elif [[ "$TARGET" == *"windows"* ]]; then
    cargo build --release --target "$TARGET"
    cp "target/$TARGET/release/hello.exe" "$OUTPUT_DIR/"
else
    cargo build --release
    cp "target/release/hello" "$OUTPUT_DIR/"
fi

# Copy assets
echo ""
echo "Copying assets..."
mkdir -p "$OUTPUT_DIR/assets"
cp "$ASSETS_DIR"/* "$OUTPUT_DIR/assets/" 2>/dev/null || true

# Display results
echo ""
echo "=== Build Complete ==="
echo "Output directory: $OUTPUT_DIR"
ls -lh "$OUTPUT_DIR/" || true
```

**Make it executable:**
```bash
chmod +x scripts/build_hello_asset.sh
```

### Phase 3: Protection Script

**Location:** `scripts/protect_hello_asset.sh`

```bash
#!/bin/bash

# Protection script for Hello World E2E test
# Uses maxion-packer to create protected executable

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="$PROJECT_ROOT/target/e2e"
PACKER_BIN="$PROJECT_ROOT/target/release/maxion-packer"

echo "=== Hello World Protection Script ==="

# Check if packer is built
if [ ! -f "$PACKER_BIN" ]; then
    echo "Building maxion-packer..."
    cargo build --release --bin maxion-packer
fi

# Check if hello executable exists
HELLO_EXE="$OUTPUT_DIR/hello.exe"
if [ ! -f "$HELLO_EXE" ]; then
    echo "Error: $HELLO_EXE not found. Run build_hello_asset.sh first."
    exit 1
fi

echo "Input: $HELLO_EXE"
echo "Assets: $OUTPUT_DIR/assets"
echo "Output: $OUTPUT_DIR/hello_packed.exe"
echo ""

# Run packer in protect mode
echo "Running maxion-packer..."
"$PACKER_BIN" protect \
    --input "$HELLO_EXE" \
    --assets "$OUTPUT_DIR/assets" \
    --output "$OUTPUT_DIR/hello_packed.exe" \
    --chunk-size 65536 \
    --compress \
    --compression-level 6

echo ""
echo "=== Protection Complete ==="
ls -lh "$OUTPUT_DIR/"*.exe
```

**Make it executable:**
```bash
chmod +x scripts/protect_hello_asset.sh
```

### Phase 4: Benchmark Script

**Location:** `scripts/benchmark_hello_asset.sh`

```bash
#!/bin/bash

# Benchmark script for Hello World E2E test
# Compares unpacked vs packed executables

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="$PROJECT_ROOT/target/e2e"

echo "=== Hello World Benchmark ==="
echo ""

HELLO_EXE="$OUTPUT_DIR/hello.exe"
HELLO_PACKED="$OUTPUT_DIR/hello_packed.exe"
ASSETS_DIR="$OUTPUT_DIR/assets"

# Check executables exist
if [ ! -f "$HELLO_EXE" ]; then
    echo "Error: $HELLO_EXE not found"
    exit 1
fi

if [ ! -f "$HELLO_PACKED" ]; then
    echo "Error: $HELLO_PACKED not found"
    exit 1
fi

# Display file sizes
echo "=== File Size Comparison ==="
HELLO_SIZE=$(stat -f%z "$HELLO_EXE" 2>/dev/null || stat -c%s "$HELLO_EXE" 2>/dev/null)
HELLO_PACKED_SIZE=$(stat -f%z "$HELLO_PACKED" 2>/dev/null || stat -c%s "$HELLO_PACKED" 2>/dev/null)

echo "Unpacked executable: $HELLO_SIZE bytes"
echo "Packed executable:   $HELLO_PACKED_SIZE bytes"
echo "Overhead:            $((HELLO_PACKED_SIZE - HELLO_SIZE)) bytes"

if [ $HELLO_SIZE -gt 0 ]; then
    OVERHEAD_PERCENT=$(( (HELLO_PACKED_SIZE * 100 / HELLO_SIZE) - 100 ))
    echo "Overhead percentage: ${OVERHEAD_PERCENT}%"
fi

echo ""

# Calculate total size with external assets
echo "=== Total Size Comparison ==="
ASSETS_SIZE=0
if [ -d "$ASSETS_DIR" ]; then
    ASSETS_SIZE=$(du -sb "$ASSETS_DIR" 2>/dev/null | cut -f1 || du -sk "$ASSETS_DIR" 2>/dev/null | awk '{print $1 * 1024}')
fi

TOTAL_UNPACKED=$((HELLO_SIZE + ASSETS_SIZE))
TOTAL_PACKED=$HELLO_PACKED_SIZE

echo "Unpacked total: $TOTAL_UNPACKED bytes (exe + assets)"
echo "Packed total:   $TOTAL_PACKED bytes (single exe)"

if [ $TOTAL_UNPACKED -gt 0 ]; then
    SAVINGS=$((TOTAL_UNPACKED - TOTAL_PACKED))
    SAVINGS_PERCENT=$((SAVINGS * 100 / TOTAL_UNPACKED))
    echo "Space saved:    $SAVINGS bytes (${SAVINGS_PERCENT}%)"
fi

echo ""

# Note about execution testing
echo "=== Execution Testing ==="
echo "Note: Cannot execute Windows binaries on macOS ($OSTYPE)"
echo ""
echo "To test execution on Windows:"
echo "1. Copy $OUTPUT_DIR/hello.exe to Windows machine"
echo "2. Copy $OUTPUT_DIR/hello_packed.exe to Windows machine"
echo "3. Run both and compare output and performance"
echo ""
echo "Expected output:"
echo "  Hello Asset Loader Test"
echo "  ========================"
echo "  ✓ Found asset at: assets/sirref.png"
echo "  ✓ File size: <bytes>"
echo "  ✓ Image loaded successfully"
echo "    Dimensions: <width>x<height>"
echo "    Color type: <type>"
echo ""
echo "  ✓ Test completed successfully!"

echo ""
echo "=== Benchmark Complete ==="
```

**Make it executable:**
```bash
chmod +x scripts/benchmark_hello_asset.sh
```

### Phase 5: Documentation

**Location:** `examples/README_E2E.md`

```markdown
# E2E Test: Hello World Asset Loader

This example demonstrates the complete Maxion Protector workflow using a minimal application that loads an external asset.

## Purpose

- Validate the protection workflow end-to-end
- Compare file sizes and overhead
- Demonstrate asset encryption and packing
- Provide a baseline for performance testing

## Quick Start

### Build the Application

```bash
# Build for Windows from macOS (default)
./scripts/build_hello_asset.sh

# Or build for native platform
TARGET=x86_64-unknown-linux-gnu ./scripts/build_hello_asset.sh
```

### Protect the Application

```bash
./scripts/protect_hello_asset.sh
```

### Benchmark and Compare

```bash
./scripts/benchmark_hello_asset.sh
```

## Files Generated

After running the scripts, you'll find:

- `target/e2e/hello.exe` - Original unprotected executable
- `target/e2e/hello_packed.exe` - Protected executable with embedded assets
- `target/e2e/assets/` - Original external assets (for comparison)

## Testing on Windows

Since we're building from macOS, you'll need to test execution on a Windows machine:

1. Copy `target/e2e/hello.exe` and `target/e2e/assets/` to Windows
2. Run: `hello.exe` (should work with external assets)
3. Copy `target/e2e/hello_packed.exe` to Windows (no assets needed)
4. Run: `hello_packed.exe` (should work with embedded assets)
5. Compare output and performance

## Expected Behavior

Both executables should:
- Load the `sirref.png` image
- Display file information
- Successfully decode the image
- Exit cleanly

## Performance Metrics

The benchmark script will report:
- File size comparison
- Total size with/without assets
- Space savings percentage
- Overhead percentage

## Troubleshooting

### Build Errors

Ensure you have the Windows target installed:
```bash
rustup target add x86_64-pc-windows-gnu
```

### Protection Errors

Ensure maxion-packer is built:
```bash
cargo build --release --bin maxion-packer
```

### Missing Output

Check the output directory:
```bash
ls -lh target/e2e/
```

## Next Steps

Once validated, this example can be extended to:
- Add more complex asset types
- Test with larger asset sets
- Measure runtime performance
- Test different compression levels
- Validate encryption integrity
```

## Execution Plan

### Step 1: Create File Structure

```bash
mkdir -p examples/hello_asset/src
mkdir -p scripts
```

### Step 2: Create Application Files

Create the files as specified in Phase 1:
- `examples/hello_asset/Cargo.toml`
- `examples/hello_asset/src/main.rs`
- `examples/hello_asset/build.rs`

### Step 3: Create Build Scripts

Create the scripts as specified in Phases 2-4:
- `scripts/build_hello_asset.sh`
- `scripts/protect_hello_asset.sh`
- `scripts/benchmark_hello_asset.sh`

Make them executable:
```bash
chmod +x scripts/*.sh
```

### Step 4: Build and Test

```bash
# Build the hello application
./scripts/build_hello_asset.sh

# Protect the application
./scripts/protect_hello_asset.sh

# Benchmark the results
./scripts/benchmark_hello_asset.sh
```

### Step 5: Verify Output

Check `target/e2e/` for generated files:
- `hello.exe` (or `hello` on non-Windows)
- `hello_packed.exe`
- `assets/` directory with `sirref.png`

### Step 6: Document Results

Record the following in `ISSUES.md` or a dedicated E2E test log:
- Build successful (yes/no)
- File sizes
- Overhead percentage
- Space saved
- Any errors encountered

## Success Criteria

The E2E test is considered successful when:

1. ✅ `hello_asset` application builds without errors
2. ✅ `hello_asset` loads `sirref.png` successfully (when run with assets)
3. ✅ `maxion-packer protect` runs without errors
4. ✅ `hello_packed.exe` is generated
5. ✅ File sizes are reasonable (overhead < 10MB for this minimal example)
6. ✅ Space savings are demonstrated (packed < unpacked + assets)
7. ⚠️  Execution verified on Windows machine (separate step)

## Known Limitations

1. **Cross-platform execution**: Cannot test Windows binaries on macOS
2. **Runtime performance**: Can only measure file size, not runtime performance from macOS
3. **Integration testing**: Full integration requires Windows environment for execution

## Next Actions

1. [ ] Create all files as specified
2. [ ] Run build script and verify compilation
3. [ ] Run protection script and verify packer integration
4. [ ] Run benchmark script and record metrics
5. [ ] Update `ISSUES.md` with E2E test results
6. [ ] Document any issues or edge cases discovered
7. [ ] Plan Windows execution testing (manual step)

## Related Documentation

- [001_plan.md](./001_plan.md) - Overall project plan
- [ISSUES.md](../ISSUES.md) - Current issues and status
- [examples/README_E2E.md](../examples/README_E2E.md) - E2E test documentation

## Status

**Status:** 📝 Planned
**Priority:** High (Phase 4 - Testing)
**Estimated Time:** 2-3 hours for setup, 30 minutes for execution
**Dependencies:** maxion-packer, maxion-injector
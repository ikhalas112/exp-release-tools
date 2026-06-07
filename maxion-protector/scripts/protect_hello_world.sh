#!/bin/bash

# Protection script for Hello World E2E test
# Uses pnp to create protected executable
# Note: Full protection (PE injection) requires Windows platform

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="$PROJECT_ROOT/target/e2e"
PACKER_BIN="$PROJECT_ROOT/target/release/pnp"

echo "=== Hello World Protection Script ==="

# Check if packer is built
if [ ! -f "$PACKER_BIN" ]; then
    echo "Building pnp..."
    cd "$PROJECT_ROOT"
    cargo build --release -p maxion-packer
fi

# Check if hello executable exists
HELLO_EXE="$OUTPUT_DIR/hello"
HELLO_PACKED="$OUTPUT_DIR/hello_packed"
# Check for Windows extension
if [ -f "$OUTPUT_DIR/hello.exe" ]; then
    HELLO_EXE="$OUTPUT_DIR/hello.exe"
    HELLO_PACKED="$OUTPUT_DIR/hello_packed.exe"
elif [ ! -f "$HELLO_EXE" ]; then
    echo "Error: Neither hello.exe nor hello found in $OUTPUT_DIR"
    echo "Run build_hello_world.sh first."
    exit 1
fi

echo "Found executable: $HELLO_EXE"

# Check if assets directory exists
ASSETS_DIR="$OUTPUT_DIR/assets"
if [ ! -d "$ASSETS_DIR" ]; then
    echo "Error: Assets directory not found at $ASSETS_DIR"
    exit 1
fi

# Check if assets exist
if [ ! -f "$ASSETS_DIR/sirref.png" ]; then
    echo "Error: Asset sirref.png not found in $ASSETS_DIR"
    exit 1
fi

echo "Input executable: $HELLO_EXE"
echo "Assets directory: $ASSETS_DIR"
echo "Output: $HELLO_PACKED"
echo ""

# Display asset info
ASSET_SIZE=$(stat -f%z "$ASSETS_DIR/sirref.png" 2>/dev/null || stat -c%s "$ASSETS_DIR/sirref.png" 2>/dev/null)
echo "Asset size: $ASSET_SIZE bytes"

# Run packer in protect mode
echo ""
echo "Running pnp protect..."

# Check if we're on Windows for full PE injection
if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]] || [[ "$OSTYPE" == "windows" ]]; then
    echo "Windows platform detected - performing full PE injection..."
    "$PACKER_BIN" protect \
        --input "$HELLO_EXE" \
        --assets "$ASSETS_DIR" \
        --output "$HELLO_PACKED" \
        --chunk-size 65536 \
        --compress \
        --compression-level 6 \
        --stub-dll "$PROJECT_ROOT/target/release/maxion_stub.dll"

    # Check if protection succeeded
    if [ ! -f "$HELLO_PACKED" ]; then
        echo ""
        echo "Error: Protection failed - protected executable not created"
        exit 1
    fi

    echo ""
    echo "=== Protection Summary ==="
    echo "Platform: $OSTYPE"
    echo "Protection type: Full PE injection"
    echo ""
    echo "✓ Protection complete!"
    echo ""
    echo "Note: For testing on non-Windows platforms, you can:"
    echo "1. Copy hello.exe and hello_packed.exe to a Windows machine"
    echo "2. Run both executables to verify they work correctly"
    echo "3. Compare outputs to ensure asset loading works"
else
    echo "Note: Non-Windows platform detected"
    echo "Creating encrypted archive only (PE injection requires Windows)"
    echo ""
    echo "To complete protection on Windows:"
    echo "  1. Copy this project to a Windows machine"
    echo "  2. Run: cargo build --release -p maxion-packer"
    echo "  3. Run: ./scripts/protect_hello_world.sh"
    echo ""
    echo "For now, creating encrypted archive only..."

    # Create encrypted archive only
    "$PACKER_BIN" pack \
        --assets "$ASSETS_DIR" \
        --output "$OUTPUT_DIR/hello.archive" \
        --chunk-size 65536 \
        --compress \
        --compression-level 6

    echo ""
    echo "✓ Encrypted archive created: $OUTPUT_DIR/hello.archive"
    echo ""
    echo "Note: Full executable protection requires Windows for PE injection"
    echo "The encrypted archive can be manually embedded on Windows if needed"

    # Exit gracefully since we only created an archive
    exit 0
fi

echo ""
echo "=== Protection Complete ==="

# Display file size comparison
HELLO_SIZE=$(stat -f%z "$HELLO_EXE" 2>/dev/null || stat -c%s "$HELLO_EXE" 2>/dev/null)
PACKED_SIZE=$(stat -f%z "$HELLO_PACKED" 2>/dev/null || stat -c%s "$HELLO_PACKED" 2>/dev/null)
OVERHEAD=$((PACKED_SIZE - HELLO_SIZE))

echo ""
echo "File Size Comparison:"
echo "  Original:      $HELLO_SIZE bytes"
echo "  Protected:     $PACKED_SIZE bytes"
echo "  Overhead:      $OVERHEAD bytes"
if [ $HELLO_SIZE -gt 0 ]; then
    OVERHEAD_PERCENT=$(( (OVERHEAD * 100) / HELLO_SIZE ))
    echo "  Overhead:      ${OVERHEAD_PERCENT}%"
fi

echo ""
echo "Output files:"
ls -lh "$OUTPUT_DIR/"*.exe

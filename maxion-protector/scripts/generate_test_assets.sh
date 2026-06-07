#!/bin/bash

# Asset Generation Script for Benchmark Testing
# Creates test assets of various sizes for performance testing

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ASSETS_DIR="$PROJECT_ROOT/examples/assets"

echo "=== Asset Generation Script ==="
echo "Output directory: $ASSETS_DIR"
echo ""

# Create assets directory
mkdir -p "$ASSETS_DIR"

# Clean up existing test assets (optional)
read -p "Clean up existing test assets? (y/N): " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Cleaning up test assets..."
    rm -f "$ASSETS_DIR"/test_asset_*.bin
    rm -f "$ASSETS_DIR"/large_asset.bin
    rm -f "$ASSETS_DIR"/medium_*.bin
    rm -f "$ASSETS_DIR"/small_*.bin
    echo "✓ Cleanup complete"
    echo ""
fi

# Generate small assets (KB range)
echo "Generating small assets (1KB - 10KB)..."
for i in $(seq 1 5); do
    SIZE=$((1024 * i))
    FILE="$ASSETS_DIR/small_${i}.bin"
    dd if=/dev/zero of="$FILE" bs=$SIZE count=1 2>/dev/null
    echo "  Created: $FILE ($SIZE bytes)"
done
echo ""

# Generate medium assets (100KB - 1MB)
echo "Generating medium assets (100KB - 1MB)..."
for i in $(seq 1 3); do
    SIZE=$((102400 * i))
    FILE="$ASSETS_DIR/medium_${i}.bin"
    dd if=/dev/zero of="$FILE" bs=$SIZE count=1 2>/dev/null
    echo "  Created: $FILE ($SIZE bytes)"
done
echo ""

# Generate large asset (5MB)
echo "Generating large asset (5MB)..."
LARGE_SIZE=$((5 * 1024 * 1024))
LARGE_FILE="$ASSETS_DIR/large_asset.bin"
dd if=/dev/zero of="$LARGE_FILE" bs=1024 count=$((LARGE_SIZE / 1024)) 2>/dev/null
echo "  Created: $LARGE_FILE ($LARGE_SIZE bytes)"
echo ""

# Generate mixed test assets for bundle testing
echo "Generating mixed test assets for bundle testing..."
for i in $(seq 1 10); do
    SIZES=(512 1024 2048 4096 8192 16384 32768 65536)
    SIZE=${SIZES[$((i % ${#SIZES[@]}))]}
    FILE="$ASSETS_DIR/test_asset_${SIZE}_${i}.bin"
    dd if=/dev/zero of="$FILE" bs=$SIZE count=1 2>/dev/null
done
echo "  Created 10 mixed assets (512B - 64KB)"
echo ""

# Create additional PNG test images if image magick is available
if command -v convert &> /dev/null; then
    echo "Generating PNG test images..."

    # Small PNG (16x16)
    convert -size 16x16 xc:white "$ASSETS_DIR/test_small.png" 2>/dev/null || true
    echo "  Created: test_small.png (16x16)"

    # Medium PNG (64x64)
    convert -size 64x64 xc:white "$ASSETS_DIR/test_medium.png" 2>/dev/null || true
    echo "  Created: test_medium.png (64x64)"

    # Large PNG (256x256)
    convert -size 256x256 xc:white "$ASSETS_DIR/test_large.png" 2>/dev/null || true
    echo "  Created: test_large.png (256x256)"

    echo ""
fi

# Calculate and display statistics
echo "=== Asset Statistics ==="
SMALL_COUNT=$(find "$ASSETS_DIR" -name "small_*.bin" | wc -l)
MEDIUM_COUNT=$(find "$ASSETS_DIR" -name "medium_*.bin" | wc -l)
LARGE_COUNT=$(find "$ASSETS_DIR" -name "large_*.bin" | wc -l)
MIXED_COUNT=$(find "$ASSETS_DIR" -name "test_asset_*.bin" | wc -l)
PNG_COUNT=$(find "$ASSETS_DIR" -name "test_*.png" | wc -l)

TOTAL_FILES=$((SMALL_COUNT + MEDIUM_COUNT + LARGE_COUNT + MIXED_COUNT + PNG_COUNT))
TOTAL_SIZE=$(du -sb "$ASSETS_DIR" | cut -f1)

echo "Small assets: $SMALL_COUNT files"
echo "Medium assets: $MEDIUM_COUNT files"
echo "Large assets: $LARGE_COUNT files"
echo "Mixed assets: $MIXED_COUNT files"
echo "PNG images: $PNG_COUNT files"
echo ""
echo "Total files: $TOTAL_FILES"
echo "Total size: $TOTAL_SIZE bytes ($((TOTAL_SIZE / 1024 / 1024)) MB)"
echo ""

echo "✓ Asset generation complete!"
echo ""
echo "Next steps:"
echo "1. Build hello-world: ./scripts/build_hello_world.sh"
echo "2. Run benchmarks: ./scripts/run_benchmarks.sh"

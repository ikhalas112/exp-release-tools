#!/bin/bash

# Maxion Protector Simple Benchmark Runner
# Runs simple_bench example and saves results with auto-numbering

set -e

# Default values
OUTPUT_DIR="benchmark_results"
OUTPUT_PREFIX="benchmark"
RELEASE=true
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --prefix)
            OUTPUT_PREFIX="$2"
            shift 2
            ;;
        --debug)
            RELEASE=false
            shift
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --output-dir DIR    Output directory (default: benchmark_results)"
            echo "  --prefix PREFIX     Output file prefix (default: benchmark)"
            echo "  --debug             Run debug build instead of release"
            echo "  --help              Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Set full output path
OUTPUT_PATH="$PROJECT_ROOT/$OUTPUT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
GRAY='\033[0;37m'
WHITE='\033[1;37m'
NC='\033[0m' # No Color

echo -e "${CYAN}=== Maxion Protector Simple Benchmark Runner ===${NC}"
echo ""

# Create output directory if it doesn't exist
if [ ! -d "$OUTPUT_PATH" ]; then
    mkdir -p "$OUTPUT_PATH"
    echo -e "${GRAY}Created directory: $OUTPUT_PATH${NC}"
    echo ""
fi

# Find next available number
NEXT_NUM=1
while true; do
    PADDED_NUM=$(printf "%03d" $NEXT_NUM)
    EXISTING_FILE="$OUTPUT_PATH/${PADDED_NUM}_${OUTPUT_PREFIX}.txt"
    if [ ! -f "$EXISTING_FILE" ]; then
        break
    fi
    ((NEXT_NUM++))
done

OUTPUT_FILE="$OUTPUT_PATH/${PADDED_NUM}_${OUTPUT_PREFIX}.txt"

echo -e "${YELLOW}Configuration:${NC}"
echo -e "  Output directory: $OUTPUT_PATH"
echo -e "  Output file:      $OUTPUT_FILE"
echo -e "  Release build:     $RELEASE"
echo ""

# Build command
BUILD_CMD="cargo run -p maxion-core --example simple_bench"
if [ "$RELEASE" = true ]; then
    BUILD_CMD="$BUILD_CMD --release"
fi

echo -e "${CYAN}=== Running Benchmark ===${NC}"
echo -e "${GRAY}Command: $BUILD_CMD${NC}"
echo ""

# Run benchmark and capture output
cd "$PROJECT_ROOT"
OUTPUT=$($BUILD_CMD 2>&1)
EXIT_CODE=$?

# Save output to file
echo "$OUTPUT" > "$OUTPUT_FILE"

if [ $EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✓ Benchmark completed successfully${NC}"
    echo -e "${GREEN}✓ Results saved to: $OUTPUT_FILE${NC}"
    echo ""

    # Show quick summary from output
    echo -e "${YELLOW}Key Results:${NC}"
    echo "$OUTPUT" | grep -E "PASS|FAIL|SLOW|✅|⚠️|Total Throughput|Encryption|Compression|Write|Read" | while IFS= read -r line; do
        echo -e "  ${WHITE}$line${NC}"
    done
else
    echo -e "${RED}✗ Benchmark failed with exit code: $EXIT_CODE${NC}"
    echo -e "${YELLOW}  Output saved to: $OUTPUT_FILE${NC}"
    echo ""
    echo -e "${YELLOW}Last 20 lines of output:${NC}"
    echo "$OUTPUT" | tail -n 20 | while IFS= read -r line; do
        echo -e "  ${GRAY}$line${NC}"
    done
    exit $EXIT_CODE
fi

echo ""
echo -e "${CYAN}=== Complete ===${NC}"
echo -e "${GRAY}Run: tail -n 50 '$OUTPUT_FILE'${NC}"
echo ""

exit 0

#!/bin/bash

# Unified Benchmark Runner for Maxion Protector
# Runs comprehensive performance benchmarks comparing unpacked vs packed executables

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
HELLO_DIR="$PROJECT_ROOT/examples/hello-world"
OUTPUT_DIR="$PROJECT_ROOT/target/e2e"
BENCHMARK_DIR="$PROJECT_ROOT/target/benchmarks"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo "=== Maxion Protector Benchmark Runner ==="
echo ""

# Parse command line arguments
SCENARIO="${1:-all}"
ITERATIONS="${2:-10}"

case "$SCENARIO" in
    small|medium|large|mixed|all)
        echo "Running scenario: $SCENARIO"
        ;;
    *)
        echo "Usage: $0 [scenario] [iterations]"
        echo ""
        echo "Scenarios:"
        echo "  small   - Benchmark small asset load (240 bytes)"
        echo "  medium  - Benchmark medium asset bundle (multiple 1KB files)"
        echo "  large   - Benchmark large asset stream (5MB)"
        echo "  mixed   - Benchmark mixed asset load (various sizes)"
        echo "  all     - Run all scenarios (default)"
        echo ""
        echo "Iterations: Number of times to run each scenario (default: 10)"
        exit 1
        ;;
esac

echo "Iterations: $ITERATIONS"
echo ""

# Create output directories
mkdir -p "$OUTPUT_DIR"
mkdir -p "$BENCHMARK_DIR"

# Step 1: Build hello-world application
echo "=== Step 1: Building hello-world application ==="
cd "$HELLO_DIR"
cargo build --release

# Detect platform and set appropriate binary name
if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]] || [[ "$OSTYPE" == "windows" ]]; then
    HELLO_EXE="$HELLO_DIR/target/release/hello.exe"
else
    HELLO_EXE="$HELLO_DIR/target/release/hello"
fi

if [ ! -f "$HELLO_EXE" ]; then
    echo -e "${RED}Error: Failed to build hello-world${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Built: $HELLO_EXE${NC}"
echo ""

# Step 2: Run unpacked benchmarks
echo "=== Step 2: Running unpacked benchmarks ==="
UNPACKED_METRICS="$BENCHMARK_DIR/unpacked_metrics.json"
UNPACKED_OUTPUT="$BENCHMARK_DIR/unpacked_output.txt"

cd "$OUTPUT_DIR"
echo "Running unpacked benchmark..."

for i in $(seq 1 $ITERATIONS); do
    echo "  Iteration $i/$ITERATIONS"
    if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]] || [[ "$OSTYPE" == "windows" ]]; then
        # Windows
        "$HELLO_EXE" "$SCENARIO" >> "$UNPACKED_OUTPUT" 2>&1 || true
    else
        # macOS/Linux
        "$HELLO_EXE" "$SCENARIO" >> "$UNPACKED_OUTPUT" 2>&1 || true
    fi
done

if [ -f "$OUTPUT_DIR/benchmark_metrics.json" ]; then
    cp "$OUTPUT_DIR/benchmark_metrics.json" "$UNPACKED_METRICS"
    echo -e "${GREEN}✓ Metrics saved to: $UNPACKED_METRICS${NC}"
else
    echo -e "${YELLOW}⚠ Warning: No metrics generated (expected for non-Windows)${NC}"
fi

echo ""

# Step 3: Build pnp
echo "=== Step 3: Building pnp ==="
cd "$PROJECT_ROOT"
cargo build --release -p maxion-packer
PACKER_BIN="$PROJECT_ROOT/target/release/pnp"

if [ ! -f "$PACKER_BIN" ]; then
    echo -e "${RED}Error: Failed to build pnp${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Built: $PACKER_BIN${NC}"
echo ""

# Step 4: Protect hello-world application
echo "=== Step 4: Protecting hello-world application ==="
PACKED_EXE="$OUTPUT_DIR/hello_packed.exe"

# Detect platform
if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]] || [[ "$OSTYPE" == "windows" ]]; then
    # Windows - full protection
    INPUT_EXE="$HELLO_EXE"
else
    # macOS/Linux - create packed binary name but may not work
    INPUT_EXE="$HELLO_EXE"
    PACKED_EXE="$OUTPUT_DIR/hello_packed"
fi

echo "Input: $INPUT_EXE"
echo "Output: $PACKED_EXE"

# Run packer
"$PACKER_BIN" protect \
    --input "$INPUT_EXE" \
    --assets "$OUTPUT_DIR/assets" \
    --output "$PACKED_EXE" \
    --chunk-size 65536 \
    --compress \
    --compression-level 6 || {
    echo -e "${YELLOW}⚠ Warning: Protection failed (expected on non-Windows)${NC}"
    echo -e "${YELLOW}⚠ Skipping packed benchmarks${NC}"
    PACKED_AVAILABLE=false
}

if [ -f "$PACKED_EXE" ]; then
    echo -e "${GREEN}✓ Protected: $PACKED_EXE${NC}"
    PACKED_AVAILABLE=true
else
    echo -e "${YELLOW}⚠ Protected executable not created${NC}"
    PACKED_AVAILABLE=false
fi

echo ""

# Step 5: Run packed benchmarks (if available)
if [ "$PACKED_AVAILABLE" = true ]; then
    echo "=== Step 5: Running packed benchmarks ==="
    PACKED_METRICS="$BENCHMARK_DIR/packed_metrics.json"
    PACKED_OUTPUT="$BENCHMARK_DIR/packed_output.txt"

    for i in $(seq 1 $ITERATIONS); do
        echo "  Iteration $i/$ITERATIONS"
        if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]] || [[ "$OSTYPE" == "windows" ]]; then
            "$PACKED_EXE" "$SCENARIO" >> "$PACKED_OUTPUT" 2>&1 || true
        else
            # On non-Windows, packed binary won't execute properly
            echo "    (Skipping - requires Windows execution)"
        fi
    done

    if [ -f "$OUTPUT_DIR/benchmark_metrics.json" ]; then
        # Rename packed metrics
        mv "$OUTPUT_DIR/benchmark_metrics.json" "$PACKED_METRICS"
        echo -e "${GREEN}✓ Metrics saved to: $PACKED_METRICS${NC}"
    fi
else
    echo "=== Step 5: Packed benchmarks skipped ==="
    echo "Packed executable not available (requires Windows)"
    PACKED_METRICS=""
fi

echo ""

# Step 6: Generate comparison report
echo "=== Step 6: Generating comparison report ==="
REPORT_FILE="$BENCHMARK_DIR/benchmark_report_$(date +%Y%m%d_%H%M%S).md"

cat > "$REPORT_FILE" <<EOF
# Maxion Protector Benchmark Report

**Date:** $(date)
**Scenario:** $SCENARIO
**Iterations:** $ITERATIONS
**Platform:** $OSTYPE

## Executive Summary

This report compares the performance of unpacked vs packed executables using the Maxion Protector system.

## Environment

- **Scenario:** $SCENARIO
- **Iterations per test:** $ITERATIONS
- **Output directory:** $BENCHMARK_DIR

## File Size Comparison

EOF

# File size comparison
if [ -f "$HELLO_EXE" ]; then
    UNPACKED_SIZE=$(stat -f%z "$HELLO_EXE" 2>/dev/null || stat -c%s "$HELLO_EXE" 2>/dev/null)
    echo "- **Unpacked executable:** $UNPACKED_SIZE bytes" >> "$REPORT_FILE"
fi

if [ -f "$PACKED_EXE" ]; then
    PACKED_SIZE=$(stat -f%z "$PACKED_EXE" 2>/dev/null || stat -c%s "$PACKED_EXE" 2>/dev/null)
    OVERHEAD=$((PACKED_SIZE - UNPACKED_SIZE))
    OVERHEAD_PCT=$(( (OVERHEAD * 100) / UNPACKED_SIZE ))

    echo "- **Packed executable:** $PACKED_SIZE bytes" >> "$REPORT_FILE"
    echo "- **Overhead:** $OVERHEAD bytes (${OVERHEAD_PCT}%)" >> "$REPORT_FILE"
fi

# Assets size
if [ -d "$OUTPUT_DIR/assets" ]; then
    ASSETS_SIZE=$(du -sb "$OUTPUT_DIR/assets" 2>/dev/null | cut -f1 || du -sk "$OUTPUT_DIR/assets" 2>/dev/null | awk '{print $1 * 1024}')
    echo "- **Assets directory:** $ASSETS_SIZE bytes" >> "$REPORT_FILE"

    if [ -n "$PACKED_SIZE" ]; then
        TOTAL_UNPACKED=$((UNPACKED_SIZE + ASSETS_SIZE))
        SAVINGS=$((TOTAL_UNPACKED - PACKED_SIZE))
        SAVINGS_PCT=$((SAVINGS * 100 / TOTAL_UNPACKED))

        echo "- **Total unpacked:** $TOTAL_UNPACKED bytes" >> "$REPORT_FILE"
        echo "- **Total packed:** $PACKED_SIZE bytes" >> "$REPORT_FILE"
        echo "- **Space saved:** $SAVINGS bytes (${SAVINGS_PCT}%)" >> "$REPORT_FILE"
    fi
fi

cat >> "$REPORT_FILE" <<EOF

## Performance Metrics

### Unpacked Performance

EOF

# Parse unpacked metrics if available
if [ -f "$UNPACKED_METRICS" ]; then
    echo "Unpacked metrics available at: $UNPACKED_METRICS" >> "$REPORT_FILE"

    # Extract timing information using jq if available
    if command -v jq &> /dev/null; then
        echo "" >> "$REPORT_FILE"
        echo '```json' >> "$REPORT_FILE"
        jq '.' "$UNPACKED_METRICS" >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
    fi
else
    echo "No metrics available (execution testing requires Windows)" >> "$REPORT_FILE"
fi

cat >> "$REPORT_FILE" <<EOF

### Packed Performance

EOF

# Parse packed metrics if available
if [ -n "$PACKED_METRICS" ] && [ -f "$PACKED_METRICS" ]; then
    echo "Packed metrics available at: $PACKED_METRICS" >> "$REPORT_FILE"

    # Extract timing information using jq if available
    if command -v jq &> /dev/null; then
        echo "" >> "$REPORT_FILE"
        echo '```json' >> "$REPORT_FILE"
        jq '.' "$PACKED_METRICS" >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
    fi
else
    echo "No metrics available (execution testing requires Windows)" >> "$REPORT_FILE"
fi

# Performance comparison
if [ -f "$UNPACKED_METRICS" ] && [ -n "$PACKED_METRICS" ] && [ -f "$PACKED_METRICS" ]; then
    cat >> "$REPORT_FILE" <<EOF

### Performance Comparison

EOF

    if command -v jq &> /dev/null; then
        echo "Comparing timing metrics..." >> "$REPORT_FILE"

        # Get average timings for each operation
        echo "" >> "$REPORT_FILE"
        echo "| Operation | Unpacked (ms) | Packed (ms) | Overhead |" >> "$REPORT_FILE"
        echo "|-----------|---------------|-------------|----------|" >> "$REPORT_FILE"

        # Extract and compare timings
        jq -r '.summary.timings | to_entries[] | "\(.key) | \(.value.avg_ms) | N/A | N/A"' "$UNPACKED_METRICS" >> "$REPORT_FILE" || true
    fi
fi

cat >> "$REPORT_FILE" <<EOF

## Detailed Output

### Unpacked Output

\`\`\`
EOF

cat "$UNPACKED_OUTPUT" >> "$REPORT_FILE" 2>/dev/null || echo "(no output)" >> "$REPORT_FILE"

cat >> "$REPORT_FILE" <<EOF
\`\`\`

### Packed Output

\`\`\`
EOF

if [ -f "$PACKED_OUTPUT" ]; then
    cat "$PACKED_OUTPUT" >> "$REPORT_FILE" 2>/dev/null || echo "(no output)" >> "$REPORT_FILE"
else
    echo "(not available)" >> "$REPORT_FILE"
fi

cat >> "$REPORT_FILE" <<EOF
\`\`\`

## Conclusion

EOF

# Write conclusion based on results
if [ "$PACKED_AVAILABLE" = true ]; then
    cat >> "$REPORT_FILE" <<EOF
Both unpacked and packed benchmarks completed successfully.
Review the metrics above to understand the performance impact of using Maxion Protector.
EOF
else
    cat >> "$REPORT_FILE" <<EOF
Unpacked benchmarks completed successfully.
Packed benchmarks require Windows execution environment.
EOF
fi

cat >> "$REPORT_FILE" <<EOF

## Files Generated

- Unpacked metrics: \`$UNPACKED_METRICS\`
- Unpacked output: \`$UNPACKED_OUTPUT\`
- Packed metrics: \`$PACKED_METRICS\`
- Packed output: \`$PACKED_OUTPUT\`
- Executables: \`$OUTPUT_DIR/\`

## Next Steps

1. Review the metrics above to identify performance bottlenecks
2. If overhead is significant, consider:
   - Adjusting compression level
   - Changing chunk size
   - Optimizing asset access patterns
3. For accurate runtime metrics, run on Windows platform

---

*Generated by Maxion Protector Benchmark Runner*
EOF

echo -e "${GREEN}✓ Report generated: $REPORT_FILE${NC}"
echo ""

# Step 7: Summary
echo "=== Benchmark Summary ==="
echo ""
echo "Scenario: $SCENARIO"
echo "Iterations: $ITERATIONS"
echo ""

if [ -f "$HELLO_EXE" ]; then
    UNPACKED_SIZE=$(stat -f%z "$HELLO_EXE" 2>/dev/null || stat -c%s "$HELLO_EXE" 2>/dev/null)
    echo -e "${BLUE}Unpacked executable:${NC} $UNPACKED_SIZE bytes"
fi

if [ "$PACKED_AVAILABLE" = true ] && [ -f "$PACKED_EXE" ]; then
    PACKED_SIZE=$(stat -f%z "$PACKED_EXE" 2>/dev/null || stat -c%s "$PACKED_EXE" 2>/dev/null)
    OVERHEAD=$((PACKED_SIZE - UNPACKED_SIZE))
    OVERHEAD_PCT=$(( (OVERHEAD * 100) / UNPACKED_SIZE ))

    echo -e "${BLUE}Packed executable:${NC}   $PACKED_SIZE bytes"
    echo -e "${BLUE}Overhead:${NC}           $OVERHEAD bytes (${OVERHEAD_PCT}%)"

    if [ -d "$OUTPUT_DIR/assets" ]; then
        ASSETS_SIZE=$(du -sb "$OUTPUT_DIR/assets" 2>/dev/null | cut -f1 || du -sk "$OUTPUT_DIR/assets" 2>/dev/null | awk '{print $1 * 1024}')
        TOTAL_UNPACKED=$((UNPACKED_SIZE + ASSETS_SIZE))
        SAVINGS=$((TOTAL_UNPACKED - PACKED_SIZE))
        SAVINGS_PCT=$((SAVINGS * 100 / TOTAL_UNPACKED))

        echo -e "${BLUE}Assets directory:${NC}   $ASSETS_SIZE bytes"
        echo -e "${BLUE}Space saved:${NC}        $SAVINGS bytes (${SAVINGS_PCT}%)"
    fi
else
    echo -e "${YELLOW}Packed executable: Not available (requires Windows)${NC}"
fi

echo ""
echo -e "${GREEN}✓ Benchmark complete!${NC}"
echo ""
echo "View detailed report: $REPORT_FILE"
echo "Benchmark directory: $BENCHMARK_DIR"
echo ""
echo "To analyze metrics further:"
echo "  jq '.' $UNPACKED_METRICS"

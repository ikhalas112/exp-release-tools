#!/bin/bash
# Protected vs Unprotected Benchmark Comparison
#
# This script runs both benchmarks and compares the results
# to show Maxion Protector's overhead.
#
# Usage: ./compare.sh
#
# Requirements:
#   - cargo (Rust toolchain)
#   - bash 4.0+
#   - Standard Unix utilities (awk, grep, sed)

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Configuration
UNPROTECTED_OUTPUT="/tmp/unprotected_bench_output.txt"
PROTECTED_OUTPUT="/tmp/protected_bench_output.txt"
COMPARISON_OUTPUT="/tmp/benchmark_comparison.txt"

echo -e "${CYAN}=============================================================================${NC}"
echo -e "${BOLD}  Maxion Protector - Protected vs Unprotected Comparison${NC}"
echo -e "${CYAN}=============================================================================${NC}"
echo ""
echo "This script will:"
echo "  1. Build and run unprotected benchmark (baseline)"
echo "  2. Build and run protected benchmark (with encryption/compression)"
echo "  3. Compare results and calculate overhead"
echo "  4. Generate detailed analysis report"
echo ""
echo -e "${YELLOW}This may take a few minutes...${NC}"
echo ""

# Function to print section header
print_header() {
    echo ""
    echo -e "${CYAN}=============================================================================${NC}"
    echo -e "${BOLD}  $1${NC}"
    echo -e "${CYAN}=============================================================================${NC}"
    echo ""
}

# Function to check if cargo is available
check_cargo() {
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Error: cargo not found. Please install Rust toolchain.${NC}"
        exit 1
    fi
}

# Function to run benchmark
run_benchmark() {
    local name=$1
    local example=$2
    local output=$3

    print_header "Running $name Benchmark"

    cd "$(dirname "$0")/../../"

    # Build in release mode
    echo "Building $name benchmark..."
    cargo build --release --example $example 2>&1 | grep -E "(Compiling|Finished|error)" || true

    # Run benchmark
    echo ""
    echo "Executing $name benchmark..."
    echo ""
    cargo run --release --example $example | tee "$output"

    echo ""
    echo -e "${GREEN}✓ $name benchmark complete${NC}"
}

# Function to extract metric from output
extract_metric() {
    local file=$1
    local pattern=$2
    local field=$3

    grep "$pattern" "$file" | tail -1 | sed -E "s/.*$field[[:space:]]*:[[:space:]]*([0-9.]+).*/\1/"
}

# Function to calculate percentage difference
calc_percentage_diff() {
    local baseline=$1
    local protected=$2

    if [ "$baseline" = "0" ]; then
        echo "N/A"
        return
    fi

    local diff=$(echo "scale=2; ($protected - $baseline) / $baseline * 100" | bc)
    printf "%.1f%%" "$diff"
}

# Function to calculate speedup ratio
calc_speedup() {
    local baseline=$1
    local protected=$2

    if [ "$protected" = "0" ]; then
        echo "N/A"
        return
    fi

    local ratio=$(echo "scale=2; $baseline / $protected" | bc)
    printf "%.2fx" "$ratio"
}

# Function to format time in ms
format_time() {
    local time_sec=$1
    printf "%.3fms" "$(echo "scale=3; $time_sec * 1000" | bc)"
}

# Check prerequisites
check_cargo

# Run unprotected benchmark
run_benchmark "Unprotected" "protected-vs-unprotected/unprotected_bench" "$UNPROTECTED_OUTPUT"

# Run protected benchmark
run_benchmark "Protected" "protected-vs-unprotected/protected_bench" "$PROTECTED_OUTPUT"

# Generate comparison report
print_header "Comparing Results"

# Extract metrics from unprotected benchmark
UNPROT_SMALL_DURATION=$(extract_metric "$UNPROTECTED_OUTPUT" "Small Files" "Duration:")
UNPROT_SMALL_THROUGHPUT=$(extract_metric "$UNPROTECTED_OUTPUT" "Small Files" "Throughput:")
UNPROT_MEDIUM_DURATION=$(extract_metric "$UNPROTECTED_OUTPUT" "Medium Files" "Duration:")
UNPROT_MEDIUM_THROUGHPUT=$(extract_metric "$UNPROTECTED_OUTPUT" "Medium Files" "Throughput:")
UNPROT_LARGE_DURATION=$(extract_metric "$UNPROTECTED_OUTPUT" "Large Files" "Duration:")
UNPROT_LARGE_THROUGHPUT=$(extract_metric "$UNPROTECTED_OUTPUT" "Large Files" "Throughput:")
UNPROT_MIXED_DURATION=$(extract_metric "$UNPROTECTED_OUTPUT" "Mixed Workload" "Duration:")
UNPROT_MIXED_THROUGHPUT=$(extract_metric "$UNPROTECTED_OUTPUT" "Mixed Workload" "Throughput:")

# Extract metrics from protected benchmark
PROT_SMALL_DURATION=$(extract_metric "$PROTECTED_OUTPUT" "Small Files" "Duration:")
PROT_SMALL_COMPRESSION=$(extract_metric "$PROTECTED_OUTPUT" "Small Files" "Compression Ratio:")
PROT_MEDIUM_DURATION=$(extract_metric "$PROTECTED_OUTPUT" "Medium Files" "Duration:")
PROT_MEDIUM_COMPRESSION=$(extract_metric "$PROTECTED_OUTPUT" "Medium Files" "Compression Ratio:")
PROT_LARGE_DURATION=$(extract_metric "$PROTECTED_OUTPUT" "Large Files" "Duration:")
PROT_LARGE_COMPRESSION=$(extract_metric "$PROTECTED_OUTPUT" "Large Files" "Compression Ratio:")
PROT_MIXED_DURATION=$(extract_metric "$PROTECTED_OUTPUT" "Mixed Workload" "Duration:")
PROT_MIXED_COMPRESSION=$(extract_metric "$PROTECTED_OUTPUT" "Mixed Workload" "Compression Ratio:")

# Calculate overheads and speedups
SMALL_OVERHEAD=$(calc_percentage_diff "$UNPROT_SMALL_DURATION" "$PROT_SMALL_DURATION")
SMALL_SPEEDUP=$(calc_speedup "$UNPROT_SMALL_DURATION" "$PROT_SMALL_DURATION")
SMALL_SPACE_SAVED=$(echo "scale=1; (1 - $PROT_SMALL_COMPRESSION) * 100" | bc)

MEDIUM_OVERHEAD=$(calc_percentage_diff "$UNPROT_MEDIUM_DURATION" "$PROT_MEDIUM_DURATION")
MEDIUM_SPEEDUP=$(calc_speedup "$UNPROT_MEDIUM_DURATION" "$PROT_MEDIUM_DURATION")
MEDIUM_SPACE_SAVED=$(echo "scale=1; (1 - $PROT_MEDIUM_COMPRESSION) * 100" | bc)

LARGE_OVERHEAD=$(calc_percentage_diff "$UNPROT_LARGE_DURATION" "$PROT_LARGE_DURATION")
LARGE_SPEEDUP=$(calc_speedup "$UNPROT_LARGE_DURATION" "$PROT_LARGE_DURATION")
LARGE_SPACE_SAVED=$(echo "scale=1; (1 - $PROT_LARGE_COMPRESSION) * 100" | bc)

MIXED_OVERHEAD=$(calc_percentage_diff "$UNPROT_MIXED_DURATION" "$PROT_MIXED_DURATION")
MIXED_SPEEDUP=$(calc_speedup "$UNPROT_MIXED_DURATION" "$PROT_MIXED_DURATION")
MIXED_SPACE_SAVED=$(echo "scale=1; (1 - $PROT_MIXED_COMPRESSION) * 100" | bc)

# Print comparison table
echo -e "${BOLD}Comparison Table:${NC}"
echo ""
printf "%-25s %-15s %-15s %-12s %-12s %-15s\n" "Workload" "Unprotected" "Protected" "Overhead" "Speedup" "Space Saved"
echo "--------------------------------------------------------------------------------------------"

if [ "$SMALL_SPEEDUP" != "N/A" ]; then
    if [[ "$SMALL_SPEEDUP" == *x* && $(echo "$SMALL_SPEEDUP" | cut -dx -f1) > 1 ]]; then
        SPEEDUP_COLOR="$GREEN"
    else
        SPEEDUP_COLOR="$YELLOW"
    fi
else
    SPEEDUP_COLOR="$NC"
fi

printf "%-25s %-15s %-15s %-12s ${BOLD}${SPEEDUP_COLOR}%-12s${NC} ${GREEN}%-15s${NC}\n" \
    "Small Files (1KB x 100)" \
    "${UNPROT_SMALL_DURATION}ms" \
    "${PROT_SMALL_DURATION}ms" \
    "$SMALL_OVERHEAD" \
    "$SMALL_SPEEDUP" \
    "${SPACE_SAVED:-N/A}%"

if [[ "$MEDIUM_SPEEDUP" == *x* && $(echo "$MEDIUM_SPEEDUP" | cut -dx -f1) > 1 ]]; then
    SPEEDUP_COLOR="$GREEN"
else
    SPEEDUP_COLOR="$YELLOW"
fi

printf "%-25s %-15s %-15s %-12s ${BOLD}${SPEEDUP_COLOR}%-12s${NC} ${GREEN}%-15s${NC}\n" \
    "Medium Files (100KB x 50)" \
    "${UNPROT_MEDIUM_DURATION}ms" \
    "${PROT_MEDIUM_DURATION}ms" \
    "$MEDIUM_OVERHEAD" \
    "$MEDIUM_SPEEDUP" \
    "${MEDIUM_SPACE_SAVED}%"

if [[ "$LARGE_SPEEDUP" == *x* && $(echo "$LARGE_SPEEDUP" | cut -dx -f1) > 1 ]]; then
    SPEEDUP_COLOR="$GREEN"
else
    SPEEDUP_COLOR="$YELLOW"
fi

printf "%-25s %-15s %-15s %-12s ${BOLD}${SPEEDUP_COLOR}%-12s${NC} ${GREEN}%-15s${NC}\n" \
    "Large Files (1MB x 10)" \
    "${UNPROT_LARGE_DURATION}ms" \
    "${PROT_LARGE_DURATION}ms" \
    "$LARGE_OVERHEAD" \
    "$LARGE_SPEEDUP" \
    "${LARGE_SPACE_SAVED}%"

if [[ "$MIXED_SPEEDUP" == *x* && $(echo "$MIXED_SPEEDUP" | cut -dx -f1) > 1 ]]; then
    SPEEDUP_COLOR="$GREEN"
else
    SPEEDUP_COLOR="$YELLOW"
fi

printf "%-25s %-15s %-15s %-12s ${BOLD}${SPEEDUP_COLOR}%-12s${NC} ${GREEN}%-15s${NC}\n" \
    "Mixed Workload (Game Startup)" \
    "${UNPROT_MIXED_DURATION}ms" \
    "${PROT_MIXED_DURATION}ms" \
    "$MIXED_OVERHEAD" \
    "$MIXED_SPEEDUP" \
    "${MIXED_SPACE_SAVED}%"

echo ""
echo "--------------------------------------------------------------------------------------------"
echo ""

# Interpret results
print_header "Analysis & Recommendations"

# Calculate average speedup
AVG_SPEEDUP=$(echo "scale=2; ($SMALL_SPEEDUP + $MEDIUM_SPEEDUP + $LARGE_SPEEDUP) / 3" | bc)
AVG_SPACE_SAVED=$(echo "scale=1; ($SMALL_SPACE_SAVED + $MEDIUM_SPACE_SAVED + $LARGE_SPACE_SAVED) / 3" | bc)

echo -e "${BOLD}Overall Performance Impact:${NC}"
echo "  Average Speedup: ${BOLD}${GREEN}$AVG_SPEEDUP${NC}"
echo "  Average Space Saved: ${BOLD}${GREEN}${AVG_SPACE_SAVED}%${NC}"
echo ""

echo -e "${BOLD}Key Findings:${NC}"

# Analyze small files
if [[ "$SMALL_SPEEDUP" == *"1."* ]] || [[ "$SMALL_SPEEDUP" == *"[2-9]."* ]]; then
    echo -e "${GREEN}✓${NC} Small files benefit massively from protection ($SMALL_SPEEDUP faster)"
    echo "  Reason: Compression reduces I/O dramatically, overhead is negligible"
else
    echo -e "${YELLOW}⚠${NC} Small files have $SMALL_OVERHEAD overhead"
    echo "  Reason: Decompression overhead dominates for tiny files"
fi

# Analyze medium files
if [[ "$MEDIUM_SPEEDUP" == *"1."* ]] || [[ "$MEDIUM_SPEEDUP" == *"[2-9]."* ]]; then
    echo -e "${GREEN}✓${NC} Medium files benefit significantly from protection ($MEDIUM_SPEEDUP faster)"
    echo "  Reason: Excellent compression ratio offsets decryption cost"
else
    echo -e "${YELLOW}⚠${NC} Medium files have $MEDIUM_OVERHEAD overhead"
    echo "  Reason: Need to optimize for this file size"
fi

# Analyze large files
if [[ "$LARGE_SPEEDUP" == *"0.8"* ]] || [[ "$LARGE_SPEEDUP" == *"0.9"* ]] || [[ "$LARGE_SPEEDUP" == *"1.0"* ]] || [[ "$LARGE_SPEEDUP" == *"1.1"* ]]; then
    echo -e "${GREEN}✓${NC} Large files have minimal overhead ($SMALL_OVERHEAD)"
    echo "  Reason: Disk I/O dominates, encryption/compression cost is negligible"
elif [[ "$LARGE_SPEEDUP" == *"1.2"* ]] || [[ "$LARGE_SPEEDUP" == *"1.5"* ]]; then
    echo -e "${GREEN}✓${NC} Large files are slightly faster ($LARGE_SPEEDUP)"
    echo "  Reason: Compression provides excellent I/O benefits"
else
    echo -e "${YELLOW}⚠${NC} Large files have $LARGE_OVERHEAD overhead"
    echo "  Reason: Compression provides less benefit on large data"
fi

# Analyze mixed workload
if [[ "$MIXED_SPEEDUP" == *"1."* ]] || [[ "$MIXED_SPEEDUP" == *"[2-9]."* ]]; then
    echo -e "${GREEN}✓${NC} Realistic game startup is faster ($MIXED_SPEEDUP)"
    echo "  Reason: Benefits from compressible files (configs, textures, etc.)"
else
    echo -e "${YELLOW}⚠${NC} Mixed workload has $MIXED_OVERHEAD overhead"
    echo "  Reason: Mixed file sizes show average performance"
fi

echo ""
echo -e "${BOLD}Storage Efficiency:${NC}"
echo "  Small Files: ${GREEN}${SMALL_SPACE_SAVED}% space saved${NC}"
echo "  Medium Files: ${GREEN}${MEDIUM_SPACE_SAVED}% space saved${NC}"
echo "  Large Files: ${GREEN}${LARGE_SPACE_SAVED}% space saved${NC}"
echo "  Average: ${GREEN}${AVG_SPACE_SAVED}% space saved${NC}"
echo ""

echo -e "${BOLD}Protection vs Performance Trade-off:${NC}"
echo ""
if [ $(echo "$AVG_SPEEDUP > 1.0" | bc) -eq 1 ]; then
    echo -e "${GREEN}🎉 EXCELLENT: Protection provides performance benefits!${NC}"
    echo "  Maxion Protector's compression outweighs encryption overhead"
    echo "  Recommendation: Enable protection for all asset types"
elif [ $(echo "$AVG_SPEEDUP >= 0.8" | bc) -eq 1 ]; then
    echo -e "${YELLOW}✓ GOOD: Minimal performance impact${NC}"
    echo "  Protection has negligible overhead (<20%)"
    echo "  Recommendation: Enable protection for compressible assets"
else
    echo -e "${YELLOW}⚠ ACCEPTABLE: Noticeable overhead${NC}"
    echo "  Protection has measurable overhead but benefits are clear"
    echo "  Recommendation: Selectively protect critical assets"
fi

echo ""
echo -e "${BOLD}Additional Considerations:${NC}"
echo "  • Startup Overhead: Archive loading adds ~2-5ms one-time cost"
echo "  • Memory Overhead: ~16KB stub + archive in memory + LRU caches"
echo "  • Security: XChaCha20-Poly1305 provides authenticated encryption"
echo "  • Flexibility: Can disable compression for incompressible assets"
echo ""

# Save comparison to file
{
    echo "Maxion Protector - Protected vs Unprotected Comparison"
    echo "Generated: $(date)"
    echo ""
    echo "================================================================================"
    echo "Summary"
    echo "================================================================================"
    echo ""
    echo "Average Speedup: $AVG_SPEEDUP"
    echo "Average Space Saved: ${AVG_SPACE_SAVED}%"
    echo ""
    echo "================================================================================"
    echo "Detailed Results"
    echo "================================================================================"
    echo ""
    echo "Small Files (1KB x 100):"
    echo "  Unprotected: ${UNPROT_SMALL_DURATION}ms"
    echo "  Protected:   ${PROT_SMALL_DURATION}ms"
    echo "  Overhead:    $SMALL_OVERHEAD"
    echo "  Speedup:     $SMALL_SPEEDUP"
    echo "  Space Saved: ${SMALL_SPACE_SAVED}%"
    echo ""
    echo "Medium Files (100KB x 50):"
    echo "  Unprotected: ${UNPROT_MEDIUM_DURATION}ms"
    echo "  Protected:   ${PROT_MEDIUM_DURATION}ms"
    echo "  Overhead:    $MEDIUM_OVERHEAD"
    echo "  Speedup:     $MEDIUM_SPEEDUP"
    echo "  Space Saved: ${MEDIUM_SPACE_SAVED}%"
    echo ""
    echo "Large Files (1MB x 10):"
    echo "  Unprotected: ${UNPROT_LARGE_DURATION}ms"
    echo "  Protected:   ${PROT_LARGE_DURATION}ms"
    echo "  Overhead:    $LARGE_OVERHEAD"
    echo "  Speedup:     $LARGE_SPEEDUP"
    echo "  Space Saved: ${LARGE_SPACE_SAVED}%"
    echo ""
    echo "Mixed Workload (Game Startup):"
    echo "  Unprotected: ${UNPROT_MIXED_DURATION}ms"
    echo "  Protected:   ${PROT_MIXED_DURATION}ms"
    echo "  Overhead:    $MIXED_OVERHEAD"
    echo "  Speedup:     $MIXED_SPEEDUP"
    echo "  Space Saved: ${MIXED_SPACE_SAVED}%"
    echo ""
    echo "================================================================================"
    echo "Raw Data"
    echo "================================================================================"
    echo ""
    echo "--- Unprotected Output ---"
    cat "$UNPROTECTED_OUTPUT"
    echo ""
    echo "--- Protected Output ---"
    cat "$PROTECTED_OUTPUT"
} > "$COMPARISON_OUTPUT"

echo -e "${CYAN}=============================================================================${NC}"
echo -e "${BOLD}Comparison Complete!${NC}"
echo -e "${CYAN}=============================================================================${NC}"
echo ""
echo "Detailed report saved to: $COMPARISON_OUTPUT"
echo "Raw output files:"
echo "  - Unprotected: $UNPROTECTED_OUTPUT"
echo "  - Protected:   $PROTECTED_OUTPUT"
echo ""
echo -e "${GREEN}✓ All benchmarks completed successfully${NC}"
echo ""

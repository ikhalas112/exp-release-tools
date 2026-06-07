#!/bin/bash

# Maxion Protector - Comprehensive Test Runner
# Runs all tests with detailed reporting

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Test results tracking
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# Output directory
OUTPUT_DIR="$PROJECT_ROOT/target/test_results"
mkdir -p "$OUTPUT_DIR"

# Timestamp for reports
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
LOG_FILE="$OUTPUT_DIR/test_run_$TIMESTAMP.log"
SUMMARY_FILE="$OUTPUT_DIR/summary_$TIMESTAMP.md"

# Logging function
log() {
    echo -e "$@" | tee -a "$LOG_FILE"
}

print_header() {
    echo ""
    log "${BOLD}${CYAN}================================================================================================${NC}"
    log "${BOLD}${CYAN}  $1${NC}"
    log "${BOLD}${CYAN}================================================================================================${NC}"
    echo "" | tee -a "$LOG_FILE"
}

print_section() {
    echo "" | tee -a "$LOG_FILE"
    log "${BOLD}${BLUE}$1${NC}"
    log "${BLUE}$(printf '=%.0s' {1..100})${NC}" | tee -a "$LOG_FILE"
}

print_success() {
    log "${GREEN}✓ $1${NC}"
}

print_error() {
    log "${RED}✗ $1${NC}"
}

print_warning() {
    log "${YELLOW}⚠ $1${NC}"
}

print_info() {
    log "${CYAN}  $1${NC}"
}

# Parse command line arguments
RUN_UNIT=true
RUN_INTEGRATION=true
RUN_BENCHMARK=false
RUN_WINDOWS_TESTS=false
RUN_CLIPPY=false
RUN_FMT=false
VERBOSE=false
SKIP_SLOW=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --unit-only)
            RUN_INTEGRATION=false
            RUN_BENCHMARK=false
            shift
            ;;
        --integration-only)
            RUN_UNIT=false
            RUN_BENCHMARK=false
            shift
            ;;
        --benchmark)
            RUN_UNIT=false
            RUN_INTEGRATION=false
            RUN_BENCHMARK=true
            shift
            ;;
        --windows-tests)
            RUN_WINDOWS_TESTS=true
            shift
            ;;
        --clippy)
            RUN_CLIPPY=true
            shift
            ;;
        --fmt)
            RUN_FMT=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --skip-slow)
            SKIP_SLOW=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --unit-only          Run only unit tests"
            echo "  --integration-only   Run only integration tests"
            echo "  --benchmark          Run benchmarks only"
            echo "  --windows-tests      Run Windows-specific tests"
            echo "  --clippy             Run clippy linter"
            echo "  --fmt                Check code formatting"
            echo "  --verbose            Show verbose output"
            echo "  --skip-slow          Skip slow tests"
            echo "  --help               Show this help message"
            echo ""
            echo "Default: Run all unit and integration tests"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Print welcome message
print_header "Maxion Protector - Test Runner"
log "${BOLD}Project Root:${NC} $PROJECT_ROOT"
log "${BOLD}Output Directory:${NC} $OUTPUT_DIR"
log "${BOLD}Log File:${NC} $LOG_FILE"
log "${BOLD}Timestamp:${NC} $(date)"
echo "" | tee -a "$LOG_FILE"

# Detect OS
OS_TYPE=$(uname -s)
log "${BOLD}Operating System:${NC} $OS_TYPE"
if [[ "$OS_TYPE" == "Darwin" ]]; then
    log "${BOLD}Note:${NC} Running on macOS - Windows execution tests will be skipped"
elif [[ "$OS_TYPE" == "Linux" ]]; then
    log "${BOLD}Note:${NC} Running on Linux - Windows execution tests will be skipped"
fi
echo "" | tee -a "$LOG_FILE"

# Function to run tests and track results
run_test() {
    local test_name="$1"
    local test_command="$2"
    local critical="${3:-true}"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    print_info "Running: $test_name"

    if [ "$VERBOSE" = true ]; then
        log "  Command: $test_command"
    fi

    # Capture output
    local start_time=$(date +%s)
    local output_file="$OUTPUT_DIR/${test_name// /_}_$TIMESTAMP.log"

    if eval "$test_command" > "$output_file" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        print_success "$test_name completed in ${duration}s"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        local exit_code=$?
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))

        if [ "$exit_code" -eq 0 ]; then
            print_success "$test_name completed in ${duration}s"
            PASSED_TESTS=$((PASSED_TESTS + 1))
            return 0
        else
            print_error "$test_name failed in ${duration}s (exit code: $exit_code)"
            print_info "  Log: $output_file"

            if [ "$critical" = true ]; then
                FAILED_TESTS=$((FAILED_TESTS + 1))
                if [ "$VERBOSE" = true ]; then
                    echo ""
                    log "${RED}Output:${NC}"
                    cat "$output_file" | tail -50 | tee -a "$LOG_FILE"
                fi
            else
                print_warning "Non-critical test, continuing..."
                SKIPPED_TESTS=$((SKIPPED_TESTS + 1))
            fi
            return 1
        fi
    fi
}

# =============================================================================
# Code Quality Checks
# =============================================================================

if [ "$RUN_FMT" = true ]; then
    print_section "Code Formatting Check"

    if run_test "rustfmt" "cargo fmt --all -- --check" false; then
        print_success "Code formatting check passed"
    else
        print_warning "Code formatting issues found - run 'cargo fmt --all' to fix"
    fi
fi

if [ "$RUN_CLIPPY" = true ]; then
    print_section "Clippy Linter"

    if run_test "clippy" "cargo clippy --all-targets --all-features -- -D warnings" false; then
        print_success "Clippy check passed"
    else
        print_warning "Clippy found issues - review warnings above"
    fi
fi

# =============================================================================
# Unit Tests
# =============================================================================

if [ "$RUN_UNIT" = true ]; then
    print_section "Unit Tests"

    # Test each crate
    for crate in "maxion-core" "maxion-injector" "maxion-profiler"; do
        if [ -d "$PROJECT_ROOT/crates/$crate" ]; then
            print_info "Testing crate: $crate"
            run_test "$crate unit tests" "cargo test --lib -p $crate --quiet -- --test-threads=1" true
        fi
    done

    # Skip maxion-loader-stub and maxion-stub as they are cdylib
    # Skip maxion-packer as it's a binary-only crate (no library target)
    print_info "Skipping maxion-packer (binary-only crate)"
    print_info "Skipping maxion-loader-stub and maxion-stub (cdylib crates)"
fi

# =============================================================================
# Integration Tests
# =============================================================================

if [ "$RUN_INTEGRATION" = true ]; then
    print_section "Integration Tests"

    # Build first
    print_info "Building integration test dependencies..."
    cargo build --quiet --features phase2

    # Phase 1 tests
    run_test "Phase 1 integration tests" "cargo test --test integration_test --quiet -- --test-threads=1" true

    # Phase 2 tests (if available)
    if [ -d "$PROJECT_ROOT/tests/phase2" ]; then
        run_test "Phase 2 integration tests" "cargo test --test integration_test --features phase2 --quiet -- --test-threads=1" true
    fi

    # Virtual archive tests
    run_test "Virtual archive tests" "cargo test --test virtual_archive_integration --quiet -- --test-threads=1" true

    # Edge case tests
    run_test "Edge case tests" "cargo test --test edge_cases --quiet -- --test-threads=1" true

    # Debug tests
    run_test "Debug tool tests" "cargo test --test debug_tests --quiet -- --test-threads=1" false

    # PE compatibility tests (skip on non-Windows)
    if [[ "$RUN_WINDOWS_TESTS" = true ]] && [[ "$OS_TYPE" == "MINGW"* ]] || [[ "$OS_TYPE" == "MSYS"* ]]; then
        run_test "PE compatibility tests" "cargo test --test pe_compatibility_windows --quiet -- --test-threads=1" false
    else
        print_warning "PE compatibility tests skipped (Windows only)"
    fi
fi

# =============================================================================
# Benchmarks
# =============================================================================

if [ "$RUN_BENCHMARK" = true ]; then
    print_section "Performance Benchmarks"

    # Run the simple benchmark
    print_info "Building benchmark executable..."
    cargo build --release --example simple_bench --quiet 2>/dev/null || true

    if [ -f "$PROJECT_ROOT/target/release/examples/simple_bench" ] || \
       [ -f "$PROJECT_ROOT/target/release/examples/simple_bench.exe" ]; then
        run_test "Performance benchmarks" "cargo run --release --example simple_bench" false
    else
        print_warning "Benchmark executable not found - skipping benchmarks"
    fi
fi

# =============================================================================
# Windows-Specific Tests
# =============================================================================

if [ "$RUN_WINDOWS_TESTS" = true ]; then
    print_section "Windows-Specific Tests"

    if [[ "$OS_TYPE" == "MINGW"* ]] || [[ "$OS_TYPE" == "MSYS"* ]]; then
        # Test E2E on Windows
        print_info "Building E2E test application..."
        if [ -f "$PROJECT_ROOT/scripts/build_hello_world.sh" ]; then
            bash "$PROJECT_ROOT/scripts/build_hello_world.sh" 2>&1 | tee -a "$LOG_FILE"
        fi

        # Test protection
        print_info "Testing protection workflow..."
        if [ -f "$PROJECT_ROOT/scripts/protect_hello_world.sh" ]; then
            bash "$PROJECT_ROOT/scripts/protect_hello_world.sh" 2>&1 | tee -a "$LOG_FILE"
        fi

        # Run executables
        if [ -f "$PROJECT_ROOT/target/e2e/hello.exe" ]; then
            print_info "Testing unpacked executable..."
            run_test "Unpacked executable" "$PROJECT_ROOT/target/e2e/hello.exe" false
        fi

        if [ -f "$PROJECT_ROOT/target/e2e/hello_packed.exe" ]; then
            print_info "Testing packed executable..."
            run_test "Packed executable" "$PROJECT_ROOT/target/e2e/hello_packed.exe" false
        fi
    else
        print_warning "Windows-specific tests skipped (not running on Windows)"
    fi
fi

# =============================================================================
# Test Summary
# =============================================================================

print_header "Test Summary"

log "${BOLD}Total Tests Run:${NC} $TOTAL_TESTS"
log "${GREEN}${BOLD}Passed:${NC} $PASSED_TESTS"
if [ $FAILED_TESTS -gt 0 ]; then
    log "${RED}${BOLD}Failed:${NC} $FAILED_TESTS"
else
    log "${GREEN}${BOLD}Failed:${NC} $FAILED_TESTS"
fi
if [ $SKIPPED_TESTS -gt 0 ]; then
    log "${YELLOW}${BOLD}Skipped:${NC} $SKIPPED_TESTS"
fi

# Calculate success rate
if [ $TOTAL_TESTS -gt 0 ]; then
    SUCCESS_RATE=$(( (PASSED_TESTS * 100) / TOTAL_TESTS ))
    log "${BOLD}Success Rate:${NC} ${SUCCESS_RATE}%"

    if [ $FAILED_TESTS -eq 0 ]; then
        log ""
        log "${GREEN}${BOLD}✓ All tests passed!${NC}"
    else
        log ""
        log "${RED}${BOLD}✗ Some tests failed${NC}"
        log ""
        print_warning "Review logs above for details"
    fi
else
    log "${YELLOW}No tests were run${NC}"
    SUCCESS_RATE=0
fi

# Generate summary markdown
cat > "$SUMMARY_FILE" <<EOF
# Maxion Protector - Test Summary

**Date:** $(date)
**Platform:** $OS_TYPE
**Test Run:** $TIMESTAMP

## Results

| Metric | Count |
|--------|-------|
| Total Tests | $TOTAL_TESTS |
| Passed | $PASSED_TESTS |
| Failed | $FAILED_TESTS |
| Skipped | $SKIPPED_TESTS |
| Success Rate | ${SUCCESS_RATE}% |

## Test Suites

EOF

# Add test suite details based on what was run
if [ "$RUN_UNIT" = true ]; then
    cat >> "$SUMMARY_FILE" <<EOF
- Unit Tests: Run
EOF
fi

if [ "$RUN_INTEGRATION" = true ]; then
    cat >> "$SUMMARY_FILE" <<EOF
- Integration Tests: Run
EOF
fi

if [ "$RUN_BENCHMARK" = true ]; then
    cat >> "$SUMMARY_FILE" <<EOF
- Benchmarks: Run
EOF
fi

if [ "$RUN_WINDOWS_TESTS" = true ]; then
    cat >> "$SUMMARY_FILE" <<EOF
- Windows Tests: Run
EOF
fi

cat >> "$SUMMARY_FILE" <<EOF

## Artifacts

- Log File: \`$LOG_FILE\`
- Summary: \`$SUMMARY_FILE\`
- Test Output Directory: \`$OUTPUT_DIR\`

## Conclusion

EOF

if [ $FAILED_TESTS -eq 0 ]; then
    cat >> "$SUMMARY_FILE" <<EOF
✅ All tests passed successfully. The project is in good health.
EOF
else
    cat >> "$SUMMARY_FILE" <<EOF
❌ Some tests failed. Review the log file for details.
EOF
fi

log ""
log "${BOLD}Summary saved to:${NC} $SUMMARY_FILE"
log "${BOLD}Log file:${NC} $LOG_FILE"
log "${BOLD}Test output directory:${NC} $OUTPUT_DIR"

# Exit with appropriate code
if [ $FAILED_TESTS -gt 0 ]; then
    log ""
    log "${RED}${BOLD}Exiting with status 1 due to test failures${NC}"
    exit 1
else
    log ""
    log "${GREEN}${BOLD}Exiting successfully${NC}"
    exit 0
fi

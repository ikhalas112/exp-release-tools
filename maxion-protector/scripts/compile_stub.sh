#!/usr/bin/env bash
# compile_stub.sh - Compile maxion-stub for embedding into protected PE files
#
# This script compiles the maxion-stub crate into a raw binary suitable for
# injection into Windows PE executables.
#
# Usage:
#   ./scripts/compile_stub.sh [options]
#
# Options:
#   --release         Build in release mode (default)
#   --debug           Build in debug mode (for development)
#   --target <triple> Target triple (e.g., x86_64-pc-windows-msvc)
#   --output <path>   Output file path (default: target/stub.bin)
#   --help            Show this help message

set -e

# Default values
MODE="release"
OUTPUT="target/stub.bin"
TARGET=""
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Print colored message
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Show help
show_help() {
    cat << EOF
compile_stub.sh - Compile maxion-stub for embedding

Usage:
    ./scripts/compile_stub.sh [options]

Options:
    --release         Build in release mode (default)
    --debug           Build in debug mode (for development)
    --target <triple> Target triple (e.g., x86_64-pc-windows-msvc)
    --output <path>   Output file path (default: target/stub.bin)
    --help            Show this help message

Examples:
    # Build for Windows x86_64 (default)
    ./scripts/compile_stub.sh

    # Build for specific target
    ./scripts/compile_stub.sh --target x86_64-pc-windows-gnu

    # Build debug version
    ./scripts/compile_stub.sh --debug

    # Custom output path
    ./scripts/compile_stub.sh --output build/stub.bin
EOF
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            MODE="release"
            shift
            ;;
        --debug)
            MODE="debug"
            shift
            ;;
        --target)
            TARGET="$2"
            shift 2
            ;;
        --output)
            OUTPUT="$2"
            shift 2
            ;;
        --help)
            show_help
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Change to project root
cd "${PROJECT_ROOT}"

print_info "Maxion Stub Compilation Script"
echo "-----------------------------------"
echo "Mode: ${MODE}"
echo "Output: ${OUTPUT}"
if [[ -n "${TARGET}" ]]; then
    echo "Target: ${TARGET}"
fi
echo ""

# Determine target triple
if [[ -z "${TARGET}" ]]; then
    # Try to detect Windows target
    if rustup target list --installed 2>/dev/null | grep -q "x86_64-pc-windows-msvc"; then
        TARGET="x86_64-pc-windows-msvc"
    elif rustup target list --installed 2>/dev/null | grep -q "x86_64-pc-windows-gnu"; then
        TARGET="x86_64-pc-windows-gnu"
    else
        print_error "No Windows target installed. Please install one:"
        echo "  rustup target add x86_64-pc-windows-msvc"
        echo "  rustup target add x86_64-pc-windows-gnu"
        exit 1
    fi
fi

# Install target if needed
if ! rustup target list --installed 2>/dev/null | grep -q "${TARGET}"; then
    print_warn "Target ${TARGET} not installed, installing..."
    rustup target add "${TARGET}"
fi

# Build flags
BUILD_FLAGS=()
if [[ "${MODE}" == "release" ]]; then
    BUILD_FLAGS+=("--release")
    print_info "Building in release mode with optimizations..."
else
    BUILD_FLAGS+=("--debug")
    print_info "Building in debug mode..."
fi

# Additional optimization flags for stub
STUB_RUSTFLAGS=""

if [[ "${MODE}" == "release" ]]; then
    # Maximize size optimization for stub
    # Note: LTO removed due to incompatibility with embed-bitcode=no in dependencies
    STUB_RUSTFLAGS="-C opt-level=z -C codegen-units=1 -C panic=abort"
fi

# Compile the stub as a dynamic library (cdylib)
print_info "Compiling maxion-stub crate..."

# Set RUSTFLAGS for stub optimization
export RUSTFLAGS="${STUB_RUSTFLAGS}"

# Build the crate
cargo build --package maxion-stub --target "${TARGET}" "${BUILD_FLAGS[@]}"

if [[ $? -ne 0 ]]; then
    print_error "Failed to compile stub"
    exit 1
fi

# Determine build output path (maxion-stub is a cdylib, produces DLL on Windows)
if [[ "${MODE}" == "release" ]]; then
    if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]] || [[ "$OSTYPE" == "windows" ]]; then
        STUB_DLL="target/${TARGET}/release/maxion_stub.dll"
    else
        STUB_DLL="target/${TARGET}/release/libmaxion_stub.so"
    fi
else
    if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]] || [[ "$OSTYPE" == "windows" ]]; then
        STUB_DLL="target/${TARGET}/debug/maxion_stub.dll"
    else
        STUB_DLL="target/${TARGET}/debug/libmaxion_stub.so"
    fi
fi

if [[ ! -f "${STUB_DLL}" ]]; then
    print_error "Compiled library not found: ${STUB_DLL}"
    exit 1
fi

print_info "Stub library compiled: ${STUB_DLL}"

# Copy DLL to output location if specified
if [[ -n "${OUTPUT}" ]]; then
    mkdir -p "$(dirname "${OUTPUT}")"
    cp "${STUB_DLL}" "${OUTPUT}"
    print_info "Stub binary copied to: ${OUTPUT}"

    # Show file size
    STUB_SIZE=$(stat -f%z "${OUTPUT}" 2>/dev/null || stat -c%s "${OUTPUT}" 2>/dev/null)
    print_info "Stub binary size: ${STUB_SIZE} bytes"
fi

# Print summary
echo ""
print_info "Compilation complete!"
echo ""
echo "Artifacts:"
if [[ -f "${STUB_DLL}" ]]; then
    echo "  Library: ${STUB_DLL}"
fi
if [[ -n "${OUTPUT}" ]] && [[ -f "${OUTPUT}" ]]; then
    echo "  Binary:  ${OUTPUT}"
fi
echo ""
echo "Next steps:"
echo "  1. The stub DLL can be loaded at runtime for asset decryption"
echo "  2. For injection-based protection, the DLL is already embedded in pnp"
echo "  3. No additional binary extraction required"
echo ""

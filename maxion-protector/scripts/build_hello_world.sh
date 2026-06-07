#!/bin/bash

# Build script for Hello World E2E test
# Supports building for Windows from macOS

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
EXAMPLES_DIR="$PROJECT_ROOT/examples"
HELLO_DIR="$EXAMPLES_DIR/hello-world"
ASSETS_DIR="$EXAMPLES_DIR/assets"
OUTPUT_DIR="$PROJECT_ROOT/target/e2e"

echo "=== Hello World E2E Build Script ==="
echo "Project root: $PROJECT_ROOT"

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Determine target
if [ -z "$TARGET" ]; then
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # Check if MinGW is available for cross-compilation
        if command -v x86_64-w64-mingw32-gcc &> /dev/null; then
            TARGET="x86_64-pc-windows-gnu"
            echo "Cross-compiling for Windows from macOS"
        else
            echo "Warning: MinGW linker not found"
            echo "To install MinGW on macOS:"
            echo "  brew install mingw-w64"
            echo ""
            echo "Falling back to native build..."
            TARGET=$(rustc -vV | grep "^host:" | awk '{print $2}')
            echo "Building for native platform: $TARGET"
        fi
    else
        TARGET=$(rustc -vV | grep "^host:" | awk '{print $2}')
        echo "Building for native platform: $TARGET"
    fi
else
    echo "Building for target: $TARGET"
fi

# Install cross-compilation toolchain if needed
if [[ "$TARGET" == *"windows"* ]]; then
    echo "Checking for Windows cross-compilation toolchain..."
    if ! rustup target list --installed | grep -q "$TARGET"; then
        echo "Installing $TARGET target..."
        rustup target add "$TARGET"
    fi

    # Verify MinGW linker for GNU targets
    if [[ "$TARGET" == *"-gnu"* ]] && ! command -v x86_64-w64-mingw32-gcc &> /dev/null; then
        echo ""
        echo "Error: MinGW linker not found for target $TARGET"
        echo ""
        echo "To install MinGW:"
        echo "  macOS:   brew install mingw-w64"
        echo "  Ubuntu:  sudo apt install mingw-w64"
        echo "  Fedora:  sudo dnf install mingw64-gcc"
        echo ""
        echo "Alternatively, build for Windows MSVC target:"
        echo "  rustup target add x86_64-pc-windows-msvc"
        echo "  TARGET=x86_64-pc-windows-msvc $0"
        echo ""
        exit 1
    fi
fi

# Build hello world application
echo ""
echo "Building hello-world..."
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
if [ -d "$ASSETS_DIR" ]; then
    cp "$ASSETS_DIR"/* "$OUTPUT_DIR/assets/" 2>/dev/null || true
fi

# Display results
echo ""
echo "=== Build Complete ==="
echo "Output directory: $OUTPUT_DIR"
ls -lh "$OUTPUT_DIR/" || true

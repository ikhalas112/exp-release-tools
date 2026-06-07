#!/bin/bash

# Script to compile and run auto_protected demo
# Usage: ./scripts/run_auto_protected_demo.sh

set -e  # Exit on error

echo "=================================="
echo "AutoProtected<T> Demo Builder"
echo "=================================="
echo ""

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXAMPLE_DIR="$(dirname "$SCRIPT_DIR")"

cd "$EXAMPLE_DIR"

echo "📁 Working directory: $(pwd)"
echo ""

# Check if source file exists
if [ ! -f "auto_protected_demo.cpp" ]; then
    echo "❌ Error: auto_protected_demo.cpp not found"
    exit 1
fi

# Check if header file exists
if [ ! -f "auto_protected.h" ]; then
    echo "❌ Error: auto_protected.h not found"
    exit 1
fi

echo "✅ Found source files"
echo ""

# Compile
echo "🔨 Compiling auto_protected_demo.cpp..."
g++ -std=c++17 -o auto_protected_demo auto_protected_demo.cpp -I. -O2

if [ $? -ne 0 ]; then
    echo ""
    echo "❌ Compilation failed!"
    exit 1
fi

echo "✅ Compilation successful!"
echo ""

# Run the demo
echo "🚀 Running auto_protected_demo..."
echo "=================================="
echo ""

./auto_protected_demo

# Clean up
echo ""
echo "=================================="
echo "🧹 Cleaning up..."
rm -f auto_protected_demo
echo "✅ Done!"

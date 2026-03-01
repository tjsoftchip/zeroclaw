#!/usr/bin/env bash
# Local development test script for Windows (Git Bash / WSL)
# This script allows testing OpenWrt-related code on Windows

set -e

echo "=== ZeroClaw Local Development Test ==="
echo ""

TARGET="${1:-x86_64-unknown-linux-musl}"

echo "Building for target: ${TARGET}"
echo ""

if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]] || [[ "$OSTYPE" == "cygwin" ]]; then
    echo "Detected Windows environment"
    echo ""
    echo "For OpenWrt cross-compilation, please use one of these options:"
    echo ""
    echo "1. GitHub Actions (Recommended):"
    echo "   - Push changes to GitHub"
    echo "   - CI will automatically build for all architectures"
    echo ""
    echo "2. WSL2 (Windows Subsystem for Linux):"
    echo "   - Install WSL2 with Ubuntu"
    echo "   - Run this script inside WSL2"
    echo ""
    echo "3. Docker:"
    echo "   - Run: docker run --rm -v $(pwd):/src -w /src rust:alpine cargo build --release-openwrt --target ${TARGET}"
    echo ""
    echo "Building native Windows binary for testing..."
    cargo build --release --features wasm-tools
    echo ""
    echo "Native Windows binary built at: target/release/zeroclaw.exe"
    echo ""
    echo "You can test the rollback and UCI modules with:"
    echo "  cargo test --lib rollback::"
    echo "  cargo test --lib openwrt::"
    echo ""
else
    echo "Detected Linux/Unix environment"
    
    if command -v rustup &> /dev/null; then
        echo "Installing target ${TARGET}..."
        rustup target add "${TARGET}" || true
    fi
    
    echo "Building for ${TARGET}..."
    cargo build --release-openwrt --target "${TARGET}" --features wasm-tools || {
        echo ""
        echo "Build failed. You may need to install cross-compilation tools:"
        echo "  Ubuntu/Debian: sudo apt-get install musl-tools"
        echo "  Or use: cargo install cross"
        echo "  Then: cross build --target ${TARGET}"
    }
fi

echo ""
echo "=== Available test commands ==="
echo "  cargo test                    # Run all tests"
echo "  cargo test --lib rollback::   # Test rollback module"
echo "  cargo test --lib openwrt::    # Test UCI module"
echo "  cargo clippy -- -D warnings   # Run linter"
echo "  cargo fmt -- --check          # Check formatting"

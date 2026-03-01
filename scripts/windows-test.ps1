# ZeroClaw Windows Development Test Script
# Run this script in PowerShell to test the project on Windows

Write-Host "=== ZeroClaw Windows Development Test ===" -ForegroundColor Cyan
Write-Host ""

# Check Rust installation
Write-Host "Checking Rust installation..." -ForegroundColor Yellow
if (Get-Command rustc -ErrorAction SilentlyContinue) {
    $rustVersion = rustc --version
    Write-Host "  Rust: $rustVersion" -ForegroundColor Green
} else {
    Write-Host "  Rust not found! Please install from https://rustup.rs/" -ForegroundColor Red
    exit 1
}

# Check Cargo
if (Get-Command cargo -ErrorAction SilentlyContinue) {
    $cargoVersion = cargo --version
    Write-Host "  Cargo: $cargoVersion" -ForegroundColor Green
} else {
    Write-Host "  Cargo not found!" -ForegroundColor Red
    exit 1
}

Write-Host ""

# Run tests
Write-Host "Running unit tests..." -ForegroundColor Yellow
Write-Host ""

# Test rollback module
Write-Host "Testing rollback module..." -ForegroundColor Cyan
cargo test --lib rollback:: 2>&1 | ForEach-Object { Write-Host "  $_" }
Write-Host ""

# Test openwrt module
Write-Host "Testing openwrt module..." -ForegroundColor Cyan
cargo test --lib openwrt:: 2>&1 | ForEach-Object { Write-Host "  $_" }
Write-Host ""

# Test network module
Write-Host "Testing network module..." -ForegroundColor Cyan
cargo test --lib network:: 2>&1 | ForEach-Object { Write-Host "  $_" }
Write-Host ""

# Test parental module
Write-Host "Testing parental module..." -ForegroundColor Cyan
cargo test --lib parental:: 2>&1 | ForEach-Object { Write-Host "  $_" }
Write-Host ""

# Run clippy
Write-Host "Running clippy linter..." -ForegroundColor Yellow
cargo clippy 2>&1 | ForEach-Object { Write-Host "  $_" }
Write-Host ""

# Check formatting
Write-Host "Checking code formatting..." -ForegroundColor Yellow
cargo fmt --check 2>&1 | ForEach-Object { Write-Host "  $_" }
Write-Host ""

# Build for Windows
Write-Host "Building for Windows (debug)..." -ForegroundColor Yellow
cargo build 2>&1 | ForEach-Object { Write-Host "  $_" }
Write-Host ""

# Summary
Write-Host "=== Test Summary ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "For OpenWrt-specific testing:" -ForegroundColor Yellow
Write-Host "  1. Use WSL2: wsl --install -d Ubuntu" -ForegroundColor White
Write-Host "  2. Or use GitHub Actions for cross-compilation" -ForegroundColor White
Write-Host ""
Write-Host "To build for x86_64 Linux:" -ForegroundColor Yellow
Write-Host "  rustup target add x86_64-unknown-linux-musl" -ForegroundColor White
Write-Host "  cargo build --target x86_64-unknown-linux-musl" -ForegroundColor White
Write-Host ""
Write-Host "For ARM/MIPS targets, use GitHub Actions." -ForegroundColor Yellow

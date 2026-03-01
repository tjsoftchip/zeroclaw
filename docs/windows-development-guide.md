# Windows Development Guide for ZeroClaw

## Prerequisites

1. Install Rust: https://rustup.rs/
2. Install Git: https://git-scm.com/download/win
3. Install Visual Studio Build Tools (for some native dependencies)

## Testing on Windows

### 1. Core Logic Tests (Run on Windows)

```powershell
# Test rollback system
cargo test --lib rollback::

# Test UCI system (mocked)
cargo test --lib openwrt::

# Test network tools (mocked)
cargo test --lib network::

# Test parental control (mocked)
cargo test --lib parental::

# Run all tests
cargo test --lib
```

### 2. OpenWrt-Specific Tests (Run on WSL2)

```bash
# Install WSL2 with Ubuntu
wsl --install -d Ubuntu

# Install Rust in WSL2
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Run tests in WSL2
cargo test --lib
```

### 3. Mocking Linux-Specific Code

For Windows testing, create mock implementations:

```rust
#[cfg(not(target_os = "linux"))]
pub fn read_arp_table() -> Vec<ArpEntry> {
    vec![] // Mock for Windows
}

#[cfg(target_os = "linux")]
pub fn read_arp_table() -> Vec<ArpEntry> {
    // Real implementation
}
```

## Building for Windows

```powershell
# Debug build
cargo build --features wasm-tools

# Release build
cargo build --release --features wasm-tools

# Run the binary
.\target\release\zeroclaw.exe
```

## Cross-Compilation via GitHub Actions

1. Fork the ZeroClaw repository
2. Push your changes
3. GitHub Actions will automatically:
   - Build for ARM, MIPS, x86 architectures
   - Create .ipk packages
   - Upload as artifacts
4. Download artifacts from Actions page

## Common Issues

### Issue: "Failed to open /proc/net/arp"

**Solution**: This is expected on Windows. Use WSL2 or GitHub Actions for Linux-specific testing.

### Issue: "Target not found"

**Solution**: Add the target:
```bash
rustup target add x86_64-unknown-linux-musl
```

### Issue: "Linker not found"

**Solution**: Use GitHub Actions for cross-compilation instead of local cross-compilation.

## Recommended Workflow

1. **Develop on Windows**:
   - Write code in VS Code
   - Run `cargo test` for unit tests
   - Run `cargo clippy` for linting

2. **Commit and Push**:
   ```bash
   git add .
   git commit -m "feat: add new feature"
   git push
   ```

3. **Verify with CI**:
   - Check GitHub Actions status
   - Download and test .ipk packages

4. **Deploy to Router**:
   - Copy .ipk to router
   - Run: `opkg install zeroclaw_*.ipk`

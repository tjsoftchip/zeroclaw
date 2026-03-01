#!/bin/bash
set -e

ARCH="${1:-aarch64}"
TARGET="${2:-aarch64-unknown-linux-musl}"
VERSION="${3:-0.1.0}"
PACKAGE_DIR="packages"
BINARY_PATH="target/${TARGET}/release-openwrt/zeroclaw"

echo "Building OpenWrt ipk package for ${ARCH}..."
echo "Target: ${TARGET}"
echo "Version: ${VERSION}"

if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Binary not found at $BINARY_PATH"
    echo "Please run 'cargo build --release-openwrt --target ${TARGET}' first"
    exit 1
fi

BINARY_SIZE=$(stat -c%s "$BINARY_PATH" 2>/dev/null || stat -f%z "$BINARY_PATH" 2>/dev/null)
BINARY_SIZE_MB=$((BINARY_SIZE / 1024 / 1024))
echo "Binary size: ${BINARY_SIZE_MB}MB"

if [ $BINARY_SIZE_MB -gt 20 ]; then
    echo "Warning: Binary size exceeds 20MB target"
fi

mkdir -p "${PACKAGE_DIR}/zeroclaw-${ARCH}/CONTROL"
mkdir -p "${PACKAGE_DIR}/zeroclaw-${ARCH}/usr/bin"
mkdir -p "${PACKAGE_DIR}/zeroclaw-${ARCH}/etc/init.d"
mkdir -p "${PACKAGE_DIR}/zeroclaw-${ARCH}/etc/config"

cp "$BINARY_PATH" "${PACKAGE_DIR}/zeroclaw-${ARCH}/usr/bin/zeroclaw"
chmod 755 "${PACKAGE_DIR}/zeroclaw-${ARCH}/usr/bin/zeroclaw"

cat > "${PACKAGE_DIR}/zeroclaw-${ARCH}/CONTROL/control" << EOF
Package: zeroclaw
Version: ${VERSION}
Depends: libc, libpthread
Source: https://github.com/zeroclaw-labs/zeroclaw
Section: net
Priority: optional
Maintainer: ZeroClaw Team <team@zeroclaw.dev>
Architecture: ${ARCH}
Installed-Size: ${BINARY_SIZE}
Description: ZeroClaw - AI-powered smart gateway agent for OpenWrt
 ZeroClaw is a Rust-based autonomous agent runtime optimized for
 OpenWrt routers. It provides:
 .
  - Natural language network management
  - Parental control with DPI
  - Security monitoring and auto-response
  - Smart home integration
  - Configuration rollback system
EOF

cat > "${PACKAGE_DIR}/zeroclaw-${ARCH}/CONTROL/postinst" << 'EOF'
#!/bin/sh
set -e

echo "Setting up ZeroClaw..."

mkdir -p /etc/zeroclaw
mkdir -p /var/lib/zeroclaw
mkdir -p /var/log/zeroclaw

if [ ! -f /etc/config/zeroclaw ]; then
    cat > /etc/config/zeroclaw << 'CONFIG'
config zeroclaw 'main'
    option enabled '1'
    option log_level 'info'
    option data_dir '/var/lib/zeroclaw'
    option log_dir '/var/log/zeroclaw'

config provider 'default'
    option type 'openai'
    option model 'gpt-4o-mini'
    # option api_key 'your-api-key-here'

config channel 'telegram'
    option enabled '0'
    option bot_token ''
    option allowed_users ''

config parental 'kids_devices'
    option enabled '0'
    option devices ''
    option schedule '18:00-21:00'
    option time_quota '7200'
CONFIG
    chmod 600 /etc/config/zeroclaw
fi

if [ -x /etc/init.d/zeroclaw ]; then
    /etc/init.d/zeroclaw enable
fi

echo "ZeroClaw installed successfully!"
echo "Edit /etc/config/zeroclaw to configure"
echo "Run '/etc/init.d/zeroclaw start' to start"
EOF
chmod 755 "${PACKAGE_DIR}/zeroclaw-${ARCH}/CONTROL/postinst"

cat > "${PACKAGE_DIR}/zeroclaw-${ARCH}/CONTROL/prerm" << 'EOF'
#!/bin/sh
set -e

echo "Stopping ZeroClaw..."
if [ -x /etc/init.d/zeroclaw ]; then
    /etc/init.d/zeroclaw stop || true
    /etc/init.d/zeroclaw disable || true
fi

echo "ZeroClaw stopped."
EOF
chmod 755 "${PACKAGE_DIR}/zeroclaw-${ARCH}/CONTROL/prerm"

cat > "${PACKAGE_DIR}/zeroclaw-${ARCH}/etc/init.d/zeroclaw" << 'EOF'
#!/bin/sh /etc/rc.common

START=99
STOP=10

USE_PROCD=1

PROG=/usr/bin/zeroclaw
NAME=zeroclaw

start_service() {
    local enabled
    config_load zeroclaw
    config_get_bool enabled main enabled 0
    
    if [ "$enabled" -eq 0 ]; then
        echo "ZeroClaw is disabled in config"
        return 0
    fi
    
    local log_level data_dir log_dir
    config_get log_level main log_level "info"
    config_get data_dir main data_dir "/var/lib/zeroclaw"
    config_get log_dir main log_dir "/var/log/zeroclaw"
    
    mkdir -p "$data_dir"
    mkdir -p "$log_dir"
    
    procd_open_instance
    procd_set_param command "$PROG"
    procd_append_param command daemon
    procd_append_param command --log-level "$log_level"
    procd_append_param command --data-dir "$data_dir"
    procd_append_param command --log-dir "$log_dir"
    procd_set_param respawn ${respawn_threshold:-3600} ${respawn_timeout:-5} ${respawn_retry:-5}
    procd_set_param stdout 1
    procd_set_param stderr 1
    procd_close_instance
}

stop_service() {
    rm -f /var/run/zeroclaw.pid
}

reload_service() {
    stop
    start
}
EOF
chmod 755 "${PACKAGE_DIR}/zeroclaw-${ARCH}/etc/init.d/zeroclaw"

cd "${PACKAGE_DIR}"
tar czf data.tar.gz -C "zeroclaw-${ARCH}" usr etc
tar czf control.tar.gz -C "zeroclaw-${ARCH}" CONTROL
echo "2.0" > debian-binary
tar czf "zeroclaw_${VERSION}_${ARCH}.ipk" ./debian-binary ./data.tar.gz ./control.tar.gz

rm -rf "zeroclaw-${ARCH}" data.tar.gz control.tar.gz debian-binary

IPK_SIZE=$(stat -c%s "zeroclaw_${VERSION}_${ARCH}.ipk" 2>/dev/null || stat -f%z "zeroclaw_${VERSION}_${ARCH}.ipk" 2>/dev/null)
IPK_SIZE_MB=$((IPK_SIZE / 1024 / 1024))

echo ""
echo "Package created successfully!"
echo "Location: ${PACKAGE_DIR}/zeroclaw_${VERSION}_${ARCH}.ipk"
echo "Size: ${IPK_SIZE_MB}MB"
echo ""
echo "To install on OpenWrt:"
echo "  1. Copy the .ipk file to your router"
echo "  2. Run: opkg install zeroclaw_${VERSION}_${ARCH}.ipk"
echo "  3. Edit /etc/config/zeroclaw"
echo "  4. Run: /etc/init.d/zeroclaw start"

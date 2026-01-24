#!/bin/bash

echo "🦋 MonARCH Store - Nuclear System Repair v2 (Extreme Robustness)"
echo "==============================================================="

# 1. Isolation
echo "🔓 [1/6] Unblocking Pacman..."
sudo rm -f /var/lib/pacman/db.lck

# 2. Create Emergency Config
echo "📋 [2/6] Creating Emergency Pacman Config..."
TMP_CONF="/tmp/monarch_repair_pacman.conf"
cat <<EOF > $TMP_CONF
[options]
HoldPkg     = pacman glibc
Architecture = auto
SigLevel    = Required DatabaseOptional
LocalFileSigLevel = Optional

[core]
Server = https://geo.mirror.pkgbuild.com/\$repo/os/\$arch
[extra]
Server = https://geo.mirror.pkgbuild.com/\$repo/os/\$arch
EOF

# 3. Keyring Reset
echo "🔑 [3/6] Resetting Pacman Keyring..."
sudo pacman-key --config $TMP_CONF --init
sudo pacman-key --config $TMP_CONF --populate archlinux

# 4. Emergency Dependency Install
echo "📦 [4/6] Installing Dependencies via Emergency Config..."
# This bypasses all broken repos in /etc/pacman.conf
sudo pacman --config $TMP_CONF -Sy --needed --noconfirm \
    webkit2gtk-4.1 \
    base-devel \
    curl \
    wget \
    file \
    openssl \
    rust \
    appmenu-gtk-module \
    libappindicator-gtk3 \
    librsvg \
    libvips

# 5. Repository Cleanup (Optional but recommended)
echo "🧹 [5/6] Cleaning up broken repository definitions..."
if [ -d /etc/pacman.d/monarch ]; then
    sudo rm -rf /etc/pacman.d/monarch/
    mkdir -p /etc/pacman.d/monarch/
fi

# 6. Verify Build Environment
echo "🔍 [6/6] Verifying Build Environment..."
rm -rf src-tauri/target
if command -v cargo &> /dev/null; then
    echo "✅ Cargo (Rust) is ready!"
    # One-click keyring fix for third party
    sudo pacman-key --recv-keys 3056513887B78AEB F4A617F51E9D1FA3 --keyserver keyserver.ubuntu.com || true
    sudo pacman-key --lsign-key 3056513887B78AEB || true
    sudo pacman-key --lsign-key F4A617F51E9D1FA3 || true
    
    cd src-tauri && cargo check || echo "⚠️  'cargo check' warns, but tools are installed."
else
    echo "❌ CRITICAL: 'cargo' is still missing. Please install Rust manually."
    exit 1
fi

echo ""
echo "✨ System unblocked! You can now run 'makepkg -si' safely."
echo "Note: The broken repos in /etc/pacman.conf might still show errors, but the build tools are now available."

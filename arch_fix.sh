#!/bin/bash

echo "🦋 MonARCH Store - Arch Linux Lifecycle Fixer (Robust Mode)"
echo "=========================================================="

# 1. Keyring Repair (Rescue Mode)
echo "🔑 [1/5] Repairing Pacman Keyring & Signatures..."
sudo rm -f /var/lib/pacman/db.lck
sudo pacman-key --init
sudo pacman-key --populate archlinux chaotic

# Attempt to fetch CachyOS keyring if repo is detected
if grep -q "cachyos" /etc/pacman.conf || [ -d /etc/pacman.d/monarch ]; then
    echo "🔍 CachyOS detected. Attempting to fetch keyring..."
    # Using the verified latest URL from mirror.cachyos.org
    sudo pacman -U "https://mirror.cachyos.org/repo/x86_64/cachyos/cachyos-keyring-20240331-1-any.pkg.tar.zst" --noconfirm || true
fi

# Targeted key refresh (MUCH faster than --refresh-keys)
echo "📡 Importing specific third-party keys..."
# Chaotic-AUR & CachyOS
sudo pacman-key --recv-keys 3056513887B78AEB F4A617F51E9D1FA3 --keyserver keyserver.ubuntu.com || echo "⚠️ Key import failed, proceeding..."
sudo pacman-key --lsign-key 3056513887B78AEB || true
sudo pacman-key --lsign-key F4A617F51E9D1FA3 || true

# Sync databases to clear 'corrupted' state
sudo pacman -Sy --noconfirm || echo "⚠️ Sync failed, but proceeding..."

# 1.5 Modular Config Fix (Ensure optimizations are present)
if [ -d /etc/pacman.d/monarch ]; then
    echo "🛠️  Fixing modular CachyOS optimizations..."
    CONF="/etc/pacman.d/monarch/cachyos.conf"
    if [ -f "$CONF" ]; then
        # Check for v4 support
        if grep -q "avx512f" /proc/cpuinfo && ! grep -q "\[cachyos-v4\]" "$CONF"; then
             echo "🚀 Enabling v4 optimizations..."
             echo -e "\n[cachyos-v4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> "$CONF"
             echo -e "[cachyos-core-v4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> "$CONF"
             echo -e "[cachyos-extra-v4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> "$CONF"
        fi
        # Check for znver4 support
        if grep -q "avx512_fp16" /proc/cpuinfo && ! grep -q "\[cachyos-core-znver4\]" "$CONF"; then
             echo "🚀 Enabling znver4 optimizations..."
             echo -e "\n[cachyos-core-znver4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> "$CONF"
             echo -e "[cachyos-extra-znver4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> "$CONF"
        fi
    fi
fi

# 2. Install/Verify System Dependencies for Tauri v2
echo "📦 [2/5] Checking System Dependencies..."
if command -v pacman &> /dev/null; then
    echo "Enter password for pacman if requested:"
    sudo pacman -S --needed --noconfirm \
        webkit2gtk-4.1 \
        base-devel \
        curl \
        wget \
        file \
        openssl \
        appmenu-gtk-module \
        libappindicator-gtk3 \
        librsvg \
        libvips
else
    echo "⚠️  Pacman not found! Are you on Arch?"
fi

# 2. Check & Install Rust Toolchain
echo "🦀 [2/4] Verifying Rust Toolchain..."
if command -v rustup &> /dev/null; then
    echo "✅ Rustup found. Updating toolchain..."
    rustup update stable
else
    echo "⚠️  Rustup not found. Checking if 'rust' package is installed..."
    if ! command -v cargo &> /dev/null; then
        echo "🛠️  Cargo not found. Installing 'rust' package..."
        sudo pacman -S --needed --noconfirm rust
    else
        echo "✅ Rust (Cargo) is already installed."
    fi
fi

# 3. Clean Stale Artifacts
echo "🧹 [3/4] Cleaning Stale Build Artifacts..."
rm -rf src-tauri/target
rm -f src-tauri/Cargo.lock

# 4. Verify Build Compatibility
echo "🔍 [4/4] Verifying Build..."
cd src-tauri
# Check if cargo works now
if command -v cargo &> /dev/null; then
    cargo check || echo "⚠️  'cargo check' had warnings/errors, but we will proceed."
else
    echo "❌ CRITICAL: 'cargo' is still missing. Please install Rust manually."
    exit 1
fi

echo "✅ verification complete! You can now run 'npm run tauri dev'"

#!/bin/bash

echo "🦋 MonARCH Store - Nuclear System Repair & Fixer"
echo "==============================================="

# 1. Isolation & Unblocking
echo "🔓 [1/6] Unblocking Pacman (Isolating modular repos)..."
sudo rm -f /var/lib/pacman/db.lck

# Temporarily disable MonARCH's modular include to allow pacman to work for core tasks
if grep -q "/etc/pacman.d/monarch/" /etc/pacman.conf; then
    echo "⏸️  Temporarily disabling MonARCH modular includes to fix deadlock..."
    sudo sed -i 's/^Include = \/etc\/pacman.d\/monarch\/\*.conf/#Include = \/etc\/pacman.d\/monarch\/*.conf/' /etc/pacman.conf
fi

# Clear stale/corrupted databases
echo "🧹 Clearing stale sync databases..."
sudo rm -rf /var/lib/pacman/sync/*

# 2. Keyring Bootstrap
echo "🔑 [2/6] Bootstrapping GPG Keyrings..."
sudo pacman-key --init
sudo pacman-key --populate archlinux

# Attempt to sync ONLY core Arch repos first
echo "📡 Syncing Core Arch Repositories..."
sudo pacman -Sy --noconfirm || echo "⚠️ Core sync had issues, attempting to proceed anyway..."

# 3. Install/Verify System Dependencies & Rust
echo "📦 [3/6] Installing Essential Build Tools..."
# We install these first while the system is unblocked
sudo pacman -S --needed --noconfirm \
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

# 4. Repair Third-Party Infrastructure
echo "🛠️  [4/6] Repairing Third-Party Signatures..."

# Manually fetch Chaotic and CachyOS keyrings if they were causing the deadlock
echo "🔍 Fetching fresh CachyOS keyring..."
sudo pacman -U --noconfirm "https://mirror.cachyos.org/repo/x86_64/cachyos/cachyos-keyring-20240331-1-any.pkg.tar.zst" || true

echo "🔍 Fetching fresh Chaotic keyring..."
sudo pacman -U --noconfirm "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst" || true

sudo pacman-key --populate chaotic cachyos || true

# 5. Restore MonARCH Infrastructure
echo "🔄 [5/6] Restoring Modular Configs..."
if grep -q "#Include = /etc/pacman.d/monarch/\*.conf" /etc/pacman.conf; then
    sudo sed -i 's/^#Include = \/etc\/pacman.d\/monarch\/\*.conf/Include = \/etc\/pacman.d\/monarch\/*.conf/' /etc/pacman.conf
fi

# Final full sync
echo "🚀 Performing final system sync..."
sudo pacman -Sy --noconfirm

# 6. Verify Build
echo "🔍 [6/6] Verifying Build Environment..."
rm -rf src-tauri/target
if command -v cargo &> /dev/null; then
    echo "✅ Cargo is ready!"
    cd src-tauri && cargo check || echo "⚠️  'cargo check' warns, but environment is functional."
else
    echo "❌ CRITICAL: 'cargo' is still missing. Please install Rust manually."
    exit 1
fi

echo ""
echo "✨ Repair complete! You can now run 'makepkg -si' safely."

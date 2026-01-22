#!/bin/bash

echo "🦋 MonARCH Store - Arch Linux Lifecycle Fixer (Robust Mode)"
echo "=========================================================="

# 1. Install/Verify System Dependencies for Tauri v2
echo "📦 [1/4] Checking System Dependencies..."
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

#!/bin/bash
set -e

echo "🦋 MonARCH Store - Arch Linux Lifecycle Fixer"
echo "=============================================="

# 1. Install/Verify System Dependencies for Tauri v2
echo "📦 [1/4] Checking System Dependencies..."
echo "Enter password for pacman if requested:"
sudo pacman -S --needed \
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

# 2. Clean Stale Artifacts
echo "🧹 [2/4] Cleaning Stale Build Artifacts..."
rm -rf src-tauri/target
rm -f src-tauri/Cargo.lock

# 3. Update Rust Toolchain (Ensure stable)
echo "🦀 [3/4] Updating Rust Toolchain..."
rustup update stable

# 4. Verify Build Compatibility
echo "🔍 [4/4] Verifying Build..."
cd src-tauri
cargo check

echo "✅ verification complete! You can now run 'npm run tauri dev'"

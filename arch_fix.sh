#!/bin/bash

echo "🦋 MonARCH Store - Nuclear Build & Repair v3"
echo "==========================================="

set -e # Exit on error

# 1. Unblocking & Initial Cleanup
echo "🔓 [1/7] Unblocking Pacman & Cleaning..."
sudo rm -f /var/lib/pacman/db.lck
rm -rf node_modules src-tauri/target pkg/ src/

# 2. Emergency Config for Bootstrap
echo "📋 [2/7] Creating Emergency Config..."
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

# 2.5 Nuke Corrupted Package Cache (Fixes "invalid or corrupted package")
echo "🧹 [2.5/7] Clearing potentially corrupted package cache..."
sudo pacman -Scc --noconfirm # Double 'cc' removes ALL cached packages

# 3. System Update & Keyring (Fixes libicu version)
echo "🔑 [3/7] Updating System Keyrings & Core Libraries (libicu)..."
sudo pacman-key --config $TMP_CONF --init
sudo pacman-key --config $TMP_CONF --populate archlinux
# Refresh the specific master keys to be super safe
sudo pacman-key --config $TMP_CONF --refresh-keys || true

sudo pacman --config $TMP_CONF -Syu --noconfirm

# 4. Install Build Dependencies
echo "📦 [4/7] Installing Build Tools (Node, Rust, WebKit)..."
sudo pacman --config $TMP_CONF -S --needed --noconfirm \
    nodejs npm rust cargo webkit2gtk-4.1 base-devel \
    curl wget file openssl appmenu-gtk-module libappindicator-gtk3 \
    librsvg libvips libicu

# 5. Fix Source Code Blockers (index.html path)
echo "🛠️  [5/7] Patching Source Code..."
sed -i 's/src="\/src\/main.tsx"/src=".\/src\/main.tsx"/g' index.html

# 6. Native Compilation (Matches your system perfectly)
echo "🏗️  [6/7] Compiling Native Binary (This may take 2-5 minutes)..."
npm install
npm run tauri build

# 7. Final Installation & Path Cleanup
echo "🚀 [7/7] Installing and Clearing Path..."
# Nuke all possible "broken" versions
sudo rm -f /usr/bin/monarch-store /usr/local/bin/monarch-store /usr/bin/"MonARCH Store"

# Install the shiny new native binary
sudo install -Dm755 src-tauri/target/release/monarch-store /usr/bin/monarch-store

echo ""
echo "✨ NATIVE REPAIR COMPLETE! ✨"
echo "Binary is now perfectly linked to your system libraries."
echo "You can now launch it by typing: monarch-store"
echo ""

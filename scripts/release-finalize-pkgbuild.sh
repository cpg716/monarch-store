#!/usr/bin/env bash
# Run this AFTER pushing main and tag v0.3.5_alpha to GitHub.
# Switches PKGBUILD to release tarball, runs updpkgsums, regenerates .SRCINFO.
set -e
cd "$(dirname "$0")/.."
pkgname=monarch-store
# 1. Get version from PKGBUILD if not provided
pkgver=${1:-$(grep "^pkgver=" PKGBUILD | cut -d= -f2)}
if [ -z "$pkgver" ]; then
    echo "Error: Could not determine pkgver from PKGBUILD and no argument provided."
    exit 1
fi
echo "Using version: $pkgver"

# 1. Switch PKGBUILD to release tarball and correct cd paths
sed -i "s|^source=(.*github.com/cpg716/monarch-store.*)|source=(\"https://github.com/cpg716/monarch-store/archive/refs/tags/v${pkgver}.tar.gz\")|" PKGBUILD
sed -i 's|^sha256sums=.*|sha256sums=('\''SKIP'\'')|' PKGBUILD
# Replace cd "$pkgname" / cd "$pkgname-$pkgver" / cd "monarch-store-*" with literal dir so tarball dir matches (avoids empty $pkgver in any build context; idempotent for re-runs)
sed -i "s|cd \"\\\$pkgname\"|cd \"$pkgname-$pkgver\"|g" PKGBUILD
sed -i "s|cd \"\\\$pkgname-\\\$pkgver\"|cd \"$pkgname-$pkgver\"|g" PKGBUILD
sed -i "s|cd \"$pkgname-[^\"]*\"|cd \"$pkgname-$pkgver\"|g" PKGBUILD

# 2. Download tarball and fill checksums (requires tag on GitHub)
updpkgsums

# 3. Regenerate .SRCINFO
makepkg --printsrcinfo > .SRCINFO

# 4. Commit and push
git add PKGBUILD .SRCINFO
git commit -m "PKGBUILD: release tarball + checksums for v${pkgver}"
echo "Run: git push origin main"
git push origin main || true

#!/usr/bin/env bash
# Fetch source logos from official or high-quality URLs.
# Run from this directory: ./fetch-source-logos.sh
# Requires: curl

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Don't exit on first failure so we get all that work
fetch() { if curl -sLf -o "$2" "$1"; then echo "  OK $2"; else echo "  SKIP $2 ($1)"; fi; }

echo "Fetching Arch (official dark)..."
fetch "https://archlinux.org/static/logos/archlinux-logo-dark-scalable.svg" arch-official.svg
test -s arch-official.svg || fetch "https://upload.wikimedia.org/wikipedia/commons/f/f9/Archlinux-logo-standard-version.svg" arch-official.svg

echo "Fetching Flatpak (Simple Icons)..."
fetch "https://cdn.jsdelivr.net/npm/simple-icons@v11/icons/flatpak.svg" flatpak.svg

echo "Fetching CachyOS (Commons)..."
fetch "https://upload.wikimedia.org/wikipedia/commons/b/b8/CachyOS_Logo.svg" cachyos.svg

echo "Fetching Manjaro (Commons 3/3e)..."
fetch "https://upload.wikimedia.org/wikipedia/commons/3/3e/Manjaro-logo.svg" manjaro.svg

echo "Fetching EndeavourOS (GitHub Branding icons)..."
fetch "https://raw.githubusercontent.com/endeavouros-team/Branding/main/icons/endeavouros.svg" endeavouros.svg

echo "Fetching Garuda (Commons 8/88)..."
fetch "https://upload.wikimedia.org/wikipedia/commons/8/88/Garuda-blue-sgs.svg" garuda.svg

echo "Fetching AUR (Simple Icons archlinux, commonly used for AUR)..."
fetch "https://cdn.jsdelivr.net/npm/simple-icons@v11/icons/archlinux.svg" aur.svg

echo "Fetching Chaotic-AUR (aur.chaotic.cx favicon, PNG)..."
fetch "https://aur.chaotic.cx/favicon-32x32.png" chaotic-aur.png

echo "Done. AUR uses Arch-style icon (no official AUR logo); Chaotic-AUR uses site favicon (PNG)."

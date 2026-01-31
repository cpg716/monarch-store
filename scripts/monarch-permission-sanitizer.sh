#!/usr/bin/env bash
# MonARCH Store — Permission Sanitizer
# Resets build cache and toolchain state to fix AUR build failures (e.g. "An unknown error has occurred").
# Run as the same user that runs MonARCH Store (never as root).

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "=== MonARCH Permission Sanitizer ==="

# Must not run as root (same as makepkg)
if [ "$(id -u)" -eq 0 ]; then
    echo -e "${RED}Do not run this script as root. Run it as your normal user.${NC}"
    exit 1
fi

# 1. /tmp/monarch-install — shared dir for built packages before root install
if [ -d /tmp/monarch-install ]; then
    echo "Cleaning /tmp/monarch-install ..."
    rm -rf /tmp/monarch-install/*
    # Ensure dir is writable by user (in case a previous build ran as root)
    chmod 1777 /tmp/monarch-install 2>/dev/null || true
    echo -e "${GREEN}  /tmp/monarch-install reset.${NC}"
else
    mkdir -p /tmp/monarch-install
    chmod 1777 /tmp/monarch-install
    echo -e "${GREEN}  /tmp/monarch-install created.${NC}"
fi

# 2. User cache (if used by app)
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/monarch-store"
if [ -d "$CACHE_DIR" ]; then
    echo "Cleaning $CACHE_DIR ..."
    rm -rf "$CACHE_DIR"/*
    echo -e "${GREEN}  Cache cleared.${NC}"
else
    mkdir -p "$CACHE_DIR"
    echo -e "${GREEN}  Cache dir created.${NC}"
fi

# 3. Stale temp command files (GUI uses /var/tmp so root can read; also clean legacy /tmp)
echo "Removing stale monarch-cmd-*.json in /var/tmp and /tmp ..."
rm -f /var/tmp/monarch-cmd-*.json /tmp/monarch-cmd-*.json 2>/dev/null || true
echo -e "${GREEN}  Stale command files removed.${NC}"

# 4. Toolchain presence
echo ""
echo "Checking AUR build toolchain..."
MISSING=""
command -v makepkg >/dev/null 2>&1 || MISSING="$MISSING makepkg"
command -v git >/dev/null 2>&1 || MISSING="$MISSING git"
command -v fakeroot >/dev/null 2>&1 || MISSING="$MISSING fakeroot"
command -v strip >/dev/null 2>&1 || MISSING="$MISSING strip (binutils)"

if [ -n "$MISSING" ]; then
    echo -e "${YELLOW}  Missing:$MISSING${NC}"
    echo "  Install base-devel and git:"
    echo "    sudo pacman -S --needed base-devel git"
else
    echo -e "${GREEN}  makepkg, git, fakeroot, strip found.${NC}"
fi

echo ""
echo -e "${GREEN}Permission sanitizer finished.${NC}"
echo "If AUR builds still fail with 'unknown error', ensure base-devel is fully installed:"
echo "  sudo pacman -S --needed base-devel git"

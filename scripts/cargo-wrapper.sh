#!/usr/bin/env bash
# Run cargo by invoking the active toolchain's binary directly.
# Use this when the IDE (e.g. Cursor) breaks rustup by setting argv[0] to something
# other than "cargo", causing: "unknown proxy name: 'Cursor-2.4.28-...'"
#
# Usage: ./scripts/cargo-wrapper.sh [cargo args...]
# Example: ./scripts/cargo-wrapper.sh check -p monarch-gtk
set -e
RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
if [ -f "$RUSTUP_HOME/settings.toml" ]; then
  DEFAULT=$(grep '^default_toolchain' "$RUSTUP_HOME/settings.toml" 2>/dev/null | head -1 | sed 's/.*= *"\([^"]*\)".*/\1/')
fi
TOOLCHAIN="${DEFAULT:-stable-x86_64-unknown-linux-gnu}"
CARGO_BIN="$RUSTUP_HOME/toolchains/$TOOLCHAIN/bin/cargo"
if [ ! -x "$CARGO_BIN" ]; then
  echo "cargo-wrapper: $CARGO_BIN not found. Is the $TOOLCHAIN toolchain installed?" >&2
  exit 1
fi
exec "$CARGO_BIN" "$@"

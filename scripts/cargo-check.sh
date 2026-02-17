#!/usr/bin/env bash
# Run cargo check from repo root. Uses a clean env so Cursor (and other IDEs)
# don't break rustup (CARGO/RUSTC or invoker name "Cursor-2.4.28-...").
set -e
SRC_TAURI="$(dirname "$0")/../src-tauri"
PATH="${HOME}/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
cd "$SRC_TAURI"
exec env -i HOME="$HOME" PATH="$PATH" \
  RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" \
  CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" \
  cargo check -p monarch-store

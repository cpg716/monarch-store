# AGENTS.md - MonARCH Store

**Last updated:** 2026-03-09

For architectural invariants and forbidden patterns, see [.cursorrules](/home/chris/Downloads/monarch-store/.cursorrules).

## Build commands

### Primary GTK workflow

Run all Rust commands from [src-tauri](/home/chris/Downloads/monarch-store/src-tauri):

- `cd src-tauri && cargo check`
- `cd src-tauri && cargo test -p monarch-core`
- `cd src-tauri && cargo run -p monarch-gtk`

These are the current day-to-day commands for the active product.

### Legacy/reference workflow

The Tauri/React frontend is legacy/reference-only. Use Tauri commands only when comparing historical behavior or maintaining legacy code:

- `npm run tauri dev`
- `npm run dev`
- `npm run build`
- `npm run tauri build`

## Architecture

- **Active frontend:** `src-tauri/monarch-gtk/`
- **Backend source of truth:** `src-tauri/monarch-core/`
- **Privileged helper:** `src-tauri/monarch-helper/`
- **Legacy frontend:** `src/` and Tauri/React integration code

### Iron Core contract

Iron Core is the canonical package and metadata pipeline:
- canonical package identity
- merged stable source availability
- discovery/search/category payloads
- detail payloads and source-specific facts
- settings state used by GTK

GTK is a dumb UI over Iron Core:
- no GTK-side metadata parsing
- no GTK-side source merge logic
- no GTK-side category guessing

**UI reference:** Package detail (hero, data bar, actions) follows [Bazaar](https://github.com/kolunmi/bazaar); docs and styling should note Bazaar where relevant.

## Repo behavior

- Repositories are discovered from the host system
- Source toggles limit discovery/search visibility
- Installed apps and updates remain truthful even when discovery sources are hidden

## Critical package rules

- Never run `pacman -Sy` by itself
- Only `monarch-helper` performs ALPM writes
- AUR builds remain unprivileged and are installed via the helper
- Respect host `IgnorePkg` / `IgnoreGroup`
- Do not move package truth into GTK

## Validation

Current default validation:

```bash
cd src-tauri && cargo check
cd src-tauri && cargo test -p monarch-core
cd src-tauri && cargo run -p monarch-gtk
```

### Rustup proxy error in Cursor

If you see `unknown proxy name: 'Cursor-2.4.28-...'` when running `cargo` from the IDE, Cursor is spawning the process with a wrong executable name so rustup rejects it. Use the wrapper so the toolchain’s `cargo` is run directly:

```bash
chmod +x scripts/cargo-wrapper.sh
cd src-tauri && ../scripts/cargo-wrapper.sh check
../scripts/cargo-wrapper.sh run -p monarch-gtk
```

Or run `cargo` from a normal terminal (outside the IDE) where the proxy name is correct.

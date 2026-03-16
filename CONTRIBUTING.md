# Contributing to MonARCH Store

**Current frontend:** GTK  
**Last updated:** 2026-03-09

MonARCH Store is now a **GTK-first** product built on:
- `monarch-core` for canonical package identity, metadata hydration, and discovery/search/detail payloads
- `monarch-helper` for privileged ALPM writes
- `monarch-gtk` as the active frontend

The older Tauri/React code remains in the repo as historical/reference material while GTK reaches full parity.

## Before you contribute

Read these first:
- [README.md](/home/chris/Downloads/monarch-store/README.md)
- [ARCHITECTURE.md](/home/chris/Downloads/monarch-store/ARCHITECTURE.md)
- [docs/DEVELOPER.md](/home/chris/Downloads/monarch-store/docs/DEVELOPER.md)
- [docs/GTK_TAURI_PARITY_MATRIX.md](/home/chris/Downloads/monarch-store/docs/GTK_TAURI_PARITY_MATRIX.md)
- [AGENTS.md](/home/chris/Downloads/monarch-store/AGENTS.md)
- [.cursorrules](/home/chris/Downloads/monarch-store/.cursorrules)

## Contribution priorities

Priority order:
1. Preserve Iron Core as the single source of truth
2. Keep Arch-safe package behavior intact
3. Close GTK parity gaps against the documented MonARCH product contract
4. Keep docs aligned with the actual shipped GTK product

## Critical rules

### Iron Core rules
- Do not move metadata parsing or source-merge logic into GTK
- Do not create GTK-side category inference, icon guessing, or source-truth logic
- Cards and details must render backend-provided models

### Package-management safety
- Never run `pacman -Sy` by itself
- Only `monarch-helper` may perform ALPM writes
- AUR builds stay unprivileged and are installed via the helper
- Respect host `IgnorePkg` and `IgnoreGroup`

### Product truth
- GTK is the current product
- Tauri/React is legacy/reference-only unless a doc explicitly says otherwise
- Public docs must describe current GTK truth, not aspirational parity

## Development workflow

Primary GTK workflow:

```bash
cd src-tauri && cargo check
cd src-tauri && cargo test -p monarch-core
cd src-tauri && cargo run -p monarch-gtk
```

Use Tauri only for historical/reference comparison work.

## Pull requests

Include:
- a clear problem statement
- the product surface affected
- screenshots when UI changes are involved
- test evidence (`cargo check`, `cargo test -p monarch-core`, and GTK runtime notes)
- docs updates when behavior changes

If you touch discovery, search, categories, source switching, icons, or canonical identity, update the parity matrix if the status changed.

## Style

### Rust
- `cargo fmt`
- `cargo clippy`
- prefer explicit, stable, backend-owned package models

### GTK
- keep the frontend dumb
- prefer consistent shared card/detail composition
- do not add widget-local truth that competes with backend data
- package detail layout (hero, data bar, actions) uses [Bazaar](https://github.com/kolunmi/bazaar) as UI reference; preserve that alignment when changing the detail page

### Documentation
- write GTK-first docs
- label Tauri material as legacy/reference
- avoid stale feature claims

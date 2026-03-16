# MonARCH Store Developer Guide

**Current frontend:** GTK  
**Last updated:** 2026-03-09

This is the active engineering guide for MonARCH Store.

GTK is the current frontend. Tauri/React remains in the repository only as legacy/reference implementation material while parity work is completed.

## 1. Product architecture

MonARCH has three active layers:

### `monarch-core`
The backend source of truth.

Responsibilities:
- canonical package identity
- metadata hydration
- source merging
- search/discovery/category payloads
- settings state used by GTK
- detail payloads and source-specific variants

Important types:
- `Package`
- `PackagePresentation`
- `PackageVariant`
- `FullPackageDetails`
- `SearchOptions`
- `HomeSnapshot`
- `GtkSettings`

### `monarch-helper`
The only privileged writer.

Responsibilities:
- ALPM write transactions
- safe update/install/remove flows
- lock handling
- privileged maintenance operations

### `monarch-gtk`
The active frontend.

Responsibilities:
- render Iron Core payloads
- provide GTK navigation and interaction surfaces
- never re-implement metadata hydration or source truth

**UI/design reference:** Package detail layout (hero, data bar, action row) is aligned with [Bazaar](https://github.com/kolunmi/bazaar). When changing that UI, keep Bazaar in mind for consistency; MonARCH extends it with source selection and multi-source behaviour.

## 2. Project structure

```text
src-tauri/
  monarch-core/   backend truth and package/catalog logic
  monarch-helper/ privileged helper
  monarch-gtk/    active GTK frontend

src/              legacy/reference Tauri frontend
```

Treat `src/` as legacy/reference unless you are doing parity comparison or archival maintenance.

## 3. Core engineering rules

### Iron Core contract
- GTK must consume backend package models directly
- no GTK-side metadata parsing
- no GTK-side source merge logic
- no GTK-side taxonomy guessing
- no duplicate canonical identity logic in the frontend

### Canonical identity
Every stable app should appear once across:
- Home
- Search
- Categories
- Installed
- Updates
- Details

Channel builds remain distinct.

### Helper-only writes
Only `monarch-helper` may write to `/var/lib/pacman` or perform ALPM transactions.

### AUR build split
AUR builds remain unprivileged and are installed through the helper afterward.

## 4. Current development commands

Run from [src-tauri](/home/chris/Downloads/monarch-store/src-tauri):

```bash
cargo check
cargo test -p monarch-core
cargo run -p monarch-gtk
```

Use these as the default GTK workflow.

Tauri commands are legacy/reference-only and should not be treated as the primary validation path for the current product.

## 5. GTK parity expectations

GTK is expected to match the documented MonARCH product contract in these areas:
- backend-fed Home rails
- backend-fed category results
- canonical search results with merged stable sources
- compact Flathub-style cards outside search
- screenshot-style cards in search
- details as the single source/action surface
- settings, onboarding, updates, and news reflecting current backend capabilities

Track open gaps in [docs/GTK_TAURI_PARITY_MATRIX.md](/home/chris/Downloads/monarch-store/docs/GTK_TAURI_PARITY_MATRIX.md).

## 6. Documentation policy

Docs must follow current truth:
- GTK is primary
- Tauri is legacy/reference-only unless explicitly labeled archival
- public docs should not describe stale Tauri behavior as current

When changing product behavior:
- update the relevant user/developer docs
- update the parity matrix if status changed

## 7. Related docs

- [ARCHITECTURE.md](/home/chris/Downloads/monarch-store/ARCHITECTURE.md)
- [docs/PACKAGING_AND_METADATA_FLOW.md](/home/chris/Downloads/monarch-store/docs/PACKAGING_AND_METADATA_FLOW.md)
- [docs/UNIVERSAL_DATA_ENGINE.md](/home/chris/Downloads/monarch-store/docs/UNIVERSAL_DATA_ENGINE.md)
- [docs/GTK_TAURI_PARITY_MATRIX.md](/home/chris/Downloads/monarch-store/docs/GTK_TAURI_PARITY_MATRIX.md)
- [TESTING.md](/home/chris/Downloads/monarch-store/TESTING.md)

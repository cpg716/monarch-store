# MonARCH Store

MonARCH Store is a GTK-only software manager for Arch Linux and Arch-based distributions. The current product is built around **Iron Core**, a Rust backend that discovers host repositories, hydrates package metadata into canonical package identities, and serves a dumb GTK UI with unified cards, details, search, installed, updates, onboarding, and settings surfaces.

Status:
- Frontend of record: `monarch-gtk`
- Backend of record: `monarch-core` + `monarch-helper`
- Legacy frontend: Tauri/React, no longer part of the active workspace (kept only in history/legacy code paths if present)

## What MonARCH does

- Respects the host system instead of owning `/etc/pacman.conf`
- Aggregates Arch/distro-native repos, Chaotic-AUR, Flatpak, and AUR under one canonical package model
- Keeps Arch safety rules intact: no partial upgrades, helper-only ALPM writes, user-space AUR builds
- Uses one canonical listing for stable multi-source apps, with source switching handled by Iron Core
- Presents a GTK storefront intended to be welcoming to new users without hiding source and package-management truth from experienced Arch users

## Architecture

- `src-tauri/monarch-core/`
  The product brain. Registry, metadata hydration, canonical identity, search, home snapshot, categories, installed, updates, reviews, and source-aware details all live here.
- `src-tauri/monarch-gtk/`
  The current frontend. GTK renders the hydrated backend payloads and should not perform metadata parsing or merge logic on its own.
- `src-tauri/monarch-helper/`
  Privileged helper for ALPM write operations.
- `src/`
  Legacy Tauri/React frontend. Reference-only until removed.

## Development

GTK is the primary development target now.

```bash
cd src-tauri
cargo run -p monarch-gtk
```

Useful commands:

```bash
cd src-tauri && cargo check
cd src-tauri && cargo test -p monarch-core
```

The old Tauri workflow still exists in the repo for legacy comparison, but public docs should not treat it as the current product path.

## Core product rules

- Iron Core is the single source of truth for package metadata and source availability
- GTK is a dumb UI over hydrated backend models
- `monarch-helper` is the only process allowed to write ALPM state
- AUR builds happen unprivileged, then built artifacts are handed to the helper
- Search, Home, Categories, Installed, Updates, and Details must all agree on canonical package identity

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [USER_GUIDE.md](USER_GUIDE.md)
- [TESTING.md](TESTING.md)
- [CONTRIBUTING.md](CONTRIBUTING.md)
- [docs/DEVELOPER.md](docs/DEVELOPER.md)
- [docs/GTK_TAURI_PARITY_MATRIX.md](docs/GTK_TAURI_PARITY_MATRIX.md)
- [docs/RECENT_CHANGES.md](docs/RECENT_CHANGES.md)

## Current direction

The current effort is twofold:
- close GTK parity gaps where the Tauri frontend historically exposed richer behavior
- rewrite the repo docs so they describe GTK as current truth and Tauri as legacy/reference only

## Acknowledgements

- **Bazaar**: The GTK package detail UI (hero, data bar, action row, and overall layout) is aligned with [Bazaar](https://github.com/kolunmi/bazaar), the GNOME/Flathub app store. We use Bazaar as a visual and UX reference for the storefront while adding distro-aware multi-source behaviour (source selector, repo/AUR/Flatpak).
- **Modern GTK Dropdown UI**: The implementation of the inline source dropdown in the package detail view was inspired by the tutorial ["Define UI comprising Dropdown in Blueprint for modern GTK apps"](https://quan.hoabinh.vn/post/2025/11/define-ui-comprising-dropdown-in-blueprint-for-modern-gtk-apps).

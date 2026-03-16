# MonARCH Store Testing Guide

**Current frontend:** GTK  
**Last updated:** 2026-03-09

This guide describes the current validation path for MonARCH Store as a GTK-first product over Iron Core.

## Primary commands

Run all Rust commands from [src-tauri](/home/chris/Downloads/monarch-store/src-tauri).

```bash
cd src-tauri && cargo check
cd src-tauri && cargo test -p monarch-core
cd src-tauri && cargo run -p monarch-gtk
```

Use these as the default validation sequence:
1. `cargo check`
2. `cargo test -p monarch-core`
3. `cargo run -p monarch-gtk`

## What must be true

### Iron Core contract
- GTK cards and details render backend-hydrated package models only
- Search, Home, Categories, Installed, Updates, and Details all resolve through the same canonical package identity
- Stable multi-source apps appear once, with merged source availability
- Channel builds remain separate identities

### Home and discovery parity
- Essentials, Trending, Featured, and Categories are backend-fed
- Home does not drift toward installed-only results
- Category buttons open correct category result sets from backend taxonomy
- Discovery obeys source toggles and `show_system_apps`

### Search parity
- Search returns one canonical app listing per stable identity
- Installed bias applies only to relevant search ranking, not generic discovery
- Disabled sources are hidden from discovery/search but still remain valid in Installed/Updates

### Detail parity
- Details is the only action surface for install/open/remove/update state
- Source selection updates visible payload and action state
- Source-specific metadata reloads correctly
- Installed-source truth is preserved

### Metadata and icon fidelity
- Popular apps use real full-color app icons where trusted metadata exists
- GTK does not prefer symbolic or monochrome theme placeholders over richer icons
- Fallback presentation remains intentional when metadata is incomplete

## Automated backend checks

`cargo test -p monarch-core` is the current backend release gate.

The backend test suite should cover:
- canonical merge and variant grouping
- category/storefront query behavior
- source-priority ordering
- source-toggle visibility rules
- icon selection precedence
- detail payload selection by source identity

## Manual GTK checks

Run:

```bash
cd src-tauri && cargo run -p monarch-gtk
```

Then validate:

### Home
- Featured, Essentials, Trending, and Categories show real user-facing apps
- cards use compact Flathub-style composition
- category buttons open populated category result pages

### Search
- blank search state is useful and backend-driven
- typed search uses screenshot-style cards only in search
- source badges are ordered and consistent

### Details
- install/remove/open state is correct
- source selection visibly reloads details
- screenshots and long description appear when backend data exists

### Library and Updates
- installed apps remain visible even if discovery source toggles are off
- updates remain visible for installed apps across sources

### Settings, Onboarding, News
- source toggles persist
- `show_system_apps` changes discovery/search scope only
- onboarding and settings copy reflect GTK product behavior
- news/update/advisory surfaces remain functional

## Legacy Tauri note

`npm run tauri dev` is not the primary product validation path anymore. Use it only when comparing legacy behavior or checking historical/reference code paths.

GTK is the current frontend under test.

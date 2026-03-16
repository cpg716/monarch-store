# Universal Data Engine

**Last updated:** 2026-03-09  
**Scope:** Canonical identity, deterministic source ordering, deduplication, and stable display models for the current Iron Core + GTK product.

---

## 1. Overview

The **Universal Data Engine** is primarily a backend system in Iron Core that ensures:

- **One card per app:** Search, Trending, Categories, and Essentials never show duplicate cards for the same logical app (e.g. "Heroic" from search and "Heroic Game Launcher" from a category merge into a single card).
- **One proper name per app:** Each app is shown with a consistent, human-friendly display name (e.g. "Heroic Game Launcher", "OBS Studio") instead of raw package names or short aliases.

**Key modules:** `src-tauri/monarch-core/src/catalog.rs`, `src-tauri/monarch-core/src/bootstrap.rs`, `src-tauri/monarch-core/src/registry.rs`, and the canonical package models consumed by `monarch-gtk`.

---

## 2. Canonical Merge Key (Channel-Aware)

**Function:** `utils::canonical_merge_key(name, app_id)`

Canonical identity is generated with strict, deterministic rules:
- App-id mapping takes precedence when a trusted mapping exists.
- Packaging suffixes are stripped (`-bin`, `-git`, `-appimage`, `-desktop`, `.desktop`, `-hg`, `-svn`, `-official`, `-repo`, `-stable`).
- Channel suffixes are preserved as distinct products (`-canary`, `-beta`, `-nightly`, `-ptb`, `-dev`, `-insider`, `-esr`, `-developer-edition`).
- No generic first-segment collapse is used in the current engine.

**Rules:**

1. **App ID path:** resolve trusted app-id aliases first, then normalize.
2. **Name path:** remove packaging suffixes only; keep channel identity intact.
3. **Normalization:** output is lowercase alphanumeric key for cross-layer parity.

**Examples:**

| Input (name / app_id)              | Canonical key   |
|------------------------------------|-----------------|
| `google-chrome`, None | `googlechrome` |
| `google-chrome-canary`, None | `googlechromecanary` |
| `discord`, None | `discord` |
| `discord-ptb`, None | `discordptb` |
| `heroic-games-launcher-bin`, None | `heroicgameslauncher` |
| Any, `org.mozilla.firefox` | `firefox` |

**Files:** `utils.rs` (`canonical_merge_key`, `canonical_search_base`, `known_app_id_to_canonical`).

---

## 3. Deduplication and Display Names

**Function:** `utils::deduplicate_by_canonical_key(packages)` + aggregation V2 builder

- Groups packages by `canonical_merge_key(name, app_id)`.
- Merges `available_sources`; when merging, prefers the **longer** `display_name` so "Heroic Game Launcher" wins over "Heroic".
- After merging, source ordering is deterministic:
  - `distro-native repo > Arch Official > Chaotic-AUR > Flatpak > AUR`.
- Frontend should never infer source/install state; it renders backend-selected source/defaults.

**Installed-source truth:** installed labels are derived from ALPM/localdb + syncdb matching and exact Flatpak installed IDs, not from fuzzy metadata heuristics.

**Files:** `utils.rs`, `middleware/aggregation.rs`, `alpm_read.rs`.

---

## 4. Where Deduplication Runs

- **build_package_view_models_v2** — shared canonical builder used by search/discovery/category/details seed paths.
- **get_packages_by_names** — uses canonical dedup after backend merge.
- **get_trending** — after merging AUR + Flatpak + **official repo packages** (from metadata loader by category).
- **get_category_packages_paginated** — before featured/injected merge so appstream + featured collapse to one card per app.

**Files:** `middleware/aggregation.rs`, `commands/search/core.rs`, `commands/search/discovery.rs`, `commands/search/categories.rs`.

---

## 5. Frontend contract (Iron Core)

- **List keys:** one canonical app identity should map to one frontend record key.
- **GTK contract:** `monarch-gtk` renders backend-provided package records and must not infer source/install/category truth locally.
- **Dumb View philosophy:** frontend-side source/install inference is forbidden.

Legacy Tauri frontend files are historical/reference-only and are no longer the active frontend contract.

---

## 6. Related Fixes & The Iron Core Purge (2026-02-14)

- **Iron Core Purge:** Removed legacy hooks (`usePrewarmCards`, `usePackageMetadata`, `useRatings`). Eliminated per-card "hydration" calls.
- **Specta Enforcement:** components now import `Package` directly from `services/bindings.ts`. The loose `interface Package` definition in `PackageCard.tsx` was deleted to prevent interface drift.
- **Logic Offloading:** Regex parsing for package sizes and versions moved from `InstalledPage`/`UpdatesPage` to the backend.
- **Icon/base64 400 errors:** Backend middleware now sanitizes and wraps base64 icons, preventing browser 400 errors.
- **Channel-split correctness:** Stable/canary/beta/nightly products remain distinct identities while cross-source variants of the same identity merge into one card.

---

## 7. Document Cross-References

| Topic                    | Doc |
|--------------------------|-----|
| Unification & dropdown   | `docs/UNIFICATION_AND_DROPDOWN_REVIEW.md` |
| Duplicate cards fix      | `docs/DUPLICATE_CARDS_FIX_UPDATE.md` |
| Data flow & metadata     | `docs/PACKAGING_AND_METADATA_FLOW.md` |
| Architecture             | `ARCHITECTURE.md` |
| Recent changes           | `docs/RECENT_CHANGES.md` |

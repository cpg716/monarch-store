# Universal Data Engine

**Last updated:** 2026-02-08  
**Scope:** Canonical merge key, deduplication, and display names (one app = one card, one proper name).

---

## 1. Overview

The **Universal Data Engine** is the backend + frontend logic that ensures:

- **One card per app:** Search, Trending, Categories, and Essentials never show duplicate cards for the same logical app (e.g. "Heroic" from search and "Heroic Game Launcher" from a category merge into a single card).
- **One proper name per app:** Each app is shown with a consistent, human-friendly display name (e.g. "Heroic Game Launcher", "OBS Studio") instead of raw package names or short aliases.

**Key modules:** `middleware/aggregation.rs` (merge/dedup logic), `src-tauri/monarch-gui/src/utils.rs` (`canonical_merge_key`, `deduplicate_by_canonical_key`, `to_pretty_name`, `preferred_display_name`), `commands/search.rs` (entry point), frontend `internal_store.ts` (merge/eviction), `packageKey.ts`, `iconHelper.ts`.

---

## 2. Canonical Merge Key (No Per-App List)

**Function:** `utils::canonical_merge_key(name, app_id)`

We do **not** maintain a per-app alias map for merge keys. Instead we use a **generic first-segment rule** so that:

- Multi-segment package names (e.g. `heroic-games-launcher-bin`, `obs-studio`) share a key with the short form (`heroic`, `obs`) by using the **first segment** of the name (after suffix stripping) as the canonical key when it is a valid "brand" (length ≥ 3, not a library prefix like `lib`, `org`, `com`).
- App IDs (e.g. `com.heroicgameslauncher.hgl`) are mapped via `known_app_id_to_canonical`; that result is then run through the same first-segment rule so Flatpak "Heroic Game Launcher" and AUR "heroic" get the same key.

**Rules:**

1. **App ID path:** If `app_id` is set and looks like reverse-DNS, use `known_app_id_to_canonical`; if the result has multiple segments (e.g. `heroic-games-launcher`), use `first_segment_canonical` → e.g. `heroic`. Otherwise normalize (strip `-`/`_`) and return.
2. **Name path:** Strip variant suffixes (`-bin`, `-git`, `-flatpak`, etc.), then if the name has multiple segments (contains `-` or `_`), use the first segment as key when valid (length ≥ 3, not in `FIRST_SEGMENT_SKIP`). Otherwise use the full normalized name.

**Examples:**

| Input (name / app_id)              | Canonical key   |
|------------------------------------|-----------------|
| `heroic`, None                     | `heroic`        |
| `heroic-games-launcher-bin`, None | `heroic`        |
| Any, `com.heroicgameslauncher.hgl`| `heroic`        |
| `obs-studio`, None                 | `obs`           |
| Any, `com.obsproject.Studio`       | `obs`           |
| `firefox-developer-edition`, None | `firefox`       |
| `visual-studio-code-bin`, None    | `visual`        |

**Files:** `utils.rs` (`first_segment_canonical`, `canonical_merge_key`, `FIRST_SEGMENT_SKIP`).

---

## 3. Deduplication and Display Names

**Function:** `utils::deduplicate_by_canonical_key(packages)`

- Groups packages by `canonical_merge_key(name, app_id)`.
- Merges `available_sources`; when merging, prefers the **longer** `display_name` so "Heroic Game Launcher" wins over "Heroic".
- After building the merged list, **every package gets a proper display name:**
  - If the package’s `canonical_id` has a **preferred display name** (see below), use it so the app always shows the same full name.
  - Else if `display_name` is missing and the package name is not app_id-style (no `'.'`), set `display_name = to_pretty_name(name)` (e.g. `heroic-games-launcher-bin` → "Heroic Games Launcher").

**Preferred display names (display-only map):**

So that one app always shows one name (e.g. "Heroic Game Launcher" not sometimes "Heroic"), we keep a small **display-only** map in `utils::preferred_display_name(canonical_key)`:

| Canonical key | Display name           |
|---------------|------------------------|
| `heroic`      | Heroic Game Launcher   |
| `obs`         | OBS Studio             |
| `visual`      | Visual Studio Code     |

This is **not** used for merge logic—only for ensuring a single, consistent label in the UI. New apps can be added here when we want a fixed full name.

**Pretty name fallback:** `utils::to_pretty_name(pkg_name)` turns package names into title-case labels (e.g. `obs-studio` → "OBS Studio", hyphen/underscore segments capitalized; special cases for CLI, GUI, API, etc.).

**Files:** `utils.rs` (`deduplicate_by_canonical_key`, `preferred_display_name`, `to_pretty_name`).

---

## 4. Where Deduplication Runs

- **get_packages_by_names** — after `merge_search_results`, before any featured/injected merge.
- **get_trending** — after merging AUR + Flatpak + **official repo packages** (from metadata loader by category).
- **get_category_packages_paginated** — before featured/injected merge so appstream + featured collapse to one card per app.

**Files:** `middleware/aggregation.rs`, `commands/search.rs`.

---

## 5. Frontend: List Keys and Registry (Iron Core)

- **List keys:** Use `pkg.canonical_id || (pkg.app_id || pkg.name)` (or equivalent) so one app = one React key. List keys are stable and derived directly from the backend's `Package` struct.
- **Registry:** `internal_store.ts` serves as a simple KV-Store. The `upsertPackages` function performs a simple overwrite of entries, trusting that the backend sends fully-hydrated and enriched ViewModels.
- **Dumb View Philosophy:** All frontend-side hydration logic, regex parsing for sizes, and "icon guessing" have been removed. Components strictly render what is provided in the `Package` object.

**Files:** `src/store/internal_store.ts`, `src/utils/packageKey.ts`, `src/services/bindings.ts`.

---

## 6. Related Fixes & The Iron Core Purge (2026-02-14)

- **Iron Core Purge:** Removed legacy hooks (`usePrewarmCards`, `usePackageMetadata`, `useRatings`). Eliminated per-card "hydration" calls.
- **Specta Enforcement:** components now import `Package` directly from `services/bindings.ts`. The loose `interface Package` definition in `PackageCard.tsx` was deleted to prevent interface drift.
- **Logic Offloading:** Regex parsing for package sizes and versions moved from `InstalledPage`/`UpdatesPage` to the backend.
- **Icon/base64 400 errors:** Backend middleware now sanitizes and wraps base64 icons, preventing browser 400 errors.
- **Heroic (and similar) as one app, one name:** First-segment canonical key + preferred display name ensure one card and one label ("Heroic Game Launcher") everywhere.

---

## 7. Document Cross-References

| Topic                    | Doc |
|--------------------------|-----|
| Unification & dropdown   | `docs/UNIFICATION_AND_DROPDOWN_REVIEW.md` |
| Duplicate cards fix      | `docs/DUPLICATE_CARDS_FIX_UPDATE.md` |
| Data flow & metadata     | `docs/PACKAGING_AND_METADATA_FLOW.md` |
| Architecture             | `ARCHITECTURE.md` |
| Recent changes           | `docs/RECENT_CHANGES.md` |

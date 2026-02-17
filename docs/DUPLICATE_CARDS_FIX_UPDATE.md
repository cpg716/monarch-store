# Fix: Duplicate App Cards (e.g. Discord) & Missing Source Dropdown

**Issue:** Apps like Discord showed two cards in browse/trending/category; opening one of them did not show the source dropdown (RepoSelector).

**Update (2026-02-08):** Canonical key now uses a **first-segment rule** (no per-app alias list); display names use `preferred_display_name(canonical_id)` or `to_pretty_name(name)` so one app has one proper name everywhere. See **`docs/UNIVERSAL_DATA_ENGINE.md`** for the full Universal Data Engine spec (canonical key, dedup, display names, and related fixes).

---

**Root causes:**
1. Same app could appear from different pipelines (appstream `com.discordapp.Discord` + featured inject `Discord` from AUR/Flatpak) and not be merged.
2. List keys used `pkg.name` + `pkg.source`; `pkg.source` is an object and stringified to `[object Object]`, so duplicate entries had different keys and both rendered.
3. One of the two cards had no `available_sources`, so the details page did not show the dropdown until variants loaded (or at all if variants were not merged correctly).

---

## Changes Made

### 1. Backend: Canonical-key deduplication (`src-tauri/monarch-gui/`)

#### **New function: `utils::deduplicate_by_canonical_key`**  
**File:** `src-tauri/monarch-gui/src/utils.rs`

- **What it does:** Groups packages by `canonical_merge_key(name, app_id)` (e.g. `Discord`, `com.discordapp.Discord`, and AUR `discord` all map to `"discord"`). Keeps one package per key and:
  - Merges `available_sources` from all duplicates into the kept package.
  - Prefers keeping the package that has `available_sources` (the unified card from search/featured) so the UI shows a friendly name (e.g. "Discord") and the source dropdown.
  - When the kept package has an app_id-style name (e.g. `com.discordapp.Discord`) and the duplicate has a friendly name (e.g. `Discord`) with sources, replaces the kept package’s name/display_name with the friendly one so the single card shows "Discord" and has multiple sources.

- **Why:** Ensures the same logical app never appears twice in any list, and the remaining card is the one with multiple sources and a good display name.

#### **Where it’s used**

- **`get_packages_by_names`** (`commands/search.rs` calls `middleware/aggregation.rs`)  
  - After `merge_search_results(packages, vec![], flatpak_hits)` and before `merge_and_deduplicate`.  
  - So batch results (e.g. category featured, essentials) never contain two entries for the same app.

- **`get_trending`** (`commands/search.rs`)  
  - After `merge_search_results(Vec::new(), aur_packages, flathub_search_results)`.  
  - So trending never shows the same app twice (e.g. Discord from AUR popular + Flathub popular).

- **`get_category_packages_paginated`** (`commands/search.rs`)  
  - **Before** the existing featured/injected merge step.  
  - So appstream entries (e.g. `com.discordapp.Discord`) and featured inject (e.g. `Discord` from `get_packages_by_names`) are collapsed into one card per app before the rest of the pipeline runs.

### 2. Frontend: Stable list keys

**Problem:** Keys like `` key={`${pkg.name}-${pkg.source}`} `` made React see two different keys for the same app (`Discord-[object Object]` vs `discord-[object Object]` or similar), so both cards rendered.

**Fix:** Use a single stable identifier per app so one app = one key.

#### **Files updated**

- **`src/components/TrendingSection.tsx`**
  - Scroll row: `key={(pkg.app_id || pkg.name).toLowerCase()}` (was `` key={`${pkg.name}-${pkg.source}`} ``).
  - Grid: same key change for `PackageCard`.

- **`src/pages/SearchPage.tsx`**
  - Search results list: `key={(pkg.app_id || pkg.name).toLowerCase()}` (was `` key={`${pkg.name}-${pkg.source}`} ``).

**Result:** Same app (same `app_id` or same name) always gets the same key; duplicate entries no longer appear as two separate cards even if they briefly slip through the backend.

### 3. Details page / dropdown behavior

**No code change required** for the dropdown logic itself.

- The details page already shows the RepoSelector when:
  - `variants.length > 1`, or
  - `pkg.available_sources && pkg.available_sources.length > 1`.
- After the backend fix there is only **one** card per app, and that card is the unified one (with `available_sources` merged). So when you open it:
  - Either `pkg.available_sources` has multiple entries, or
  - `get_package_variants(pkg.name)` returns multiple variants (backend already resolves app_id-style names like `com.discordapp.Discord` to canonical `discord`).
- So the dropdown now shows for that single card.

---

## Summary table

| Area | File(s) | Change |
|------|--------|--------|
| Backend dedupe | `monarch-gui/src/utils.rs` | New `deduplicate_by_canonical_key()`; merges by `canonical_merge_key`, merges `available_sources`, prefers friendly name when the other entry has sources. |
| get_packages_by_names | `monarch-gui/src/commands/search.rs` | Call `deduplicate_by_canonical_key` after `merge_search_results`, before `merge_and_deduplicate`. |
| get_trending | `monarch-gui/src/commands/search.rs` | Call `deduplicate_by_canonical_key` after `merge_search_results`. |
| get_category_packages_paginated | `monarch-gui/src/commands/search.rs` | Call `deduplicate_by_canonical_key` before the featured/injected merge step. |
| List keys (trending) | `src/components/TrendingSection.tsx` | Use `(pkg.app_id \|\| pkg.name).toLowerCase()` for scroll and grid keys. |
| List keys (search) | `src/pages/SearchPage.tsx` | Use `(pkg.app_id \|\| pkg.name).toLowerCase()` for result list key. |
| Details dropdown | (unchanged) | Already correct; single merged card guarantees multiple sources/variants. |

---

## Existing pieces that stayed as-is

- **`merge_search_results`** in `middleware/aggregation.rs` (called by `search.rs`): still does the main Official + Flatpak + AUR merge by `canonical_merge_key`; we only added a **second** pass with `deduplicate_by_canonical_key` so lists that combine appstream + featured inject (or multiple code paths) are also collapsed.
- **`canonical_merge_key`** in `utils.rs`: unchanged; still derives one key per app (e.g. last segment of app_id, or name with variant suffixes stripped).
- **`get_package_variants`**: unchanged; already uses `canonical_base` so opening a card with name `com.discordapp.Discord` still finds AUR `discord` and Flatpak and returns multiple variants.

---

## How to verify

1. Run `npm run tauri dev`.
2. **Trending / Home:** Check that Discord (and similar apps) appear once, not twice.
3. **Category (e.g. Games / Internet):** Same — one card per app.
4. **Search "discord":** One result card.
5. **Open that card:** Source dropdown (RepoSelector) is visible and lists all sources (e.g. AUR, Flatpak).

If you still see two cards for one app, say which screen (Home, Category name, Search) and we can narrow it to a specific code path.

---

## Final Fix (Master Architect / Package Unification)

Additional changes to lock in one card per app and a reliable source dropdown:

### 1. Core key logic (`utils.rs`)

- **`canonical_merge_key`** was replaced with the spec logic:
  - **1.** If `app_id` exists and contains `.` (RDN), return the **last segment** lowercased (e.g. `com.discordapp.Discord` → `discord`).
  - **2.** Else use package name with **aggressive suffix stripping** in a loop until stable. Suffixes include: `-bin`, `-git`, `-flatpak`, `-official`, `-repo`, `-beta`, `-nightly`, `-stable`, `-appimage`, `-electron`, `-developer-edition`, `-esr`, `-dev`, `-wayland`, `-x11`, and others.

### 2. `canonical_id` on Package

- **Backend (`models.rs`):** Added `canonical_id: String` to `Package` (with `#[serde(default)]`).
- **Population:** Set in `merge_search_results` when inserting any package (Official, Flatpak, AUR) and in `deduplicate_by_canonical_key` when inserting.
- **Frontend (`PackageCard.tsx`):** Added optional `canonical_id?: string` to the `Package` interface.

### 3. Merger refactor (`commands/search.rs`)

- **Keyed map only:** Every package is keyed by `canonical_merge_key(name, app_id)`; no `direct_key` or extra match loops for Flatpak.
- **Merge strategy:** If key exists: append the new package’s source to `available_sources`; if the new package has a **friendly name** (no dots) and the existing one has an **ID name** (e.g. `com.discordapp.Discord`), overwrite `name`/`display_name` with the friendly one.
- **Source priority:** After building the map, each package’s **primary `source`** is set from `available_sources` via `best_primary_source()`: **Official/Repo first** (lowest priority value), then **Flatpak**, then **AUR**.

### 4. Frontend list keys

- **TrendingSection.tsx, SearchPage.tsx, CategoryView.tsx:** List keys use `pkg.canonical_id || (pkg.app_id || pkg.name).toLowerCase()` (or `pkg.canonical_id || pkg.name` in category) so one app = one key and old responses without `canonical_id` still work.

### Verification

- **Discord test:** Search "Discord" → exactly one card.
- **Dropdown test:** Open that card → SourceSelector shows "System (CachyOS/Repo)", "Flatpak", "AUR" where applicable.
- **Browse test:** Internet (or other) category → no duplicate cards for the same app.

See also: `docs/UNIFICATION_AND_DROPDOWN_REVIEW.md` for the full review; `docs/RECENT_CHANGES.md` for all recent changes (Chaotic Good, labels, onboarding).

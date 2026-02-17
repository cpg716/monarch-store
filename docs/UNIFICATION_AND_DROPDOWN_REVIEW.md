# Review: Single Card Per App & Details Dropdown of All Sources

**Date:** 2026-02-03  
**Scope:** Confirm (1) no duplicate cards for the same app, and (2) opening a card shows a details page with a dropdown of all downloadable sources.

---

## 1. Single Card Per App (No Duplicate Cards)

### Backend: Merge & Deduplication

| Flow | Where | What happens |
|------|--------|----------------|
| **Search** | `search_packages` → `merge_search_results(official, aur, flatpak)` | All results keyed by `canonical_merge_key(name, app_id)`. Official, AUR, and Flatpak hits for the same app (e.g. "opera", "com.opera.Opera", AUR `opera`) merge into one `Package` with `available_sources` and `canonical_id` set. |
| **Get by names** | `get_packages_by_names` | After merging ALPM + Chaotic + AUR + Flatpak via `merge_search_results`, runs `deduplicate_by_canonical_key` so the same app never appears twice in batch results. |
| **Trending** | `get_trending` | `merge_search_results(Vec::new(), aur_packages, flathub_search_results)` then `deduplicate_by_canonical_key`. One card per app. |
| **Category** | `get_category_packages_paginated` | Before featured/injected merge, runs `deduplicate_by_canonical_key` so appstream + featured inject collapse to one card per app. |

**Canonical key** (`utils::canonical_merge_key`):

- Reverse-DNS app_id (e.g. `com.discordapp.Discord`) → last segment (`discord`); known map used for e.g. `com.obsproject.Studio` → `obs-studio`.
- Package name with suffix stripping (`-bin`, `-git`, `-flatpak`, etc.) so `opera`, `opera-bin` map to the same key.

**Model:** Each merged `Package` has `canonical_id` (the key) and `available_sources: Option<Vec<PackageSource>>` (all sources for that app). Primary `source` is set from `best_primary_source(available_sources)` (Official/Repo > Flatpak > AUR).

### Frontend: One Entry Per Key

| Location | Key used | Effect |
|----------|----------|--------|
| `TrendingSection.tsx` | `pkg.canonical_id \|\| (pkg.app_id \|\| pkg.name).toLowerCase()` | One React element per canonical app. |
| `SearchPage.tsx` | `pkg.canonical_id \|\| (pkg.app_id \|\| pkg.name).toLowerCase()` | Same. |
| `CategoryView.tsx` | `pkg.canonical_id \|\| pkg.name` (featured and grid) | Same. |

**Conclusion:** Backend always merges by canonical key and sets `canonical_id`; frontend uses stable keys derived from `canonical_id`/name. **One card per app** is enforced end-to-end.

---

## 2. Details Page: Dropdown of All Downloadable Sources

### Variants Built on Details Open

**File:** `src/pages/PackageDetailsFresh.tsx`

1. **Card primary is always included**  
   `cardPrimary` is built from `pkg.source` with a non-empty version (`pkg.source.version` or `pkg.version` or `"latest"`). It is prepended to the combined list so the source shown on the card (e.g. Flatpak) is never dropped.

2. **Merge order**  
   `combined = [cardPrimary, ...fetchedVars, ...fromAvailableSources, ...propAlternatives]`  
   So: card source first, then backend `get_package_variants`, then `pkg.available_sources`, then `pkg.alternatives`.

3. **Deduplication**  
   Variants are deduped by source identity (`source_type:id:pkg_name`). First occurrence wins, so the card’s source (from `cardPrimary`) is kept when it overlaps with backend.

4. **Version filter**  
   Only variants with a non-empty version are kept; `cardPrimary` always has a non-empty version, so the card’s source remains in the list.

5. **Lookup uses canonical id**  
   `get_package_variants` is called with `pkg.canonical_id || pkg.name`, so the backend returns all variants (repo, chaotic, AUR, Flatpak) for that app.

### Backend: `get_package_variants`

**File:** `src-tauri/monarch-gui/src/commands/search.rs`

- Accepts a single name/canonical id (e.g. `"opera"`).
- Queries: **ALPM** (all syncdbs), **Chaotic** (if enabled), **AUR** (canonical base + base name), **Flatpak** (Flathub by canonical base and by app_id).
- Returns `Vec<PackageVariant>` (source, version, repo_name, pkg_name) for all matches keyed by the same canonical base.

So the details page receives repo, chaotic, AUR, and Flatpak variants when available.

### Dropdown (RepoSelector)

- **Shown when:** `variants.length > 1` or `pkg.available_sources?.length > 1` (so at least two sources).
- **Props:** `variants` (all merged variants), `selectedSource`, `onChange`.
- **Selection:** Initial selection prefers the **card’s source** (`pkg.source`): we only auto-select the installed source when it matches the card’s source; otherwise we keep the card’s source so opening a “Flatpak” card does not flip to AUR.
- **Comparison:** Both `PackageDetailsFresh` and `RepoSelector` use an `isSameSource` that treats string `"flatpak"` and object `{ id: "flathub", source_type: "flatpak" }` as the same, so the dropdown highlights the correct option and install/version logic finds the right variant.

**Conclusion:** Opening a card builds a variant list that always includes the card’s source and merges in all backend and card sources, dedupes, and shows them in `RepoSelector`. **The details page shows one listing with a dropdown of all downloadable sources** (Official/Repo, Flatpak, AUR, Chaotic when present).

---

## 3. Summary

| Claim | Status | Where enforced |
|-------|--------|-----------------|
| No duplicate cards for the same app | **Confirmed** | Backend: `merge_search_results` + `deduplicate_by_canonical_key` in search, get_packages_by_names, get_trending, get_category_packages_paginated. Frontend: list keys use `canonical_id` / stable id. |
| Card opens to a listing with dropdown of all sources | **Confirmed** | Details: `cardPrimary` + merge with `get_package_variants` + `fromAvailableSources` + alternatives; dedupe by source; RepoSelector shows when `variants.length > 1`; selection prefers card source. |

---

## 4. Related Files (Quick Reference)

- **Backend merge/dedupe:** `middleware/aggregation.rs` (logic), `src-tauri/monarch-gui/src/commands/search.rs` (entry point), `src-tauri/monarch-gui/src/utils.rs` (`canonical_merge_key`, `deduplicate_by_canonical_key`).
- **Backend model:** `src-tauri/monarch-gui/src/models.rs` (`Package.canonical_id`, `Package.available_sources`, `PackageVariant`).
- **Frontend list keys:** `TrendingSection.tsx`, `SearchPage.tsx`, `CategoryView.tsx`.
- **Details variants & dropdown:** `src/pages/PackageDetailsFresh.tsx` (effect that builds variants, selection logic), `src/components/RepoSelector.tsx` (dropdown, `isSameSource`).

See also: `docs/DUPLICATE_CARDS_FIX_UPDATE.md` for the original fix description; `docs/RECENT_CHANGES.md` for Operation Chaotic Good (Chaotic-AUR safe toggle, onboarding, Configure Source).

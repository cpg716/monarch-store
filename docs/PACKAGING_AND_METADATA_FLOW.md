# MonARCH Store — Packaging and Metadata Flow

**Last updated:** 2026-03-09

**Abstract:** This document describes the current Iron Core aggregation and hydration pipeline. MonARCH follows a **backend-as-truth** model where Rust builds canonical package payloads before the GTK frontend renders them.

---

## 1. Metadata Hydration: The "Brain" Approach

Unlike previous versions that relied on frontend hooks to "pre-warm" or "guess" icons, v0.4.6+ hydrates all metadata in the Rust backend.

### Sources
The backend aggregates metadata from:
1.  **Unified SQLite Registry**: A persistent index of all discovered AppStream applications across the host system.
2.  **AppStream XMLs**: Scanned from `/usr/share/app-info/xmls/` and Flatpak paths.
3.  **ODRS (Open Desktop Ratings Service)**: In-flight or cached ratings joined to the package by AppID.
4.  **Flathub API**: Real-time hydration for Flatpak-specific metadata.

### Automatic Enrichment
When a search or listing command is invoked (e.g., `search_packages`), the backend:
-   **Aggregates**: Parallel fetch from ALPM, AUR, and Flathub.
-   **Merges**: Groups results by `canonical_merge_key`.
-   **Hydrates**: Joins the merged results against the SQLite Registry. If a package (like AUR or Repo) matches an AppID in the registry, it is **automatically backfilled** with icons, screenshots, and descriptions.
-   **SSOT Pass 2**: (v0.4.8) A final enrichment pass is performed post-aggregation to ensure all variants (including search results) carry the richest metadata available in the local AppStream index.

---

## 2. The "Dumb View" Frontend

The active GTK frontend is a strict **subscriber** to Iron Core:
- **Model contract only**: GTK renders `Package`, `PackagePresentation`, `PackageVariant`, `FullPackageDetails`, `SearchOptions`, `HomeSnapshot`, and `GtkSettings`.
- **Zero frontend hydration**: metadata parsing, source merge logic, and taxonomy truth stay in Rust.
- **Legacy Tauri note**: the older React/Tauri frontend is historical/reference-only.

---

## 3. Canonical Merge Key & Identity

Identity is governed by `utils::canonical_merge_key` and `utils::canonical_search_base`:
- **App ID first:** trusted app-id mappings take precedence.
- **Packaging suffixes stripped:** `-bin`, `-git`, `-appimage`, `-desktop`, `.desktop`, `-hg`, `-svn`, `-official`, `-repo`, `-stable`.
- **Channel variants preserved:** canary/beta/nightly/ptb/dev/esr/developer-edition/insider remain distinct identities (not merged into stable).
- **One card per product identity:** Repo/AUR/Flatpak variants for the same identity are merged into one card with source selector.
- **No frontend inference:** selector order and default source come from backend payload.

---

## 4. Source Priority & Deduplication

When merging, the system enforces a strict priority for the primary source (badge + default selector):
1.  **Distro-native repo** (e.g. CachyOS/Manjaro/Garuda families on matching hosts)
2.  **Arch Official** (`core`, `extra`, `multilib`, etc.)
3.  **Chaotic-AUR** (repo binaries)
4.  **Flatpak** (Flathub)
5.  **AUR** (source build)

Discovery aggregation also respects enabled-source gating from Settings, so disabled sources do not leak into Search/Trending/Essentials/Categories.

Metadata is preserved from the **best metadata source** during the merge, regardless of which source is chosen as the primary install target. v0.4.8 ensures `screenshots` and `long_description` are prioritized from Flatpak sources if missing in Repo/AUR.

---

## 5. Installed Source Truth

Installed-source labels are computed from ALPM/localdb + syncdb candidate matching and exact Flatpak app-id checks, not inferred from fuzzy text metadata. This prevents false source labels (for example, incorrectly marking a package as Chaotic-installed).

---

## 6. Summary of Improvements
1.  **Eliminated 400 Errors**: Base64 icons are properly wrapped and validated in the backend/middleware.
2.  **Zero Layout Shift**: Since cards arrive fully hydrated, there is no "pop-in" of icons or descriptions.
3.  **Bulletproof FFI**: Progress callbacks are isolated to prevent panics during intensive ALPM transactions.
4.  **Rich Persistence**: Long descriptions and screenshots are now stored in the local SQLite registry for offline access and faster loading.

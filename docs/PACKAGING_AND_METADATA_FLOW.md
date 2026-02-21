# MonARCH Store — Packaging and Metadata Flow (v0.4.7-alpha)

**Abstract:** This document details the modern data aggregation and hydration pipeline in Monarch Store. The application follows a **"Backend as Truth"** model where the Rust backend is responsible for fully hydrating package ViewModels (icons, descriptions, screenshots, ratings) from multiple sources (SQLite, ODRS, registries) before the frontend renders them.

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
-   **SSOT Pass 2**: (v0.4.7) A final enrichment pass is performed post-aggregation to ensure all variants (including search results) carry the richest metadata available in the local AppStream index.

---

## 2. The "Dumb View" Frontend

The React frontend is now a strict **subscriber** to the backend registry:
- **Bindings Only**: All types and commands flow through `tauri-specta` generated `bindings.ts`.
- **Zero-Hydration Hooks**: Legacy hooks like `usePrewarmCards`, `usePackageMetadata`, and `useRatings` have been deleted.
- **Store Sync**: The `internal_store.ts` acts as a simple Key-Value cache of the hydrated ViewModels provided by the backend.

---

## 3. Canonical Merge Key & Identity

Identity is governed by the **first-segment rule**:
- **Logic:** Identifiers are normalized (suffixes stripped), and the first segment is used as a brand key (e.g. `heroic-games-launcher` -> `heroic`).
- **One Card per App:** This ensures that Repo, AUR, and Flatpak variants of the same application are unified into a single card with a source selector.
- **Full Specification:** See `docs/UNIVERSAL_DATA_ENGINE.md`.

---

## 4. Repo Priority & Deduplication

When merging, the system enforces a strict priority to determine the "Primary Source" (shown on the card badge):
1.  **Official/Repo** (Arch, CachyOS, etc.)
2.  **Flatpak** (Flathub)
3.  **AUR**

Metadata is preserved from the **best metadata source** during the merge, regardless of which source is chosen as the primary install target. v0.4.7 ensures `screenshots` and `long_description` are prioritized from Flatpak sources if missing in Repo/AUR.

---

## 5. Summary of Improvements
1.  **Eliminated 400 Errors**: Base64 icons are properly wrapped and validated in the backend/middleware.
2.  **Zero Layout Shift**: Since cards arrive fully hydrated, there is no "pop-in" of icons or descriptions.
3.  **Bulletproof FFI**: Progress callbacks are isolated to prevent panics during intensive ALPM transactions.
4.  **Rich Persistence**: Long descriptions and screenshots are now stored in the local SQLite registry for offline access and faster loading.

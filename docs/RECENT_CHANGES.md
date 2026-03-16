# Recent Changes — Full Update

**Report date:** 2026-03-09  
**Scope:** current change log and historical snapshots. GTK is the active frontend; older Tauri-specific sections are historical unless superseded by newer sections.

> Historical sections below are preserved as release-time snapshots. Where behavior changed later, the newest section takes precedence.

---

## 0. Iron Core + Fluid Identity Hardening (2026-02-27)

### Catalog Stabilization (Deterministic SSOT for Card + Details)
- Unified identity/merge pipeline for discovery/search/details seeds:
  - New shared builder: `build_package_view_models_v2(...)`
  - Used to produce one canonical package identity with deterministic source ordering.
- Source priority is now consistent end-to-end:
  - `distro-native repo > Arch Official > Chaotic-AUR > Flatpak > AUR`.
- Removed synthetic source pollution:
  - Metadata enrichment no longer appends Flathub to `available_sources` unless source matching is already trusted by canonical identity flow.
- Frontend store now treats backend payload as authoritative SSOT for package records:
  - Removed stale `available_sources` preservation logic that caused source “bouncing”.
- Details source selector stability:
  - Fixed stale-closure race when backend variants arrive.
  - Selector now resolves from backend variant payload deterministically.
- Registry hardening:
  - Added catalog schema version gate with one-time rebuild on mismatch.
  - `bulk_upsert_packages` now refreshes package fields more comprehensively and always rewrites `package_sources` rows per package (eliminates stale source ghosts).
- Source variant list in details now built from canonical `available_sources` slots (deduped), with source-specific metadata fallback from alternatives.
- **Files:** `src-tauri/monarch-gui/src/middleware/aggregation.rs`, `src-tauri/monarch-gui/src/commands/search/core.rs`, `src-tauri/monarch-gui/src/commands/search/discovery.rs`, `src-tauri/monarch-gui/src/registry.rs`, `src-tauri/monarch-gui/src/utils.rs`, `src-tauri/monarch-gui/src/commands/package.rs`, `src/store/internal_store.ts`, `src/pages/PackageDetailsFresh.tsx`, `src/App.tsx`.

### Non-blocking transaction completion (95%/finalizing hang reduction)
- **Helper ExecuteBatch** now emits completion immediately and schedules orphan housekeeping in a detached background task.
- Post-install finalization is no longer inline in the main transaction return path, reducing perceived hangs near completion.
- **InstallMonitor** now explicitly surfaces `Finalizing` messaging when housekeeping is running in background.
- **Files:** `src-tauri/monarch-helper/src/main.rs`, `src/components/InstallMonitor.tsx`.

### Host-adaptive appearance (Portal-driven)
- Added typed backend command **`get_host_appearance`** (Specta + ACL + bindings) to expose:
  - portal color scheme (`dark`/`light`/`system`)
  - portal accent color (`#RRGGBB`, when available)
  - detected desktop environment (`gnome`/`kde`/etc).
- Startup portal bridge now emits both `system-theme-changed` and `system-accent-changed`.
- Frontend `useTheme` now consumes host appearance and applies CSS vars:
  - `--system-accent`
  - `--system-bg`
  - branded accent blending for premium MonARCH look.
- **Files:** `src-tauri/monarch-gui/src/commands/system.rs`, `src-tauri/monarch-gui/src/lib.rs`, `src-tauri/monarch-gui/src/specta_gen.rs`, `src-tauri/monarch-gui/permissions/app-commands-read.toml`, `src/services/bindings.ts`, `src/hooks/useTheme.ts`, `src/App.css`.

### Native-feeling title bar behavior
- `TitleBar.tsx` now adapts control placement by host desktop profile:
  - GNOME-style: centered controls
  - KDE/Windows-style: right-aligned controls.
- **File:** `src/components/TitleBar.tsx`.

### Card rendering recovery path
- `PackageCard` fallback recovery now retries by canonical ID and then by name (`get_packages_by_names`) if registry hydration missed the card key.
- This reduces cases where cards stay on skeletons due transient registry misses.
- **File:** `src/components/PackageCard.tsx`.

---

## 1. Iron Core Purge (2026-02-14)

### "Dumb View" Transition
- **Deleted Legacy Hooks**: `usePrewarmCards`, `usePackageMetadata`, and `useRatings` have been removed. The frontend no longer manages independent metadata fetching or "pre-warming" of cards.
- **Backend Hydration**: All metadata (icons, descriptions, screenshots, ratings) is now fully hydrated in the Rust backend within the `search_packages` and `get_package_variants` commands.
- **Initialization Performance**: Resolved the "Initializing system..." hang by releasing the metadata loader lock before starting the registry sync. This ensures that the UI can remain responsive and proceed with startup commands while the background synchronization task completes.
- **Specta Enforcement**: Replaced manual TypeScript interfaces with `tauri-specta` generated types from `bindings.ts`.

### Logic Offloading
- **Regex Removal**: Stripped frontend-side regex parsing for package sizes and versions in `InstalledPage.tsx` and `UpdatesPage.tsx`. The backend now provides these as formatted strings.
- **Source Selection**: Refactored `PackageDetailsFresh` to use direct `commands` for fetching metadata and PKGBUILDs, ensuring a single source of truth.

---

## 2. Universal Data Engine (2026-02-08)

**See:** `docs/UNIVERSAL_DATA_ENGINE.md` for full spec.

### Canonical key (no per-app alias list)
- **First-segment rule (historical at the time):** Multi-segment names were grouped by first segment. This behavior is superseded by the 2026-02-27 channel-aware canonical identity rules.
- **Removed:** `known_name_alias_to_canonical` and the single `"heroic"` → `"heroicgameslauncher"` entry; merge key is now fully generic.

### One proper name per app
- **Preferred display name:** Small display-only map `preferred_display_name(canonical_key)` so one app always shows one label: e.g. `heroic` → "Heroic Game Launcher", `obs` → "OBS Studio", `visual` → "Visual Studio Code". Applied after dedup so we never show "Heroic" next to "Heroic Game Launcher".
- **Fallback:** After dedup, packages with no `display_name` (and non–app_id-style name) get `to_pretty_name(name)`. App_id-style names are left to metadata.

### Other fixes (same day)
- **Essentials / Categories not loading:** `upsertPackages` now adds currently upserted package ids to the eviction protected set so they aren’t evicted before `setEssentialsIds`/`setTrendingIds` run. CategoryView retries up to 2 times (3s, 6s) when backend returns 0 packages; TrendingSection retries when result empty (including Essentials with filterIds).
- **Trending not AUR-only:** `get_trending` includes official repo packages from metadata loader (`get_apps_by_category` for network, office, audiovideo, graphics, utility; top 25), merged with AUR and Flatpak.
- **Icon/base64 400:** `resolveIconUrl` (iconHelper.ts) strips whitespace, wraps raw base64 in `data:image/png;base64,...`, and never returns raw base64 as URL.

**Files:** `src-tauri/monarch-gui/src/utils.rs`, `commands/search.rs`, `src/store/internal_store.ts`, `src/utils/iconHelper.ts`, `src/pages/CategoryView.tsx`, `src/components/TrendingSection.tsx`.

---

## 2. Package Unification (One Card Per App)

### Backend
- **Deduplication:** `utils::deduplicate_by_canonical_key` used in `get_packages_by_names`, `get_trending`, `get_category_packages_paginated` so the same app never appears twice (e.g. Discord from appstream + featured inject → one card).
- **Canonical key (historical at the time):** `utils::canonical_merge_key(name, app_id)` used first-segment grouping. This was later superseded by channel-aware canonical identity rules in the 2026-02-27 stabilization pass.
- **Model (historical at the time):** `Package` had `canonical_id` and `available_sources` with older primary-source ordering; current ordering is documented in section 0.
- **Files:** `src-tauri/monarch-gui/src/commands/search.rs`, `utils.rs`, `models.rs`.

### Frontend
- List keys use `pkg.canonical_id || (pkg.app_id || pkg.name).toLowerCase()` (or `pkg.canonical_id || pkg.name` in category) in `TrendingSection.tsx`, `SearchPage.tsx`, `CategoryView.tsx`.

**See:** `docs/UNIFICATION_AND_DROPDOWN_REVIEW.md`, `docs/DUPLICATE_CARDS_FIX_UPDATE.md`.

---

## 3. Details Page: Dropdown of All Sources

- **Card primary always included:** `cardPrimary` built from `pkg.source` with non-empty version; prepended to variant list so the source shown on the card (e.g. Flatpak) is never dropped.
- **Merge order:** `[cardPrimary, ...fetchedVariants, ...fromAvailableSources, ...propAlternatives]`; deduped by source identity; only variants with non-empty version kept.
- **Lookup:** `get_package_variants(pkg.canonical_id || pkg.name)` returns repo, Chaotic, AUR, Flatpak variants.
- **Selection:** Initial selection prefers the **card’s source** so opening a Flatpak card does not flip to AUR. `isSameSource` treats string `"flatpak"` and object `{ id: "flathub", source_type: "flatpak" }` as the same.
- **Files:** `src/pages/PackageDetailsFresh.tsx`, `src/components/RepoSelector.tsx`, `src-tauri/monarch-gui/src/commands/search.rs` (`get_package_variants`).

**See:** `docs/UNIFICATION_AND_DROPDOWN_REVIEW.md`.

---

## 4. UI/UX Fixes (Cards & Dropdown)

- **Card dropdown edge clipping:** PackageCard version selector given spacing (margin/padding) and `ChevronDown` icon with `pr-8` so the dropdown is clearly visible and does not hit the edge.
- **RepoSelector labels:**
  - AUR entries show `pkg_name` (e.g. "AUR (vlc)" vs "AUR (vlc-git)").
  - "Other repository" entries show repo `id` in parentheses (e.g. "Other repository (repo-name)").
- **Backend labels (`labels.rs`):** "Custom Repository" → "Other repository"; mappings for "monarch" → "MonARCH Store", Manjaro repo names → "Manjaro". Arch Official (core, extra, multilib) labeled "Arch Official" regardless of `distro_id`.

**Files:** `src/components/PackageCard.tsx`, `src/components/RepoSelector.tsx`, `src-tauri/monarch-gui/src/labels.rs`.

---

## 5. Operation Chaotic Good (Safe Chaotic-AUR Toggle)

**Policy:** Read-only host — we do **not** programmatically edit `/etc/pacman.conf`. User adds the Chaotic-AUR repo block manually; we only install keyring and mirrorlist via the Helper.

### Backend
- **Distro:** `distro_context.rs::is_chaotic_compatible()` — blocks Manjaro.
- **ALPM check:** `alpm_read.rs::chaotic_aur_in_syncdbs()` — verifies Chaotic-AUR in syncdbs.
- **Commands:** `check_chaotic_status()` (compatibility + ALPM presence), `prepare_chaotic_components()` (invokes Helper to install keyring + mirrorlist).
- **Helper:** `HelperCommand::PrepareChaoticComponents` — `pkexec pacman -U --noconfirm` for Chaotic keyring and mirrorlist.
- **Files:** `distro_context.rs`, `alpm_read.rs`, `commands/system.rs`, `helper_client.rs`, `monarch-helper/main.rs`.

### Settings (Traffic Light UX)
- **SourcesTab:** Chaotic-AUR status shown as Active / Inactive / Blocked. Clicking inactive toggle runs `prepare_chaotic_components` and opens "Final Step" modal: instructions to edit `/etc/pacman.conf`, code block, "Copy to Clipboard," "Check Again" (re-fetches status and closes modal when successful).
- Descriptions/warnings added for Chaotic-AUR, AUR, and Flatpak (disabling hides from discovery; does not stop updates for already-installed packages).

### Onboarding Wizard
- **OnboardingModal** refactored into multi-step wizard:
  1. **Universal Welcome** — Distro detection, philosophy.
  2. **Source Manager** — Toggles for Flatpak (Recommended), AUR (Advanced), Chaotic-AUR (disabled on Manjaro).
  3. **Chaotic-AUR Setup** (conditional) — "Install Keys & Mirrors" → same "Final Step" modal as Settings.
  4. **Security & Privacy** — Session password, "Reduce password prompts," "Help us improve MonARCH" (Telemetry).
  5. **Theme** — Light/Dark, accent color.
  6. **Confirmation** — Summary of selections.
- Steps are 4 or 5 depending on Chaotic-AUR compatibility; progress "Step X of N"; framer-motion transitions.

### Package Cards & Details (Configure Source)
- **useChaoticStatus** hook: `compatible`, `chaotic_in_alpm`, `enabled`, `isOnlyChaoticSource(pkg)`.
- When `isOnlyChaoticSource(pkg)` and Chaotic-AUR not enabled: PackageCard shows "Setup Required" badge and "Configure Source" button (opens Settings).
- PackageDetailsFresh: when selected source is Chaotic-AUR and not enabled, install button replaced with "Configure Source" (navigate to Settings).
- **Files:** `src/hooks/useChaoticStatus.ts`, `src/components/PackageCard.tsx`, `src/pages/PackageDetailsFresh.tsx`, `src/components/settings/SourcesTab.tsx`, `src/components/OnboardingModal.tsx`; `onOpenSettings` callback passed from App, SearchPage, CategoryView, TrendingSection, HomePage.

**See:** ARCHITECTURE.md (§ Chaotic-AUR), USER_GUIDE.md (§ Sources & Onboarding), AGENTS.md (§ Settings page).

---

## 6. Discovery & Repo Behavior

- Chaotic-AUR packages appear in search when the repo is enabled; `repo_manager` discovers all system repos from `/etc/pacman.conf`.
- Distro-aware behavior (Manjaro block, Garuda/CachyOS native) unchanged; no injection — discovery only.

---

## 8. Permissions (One-Click vs Polkit), Distro, Chaotic & Onboarding (2026-02-08)

### Auth: Branded prompt vs Polkit
- **One-click ON (Reduce password prompts):** All Helper invocations use the app’s **branded** password dialog once per session; password is passed to `invoke_helper` and the Helper is run with **`sudo -S`** (password on stdin). Single prompt, one-click feel.
- **One-click OFF:** `invoke_helper` is always called with **Polkit (`pkexec`)**; `password` is ignored so advanced users get the **system auth dialog** every time and full control (per-session, agent, etc.).
- **Backend:** `helper_client::invoke_helper(app, cmd, password, use_branded_auth)`. When `use_branded_auth` is false, password is forced to `None` and the Helper is spawned with pkexec. Every privileged command (install, uninstall, sync, repair, chaotic, clear cache, unlock, cancel_install, apply_os_config, etc.) reads `RepoManager::is_one_click_enabled().await` and passes it as the fourth argument.
- **Files:** `helper_client.rs`, `commands/package.rs`, `commands/system.rs`, `commands/update.rs`, `repair.rs`, `repo_manager.rs`.

### Distro detection (all Arch-based distros)
- **`distro_context.rs`:** Added **`archlinux`** as synonym for Arch (`ID=archlinux`). Added **`ID_LIKE`** parsing: if `ID` is not in the known list but `ID_LIKE` contains **`arch`** (e.g. ArcoLinux, Archcraft), the distro is treated as **Arch** (Unlocked, Chaotic Allowed). Existing behavior kept: Manjaro = Chaotic Blocked; Garuda/CachyOS = Native; EndeavourOS/Arch = Allowed.

### Chaotic-AUR and Onboarding UX
- **Onboarding Chaotic step:** When distro is **native** (Garuda/CachyOS) or **already in ALPM** (`check_chaotic_status` when entering step), the step shows “Chaotic-AUR is already enabled” and “Ready to use” instead of “Install Keys & Mirrors”. After “Check Connection” in the Final Step modal, `chaoticAlreadyInAlpm` is set so the main step reflects success.
- **Sources step:** Chaotic row is shown as disabled using **capability** `!supportsChaotic` (not only `distro.id === 'manjaro'`), with text “Not available on this distro (incompatible with this system).”
- **Final Step modal (Onboarding + Settings):** Copy updated for new Linux users: “Open /etc/pacman.conf in a text editor (e.g. sudo nano /etc/pacman.conf), add the two lines below at the end, save and exit, then click Check Connection.”
- **Security step:** Clarified one-click wording: “Choose how you authorize installs and updates: one prompt per session (recommended for most users) or a system dialog every time (for advanced control).”

### Repo handling (install/uninstall)
- **Install:** `enabled_repos` from `repo_manager.get_all_repos().filter(enabled)` plus **core, extra, community, multilib** always so ALPM can resolve dependencies. **target_repo** passed so the Helper installs from the selected repo (e.g. chaotic-aur, cachyos-v3). For monarch-style repos we call `apply_os_config` (sync) before `AlpmInstall`. All flows use `one_click` from RepoManager for auth.
- **Uninstall:** `uninstall_package` uses `State<RepoManager>`, reads `one_click`, passes it to `invoke_helper(AlpmUninstall)`.
- **AUR:** Build runs as user (makepkg); only `AlpmInstallFiles` uses Helper with `one_click`. Paths: build in `~/.cache/monarch/build`, copy to `/tmp/monarch-install`, Helper installs from there.

**Files:** `distro_context.rs`, `OnboardingModal.tsx`, `SourcesTab.tsx`, `package.rs`, `repo_manager.rs`.

---

## 9. Document Cross-References

| Topic | Primary doc |
|-------|-------------|
| Universal Data Engine (canonical key, display names, fixes) | `docs/UNIVERSAL_DATA_ENGINE.md` |
| One card per app & details dropdown | `docs/UNIFICATION_AND_DROPDOWN_REVIEW.md` |
| Duplicate cards fix (original + master architect) | `docs/DUPLICATE_CARDS_FIX_UPDATE.md` |
| Permissions (one-click vs Polkit), distro, Chaotic, onboarding | This doc §8; `docs/STARTUP_AND_PERMISSIONS_REVIEW.md`; `docs/ONBOARDING_REVIEW.md` |
| Full architecture | `docs/STATE_OF_THE_UNION_ARCHITECTURE_REPORT.md`, `ARCHITECTURE.md` |
| Release history | `RELEASE_NOTES.md` |
| Build & rules | `AGENTS.md`, `.cursorrules` |

---

## 10. Middleware Refactoring (2026-02-11)

### Motivation
To improve code organization and testability, the core aggregation logic was extracted from `search.rs` into a dedicated `middleware` module. `search.rs` now focuses solely on command handling/dispatching.

### Changes
- **New Module:** `src-tauri/monarch-gui/src/middleware/aggregation.rs`.
- **Moved Logic:**
  - `merge_search_results`: Primary entry point for merging Repo/AUR/Flatpak results.
  - `deduplicate_and_merge_packages`: Core logic for unifying duplicate apps.
  - `enrich_packages_metadata`: Metadata backfilling.
  - `fetch_and_merge_packages_by_names_impl`.

### Impact
- **No functional change:** The application behaves exactly as before.
---

## 11. Rich Metadata & FFI Stability (2026-02-21)

### Rich Metadata Merging
- **Screenshots & Long Descriptions**: Updated the deduplication and merging logic in `aggregation.rs`. Generic variants (Repo/AUR) now automatically inherit rich content (screenshots, multi-paragraph descriptions) from matched Flatpak variants.
- **SSOT Pass 2**: Added a secondary local AppStream enrichment pass to all discovery views (Trending, Essentials, Search). This guarantees that even if external APIs return early, local metadata is systematically cross-referenced for any missing icons or details.
- **Persistence**: Expanded the SQLite registry schema to store `long_description` and `screenshots`. Applied migrations to existing developer/user databases.

### Bulletproof ALPM (FFI Stability)
- **Signal 6 Fix**: Resolved critical `signal 6 (SIGABRT)` abort panics that occurred during large official repository transactions (e.g., kernel or firmware upgrades).
- **Callback Safety**: Implemented strict pointer validation and thread boundary isolation for ALPM progress and status callbacks.
- **Optimized Hydration**: Decoupled high-frequency metadata hydration from the mission-critical ALPM operation thread to prevent race conditions during heavy IO.

### UI & Configuration
- **Settings & Sidebar**: Hardcoded version strings updated to `v0.4.8-alpha`.
- **Labels**: Refined `SourceSelector` labels for better clarity across different variants.

**Files:** `middleware/aggregation.rs`, `commands/search.rs`, `registry.rs`, `helper_client.rs`, `monarch-helper/main.rs`, `SettingsPage.tsx`, `Sidebar.tsx`.

---

## 12. Updates Hardening: Partial Success + Structured Progress (2026-02-27)

### Backend (truthful completion model)
- `perform_system_update` now emits a typed `update-complete` payload:
  - `overall`: `success | partial | failed`
  - `summary`: per-source status (`repo`, `aur`, `flatpak`), succeeded package names, failed package list with reasons, warnings, duration.
- Repo failure is treated as a hard failure and aborts AUR/Flatpak phases (Arch safety preserved).
- AUR/Flatpak failures are recorded as partial failures instead of being reported as blanket success.

### Backend (structured source progress)
- Added `update-source-progress` event for source-native progress:
  - `source`: `repo | aur | flatpak`
  - `stage`, `current`, `total`, optional `package`
- Existing `update-status` remains for human-readable text.

### Frontend (Updates UX)
- Updates page now consumes typed `update-complete` and `update-source-progress` events.
- One-Click flow asks for branded session password up front when enabled; user can still choose system prompt fallback.
- Added beginner-friendly summary card after completion:
  - updated count
  - failed count
  - failed items list with retry action.
- Added optional **Advanced Controls** drawer:
  - per-source scope toggles for current run
  - per-package include/exclude
  - retry failed-only path.

### Notes
- Command signatures remained stable (`perform_system_update(password, include_aur, include_flatpak)` unchanged).
- Update detection semantics remain source-inclusive for installed apps across repo/AUR/Flatpak.
# 2026-02-27 (Stability Pass: Catalog Truth, Details Loop, and Source Provenance)

- Fixed homepage Essentials hydration so partial canonical-id hits no longer suppress the rest of the list; missing items now fall back to `getPackagesByNames(...)` instead of returning a half-populated result.
- Removed the `PackageDetailsFresh` refetch loop that was repeatedly re-triggering `getFullPackageDetails(...)` and re-hitting Flathub metadata during StrictMode and normal source switching.
- Discovery/search/category commands now require backend-enabled source state for Chaotic/AUR/Flatpak instead of letting stale frontend flags re-enable hidden sources.
- Added hard timeouts around Chaotic fetches in shared aggregation and variant lookup paths so search, details, and discovery surfaces degrade gracefully instead of stalling on remote Chaotic latency.
- Source toggles for Chaotic-AUR, AUR, and Flatpak now invalidate backend search/list caches immediately so discovery surfaces do not keep serving stale source combinations after a user changes visibility settings. The Settings UI also skips unnecessary repo syncs when toggling Chaotic because that toggle is discovery-only.
- Source normalization now runs inside the shared deduplication pass itself, and `available_sources` are sorted deterministically by backend priority before the primary source is selected. This removes route-specific differences where Trending could keep a stale first-inserted source while Search/Details showed the correct priority.
- `PackageCard` and `PackageDetailsFresh` no longer re-sort source lists on the frontend. They now filter hidden sources but preserve backend order, so the UI follows the backend’s SSOT priority instead of recomputing its own.
- Frontend registry upserts now preserve richer backend metadata (`long_description`, screenshots, icon, display name, app ID, maintainer, license) when a later lightweight backend payload omits those fields. This prevents search/trending/category refreshes from wiping details already hydrated by `getFullPackageDetails`.
- Added duplicate-request guards for startup and category loading: app startup initialization now runs once per mount lifecycle, and `CategoryView` suppresses in-flight duplicate requests for the same category/filter/page key so StrictMode and rapid state churn do not trigger redundant identical fetches.
- Registry-sync events in `App.tsx` are now debounced and coalesced before invoking backend sync, and `PackageDetailsFresh` now coalesces repeated detail refreshes (including `install-complete` refreshes) so overlapping requests do not pile up and re-flap details state.
- Fixed dark theme startup background regression by pinning the dark app background instead of temporarily inheriting a light portal/system surface color.
- Fixed installed package provenance in ALPM reads: repo origin now prefers `%INSTALLED_DB%` from pacman's local database, and packages installed from local files without repo provenance are labeled `local` instead of being misclassified as Chaotic-AUR.
- **Files:** `src/App.tsx`, `src/App.css`, `src/components/PackageCard.tsx`, `src/pages/CategoryView.tsx`, `src/pages/PackageDetailsFresh.tsx`, `src/hooks/useSettings.ts`, `src/store/internal_store.ts`, `src-tauri/monarch-gui/src/commands/package.rs`, `src-tauri/monarch-gui/src/commands/search/core.rs`, `src-tauri/monarch-gui/src/commands/search/discovery.rs`, `src-tauri/monarch-gui/src/commands/search/categories.rs`, `src-tauri/monarch-gui/src/middleware/aggregation.rs`, `src-tauri/monarch-gui/src/alpm_read.rs`, `src-tauri/monarch-gui/src/repo_manager.rs`.

# 2026-02-27 (Stability Pass: Card Hydration, Alias-Aware Installed State, and Details De-Flapping)

- Removed Framer Motion layout interpolation from `PackageCard`, which was causing visible card overlap/ghosting during rapid list updates and registry refreshes.
- Fixed `PackageDetailsFresh` stale-closure writes: details/reviews now read the current package from refs instead of re-upserting an old `pkg` snapshot, which was causing package info to bounce back to stale values after a later review/details response landed.
- `PackageDetailsFresh` no longer rebinds its backend details refresh to every incidental `pkg` object mutation or source preference change; detail refresh is now keyed to the package identity instead of unstable object references.
- Ratings hydration is now more robust for repo-first packages that do not initially carry an `app_id`: frontend rating lookups expand known package-name aliases to likely app IDs, and registry rating merges also match against those known app IDs.
- Installed detection is now alias-aware across discovery/search/details instead of exact-name only. Canonical app families like VS Code (`visual-studio-code` vs `code`) now check known repo aliases before deciding that a package is not installed.
- Applied alias-aware installed detection to shared search/discovery/aggregation paths so `PackageCard` surfaces can show installed state without requiring a prior details-page open.
- **Files:** `src/components/PackageCard.tsx`, `src/pages/PackageDetailsFresh.tsx`, `src/store/internal_store.ts`, `src/utils/packageKey.ts`, `src-tauri/monarch-gui/src/utils.rs`, `src-tauri/monarch-gui/src/commands/package.rs`, `src-tauri/monarch-gui/src/commands/search/core.rs`, `src-tauri/monarch-gui/src/commands/search/discovery.rs`, `src-tauri/monarch-gui/src/middleware/aggregation.rs`, `src-tauri/monarch-gui/src/aur_api.rs`, `src-tauri/monarch-gui/src/repo_manager.rs`.

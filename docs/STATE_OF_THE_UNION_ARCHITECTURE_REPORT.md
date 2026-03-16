# MonARCH Store: State of the Union Architecture Report
**Date:** 2026-03-09
**Version:** v0.4.8-alpha
**Scope:** Current architecture snapshot for the GTK-first MonARCH product. Historical Tauri references below are reference-only.

---

## 1. System Summary

MonARCH Store is a **Host-Adaptive, Universal** software center for Arch-based systems. It unifies **Official Repos**, **AUR**, and **Flatpak** under one interface while maintaining a **Dumb View** GTK frontend over an Iron Core backend-hydrated registry.

### Core Capabilities (v0.4.8-alpha)

| Capability | Description |
|------------|-------------|
| **Universal Search** | Single search bar queries ALPM (repo), AUR (raur), and Flathub in parallel; results merged and deduplicated by canonical key. |
| **Rich Metadata** | (v0.4.8) Secondary enrichment pass & multi-source merging (Screenshots, Long Descriptions). |
| **Bulletproof ALPM** | (v0.4.8) FFI-safe callback isolation preventing signal 6/abort panics during heavy IO. |
| **Native AUR Builder** | User-level `makepkg` (libgit2 clone, `.SRCINFO` parse, GPG key import); built `.pkg.tar.zst` installed via monarch-helper. |
| **Flatpak** | First-class install/remove/update via Flathub API and Flatpak CLI. |
| **Host-Adaptive Repos** | Repositories discovered from ALPM/`pacman.conf`; discovery visibility is toggle-driven in app state; Manjaro blocks Chaotic by default. |
| **Silent Guard** | Batch transactions (Refresh + Upgrade + Install/Remove) in one helper invocation; Polkit `auth_admin_keep` reduces password prompts. |
| **Safe Guard** | Update-before-install; no silent full upgrade on stale DB; IgnorePkg/IgnoreGroup respected in helper. |
| **Liquid UI** | Min window 800×600; responsive grids (1–4 cols); mobile bottom nav; stacked package details on narrow viewports. |

### Mission of "Universal MonARCH"

- **One place** for Official, AUR, and Flatpak — no need to visit three different UIs.
- **Respect host config** — no repo injection; discovery from system state.
- **Iron Core Purge** — All metadata hydration and logic offloaded to the backend; frontend acts as a pure render layer.
- **Safety rails** — no `pacman -Sy` alone; full upgrade enforced when any official package is involved.

---

## 2. Data Pipeline Map: Search Query Flow

End-to-end path of a search in the current GTK product:

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│ FRONTEND (GTK)                                                                    │
├─────────────────────────────────────────────────────────────────────────────────┤
│  monarch-gtk controllers/pages request Home, Search, Details, Installed, and      │
│  Updates payloads from monarch-core, then render those hydrated package models.   │
└─────────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ GTK/core command bridge
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│ BACKEND (monarch-core, Rust)                                                      │
├─────────────────────────────────────────────────────────────────────────────────┤
│  catalog/bootstrap/registry models produce canonical package identities, merged   │
│  sources, storefront rails, category payloads, and detail payloads.               │
└─────────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ Return Vec<Package>
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│ FRONTEND (GTK - 'Dumb View')                                                      │
├─────────────────────────────────────────────────────────────────────────────────┤
│  package_card.rs renders hydrated `Package` payloads                              │
│  package_detail.rs renders hydrated `FullPackageDetails` payloads                 │
│  Source switching and card/detail composition remain backend-driven               │
└─────────────────────────────────────────────────────────────────────────────────┘
```

**Summary:** GTK asks Iron Core for search/category/storefront/detail payloads. The backend aggregates ALPM, AUR, and Flathub data, deduplicates by canonical identity, orders sources deterministically, and returns backend-owned `available_sources` and detail facts.

---

## 3. Backend Logic: Key Flows

### 3.1 Distro Detection (`distro_context.rs`)

- **Source of truth:** `/etc/os-release`.
- **Parsing:** Read `ID` and `PRETTY_NAME`; `id_val` lowercased.
- **Mapping:** `manjaro` → Manjaro, `garuda` → Garuda, `cachyos` → CachyOS, `endeavouros` → EndeavourOS, `arch` → Arch; else `Unknown(id_val)`.
- **Capabilities per distro:** RepoManagementMode (Unlocked/Locked/Managed), ChaoticSupport (Allowed/Blocked/Native), default_search_sort, description, icon_key. Manjaro: chaotic **Blocked**; CachyOS/Garuda: chaotic **Native**; Arch/Endeavour: **Allowed**.

### 3.2 Search Aggregation & the Merger (`middleware/aggregation.rs` + `utils.rs`)

- **Parallel fetch:** `repo_manager.get_packages_matching`, `aur_api::search_aur`, `flathub.search_flathub` via `tokio::join!`.
- **Merge (middleware/aggregation.rs):**
  - **Official** first: key = `canonical_merge_key(&p.name, app_id)`; insert into `package_map`.
  - **Flatpak:** match by direct_key, canonical_key, strip_package_suffix, or flathub app_id mapping; if match → append Flatpak to existing `available_sources`; else insert new Package with source Flatpak.
  - **AUR:** key = `canonical_merge_key`; if key exists → append AUR to `available_sources`; else insert.
- **Canonical key (`utils.rs`):** App-id mapping first; packaging suffixes stripped; channel suffixes (`-canary`, `-beta`, `-nightly`, `-ptb`, etc.) preserved as distinct identities.
- **merge_and_deduplicate (utils.rs):** Used for get_packages_by_names, get_trending, category pagination. Groups by app_id or strip_package_suffix(name); priority swap so higher-priority source becomes primary and lower goes to `alternatives`.

### 3.3 Helper Client — "Silent Guard" (`helper_client.rs`)

- **Debounce:** 800 ms between invocations (LAST_HELPER_INVOKE) to limit rapid calls.
- **Command delivery:** JSON serialized to temp file under `/var/tmp` (monarch-cmd-*.json); path passed as argv[1]. Stdin used only for password when using `sudo -S`.
- **Invocation:** Password present → `sudo -E -S <helper_bin> <cmd_path>`; else `pkexec --disable-internal-agent <helper_bin> <cmd_path>`.
- **Helper binary choice:** Production `/usr/lib/monarch-store/monarch-helper` preferred when exists; dev builds use `get_dev_helper_path()` (CARGO_TARGET_DIR/debug/monarch-helper, or exe-relative, or path list). `MONARCH_USE_PRODUCTION_HELPER=1` forces production.
- **Output:** stdout parsed as JSON lines; AlpmProgressEvent → emit `alpm-progress` and legacy ProgressMessage; event_type `error` → emit `install-error-classified`. stderr → `helper-output` with "[Helper Error]:".

### 3.4 Monarch-Helper (`main.rs`, `transactions.rs`)

- **Root only:** euid check; exit 125 if not root.
- **Stream segregation:** stdout/stderr redirected to log file; original stdout duplicated for JSON progress pipe.
- **Command input:** Env `MONARCH_CMD_JSON` (sudo path) or `MONARCH_CMD_FILE` or argv[1] as file path. File must be owned by invoking user (pkexec).
- **ExecuteBatch (Silent Guard):** remove_lock → refresh_db → update_system → remove_targets → install_targets → local_paths (AUR built files). Single lock acquisition for full batch.
- **Question callback:** InstallIgnorepkg → set_install(false) to respect host IgnorePkg/IgnoreGroup.
- **Cancel:** PID file `/var/tmp/monarch-helper.pid`; watcher thread checks `/var/tmp/monarch-cancel`; on create, exit so GUI can run RemoveLock.

---

## 4. Legacy frontend hierarchy reference

> Historical note: the section below documents the older Tauri/React frontend structure for parity reference. The active frontend is `monarch-gtk`, which renders the same Iron Core package models through GTK pages and controllers.

| Component | Responsibility |
|-----------|----------------|
| **App.tsx** | Root state: activeTab, searchQuery, packages, selectedPackage, preferredSource, onboarding, systemHealth, activeInstall. Routes content by tab and selection; SearchBar; MobileNav; InstallMonitor; ErrorModal; OnboardingModal. |
| **Sidebar** | Desktop nav (Explore, Search, Installed, Favorites, Updates, Settings). |
| **MobileNav** | Bottom bar (Search, Explore, Installed, Updates, Settings) on small screens. |
| **SearchBar** | Controlled input; 300ms debounce is in App.tsx useEffect. |
| **SearchPage** | Renders search results (packages), loading state; grid of PackageCards. |
| **HomePage** | HeroSection, CategoryGrid, TrendingSection, Essentials. |
| **PackageCard** | Single package card with backend-provided icon/metadata, deterministic best-source badge, and "+N sources". Includes fallback hydration by canonical ID/name if a card payload is briefly missing. |
| **PackageDetailsFresh** | Detail view: variants/source selector (card primary always included; get_package_variants + available_sources; selection prefers card source). When selected source is Chaotic-AUR and not enabled: "Configure Source" (navigate to Settings). Install/uninstall, reviews, PKGBUILD, screenshots. **Stacked layout:** header/actions `flex-col lg:flex-row`; icon + text + actions stack on narrow viewports. |
| **SourcesTab** | Host system (read-only), Chaotic-AUR "traffic light" (Active/Inactive/Blocked); inactive toggle runs prepare_chaotic_components and opens "Final Step" modal (edit pacman.conf snippet, Copy, Check Again). Flatpak/AUR toggles; useDistro + useSettings (isAurEnabled, isFlatpakEnabled, repos, toggleRepo, toggleAur, toggleFlatpak). |
| **internal_store.ts** | Frontend cache that stores backend-hydrated package records by canonical/list key; backend payload wins on merges to prevent stale source drift. |
| **useSettings** | Repo state from get_repo_states; isAurEnabled, isFlatpakEnabled, toggleAur, toggleFlatpak, toggleRepo; sync with backend. |
| **OnboardingModal** | Multi-step wizard: (1) Universal Welcome (distro, philosophy), (2) Source Manager (Flatpak, AUR, Chaotic-AUR toggles; Chaotic disabled on Manjaro), (3) Chaotic-AUR Setup conditional—"Install Keys & Mirrors" → "Final Step" modal (pacman.conf snippet, Copy, Check Again), (4) Security & Privacy (session password, Reduce prompts, Telemetry), (5) Theme (Light/Dark, accent), (6) Confirmation. Steps 4 or 5 depending on Chaotic compatibility; framer-motion transitions. |
| **useChaoticStatus** | Hook: compatible, chaotic_in_alpm, enabled, isOnlyChaoticSource(pkg). Used by PackageCard and PackageDetailsFresh for "Configure Source" when Chaotic-only and not enabled. |
| **RepoSelector** | AUR entries show pkg_name (e.g. "AUR (vlc-git)"); "Other repository" shows repo id in parentheses. isSameSource handles string vs object PackageSource for correct selection. |

**Routing:** Historical Tauri note: the legacy frontend used local state routing rather than React Router.

---

## 5. Configuration & Infrastructure Verification

| Item | Status | Notes |
|------|--------|-------|
| **Version sync** | ✅ | package.json, tauri.conf.json, monarch-gui/Cargo.toml, monarch-helper/Cargo.toml = **0.4.8-alpha**. |
| **Window constraints** | ✅ | tauri.conf.json: minWidth 800, minHeight 600. |
| **CSP** | ✅ | connect-src includes api.archlinux.org, supabase, aptabase, chaotic, cachyos, raw.githubusercontent.com. |
| **Permissions** | ✅ | Helper path /usr/lib/monarch-store/monarch-helper; command via file; capabilities/permissions for frontend/backend command surfaces. |
| **Dockerfile** | ✅ | Ubuntu 22.04; ca-certificates; libalpm built from pacman v7.1.0; Node 20; Rust; libwebkit2gtk, librsvg, etc. |
| **Tailwind** | ✅ | Tailwind 4 (@tailwindcss/postcss); no tailwind.config.js in repo (Tailwind 4 uses CSS-first config). |

---

## 6. Risk Analysis & Code Smells

### 6.1 Potential Breakage / Consistency

| Risk | Location | Description |
|------|----------|-------------|
| **Variant dedupe vs. Flatpak name** | middleware/aggregation.rs merge_search_results | Flatpak entries use `hit.name` for display; matching uses app_id and strip_package_suffix. Edge case: different naming (e.g. "Brave Browser" vs "brave-bin") could yield two entries if mapping misses. Mitigated by flathub_api::get_flathub_app_id and suffix matching. |
| **get_package_variants dedupe** | search.rs get_package_variants | Dedupe by `(source, version, pkg_name)` string key; PackageVariant from backend and from pkg.available_sources combined then filtered. Slight risk of duplicate variants if backend and frontend differ in source representation. |
| **Stale search results** | App.tsx | searchRequestIdRef correctly invalidates stale responses; 300ms debounce can still allow two in-flight requests for fast typing. Acceptable. |
| **Repo filter vs. dedupe** | get_category_packages_paginated | repo_filter applied before merge_and_deduplicate; featured/injected then deduped. Order of operations is correct. |

### 6.2 Hardcoded / Magic Values

| Location | Item | Suggestion |
|----------|------|------------|
| search.rs | `popular_apps` array (~25 names) | Consider moving to constants or config. |
| search.rs | `get_featured_apps(category)` large static lists per category | Same; could live in constants.ts or backend config. |
| get_trending | titan_names, cachy_curated, aur_curated | Centralize "curated" lists. |
| helper_client.rs | HELPER_DEBOUNCE 800 ms, CMD_FILE_DIR /var/tmp | Document in DEVELOPER.md; 800 ms is intentional. |
| PackageDetailsFresh | visibleReviewsCount 5, pagination step | Could be constant. |

### 6.3 Missing or Fragile Logic

| Item | Notes |
|------|--------|
| **Flatpak in get_packages_by_names** | get_packages_by_names does repo + chaotic + AUR; no Flathub batch. Package details still get Flatpak via get_package_variants. Acceptable. |
| **Error handling in merge_search_results** | unwrap_or_default on official_res, aur_res, flatpak_res; one failing source doesn’t kill search. Good. |
| **Card fallback hydration loops** | PackageCard can trigger canonical/name recovery when payloads are missing; ensure fallback remains deduped and bounded to avoid repeated bursts. |
| **SourcesTab chaotic toggle** | toggleRepo(chaoticRepo.id) when chaotic not in pacman.conf may no-op; UI disables with tooltip "Not available in /etc/pacman.conf." Correct. |

### 6.4 Security / Protocol

| Item | Status |
|------|--------|
| **Helper command file ownership** | main.rs checks file uid == invoking user when using pkexec. ✅ |
| **AlpmInstallFiles paths** | Restricted to /tmp/monarch-install (canonical). ✅ |
| **No RunCommand** | UNIVERSAL_PROTOCOL_REPORT_CARD: pacman-helper.sh wrapper removed; pkexec pacman used when no password. ✅ |
| **Validate package names** | utils::validate_package_name() used before shell/ALPM in critical paths. ✅ |

---

## 7. Document References

| Doc | Purpose |
|-----|---------|
| README.md | Product overview, features, install. |
| ARCHITECTURE.md | Host-Adaptive, Unified Search, Iron Core, Liquid UI. |
| RELEASE_NOTES.md | v0.4.8-alpha and prior changelogs. |
| CONTRIBUTING.md | PR rules, styleguides, repo safety (no pacman -Sy alone, SafeUpdateTransaction). |
| docs/DEVELOPER.md | Single reference: setup, structure, helper protocol, versioning. |
| docs/ERROR_SERVICE.md | ErrorService API, severity, ClassifiedError. |
| docs/APTABASE_INTEGRATION.md | Telemetry events, consent, event_category/label. |
| docs/UNIVERSAL_PROTOCOL_REPORT_CARD.md | Repo safety, Native Builder, Silent Guard, Unified State, zombie code audit. |
| docs/TROUBLESHOOTING.md | User-facing: lock, GPG, AUR unknown error, corrupt DB, Wayland. |
| docs/RECENT_CHANGES.md | Full summary of unification, details dropdown, Operation Chaotic Good, labels, onboarding. |
| AGENTS.md | Build commands, critical package rules, lock safety. |

---

---

## 8. Milestone Archive: v0.4.8-alpha (Stabilization & Source Truth)

The v0.4.8-alpha stabilization cycle hardens the existing "Iron Core" architecture:

### 8.1 FFI Stability (Bulletproof ALPM)
- **Problem:** Frequent `signal 6 (SIGABRT)` panics during large transactions (e.g. `linux` kernel updates) caused by unsafe pointer access in ALPM progress callbacks.
- **Solution:** Rationalized the FFI callback layer. Implemented safe pointer validation and ensured metadata hydration avoids blocking the ALPM transaction thread.

### 8.2 Rich Metadata Merging
- **Merging Logic:** `aggregation.rs` now allows generic variants (Repo/AUR) to inherit `screenshots` and `long_description` from rich variants (Flatpak) during the deduplication phase.
- **SSOT Pass 2:** Implementation of a secondary local AppStream enrichment pass for all discovery views, ensuring icons and descriptions are consistent regardless of the API fetching order.

### 8.3 Registry Persistence
- **Schema Migration:** Added `long_description` and `screenshots` columns to the local registry DB.
- **Performance:** Optimized `bulk_upsert_packages` to prevent UI lockup while syncing 3000+ AppStream entries.

### 8.4 Catalog and source-truth hardening
- Unified catalog builder for discovery/search/details seed paths.
- Deterministic source ordering across backend and frontend.
- Installed-source classification moved to ALPM/localdb + syncdb matching to avoid false source labels.
- Discovery source gating aligned with Settings toggles.

**Report generated from full codebase review. Use for onboarding, audits, and planning.**

# MonARCH Store: STATE OF THE UNION ARCHITECTURE REPORT (v0.4.7-alpha)
**Date:** 2026-02-21
**Version:** v0.4.7-alpha
**Scope:** Full codebase assimilation (Iron Core Purge, documentation, Rust backend, React frontend, configuration)

---

## 1. System Summary

MonARCH Store is a **Host-Adaptive, Universal** software center for Arch-based systems. It unifies **Official Repos**, **AUR**, and **Flatpak** under one interface while maintaining a **Dumb View** frontend that subscribes to a backend-hydrated registry.

### Core Capabilities (v0.4.6-alpha)

| Capability | Description |
|------------|-------------|
| **Universal Search** | Single search bar queries ALPM (repo), AUR (raur), and Flathub in parallel; results merged and deduplicated by canonical key. |
| **Native AUR Builder** | User-level `makepkg` (libgit2 clone, `.SRCINFO` parse, GPG key import); built `.pkg.tar.zst` installed via monarch-helper. |
| **Flatpak** | First-class install/remove/update via Flathub API and Flatpak CLI. |
| **Host-Adaptive Repos** | Repositories discovered from ALPM/`pacman.conf`; only `chaotic-aur` is toggled via drop-in; Manjaro blocks chaotic-aur. |
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

End-to-end path of a search from the React SearchBar to the Rust backend and back:

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│ FRONTEND (React)                                                                  │
├─────────────────────────────────────────────────────────────────────────────────┤
│  SearchBar.tsx                                                                    │
│    value={searchQuery}  onChange={setSearchQuery}                                 │
│         │                                                                          │
│         ▼                                                                          │
│  App.tsx: useEffect([searchQuery])                                                │
│    300ms debounce → invoke('search_packages', { query: searchQuery })            │
│    searchRequestIdRef guards stale responses → setPackages(results)               │
│         │                                                                          │
│         ▼                                                                          │
│  SearchPage.tsx  (or inline when activeTab === 'search')                          │
│    packages.map(pkg => <PackageCard pkg={pkg} onClick={setSelectedPackage} />)     │
└─────────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ Tauri IPC (invoke)
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│ BACKEND (monarch-gui, Rust)                                                      │
├─────────────────────────────────────────────────────────────────────────────────┤
│  commands/search.rs :: search_packages()                                         │
│    1. if query.len() < 2 → return Ok(Vec::new())                                │
│    2. tokio::join!(                                                               │
│         repo_manager.get_packages_matching(&query, distro),  // ALPM read         │
│         aur_api::search_aur(&query),                          // AUR RPC          │
│         flathub.search_flathub(&query)                         // Flathub API      │
│       )                                                                           │
│    3. middleware::aggregation::merge_search_results(...)  ← THE MERGER            │
│       - package_map keyed by utils::canonical_merge_key(name, app_id)            │
│       - Order: Official inserted first, then Flatpak (merge into existing or new), │
│         then AUR (merge or insert)                                                │
│    4. Friendly labels: labels::get_friendly_label(source.id, distro_id_str)      │
│    5. Relevance sort: calculate_relevance() + popular_apps boost                  │
│    6. Ok(results)                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ Return Vec<Package>
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│ FRONTEND (React - 'Dumb View')                                                    │
├─────────────────────────────────────────────────────────────────────────────────┤
│  PackageCard.tsx                                                                  │
│    - Renders fully-hydrated `Package` object from `bindings.ts`                   │
│    - No background metadata fetching (Legacy hooks DELETED)                       │
│    - resolveIconUrl handles sanitized backend base64                              │
│  PackageDetailsFresh.tsx (on click)                                               │
│    - fetchMetadata(pkgName) → direct command call                                 │
│    - source selector based on backend `available_sources`                         │
└─────────────────────────────────────────────────────────────────────────────────┘
```

**Summary:** Search is triggered by `searchQuery` state; after 300ms debounce the frontend calls `search_packages`. The backend runs ALPM, AUR, and Flathub in parallel, then **merge_search_results** (the "Merger") deduplicates by **canonical_merge_key** (first-segment rule for multi-segment names + app_id via known map; see `docs/UNIVERSAL_DATA_ENGINE.md`). Official > Flatpak > AUR priority; each result carries **available_sources**. After dedup, one proper display name per app. The UI shows one card per logical app with a "best" source badge and optional "+N sources."

---

## 3. Backend Logic: Key Flows

### 3.1 Distro Detection (`distro_context.rs`)

- **Source of truth:** `/etc/os-release`.
- **Parsing:** Read `ID` and `PRETTY_NAME`; `id_val` lowercased.
- **Mapping:** `manjaro` → Manjaro, `garuda` → Garuda, `cachyos` → CachyOS, `endeavouros` → EndeavourOS, `arch` → Arch; else `Unknown(id_val)`.
- **Capabilities per distro:** RepoManagementMode (Unlocked/Locked/Managed), ChaoticSupport (Allowed/Blocked/Native), default_search_sort, description, icon_key. Manjaro: chaotic **Blocked**; CachyOS/Garuda: chaotic **Native**; Arch/Endeavour: **Allowed**.

### 3.2 Search Aggregation & the Merger (`middleware/aggregation.rs` + `utils.rs`)

- **Parallel fetch:** `repo_manager.get_packages_matching`, `aur_api::search_aur`, `flathub.search_flathub` via `tokio::join!`.
- **Merge (middleware/aggregation.rs :: merge_search_results):**
  - **Official** first: key = `canonical_merge_key(&p.name, app_id)`; insert into `package_map`.
  - **Flatpak:** match by direct_key, canonical_key, strip_package_suffix, or flathub app_id mapping; if match → append Flatpak to existing `available_sources`; else insert new Package with source Flatpak.
  - **AUR:** key = `canonical_merge_key`; if key exists → append AUR to `available_sources`; else insert.
- **Canonical key (`utils.rs`):** App_id path uses known map + **first-segment rule** when result is multi-segment; name path strips **VARIANT_SUFFIXES** then uses first segment for multi-segment names (e.g. `heroic-games-launcher` and `heroic` → key `heroic`). One proper display name per app via `preferred_display_name` or `to_pretty_name`. See `docs/UNIVERSAL_DATA_ENGINE.md`.
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

## 4. Component Hierarchy (Frontend)

| Component | Responsibility |
|-----------|----------------|
| **App.tsx** | Root state: activeTab, searchQuery, packages, selectedPackage, preferredSource, onboarding, systemHealth, activeInstall. Routes content by tab and selection; SearchBar; MobileNav; InstallMonitor; ErrorModal; OnboardingModal. |
| **Sidebar** | Desktop nav (Explore, Search, Installed, Favorites, Updates, Settings). |
| **MobileNav** | Bottom bar (Search, Explore, Installed, Updates, Settings) on small screens. |
| **SearchBar** | Controlled input; 300ms debounce is in App.tsx useEffect. |
| **SearchPage** | Renders search results (packages), loading state; grid of PackageCards. |
| **HomePage** | HeroSection, CategoryGrid, TrendingSection, Essentials. |
| **PackageCard** | Single package: icon (usePackageMetadata + resolveIconUrl), RepoBadge (best source), "+N sources", rating (usePackageRating), favorites, version/variant selector (ChevronDown, spacing to avoid edge clipping). When only source is Chaotic-AUR and not enabled: "Setup Required" badge and "Configure Source" (opens Settings) via useChaoticStatus. |
| **PackageDetailsFresh** | Detail view: variants/source selector (card primary always included; get_package_variants + available_sources; selection prefers card source). When selected source is Chaotic-AUR and not enabled: "Configure Source" (navigate to Settings). Install/uninstall, reviews, PKGBUILD, screenshots. **Stacked layout:** header/actions `flex-col lg:flex-row`; icon + text + actions stack on narrow viewports. |
| **SourcesTab** | Host system (read-only), Chaotic-AUR "traffic light" (Active/Inactive/Blocked); inactive toggle runs prepare_chaotic_components and opens "Final Step" modal (edit pacman.conf snippet, Copy, Check Again). Flatpak/AUR toggles; useDistro + useSettings (isAurEnabled, isFlatpakEnabled, repos, toggleRepo, toggleAur, toggleFlatpak). |
| **usePackageMetadata** | Fetches icon/AppStream by pkgName (and optional upstreamUrl); 5-min cache; invoke('get_metadata'). |
| **useSettings** | Repo state from get_repo_states; isAurEnabled, isFlatpakEnabled, toggleAur, toggleFlatpak, toggleRepo; sync with backend. |
| **OnboardingModal** | Multi-step wizard: (1) Universal Welcome (distro, philosophy), (2) Source Manager (Flatpak, AUR, Chaotic-AUR toggles; Chaotic disabled on Manjaro), (3) Chaotic-AUR Setup conditional—"Install Keys & Mirrors" → "Final Step" modal (pacman.conf snippet, Copy, Check Again), (4) Security & Privacy (session password, Reduce prompts, Telemetry), (5) Theme (Light/Dark, accent), (6) Confirmation. Steps 4 or 5 depending on Chaotic compatibility; framer-motion transitions. |
| **useChaoticStatus** | Hook: compatible, chaotic_in_alpm, enabled, isOnlyChaoticSource(pkg). Used by PackageCard and PackageDetailsFresh for "Configure Source" when Chaotic-only and not enabled. |
| **RepoSelector** | AUR entries show pkg_name (e.g. "AUR (vlc-git)"); "Other repository" shows repo id in parentheses. isSameSource handles string vs object PackageSource for correct selection. |

**Routing:** No React Router. App uses `activeTab` and `selectedPackage` / `selectedCategory` / `viewAll` to show: Onboarding, PackageDetails, CategoryView, ViewAll (essentials/trending), or main content (SearchPage, HomePage, InstalledPage, UpdatesPage, SettingsPage).

---

## 5. Configuration & Infrastructure Verification

| Item | Status | Notes |
|------|--------|-------|
| **Version sync** | ✅ | package.json, tauri.conf.json, monarch-gui/Cargo.toml, monarch-helper/Cargo.toml = **0.4.5-alpha**. |
| **Window constraints** | ✅ | tauri.conf.json: minWidth 800, minHeight 600. |
| **CSP** | ✅ | connect-src includes api.archlinux.org, supabase, aptabase, chaotic, cachyos, raw.githubusercontent.com. |
| **Permissions** | ✅ | Helper path /usr/lib/monarch-store/monarch-helper; command via file; capabilities/permissions for Tauri commands. |
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
| **usePackageMetadata skip** | skipMetadataFetch used by PackageCard to avoid double fetch when data is pre-filled; not all call sites pass it. Minor. |
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
| RELEASE_NOTES.md | v0.4.5-alpha and prior changelogs. |
| CONTRIBUTING.md | PR rules, styleguides, repo safety (no pacman -Sy alone, SafeUpdateTransaction). |
| docs/DEVELOPER.md | Single reference: setup, structure, helper protocol, versioning. |
| docs/ERROR_SERVICE.md | ErrorService API, severity, ClassifiedError. |
| docs/APTABASE_INTEGRATION.md | Telemetry events, consent, event_category/label. |
| docs/UNIVERSAL_PROTOCOL_REPORT_CARD.md | Repo safety, Native Builder, Silent Guard, Unified State, zombie code audit. |
| docs/TROUBLESHOOTING.md | User-facing: lock, GPG, AUR unknown error, corrupt DB, Wayland. |
| docs/RECENT_CHANGES.md | Full summary of unification, details dropdown, Operation Chaotic Good, labels, onboarding. |
| AGENTS.md | Build commands, critical package rules, lock safety. |

---

**Report generated from full codebase review. Use for onboarding, audits, and planning.**

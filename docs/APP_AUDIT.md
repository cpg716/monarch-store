# MonARCH Store — Full App Audit

Exhaustive review of UI/UX, frontend, backend, and every feature area. Reference for understanding the app to the smallest details.

---

## 1. Architecture Overview

| Layer | Stack | Key Paths |
|-------|--------|-----------|
| **Frontend** | React 19, TypeScript, Tailwind CSS 4, Vite 7, Zustand, Framer Motion | `src/App.tsx`, `src/pages/`, `src/components/`, `src/hooks/`, `src/store/` |
| **Backend (GUI)** | Tauri 2, Rust workspace | `src-tauri/monarch-gui/` |
| **Helper** | Rust binary (privileged) | `src-tauri/monarch-helper/` |
| **IPC** | `invoke()` from `@tauri-apps/api/core`; events via `listen()` | Commands in `src-tauri/monarch-gui/src/commands/` |

**Routing:** SPA with local state. No URL router; `activeTab` + `selectedPackage` / `selectedCategory` / `viewAll` drive the main content. "Search" tab is a special case: it sets `activeTab` to `explore` and focuses the search input.

---

## 2. UI/UX

### 2.1 Global Layout

- **Shell:** `flex h-screen w-screen` with Sidebar (left) and main content (right).
- **Loading gate:** `isRefreshing` shows `LoadingScreen` (butterfly + "Loading MonARCH...") until startup (health check, onboarding decision, pre-warm) finishes; target min 1.5s.
- **Banner:** Unhealthy system shows red "Infrastructure Issues Detected" bar with "Repair Now" → Settings.
- **Onboarding:** `OnboardingModal` when first run or when `check_initialization_status` reports `!is_healthy`; completion stored in `localStorage` (`monarch_onboarding_v3`).

### 2.2 Theming & Accessibility

- **Theme:** `useTheme()` — `themeMode` (system/light/dark), `accentColor` (hex). Stored via Tauri store / persistence.
- **Selection:** `--tw-selection-bg` set from accent in `App.tsx`.
- **CSS:** `App.css` + Tailwind; `app-bg`, `app-fg`, `app-muted`, `app-card`, `app-border`, `app-subtle`, `app-accent` for consistency.
- **Icons:** `lucide-react` throughout; `clsx` for conditional classes.
- **Motion:** Framer Motion for list/card animations, sidebar width, modal enter/exit.

### 2.3 Sidebar

- **Tabs:** Search, Explore, Installed, Favorites, Updates, Settings.
- **State:** `monarch_sidebar_expanded` in localStorage; auto-collapse below 1024px.
- **Update badge:** `check_for_updates` on mount; red dot when count > 0.
- **Tooltips:** When collapsed, hover shows label + short description.

### 2.4 Search Bar

- Sticky under hero (Explore) or in main content; gradient border on focus.
- Single controlled input; 300ms debounced search in `App.tsx` (search runs in effect keyed by `searchQuery`).
- Enter blurs input; no submit button.

---

## 3. Frontend Deep Dive

### 3.1 App.tsx — State & Flow

**Core state:**

- `activeTab`, `activeInstall`, `viewAll`, `showOnboarding`, `searchQuery`, `packages`, `selectedPackage`, `preferredSource`, `onboardingReason`, `selectedCategory`, `loading`, `isRefreshing`, `systemHealth`, `enabledRepos`.

**Key effects:**

1. **Update listeners:** `update-progress`, `install-output`, `update-status` → store progress/phase/logs; on `phase === 'complete'` runs delayed post-update checks (`check_reboot_required`, `get_pacnew_warnings`).
2. **Startup:** `initializeStartup()` — parallel: `fetchInfraStats`, `checkTelemetry`, `get_repo_states`; then `check_initialization_status`. Onboarding vs normal: if unhealthy or first run → onboarding; else pre-warm (ESSENTIAL_IDS, get_trending, prewarmRatings).
3. **Search:** Debounced 300ms; `invoke('search_packages', { query })`; request ID used to ignore stale responses; results → `setPackages`, `addSearch`, optional `track_event`.
4. **Tab change:** "search" → focus input, clear selection; "settings" → scroll to `#system-health` after 100ms.

**Content branching:**

- Onboarding → empty main + `OnboardingModal`.
- `selectedPackage` → `PackageDetails`.
- `selectedCategory` → `CategoryView`.
- `viewAll` → sticky header + `TrendingSection` (essentials or trending).
- Else → Hero (Explore only) + sticky SearchBar + main: `SearchPage` (when query or tab search), `HomePage`, `InstalledPage`, Favorites block, `UpdatesPage`, or `SettingsPage`.

**InstallMonitor:** Rendered when `activeInstall` is set; `onClose` clears it; install/uninstall triggered from PackageDetails.

### 3.2 Pages

| Page | Role | Data / IPC |
|------|------|------------|
| **HomePage** | Essentials (via useSmartEssentials) + TrendingSection + CategoryGrid | `get_essentials_list`, `get_all_installed_names`, `get_trending` (pre-warm in App) |
| **SearchPage** | Pre-search UI (history, quick filters, favorites chips) or results grid | `searchQuery`/`packages` from App; sort (best_match/name/updated), filter by source, "Load More" +50 |
| **InstalledPage** | List of installed apps with filter, Launch, Uninstall, open details | `get_installed_packages`, `get_package_icon`/`get_metadata`, `uninstall_package`, `launch_app`, `get_packages_by_names`/`search_packages` for details |
| **UpdatesPage** | Pending updates list, "Update All", progress stepper, logs, reboot/pacnew | `check_for_updates`, `perform_system_update`, store: updateProgress/phase/logs, `repair_unlock_pacman`, `listen('update-complete')` |
| **SettingsPage** | Health dashboard, repos, AUR, sync, theme, accent, one-click, repair, privacy, advanced, about | Many: `get_repo_states`, `toggle_repo`, `reorderRepos`, `triggerManualSync`, `updateOneClick`, `install_monarch_policy`, repair commands, `set_telemetry_enabled`, etc. |
| **PackageDetailsFresh** | Single package: variants, install/uninstall, reviews, screenshots, PKGBUILD | `get_package_variants`, `check_installed_status`, `install_package`/`uninstall_package` (via parent), reviews, metadata, icons |
| **CategoryView** | Category apps with repo filter, sort, pagination, infinite scroll | `get_category_packages_paginated`, `get_chaotic_packages_batch`, `get_repo_states` |

### 3.3 Components

| Component | Purpose |
|-----------|---------|
| **PackageCard** | Card: icon, name, version selector (variants), description, source badge, rating, favorite + download buttons. Uses `usePackageMetadata`, `usePackageRating`, `useFavorites`; chaotic batch optional. |
| **PackageCardSkeleton** | Placeholder for loading grids. |
| **TrendingSection** | Fetches by `filterIds` (e.g. essentials) or `get_trending`; horizontal scroll or grid; batch chaotic info. |
| **CategoryGrid** | Grid of category tiles (from `CATEGORIES`); click → `onSelectCategory`. |
| **HeroSection** | Logo, tagline, distro badge (useDistro), repo access labels. |
| **SearchBar** | Controlled input, gradient focus ring. |
| **Sidebar** | Tab buttons, expand/collapse, update badge. |
| **InstallMonitor** | Modal: steps (Safety → Download → Install → Finalize), progress bar, logs, success/error, recovery (keyring/lock/classified errors), "Update & Install" when `failed_update_required`. Single-run guard via `actionStartedForRef`. |
| **ConfirmationModal** | Generic confirm/cancel; used for uninstall, update all, clear cache, orphans, advanced mode. |
| **EmptyState** | Icon, title, description, optional action (e.g. "Retry", "Clear filters"). |
| **RepoSelector** | Used in PackageDetails to pick source when multiple variants. |
| **SystemHealthSection** | Used in Settings for health/repair. |
| **OnboardingModal** | Welcome/repair wizard; on complete sets `monarch_onboarding_v3`. |
| **LoadingScreen** | Full-screen loading with butterfly asset. |
| **ErrorBoundary** | Wraps app for React errors. |

### 3.4 Hooks

| Hook | Role |
|------|------|
| **useFavorites** | LazyStore `favorites.json`; `favorites`, `toggleFavorite`, `isFavorite`. |
| **useSearchHistory** | localStorage `monarch_search_history`; `history`, `addSearch`, `removeSearch`, `clearHistory` (max 10). |
| **useSmartEssentials** | `get_essentials_list` + `get_all_installed_names`; filters out installed, min 4 shown. |
| **useSettings** | Repos, AUR, sync interval, notifications, one-click, advanced; `useAppStore` for update state; sync/trigger from backend. |
| **useTheme** | Theme mode + accent color (persisted). |
| **useDistro** | Distro id, pretty_name, capabilities (e.g. chaotic_aur_support); used for repo locks and Hero badge. |
| **usePackageMetadata** | Cached metadata (icon, app_id, etc.) per package. |
| **useRatings** / **usePackageRating** / **usePackageReviews** | ODRS/local ratings and reviews. |
| **useOnlineStatus** | Offline banner on Home. |
| **useInfiniteScroll** | Intersection observer for "load more" (CategoryView). |

### 3.5 Store (Zustand)

- **internal_store:** `trendingPackages`, `infraStats`, `loadingTrending`/`loadingStats`, `telemetryEnabled`, `error`; update state: `isUpdating`, `updateProgress`, `updateStatus`, `updatePhase`, `updateLogs`, `rebootRequired`, `pacnewWarnings`. Actions: `fetchTrending`, `fetchInfraStats`, `checkTelemetry`, `setTelemetry`, and all `set*` for update state.

---

## 4. Backend (Tauri Commands)

### 4.1 Command Groups

- **system.rs:** `get_system_info`, `get_all_installed_names`, `get_infra_stats`, `get_repo_counts`, `get_repo_states`, `is_aur_enabled`, `toggle_repo`, `set_aur_enabled`, `is_one_click_enabled`, `set_one_click_enabled`, `check_security_policy`, `install_monarch_policy`, `optimize_system`, `trigger_repo_sync`, `update_and_install_package`, `is_advanced_mode`, `set_advanced_mode`, `check_app_update`, `is_telemetry_enabled`, `set_telemetry_enabled`, `get_install_mode_command`.
- **package.rs:** `copy_paths_to_monarch_install`, `abort_installation`, `install_package`, `uninstall_package`, `build_aur_package`, `fetch_pkgbuild`, `get_installed_packages`, `check_for_updates`, `get_orphans`, `remove_orphans`, `check_installed_status`, `get_essentials_list`, `check_reboot_required`, `get_pacnew_warnings`.
- **update.rs:** `perform_system_update`.
- **search.rs:** `search_packages`, `search_aur`, `get_packages_by_names`, `get_trending`, `get_package_variants`, `get_category_packages_paginated`.
- **utils.rs:** `get_package_icon`, `clear_cache`, `launch_app`, `track_event`.
- **reviews.rs:** `submit_review`, `get_local_reviews`.

### 4.2 Install / Update Flow (Backend)

- **Install:** `install_package` → resolves repo/variant, then `install_package_core`; for non-AUR uses helper (temp-file command); for AUR, `build_aur_package` (unprivileged makepkg, then privileged `pacman -U`). Events: `install-output`, `alpm-progress`, `install-complete`, `install-error-classified` (when classified).
- **Uninstall:** `uninstall_package` → helper or direct pacman `-Rns`.
- **System update:** `perform_system_update` → helper `Sysupgrade`; progress via `update-progress`, completion via `update-complete`.
- **Update and install:** `update_and_install_package` → Sysupgrade then AlpmInstall for the named package (see INSTALL_UPDATE_AUDIT.md).

Helper path: production `/usr/lib/monarch-store/monarch-helper` preferred; dev fallback to target binary. Command passed via temp file to avoid argv truncation.

---

## 5. Feature-by-Feature Audit

### 5.1 Settings

- **Sections:** System health (connectivity, sync pipeline, integrity), Repository Control (sync now, repo counts, auto sync interval), Software Sources (per-repo toggle + order, Chaotic lock on Manjaro, AUR toggle), Workflow (notifications, re-run wizard), Appearance (theme, accent), System Management (one-click, repair: unlock, keyring, clear cache, orphans), Privacy (telemetry toggle), Advanced (distro-safety bypass), About (version, install mode, check updates).
- **Edge cases:** Repo locked by distro shows "Blocked by {distro}"; sync status per repo; modal config reused for confirmations.

### 5.2 Explore (Home)

- **Content:** HeroSection, Recommended Essentials (useSmartEssentials → TrendingSection with filterIds), Trending Applications (get_trending, limit 7), CategoryGrid.
- **Offline:** HomePage shows amber "No Internet Connection" when `!useOnlineStatus()`.
- **See All:** Sets `viewAll` to 'essentials' or 'trending'; same TrendingSection with no limit or ESSENTIALS_POOL.

### 5.3 Search

- **Trigger:** Typing in SearchBar (debounced) or clicking "Search" tab (focus input).
- **Pre-search:** Recent searches (useSearchHistory), Quick Filters (top:trending, top:new), From Favorites chips.
- **Magic keywords:** `@aur`, `@chaotic`, `@official` set filter chip automatically.
- **Results:** Sort (Relevant / Name / Newest), source chips (All + per enabled repo family), "Did you mean?" aliases (e.g. word → LibreOffice), Load More +50.
- **Empty:** EmptyState with optional alias suggestion or "Clear filters & search again".

### 5.4 Install Flow (UI)

- **Entry:** PackageDetails "Install" or card Download button → `setActiveInstall({ name, source, repoName, mode: 'install' })`.
- **InstallMonitor:** Auto-starts via effect with `actionStartedForRef` guard. Steps: Safety → Downloading → Installing → Finalizing. Listens to `alpm-progress`, `install-output`, `install-complete`, `install-error-classified`. Progress bar with pseudo-progress when target stuck. Logs toggle (persisted in localStorage). Success: "Launch Now" + Close. Error: keyring/lock repair buttons or classified recovery (Unlock & Retry, Repair Keys, etc.). "Update & Install" when backend sends `failed_update_required`.
- **Uninstall:** Same modal with `mode: 'uninstall'`; confirm in PackageDetails or InstalledPage (ConfirmationModal there).

### 5.5 App Pages (Package Details)

- **Data:** Variants from `get_package_variants` + pkg.alternatives; installed status from `check_installed_status`; selected source drives install/uninstall.
- **UI:** Back button, icon, name, source badges, Install/Uninstall/Launch, RepoSelector (variants), description, screenshots (lightbox), reviews (submit + list), PKGBUILD tab, "Update & Install" when package not found (navigates to InstallMonitor flow).
- **Favorites:** Heart toggle via useFavorites.

### 5.6 App Cards (PackageCard)

- **Display:** Icon (resolveIconUrl + fallback arch logo), display_name or name, version dropdown if multiple variants, description (line-clamp-2), source badge (chaotic/official/aur/other), is_optimized badge, rating (usePackageRating), favorite + download buttons (download triggers parent onClick → details then install).
- **Variants:** In-card selector updates local `displayPkg`; does not change install target until user goes to details and installs.

### 5.7 Installed Section

- **List:** `get_installed_packages` (name, version, size, install_date, description). Local filter by search string. AppIcon: `get_package_icon` else `get_metadata` icon_url, else arch logo.
- **Actions:** Launch (desktop entry), Uninstall (ConfirmationModal), row click → details via `get_packages_by_names` or `search_packages` fallback.
- **Empty:** "No applications found" with icon.

### 5.8 Updates

- **List:** `check_for_updates` → PendingUpdate (name, old_version, new_version, repo). Reboot hint if linux/nvidia in list.
- **Update All:** ConfirmationModal; optional password for AUR; `perform_system_update` fire-and-forget; progress from store/events; "Fix It" for lock/busy (repair_unlock_pacman). Reboot/pacnew banners after completion.
- **Steps:** Synchronizing Databases → Upgrading System → Updating Community Apps (derived from statusMessage).

### 5.9 Favorites

- **Storage:** LazyStore `favorites.json` (array of package names).
- **Page:** "Favorites" heading; if empty, empty state with heart icon; else TrendingSection with `filterIds={favorites}`, limit 100.
- **Toggle:** PackageCard heart and PackageDetails; `toggleFavorite` updates store and UI.

---

## 6. Constants & Config

- **ESSENTIALS_POOL** / **ESSENTIAL_IDS:** Static pool and rotated subset (by week) for homepage essentials.
- **CATEGORIES:** CategoryGrid + CategoryView; id, label, description, popular apps, icon, colors.
- **Search aliases:** SearchPage "Did you mean?" map (e.g. word → LibreOffice).

---

## 7. Findings & Recommendations

### 7.1 Consistency / Minor

- **InstalledPage** AppIcon uses `get_metadata` (exposed from `metadata.rs`); `get_package_icon` from utils — both correct.
- **PackageCard** Download button only navigates to details; install is from details. Clear for UX but could add tooltip "View details to install".
- **Search** "top:trending" / "top:new" — backend must interpret these or they act as literal query; verify behavior.
- **Sidebar** update count is from single `check_for_updates` on mount; not refreshed when leaving Updates page.
- **modalConfig** in Settings reuses one object; `onConfirm` can be stale if set multiple times quickly (rare).

### 7.2 Robustness

- **Error boundaries:** ErrorBoundary present; ensure all async paths surface errors (toast or inline) where appropriate.
- **Offline:** Home and Explore show banner; search/install may fail; no global offline queue.
- **InstallMonitor** duplicate `if (!pkg) return null;` (cosmetic).
- **useSmartEssentials** fallback to ESSENTIAL_IDS on error; backend `get_essentials_list` should match ESSENTIALS_POOL semantics.

### 7.3 Security / Rules (from AGENTS.md)

- No `pacman -Sy` alone; all repo installs `pacman -Syu --needed`; system update single `pacman -Syu`.
- Package name validated in backend before shell.
- AUR: unprivileged makepkg, only `pacman -U` privileged.
- Mutex: `if let Ok(guard) = mutex.lock()` preferred over `.unwrap()`.

### 7.4 Recommendations

1. **Sidebar:** Refresh update count when navigating to Updates or after an update completes.
2. **Search:** Document or implement `top:trending` / `top:new` in backend so Quick Filters behave as expected.
3. **InstalledPage:** Consider syncing "installed" list after uninstall from PackageDetails (or refetch on focus) so list stays in sync.
4. **Package details:** Consider showing "Installing…" or disabling Install when `installInProgress` is true (already passed as prop).
5. **Accessibility:** Ensure all modals and key actions are keyboard-accessible (focus trap, Escape to close).
6. **i18n:** No localization; all strings are English.

---

## 8. File Reference (Quick)

| Area | Files |
|------|--------|
| App shell | `src/App.tsx`, `src/App.css` |
| Pages | `src/pages/HomePage.tsx`, `SearchPage.tsx`, `InstalledPage.tsx`, `UpdatesPage.tsx`, `SettingsPage.tsx`, `PackageDetailsFresh.tsx`, `CategoryView.tsx` |
| Key components | `src/components/InstallMonitor.tsx`, `PackageCard.tsx`, `Sidebar.tsx`, `SearchBar.tsx`, `HeroSection.tsx`, `TrendingSection.tsx`, `CategoryGrid.tsx` |
| Hooks | `src/hooks/useFavorites.ts`, `useSettings.ts`, `useSmartEssentials.ts`, `useSearchHistory.ts`, `useTheme.ts`, `useDistro.ts` |
| Store | `src/store/internal_store.ts` |
| Constants | `src/constants.ts`; `src/components/CategoryGrid.tsx` (CATEGORIES) |
| Backend | `src-tauri/monarch-gui/src/commands/*.rs`, `helper_client.rs` |
| Install/Update audit | `docs/INSTALL_UPDATE_AUDIT.md` |

---

This audit reflects the codebase as of the audit date. For install/update behavior, Polkit, and helper path details, see **INSTALL_UPDATE_AUDIT.md**.

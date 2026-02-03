# Release Notes

**Current version:** v0.4.0-alpha

---

# Monarch Store Release Notes

## v0.4.0-alpha (Universal MonARCH)
**"The Host-Adaptive Update"** — v0.4.0-alpha: The Universal Update. Removes the need for manual repo configuration; repositories are discovered from your system's `pacman.conf`.

*   **Mission Control (New Settings)**: A tabbed settings overhaul for better management of Sources, AUR Builder, and System Maintenance.
*   **Unified Update System**: Parallel-checks Repo/AUR/Flatpak. Enforces full system upgrade (`-Syu`) if any official package is updated (Safety Lock). Added "Built from Source" labels for AUR clarity.
*   **Legacy Code Audit**: Successfully removed all "Ghost Commands" and legacy contexts (`RepoStatusContext`, `check_repo_status`) to ensure 100% runtime stability.
*   **Native AUR Builder**: Replaced `yay` wrapper with a native, user-level builder using `libgit2` and `tokio`. Build logs now stream to the UI.
*   **Flatpak Integration**: Full support for installing, removing, and updating Flatpak applications as first-class citizens.
*   **Manjaro Guard**: Automatically blocks `chaotic-aur` on Manjaro to prevent `glibc` breakage.
*   **Silent Guard (Atomic Batch Transactions)**: Complex operations now prompt for a password **at most once**. Polkit authorization is remembered for 5 minutes.

## v0.3.5-alpha

*   **The "Iron Core" Update**: Introduced `SafeUpdateTransaction` for atomic, robust package installation.
rict "Atomic Update Protocol". We now check for locks and enforcement a full system upgrade (`-Syu`) for *every* sync transaction, ensuring zero "partial upgrade" breakages.
- **Custom Title Bar & Permissions:** Integrated a premium client-side decoration (CSD) title bar. Fixed window control functionality (Minimize/Maximize/Close) and enabled backend permissions for the Tauri Store plugin.
- **Wayland Ghost Protocol:** Fixed black flickering/artifacts on KDE Plasma (especially Nvidia) by intelligently detecting `WAYLAND_DISPLAY` and disabling transparency effects.
- **The Chameleon (Native Themes):** Now uses XDG Portals (`ashpd`) to detect system Dark/Light mode correctly on all desktops (GNOME, KDE, Hyprland), ignoring legacy GTK theme signals.
- **Native Dialogs:** Portal-based file pickers (`rfd`) are planned; dependency added. Theme detection uses XDG Portals (`ashpd`).

---

# Release Notes v0.3.6-alpha (Reliability & Polish)

## Latest (2026-02-01)
- **Safe Update Transaction (Iron Core):** Implemented strict "Atomic Update Protocol". We now check for locks and enforcement a full system upgrade (`-Syu`) for *every* sync transaction, ensuring zero "partial upgrade" breakages.
- **Custom Title Bar & Permissions:** Integrated a premium client-side decoration (CSD) title bar. Fixed window control functionality (Minimize/Maximize/Close) and enabled backend permissions for the Tauri Store plugin.
- **Wayland Ghost Protocol:** Fixed black flickering/artifacts on KDE Plasma (especially Nvidia) by intelligently detecting `WAYLAND_DISPLAY` and disabling transparency effects.
- **The Chameleon (Native Themes):** Now uses XDG Portals (`ashpd`) to detect system Dark/Light mode correctly on all desktops (GNOME, KDE, Hyprland), ignoring legacy GTK theme signals.
- **Native Dialogs:** Portal-based file pickers (`rfd`) are planned; dependency added. Theme detection uses XDG Portals (`ashpd`).

---

# Release Notes v0.3.5-alpha

## Latest (2025-01-31)
- **Security (Fort Knox):** Helper restricts `WriteFile`/`WriteFiles` to `/etc/pacman.d/monarch/` only; command file must be owned by invoking user when using pkexec; 800 ms debounce on helper invokes. See [SECURITY_AUDIT_FORT_KNOX](docs/SECURITY_AUDIT_FORT_KNOX.md).
- **Telemetry:** Aptabase tracking verified; `onboarding_completed` and `uninstall_package` events added; privacy toggle correct in onboarding and settings; store `checkTelemetry` uses error service.
- **Error reporting:** Error service used app-wide (App, Settings, Onboarding, InstallMonitor, store, hooks, RepoStatusContext, main); no `console.error` in critical paths.
- **Double Password Prompt Fix:** Resolved a race condition in the session password dialog that caused the backend to receive an empty password, triggering an unnecessary system prompt.
- **Installation Resilience:** Installation engine now gracefully handles missing sync databases (e.g. after a force refresh) by skipping pre-flight checks and letting the main sync transaction handle it.
- **CI/CD Reliability:** Fixed the GitHub Action build pipeline (`tauri-action`) to correctly handle the nested project structure and ensure frontend assets are built before packaging.
- **Robust Installations (CRITICAL):** Fixed "Package not found" errors by replacing the manual config parser with `pacman-conf`. The helper now sees exactly what Pacman sees.
- **Safety First:** Implemented "Smart Retry" logic. If an install fails due to stale databases (404s), Monarch now automatically syncs AND performs a full system upgrade (`pacman -Syu`), preventing dangerous partial upgrades.
- **AUR Refactor:** Switched to the `raur` crate for faster, async-native AUR searches.
- **APIs & clean-up:** Typed `get_cache_size`/`get_orphans_with_size`; Rust logging and unwrap hardening; frontend `AppState` typing; docs cleaned and Fort Knox linked.

## v0.3.5-alpha (base)
- **AppStream:** `monarch-store.metainfo.xml` (com.monarch.store, developer cpg716, OARS 1.1).
- **Accessibility:** Escape key and focus trap on all modals (Onboarding, Confirmation, InstallMonitor, RepoSetup, Error, Auth, PKGBUILD, lightbox).
- **Atomic sync:** No naked `pacman -Sy`; all paths use `-Syu` / `-Syu --needed` (see [INSTALL_UPDATE_AUDIT](docs/INSTALL_UPDATE_AUDIT.md)).
- **Author:** cpg716 as developer/creator (with AI coding tools) in metainfo, package.json, README, PKGBUILD.
- **Distribution:** PKGBUILD pkgdesc < 80 chars; release tarball + checksums via `scripts/release-finalize-pkgbuild.sh` after tag push (see [RELEASE_PUSH_STEPS](docs/RELEASE_PUSH_STEPS.md)).
- **Omni-User (v0.3.5):** Self-healing (silent DB repair and auto-unlock during install; no error pop-up for corrupt DB or locked DB). **Startup unlock:** At launch the app calls `needs_startup_unlock()`; if a stale lock exists it runs `unlock_pacman_if_stale` (via Helper RemoveLock). When **Reduce password prompts** is on, the in-app password is used so the system prompt does not appear at launch. **Install cancel:** InstallMonitor Cancel button and close-with-warning; `cancel_install` stops the helper and clears the lock. Glass Cockpit: **Show Detailed Transaction Logs** (Settings → General), **Advanced Repair** (Unlock DB, Fix Keys, Refresh DBs, Clear Cache, Clean Orphans) and **Test Mirrors** per repo (Settings → Repositories; top 3 mirrors with latency via rate-mirrors/reflector). Helper `force_refresh_sync_dbs` reads `/etc/pacman.conf` directly; bootstrap `pacman -Syy` at end of repo_setup. Friendly errors (ALPM_ERR_DB_WRITE → "Auto-unlocking…" with expert view); session password passed to repair invokes. `.gitignore`: added `target` for Cargo build output.

---

# Release Notes v0.3.00-Alpha1 (The "Universal" Update)

> **"The first Distro-Aware App Manager for Arch, Manjaro, and CachyOS."**

## 🚀 Rebranding: Universal Manager
MonARCH is now the **Universal Arch Linux App Manager**. We have transitioned from a simple "Store" to a context-aware system utility that adapts its safety rails based on your specific distribution.

## 🛡️ Distro-Aware Intelligence
*   **Manjaro Stability Guard**: Automatically hides bleeding-edge Arch repos (Chaotic-AUR) on Manjaro systems to prevent library mismatch errors.
*   **CachyOS Performance Mode**: Detects AVX2/AVX-512 CPUs and prioritizes v3/v4 repositories for 10-20% faster apps.
*   **Arch Power Mode**: Unlocks full access to all repos for vanilla Arch users.

## ✨ Luminosity UI Engine
*   **Glassmorphism**: A complete UI rewrite featuring blurred backgrounds, "Ghost Text" headers, and premium topography.
*   **Responsive Stacking**: The "App Details" view now intelligently stacks metadata on mobile while expanding to a 2-column layout on desktop.
*   **Skeleton Loading**: Smoother transitions with shimmer effects replaces jarring spinners.

*   **70% Faster Startup**: Parallel ODRS rating fetches mean the homepage loads instantly.
*   **Smart Sync**: The installer uses the Helper for all ALPM write operations. We never run `pacman -Sy` alone; repo installs use `pacman -Syu --needed` in one transaction.
*   **Offline Mode**: A new global "Offline Guard" prevents crashes when the internet cuts out, serving cached data gracefully.

---


# Release Notes v0.3.00-Alpha1 - The "Butterfly" Update

## 🦋 Major Architectural Overhaul
- **Hardware-Aware Backend**: Implemented `check_requirements()` to ensure system binaries (`git`, `pkexec`) are healthy at boot.
- **Luminosity UI Engine**: Complete redesign of the App Details experience (`PackageDetailsFresh.tsx`) featuring top-aligned metadata, glassmorphism, and high-density layouts.
- **Parallel ODRS Integration**: Ratings and reviews now fetch concurrently, resulting in a ~70% speed boost on the home page.

## 📱 Responsive & Visual Mastery
- **Horizontal Mobile Header**: App title and logo now stay side-by-side even in the smallest windows.
- **Scroll-to-Reviews**: Clicking the Ratings box instantly smooth-scrolls to the user opinions.
- **Button Unity**: Action buttons now group intelligently to prevent isolated wrapping.

---

# Release Notes v0.2.40 - The "Zero-Config" Update

## 🛑 Runtime Safety & Integrity
- **Zero-Config Guarantee**: We audited the entire dependency chain. `PKGBUILD` now strictly enforces all requirements (`openssl`, `git`, `polkit`).
- **Self-Healing Startup**: The app now self-diagnoses missing binary tools (`git`, `pkexec`) at launch to prevent silent failures.
- **Polkit Standardization**: Security policies are now installed from a single "Source of Truth," ensuring password-less package management works out of the box on all distributions.

## 🌐 Data & Network Resilience
- **Ratings Fixed**: Solved the "Missing Stars" issue for popular apps (Discord, VLC, GIMP, Lutris) by implementing a manual ODRS ID translation layer.
- **Offline Safety**: Improved error handling when the ODRS API is down (like during the major outage of Jan 2026).

## 🎨 Visual Refinements
- **Responsive Layouts**: Cards no longer get "smushed" on window resize. We implemented a robust `minmax` grid system.
- **Small Screen Support**: Fixed the "Cut Off" content issue on smaller laptops by moving the main scroll container to the top level.
- **Search Grid**: Search results now respect the same adaptive layout rules as the rest of the app.

---

# Release Notes v0.2.30

# Release Notes v0.2.24
- **Icon Restoration**: Fixed missing icons for Brave, Spotify, and Chrome by restoring the robust fallback chain (checking upstream sources when local metadata fails).
- **Search Accuracy**: "Spotify" now finds the main app first! We improved search sorting to prioritize exact matches over launchers or plugins.
- **Linux Native Power**: Full support for system icons (`/usr/share/pixmaps`) and local AppStream caching on Linux devices.

---

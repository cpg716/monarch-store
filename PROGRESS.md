# 📈 MonARCH Progress Report

**Last updated:** 2025-01-31 (v0.3.5-alpha)

## 🏆 Recent Achievements

### Latest (2025-01-31) — Security, telemetry, error service & docs
- **Fort Knox security audit:** [docs/SECURITY_AUDIT_FORT_KNOX.md](docs/SECURITY_AUDIT_FORT_KNOX.md). Helper no longer allows `WriteFile`/`WriteFiles` to `/etc/pacman.conf` (only `/etc/pacman.d/monarch/`). Command file ownership check (file uid must equal `PKEXEC_UID`) when reading from path. `invoke_helper` debounce (800 ms) to limit rapid helper invocations. Makepkg root refusal clarified in code comments.
- **Aptabase / telemetry:** Privacy setting verified in onboarding and settings. `checkTelemetry` in store uses `getErrorService()`; onboarding finish and uninstall now send `onboarding_completed` and `uninstall_package` events (backend-gated). OnboardingModal `handleFinish` uses `errorService.reportError` and persists telemetry choice.
- **Error service wiring:** `getErrorService()` added for use outside React tree. All `console.error`/`.catch(console.error)` replaced with `errorService.reportError` or `reportWarning` in App, SettingsPage, OnboardingModal, SystemHealthSection, InstallMonitor, CategoryView, PackageDetailsFresh, ErrorModal, internal_store, useSettings, RepoStatusContext; `main.tsx` uses `reportCritical` for `window.onerror`.
- **Typed APIs:** `get_cache_size` and `get_orphans_with_size` return typed structs (`CacheSizeResult`, `OrphansWithSizeResult`); SettingsPage uses typed `invoke` interfaces.
- **Deep clean:** Rust `println!`→`log::*`; commented debug removed; `unwrap()` on mutex/parse replaced with `expect`/`map_err` where appropriate; frontend `useAppStore` uses `AppState`; modal z-index standardized to `z-50`.
- **Docs:** Obsolete audit/gate docs removed; Fort Knox linked from SECURITY, README, DOCUMENTATION; RELEASE_NOTES and GITHUB_RELEASE_TEMPLATE links updated; DEVELOPER doc index updated.

### v0.3.5-alpha (Release readiness & Omni-User)
- **AppStream:** Production `monarch-store.metainfo.xml` with `com.monarch.store`, developer cpg716, OARS content rating.
- **Keyboard sovereignty:** Escape key and focus trap on all modals (including Auth and PKGBUILD).
- **Atomic sync:** Full audit; no naked `pacman -Sy` in repair, repo_setup, or monarch-helper.
- **Author credits:** cpg716 listed as developer/creator (with AI coding tools) in metainfo, package.json, README, PKGBUILD.
- **Release script:** `scripts/release-finalize-pkgbuild.sh` and [RELEASE_PUSH_STEPS](docs/RELEASE_PUSH_STEPS.md) for tarball + checksums after tag push.
- **Omni-User (dual-core UX):** Self-healing (silent DB repair and auto-unlock during install), Glass Cockpit (verbose transaction logs, Advanced Repair dropdown, Test Mirrors per repo with latency). **Startup unlock:** At launch the app calls `needs_startup_unlock()`; if a stale lock exists it runs `unlock_pacman_if_stale` (via Helper RemoveLock). When **Reduce password prompts** is on, the in-app password is used so the system prompt does not appear at launch. **Install cancel:** InstallMonitor Cancel button and close-with-warning; `cancel_install` stops the helper and clears the lock. Helper `force_refresh_sync_dbs` reads `/etc/pacman.conf` directly; bootstrap `pacman -Syy` moved to end of repo_setup. `friendlyError.ts` ALPM_ERR_DB_WRITE → "Auto-unlocking…" with expertMessage; session password passed to repair invokes.

### Install & Update Reliability
- **Temp-file command**: Helper receives command via temp file (path in argv) to avoid "Invalid JSON" and argv truncation.
- **Single invocation**: InstallMonitor uses ref guard so install runs once per package (no double password prompt from React Strict Mode).
- **Production helper path**: GUI prefers `/usr/lib/monarch-store/monarch-helper` when present so Polkit policy path matches; passwordless installs work when rules are installed.
- **Update-and-install**: `update_and_install_package` now runs Sysupgrade then AlpmInstall for the named package (previously only Sysupgrade).
- **Update All (AUR filter)**: `perform_system_update` runs Sysupgrade (repos), then AUR updates only for packages **not** in any sync repo (`is_in_sync_repos`); packages available in Chaotic/CachyOS etc. are skipped for AUR build.
- **Polkit rules**: `10-monarch-store.rules` includes `com.monarch.store.package-manage`; `install_monarch_policy` copies rules to `/usr/share/polkit-1/rules.d/`. See [Install & Update Audit](docs/INSTALL_UPDATE_AUDIT.md).

### 🦋 Butterfly Engine (Backend)
- **Startup Integrity**: Verified runtime environment (`git`, `polkit`, `pkexec`) at launch.
- **Parallel Rating Delivery**: ODRS and metadata fetched in parallel for faster home load.

### 🎨 Frontend & Docs
- **Full App Audit**: [docs/APP_AUDIT.md](docs/APP_AUDIT.md) documents UI/UX, all pages, components, hooks, store, backend, and feature areas.
- **Stack**: React 19, TypeScript, Tailwind CSS 4, Vite 7, Zustand, Framer Motion.

---

## 🚧 Current Work
- [ ] **Flathub metadata**: Flathub API used for icons/descriptions/reviews for AUR and official packages (metadata only; we do not add Flatpak app support).
- [ ] **MonARCH Plugin API**: Designing the interface for community repair scripts.

---

## 🗺️ Future Roadmap
- **v0.4.x**: Theme Engine (MonARCH Accent palettes).
- **v1.0.x**: External Plugin API for community-contributed repair scripts.

## 📋 Future work (documented)
- **Expose Helper ClearCache in Settings — done.** Settings → Maintenance "Clear Cache" now runs in-memory `clear_cache` then Helper `clear_pacman_package_cache` (disk `/var/cache/pacman/pkg`). See [docs/OMEGASCOPE_PREFLIGHT_REPORT.md](docs/OMEGASCOPE_PREFLIGHT_REPORT.md).

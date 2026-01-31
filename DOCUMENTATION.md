# 📘 MonARCH Store - Technical Documentation

**Last updated:** 2025-01-31 (v0.3.5-alpha)

**Developers:** See [Developer documentation](docs/DEVELOPER.md) for setup, project structure, code style, and critical package-management rules.

## Architecture Overview: Infrastructure 2.0

MonARCH Store uses a **"Soft Disable"** architecture to balance User Experience with System Safety.

### 🛡️ System Layer (The Rock)
*   **Always On**: During Onboarding, the app configures `/etc/pacman.conf` to enable **all** supported repositories (`cachyos`, `garuda`, `chaotic`, etc.).
*   **Fail Safe**: Because `pacman` sees everything, running `pacman -Syu` (System Update) will **always** find updates for your installed apps, even if you "Hid" the source in the Store.
*   **No Password Fatigue**: Since the system is pre-configured once, toggling repos in the UI does not require root/checksum triggers.
*   **GPG Resilience**: Automatically syncs keys to both system and user keyrings, fixing "Invalid Signature" errors for both the app and manual terminal builds.

### 🖥️ Frontend Layer (The View)
*   **Soft Toggles**: Disabling a repo in `Settings` adds it to a "Hidden" list in `repos.json` and instantly clears it from the search cache.
*   **Result**: The Store stops showing *new* packages from that source, but your *existing* packages remain safe.

## Key Features & Logic

### 1. Unified Search (Chaotic-First)
When you search for "firefox", MonARCH aggregates results from all enabled sources but prioritizes instant binaries:
1.  **Hardware Optimized** (Priority #0): `cachyos-v3` / `v4` (if CPU supported).
2.  **Chaotic-AUR** (Priority #1): Pre-built binary. Fastest install.
3.  **Official Repos** (Priority #2): Standard Arch package.
4.  **AUR** (Priority #3): Source build (fallback).

*Users can manually override this choice using the "Download Source" dropdown in the package details.*

### 2. Update Consistency
We strictly enforce **"Update All"** via `perform_system_update` (Helper command `Sysupgrade`).
*   **Why?**: Arch Linux does not support partial upgrades. Updating one app without system libraries (`glibc`) can break the OS.
*   **Mechanism**: (1) The GUI invokes the Helper with `Sysupgrade`; the Helper runs a single full upgrade (sync + transaction). (2) Then we check for **AUR-only** updates: foreign packages (`pacman -Qm`) with a newer AUR version are filtered so that any package **in a sync repo** (e.g. Chaotic, CachyOS) is skipped; only truly AUR-only packages are built with makepkg and installed. We **never** run `pacman -Sy` alone. See [Install & Update Audit](docs/INSTALL_UPDATE_AUDIT.md).

### 3. Hybrid Review System
We use a composite rating strategy:
*   **Step 1:** Check **ODRS** (Open Desktop Rating Service).
*   **Step 2:** Fallback to **Supabase** community reviews.
*   **Display:** Merged 5-star rating.

### 4. Butterfly System Health & Omni-User (v0.3.5)
MonARCH includes a permission-aware health monitoring ecosystem and a **dual-core** UX: simplicity by default, power by choice.
*   **Butterfly Probes**: Verifies environmental requirements (`git`, `pkexec`, `polkit`) at startup.
*   **Parallel Rating Fetches**: ODRS and metadata fetched in parallel for faster home screen load.
*   **Permission-Safe Sensors**: Health checks are non-privileged, preventing false "Corrupted Keyring" warnings.
*   **Unified Repair Wizard**: A single authorized maintenance flow for Keyring, Security Policies, and Repo sync.
*   **Self-Healing**: On corrupt sync DBs or locked DB during install, the app triggers force refresh or unlock and retries without showing an error pop-up; user sees "Repairing databases…" or "Auto-unlocking…". **At startup**, the app calls `needs_startup_unlock()`; if a stale lock exists it runs `unlock_pacman_if_stale` (via Helper RemoveLock). When **Reduce password prompts** is on, the in-app password is used so the system prompt does not appear at launch. **Install cancel:** InstallMonitor has a Cancel button and close-with-warning; `cancel_install` stops the helper and clears the lock. Helper `force_refresh_sync_dbs` reads `/etc/pacman.conf` (and monarch includes) directly so recovery works even when ALPM is blind.
*   **Glass Cockpit**: Settings → General: **Show Detailed Transaction Logs** (InstallMonitor shows real-time pacman/makepkg stdout). Settings → Maintenance: **Advanced Repair** (Unlock DB, Fix Keys, Refresh DBs, Clear Cache, Clean Orphans) and **Test Mirrors** per repo (`test_mirrors(repo_key)` → top 3 mirrors with latency via rate-mirrors/reflector).
*   **Friendly errors**: `friendlyError.ts` maps ALPM/DB errors to user-facing messages and optional **expertMessage** for raw output.
*   **Optional single-password mode**: Settings → Workflow & Interface offers **Reduce password prompts**. When enabled, the user can enter their password once in a MonARCH dialog; it is used for installs and repairs for the session (~15 min), not persisted. This sends the password to the app and is less secure than using the system (Polkit) prompt each time; the default is system prompt every time.
*   **Error service:** Centralized error reporting (`ErrorContext` / `getErrorService()`) used app-wide; critical errors surface in ErrorModal, others as toasts; optional Aptabase `error_reported` when telemetry is enabled.
*   **Telemetry (Aptabase):** Opt-in anonymous usage stats; gated in backend (`track_event_safe`); onboarding and Settings both persist the privacy toggle; events include app_started, search, install/uninstall, review_submitted, onboarding_completed, error_reported.
*   **Security (Fort Knox):** Helper restricts writes to `/etc/pacman.d/monarch/`; command file ownership checked when using pkexec; helper invoke debounce to limit DoS. See [SECURITY_AUDIT_FORT_KNOX](docs/SECURITY_AUDIT_FORT_KNOX.md).

### 5. Frontend (Luminosity Visual Engine)
*   **Stack**: React 19, TypeScript, Tailwind CSS 4, Vite 7, Zustand, Framer Motion. See [APP_AUDIT](docs/APP_AUDIT.md) for full UI/UX and feature reference.
*   **Layout**: Glassmorphic backdrops, responsive grids, scroll-to-reviews, repo selector in package details.

## 🛠️ Build & Release

To cut a new release:
1.  Update `version` in `package.json`, `src-tauri/monarch-gui/tauri.conf.json`, and both `src-tauri/monarch-gui/Cargo.toml` and `src-tauri/monarch-helper/Cargo.toml` (e.g. `0.3.5-alpha`).
2.  Clean build: `npm run tauri build` (from repo root). Rust: `cd src-tauri && cargo check` for backend check.
3.  Tag and push: `git tag -a v0.3.5_alpha -m "Release message"` then `git push origin main && git push origin v0.3.5_alpha`.

## ☁️ Backend Configuration (Self-Hosting Community Reviews)

To enable **Community Reviews** with your own backend:
1.  Create a **Supabase** project.
2.  Run the provided SQL setup script (see `src/services/reviewService.ts`).
3.  Update env vars with your **Project URL** and **Anon Key**.

## See also

- [Developer documentation](docs/DEVELOPER.md) — setup, structure, code style, build commands (including why `tauri dev` pre-builds monarch-helper).
- [Troubleshooting](docs/TROUBLESHOOTING.md) — build stall at 711/714, "Command get_chaotic_packages_batch not found", GPG, lock, mirrors.
- [Fort Knox Security Audit](docs/SECURITY_AUDIT_FORT_KNOX.md) — security and Arch compliance (root barrier, helper, pacman.conf, rate limiting).
> Historical note: This review document describes a legacy or point-in-time parity snapshot. GTK current-state and release-gate status now live in `docs/GTK_TAURI_PARITY_MATRIX.md` and the GTK-first root docs.

# Startup and Permissions Handling Review

**Date:** 2026-02-05  
**Scope:** App startup sequence, loading screen, unlock flow, Tauri capabilities, helper invocation (Polkit/sudo), session password, and security policy checks.

---

## 1. Startup Flow

### 1.1 Entry and providers (`src/main.tsx`)

- **Order:** `ErrorBoundary` → `ToastProvider` → `ErrorProvider` → `SessionPasswordProvider` → App (or `LoadingScreen` when `?screenshot=loading`).
- **Global handler:** `window.onerror` reports to `getErrorService()?.reportCritical()` so uncaught errors are surfaced after `ErrorProvider` mounts.
- **Screenshot mode:** If URL has `?screenshot=loading`, only `LoadingScreen` is rendered (no Tauri required); used for README/assets.

### 1.2 App state and loading gate (`src/App.tsx`)

- **`isRefreshing`** starts as `true`; app shows **`LoadingScreen`** until startup completes.
- **`if (isRefreshing) return <LoadingScreen />`** — no main UI (sidebar, content, modals) is shown during refresh.
- **Startup** runs inside a `useEffect` that calls `initializeStartup()` once (deps: `fetchInfraStats`, `errorService`, `isFlatpakEnabled`).

### 1.3 `initializeStartup()` sequence

1. **Unlock (0)**  
   - `needs_startup_unlock()` → backend checks `/var/lib/pacman/db.lck` exists and no `pacman` process (pgrep -x pacman).  
   - If unlock needed:
     - **Reduce password prompts on:** `requestSessionPassword()` (modal), then `unlock_pacman_if_stale({ password })` (Helper `ExecuteBatch` with `remove_lock: true` via sudo -S).
     - **Off:** `unlock_pacman_if_stale()` with no password (Polkit/pkexec).
   - Failures are reported with `reportWarning`; startup continues.

2. **Parallel / background (1)**  
   - `fetchInfraStats()`, `checkTelemetry()`, `get_repo_states()` (result → `setEnabledRepos`).  
   - No blocking of the rest of startup.

3. **Health and onboarding (2)**  
   - `check_initialization_status()` → `status` (policy, keyring, migration, sync DB, `is_healthy`, `reasons`).  
   - `setSystemHealth(status)`.  
   - Completion: `monarch_onboarding_v3` and legacy keys.  
   - **Grandma-proof:** If only issue is sync DB repair → `setPendingDbRepair(true)` (one-step overlay later).  
   - If other defects → `setOnboardingReason`, `setShowSystemFixPopup(true)`, `redoOnboarding = true`.  
   - If `redoOnboarding` → `setShowOnboarding(true)`.

4. **Post-decision (3)**  
   - If not redo-onboarding: optional sync (refresh requested or sync-on-startup + stale DB), then prewarm essentials and trending.  
   - All in same async flow; exceptions go to `reportError`.

5. **Finally**  
   - `setTimeout(() => setIsRefreshing(false), Math.max(0, 1500 - elapsed))` so the loading screen stays at least **1.5 s** (smoother UX).  
   - After that, main UI renders; onboarding or DB-repair overlay show based on state.

### 1.4 Backend setup (`src-tauri/monarch-gui/src/lib.rs`)

- **Required bins:** `which::which` for `git`, `checkupdates`, `pkexec`. On missing, **log only** (no crash); frontend can show issues later.
- **Async spawn after setup:**  
  - `track_event_safe("app_started")`.  
  - `RepoManager::load_initial_cache()`.  
  - `DiscoveryManager::load_from_disk()` + `refresh_if_stale()`.  
  - `MetadataState::init(24)`.  
- **Theme:** On Linux, XDG Settings portal `org.freedesktop.appearance` `color-scheme` is read and emitted as `system-theme-changed` (light/dark/auto).  
- **Wayland:** If `WAYLAND_DISPLAY` is set, shadow is disabled on the main window to avoid transparency artifacts.

### 1.5 LoadingScreen (`src/components/LoadingScreen.tsx`)

- Shows distro-aware tips and listens for `sync-progress` to update status and a simple progress heuristic.
- No Tauri-specific requirement except when events are used; works in screenshot mode without invoke.

---

## 2. Permissions and Privilege Model

### 2.1 Tauri capability (`capabilities/default.json`)

- **Windows:** `main` only.  
- **Permissions:**  
  - `core:default`, `opener:default`, `notification:default`, `aptabase:default`, window controls (minimize, maximize, close, drag).  
  - **`app-commands-read`** and **`app-commands-privileged`** — custom commands split into read-only (search, metadata, status, user cache, telemetry) vs privileged (install, repair, sync, policy, repo/settings write). See `permissions/app-commands-read.toml` and `permissions/app-commands-privileged.toml`.  
  - **Store:** allow-load, allow-save, has, get, set, delete, keys.  
  - **FS:** `fs:allow-read` scoped to `$CACHE/monarch-store/**` only.  
- **Implication:** Frontend can only read under app cache; all install/update/sync/repair go through **backend commands** that invoke the **Helper** (Polkit or sudo). No broad filesystem or shell from the webview.

### 2.2 Helper invocation (`helper_client.rs`)

- **Entry:** `invoke_helper(app, cmd, password, use_branded_auth)`. Fourth argument controls auth mode.
- **Debounce:** 800 ms (`constants::HELPER_DEBOUNCE`) between invocations to limit rapid/spam calls.
- **Command delivery:** Always via **temp file** at `/var/tmp/monarch-cmd-<nanos>.json` (argv[1]); stdin is **only** for sudo password. Avoids pkexec stdin issues.
- **Auth (one-click vs Polkit):**
  - **One-click ON (`use_branded_auth = true`):** Password (from session) is used; Helper run with **`sudo -E -S <helper_bin> <cmd_path>`**, password on stdin. Single branded prompt per session.
  - **One-click OFF (`use_branded_auth = false`):** `password` is ignored; Helper always run with **`pkexec --disable-internal-agent <helper_bin> <cmd_path>`** (Polkit). Advanced users get the system auth dialog every time.
  - All privileged commands (install, uninstall, sync, repair, chaotic, clear cache, unlock, cancel_install, apply_os_config) obtain `one_click` from `RepoManager::is_one_click_enabled().await` and pass it as the fourth argument. See `docs/RECENT_CHANGES.md` §8.
- **Helper binary:**
  - `MONARCH_USE_PRODUCTION_HELPER=1` and production exists → `/usr/lib/monarch-store/monarch-helper`.
  - Debug build: prefer dev helper (`get_dev_helper_path()`: CARGO_TARGET_DIR, same-dir-as-exe, then relative paths); else production if present; else error with hint to build monarch-helper.
  - Release: production path if exists, else dev path if present.
- **Security:** Command file is created, permissions set (0o644 on Unix), content verified; on spawn failure file is removed. Helper reads command from file path only.

### 2.3 Session password (`SessionPasswordContext.tsx`)

- **When used:** Only when **Reduce password prompts** is on (`reducePasswordPrompts` from store).
- **Flow:** `requestSessionPassword()` returns a promise. If a cached password is valid (12 h TTL), resolve immediately; else show modal “MonARCH One-Click Auth”, user can “Use for session” (cache) or “Use system prompt” (null → later Polkit).
- **Storage:** Password kept in module-level variables, not in React state; cleared on use or expiry. Documented as “cleared when you close the app.”
- **Used at startup:** If unlock needed and reduce prompts on, App requests password once and passes it to `unlock_pacman_if_stale({ password })` so the first privileged action doesn’t hit Polkit.

### 2.4 Security policy check

- **Backend:** `check_security_policy()` (`commands/system.rs`) returns true only if **both** exist:
  - `/usr/lib/monarch-store/monarch-helper`
  - `/usr/share/polkit-1/actions/com.monarch.store.policy`
- **Frontend:** After startup (when `!isRefreshing`), a `useEffect` runs once (`polkitCheckedRef`): calls `check_security_policy()` and, if false, shows a **warning toast**: “Polkit rule not installed. Install and system actions may prompt for password. Enable One-Click in Settings to fix.”
- **Install path:** Settings (e.g. “Reduce password prompts” or repair) can call `install_monarch_policy({ password })`, which uses `run_privileged` to install the policy and rules (script writing policy + rules files). One-click mode sets `auth_admin_keep` so the user stays authorized for a period.

### 2.5 Unlock and repair

- **`needs_startup_unlock()`:** True iff `db.lck` exists and `pgrep -x pacman` is false (stale lock).
- **`unlock_pacman_if_stale(app, password)`:** If lock exists and pacman not running, invokes Helper `ExecuteBatch { remove_lock: true }` with optional password; drains progress channel.
- **`run_privileged` (repair.rs):** Holds `PRIVILEGED_LOCK`, then runs either `pkexec` (no password) or `sudo -S` (with password) for a given command and args. Used for policy install script and similar one-off privileged tasks, not for normal install/update (those go through `invoke_helper`).

---

## 3. Summary Table

| Area | Behavior |
|------|----------|
| **Startup gate** | `isRefreshing` true → LoadingScreen; false after initializeStartup + min 1.5 s. |
| **Unlock** | needs_startup_unlock → optional session password → unlock_pacman_if_stale (Helper RemoveLock). |
| **Health** | check_initialization_status → onboarding / DB-repair overlay / normal home. |
| **Polkit check** | After loading; toast if policy or helper missing. |
| **Tauri** | Single main window; app-commands-read + app-commands-privileged; fs read only $CACHE/monarch-store/**. |
| **Helper** | Command via /var/tmp file; **one-click ON** → sudo -S (branded), **one-click OFF** → pkexec (Polkit); 800 ms debounce; dev vs prod path. |
| **Session password** | Optional 12 h cache when “Reduce password prompts” on; modal or “Use system prompt”. |
| **Policy install** | install_monarch_policy via run_privileged (script); triggered from Settings. |

---

## 4. Recommendations

1. **Required bins:** Consider surfacing missing `git` / `checkupdates` / `pkexec` in the UI. *Implemented: `get_missing_required_bins` command; Settings → Security & Privacy shows an amber alert when any are missing, with install hints.*
2. **Polkit check timing:** Current behavior (run when `!isRefreshing`) is correct so the toast appears after the main UI; no change needed unless you want the warning on the loading screen.
3. **Capability granularity:** Split app commands into read vs privileged. *Implemented: `app-commands-read` (search, queries, status, user cache, telemetry) and `app-commands-privileged` (install, repair, sync, policy, repo/settings write). Main window has both; future restricted windows can grant only read.*
4. **Session password lifetime:** 12 h is documented in code; consider documenting in UI. *Implemented: Session password modal and Settings One-Click description now state "Password is cached for up to 12 hours".*
---

## 5. Files Reference

| Topic | Files |
|------|--------|
| Startup | `src/main.tsx`, `src/App.tsx` (initializeStartup, isRefreshing), `src-tauri/monarch-gui/src/lib.rs` (setup) |
| Loading | `src/components/LoadingScreen.tsx` |
| Unlock | `src-tauri/monarch-gui/src/repair.rs` (needs_startup_unlock, unlock_pacman_if_stale) |
| Helper | `src-tauri/monarch-gui/src/helper_client.rs`, `constants.rs` (HELPER_DEBOUNCE, CMD_FILE_*) |
| Helper path | `src-tauri/monarch-gui/src/utils.rs` (MONARCH_PK_HELPER, get_dev_helper_path, monarch_helper_available) |
| Session password | `src/context/SessionPasswordContext.tsx`, `src/context/useSessionPassword` |
| Policy | `src-tauri/monarch-gui/src/commands/system.rs` (check_security_policy, install_monarch_policy), `repair.rs` (run_privileged) |
| Tauri permissions | `src-tauri/monarch-gui/capabilities/default.json`, `permissions/app-commands-read.toml`, `permissions/app-commands-privileged.toml` |

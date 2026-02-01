# MonARCH Helper: Full Issues Report and Resolution

**Date:** 2026-02-01 (v0.3.6-alpha)
  
**Scope:** `monarch-helper` binary path, build layout, install/update failures, download progress, cancel flow, and startup unlock.  
**Status:** All identified issues resolved.

---

## Executive Summary

Installs and updates were failing or stalling because **the application was not running the helper binary that was being built**. v0.3.6 introduces **The Iron Core** (`SafeUpdateTransaction`) to ensure that all sync operations are atomic and follow strict `-Syu` logic. This prevents partial upgrades and provides a robust state machine for ALPM transactions.

---

## Part 1: The Primary Bug — Wrong Helper Binary at Runtime

### 1.1 What Was Happening

- **Symptom:** Install and update from the app did not work (or showed "unknown variant `AlpmInstall`", or stalled at 0%).
- **User workflow:** Only `npm run tauri dev`; no manual choice of binary.
- **Expectation:** The helper built by the npm script should be the one the app invokes.

### 1.2 Root Cause: Target Directory Mismatch

| Component | Expected (documented) | Actual (before fix) |
|-----------|----------------------|----------------------|
| **npm script** | Sets `CARGO_TARGET_DIR="${PWD}/src-tauri/target"` | Set correctly |
| **Pre-step** | `(cd src-tauri && cargo build -p monarch-helper)` | Ran with that env |
| **Cargo config** | Use `CARGO_TARGET_DIR` when set | **Overridden** by `target-dir = "../target"` in `src-tauri/.cargo/config.toml` |
| **Where helper was built** | `src-tauri/target/debug/monarch-helper` | **`project_root/target/debug/monarch-helper`** |
| **Where the app looked** | `CARGO_TARGET_DIR/debug/monarch-helper` → `src-tauri/target/debug/` | Same |
| **Result** | App runs the freshly built helper | **App ran whatever existed in `src-tauri/target/debug/`** (stale, old, or missing) |

So:

1. The **pre-step** built the helper into **project root** `target/` (because of `target-dir = "../target"`).
2. The **GUI** resolved the helper from **`CARGO_TARGET_DIR`** → **`src-tauri/target/`**.
3. Those are **different directories**. The binary the app executed was **not** the one just built.

### 1.3 Why This Caused Install/Update to Fail

- The binary in `src-tauri/target/debug/` was often **old**: it did not include `AlpmInstall`, `AlpmUpgrade`, `AlpmUninstall`, or `AlpmInstallFiles` (only legacy `InstallTargets`, `InstallFiles`, `Sysupgrade`, etc.).
- When the GUI sent `AlpmInstall`, the **old** helper replied with:  
  `unknown variant 'AlpmInstall', expected one of 'InstallTargets', 'InstallFiles', ...`
- From the user’s perspective: **installs and updates did not work**, even though the **source code** had been fixed and the **pre-step** had just built a new helper (into the wrong place).

### 1.4 Resolution

- **Removed** the `target-dir` override from `src-tauri/.cargo/config.toml`.
- **Result:** When `npm run tauri dev` sets `CARGO_TARGET_DIR=src-tauri/target`, the pre-step now builds the helper **into that same directory**. The app and the built helper now share **one** target dir; the app runs the **correct** binary.

**Files changed:**

- `src-tauri/.cargo/config.toml` — removed `[build] target-dir = "../target"` and added a comment explaining why it must not be set.

---

## Part 2: Cascading Effects of the Wrong Binary

Because the app was often running an **old** helper, the following appeared broken even though the **code** had been updated:

| Symptom | Real cause |
|--------|------------|
| "Unknown variant AlpmInstall" | Old helper enum; new GUI was sending `AlpmInstall`. |
| Downloads stuck at 0% | New helper had progress/callback fixes; they never ran. |
| Progress bar spamming "0%" | New helper had throttling and shared state; old one did not. |
| Keyring pre-flight blocking installs | New helper had non-fatal keyring; old one aborted. |
| Onboarding deploying wrong helper | Repo_setup used its own path logic; could copy from wrong place. |

Fixing the **target-dir** mismatch made all of these improvements **actually take effect** when using `npm run tauri dev`.

---

## Part 3: Download and Progress Fixes (Helper Side)

These fixes were correct in code but only mattered once the **right** helper was running.

### 3.1 Download Callback Blocking stdout

- **Issue:** The ALPM download callback wrote progress to stdout and called `flush()`. If the GUI read the pipe slowly, the pipe buffer filled, the callback blocked, and ALPM waited — so the download appeared stuck at 0%.
- **Fix:** All progress output goes through a **dedicated writer thread** and a bounded channel. The callback only does a **non-blocking** `try_send`; it never blocks on I/O. Implemented in `src-tauri/monarch-helper/src/progress.rs` and used by both `main.rs` and `transactions.rs`.

### 3.2 Two Threads Writing to stdout

- **Issue:** `main.rs` had `emit_progress()` writing directly to stdout while `transactions.rs` sent progress via a channel to another thread. Two producers on the same stdout could interleave output and still block when the pipe was full.
- **Fix:** **Single** progress module: one channel, one writer thread. Both `main.rs` and `transactions.rs` send pre-serialized JSON lines via `progress::send_progress_line()`. Only the writer thread touches stdout.

### 3.3 Progress Spam (0% Repeated)

- **Issue:** ALPM uses **parallel downloads** and invokes the download callback from **multiple threads**. The code used **thread_local** state to throttle “0%” and “percent” updates. Each thread had its own map/set, so every worker thread emitted “Downloading … 0%” and the log was spammed.
- **Fix:** Replaced thread-local state with **shared** `Mutex<HashMap<String, u8>>` and `Mutex<HashSet<String>>` so we emit “0%” (or “connecting”) **once per file** and throttle percent updates **globally** per file. See `download_progress_state()` and `setup_progress_callbacks()` in `transactions.rs`.

### 3.4 Keyring Pre-Flight Aborting All Installs

- **Issue:** If “Pre-Flight: Verifying security keys…” failed (e.g. network or keyring issue), the helper returned an error and the install never reached the real download step.
- **Fix:** Keyring pre-flight is **non-fatal**: on failure we log a warning, emit “Keyring update skipped; proceeding with transaction…”, and **continue**. ALPM can still fail later with a concrete error if needed. See `execute_alpm_install()` in `transactions.rs`.

**Files changed:**

- `src-tauri/monarch-helper/src/progress.rs` (new) — single writer thread, `send_progress_line()`.
- `src-tauri/monarch-helper/src/transactions.rs` — use `progress::send_progress_line()`; shared Mutex state for download throttle; keyring pre-flight non-fatal.
- `src-tauri/monarch-helper/src/main.rs` — `emit_progress()` uses `progress::send_progress_line()`.
- `src-tauri/monarch-helper/Cargo.toml` — added `crossbeam-channel` for non-blocking `try_send`.

---

## Part 4: Single Source of Truth for Helper Path

### 4.1 Problem

- Helper path was resolved in **three** places with duplicated logic:
  - `helper_client.rs` (which binary to run for install/update).
  - `utils.rs` (`monarch_helper_available()`).
  - `repo_setup.rs` (which binary to copy to `/usr/lib/monarch-store/` during onboarding).
- Repo_setup had a fallback `parent/../monarch-helper` that could point to the **wrong** binary, so onboarding could deploy an old helper.

### 4.2 Resolution

- **Single function:** `utils::get_dev_helper_path() -> Option<PathBuf>` with one resolution order:  
  `CARGO_TARGET_DIR` → exe sibling → fallback list (`src-tauri/target/debug/`, `./target/debug/`, etc.).
- **helper_client.rs** — uses `crate::utils::get_dev_helper_path()` only (no inline path logic).
- **repo_setup.rs** — when not running from `/usr`, uses `get_dev_helper_path()` for the “helper source” to deploy; fallback only to exe sibling if that exists.
- **utils.rs** — `monarch_helper_available()` now uses `get_dev_helper_path().is_some()` so the same paths define “available”.

**Files changed:**

- `src-tauri/monarch-gui/src/utils.rs` — added `get_dev_helper_path()`, refactored `monarch_helper_available()`.
- `src-tauri/monarch-gui/src/helper_client.rs` — use `utils::get_dev_helper_path()`.
- `src-tauri/monarch-gui/src/repo_setup.rs` — use `utils::get_dev_helper_path()` for deployment source.

---

## Part 5: Test Script and PKGBUILD

### 5.1 test_install_flow.sh

- **Issue:** Script used `HELPER_BIN="./target/debug/monarch-helper"` and ran `cargo build` from the current directory. When run from `monarch-helper/` or `monarch-helper/tests/`, the workspace builds into **`src-tauri/target/`**, not `monarch-helper/target/`, so the script could point at a missing or wrong binary.
- **Fix:** Script derives workspace root (`SCRIPT_DIR/../..`), sets `HELPER_BIN="$WORKSPACE_ROOT/target/debug/monarch-helper"`, and runs `(cd "$WORKSPACE_ROOT" && cargo build -p monarch-helper)` so the helper is always taken from `src-tauri/target/debug/`.

**File changed:** `src-tauri/monarch-helper/tests/test_install_flow.sh`

### 5.2 PKGBUILD

- **Issue:** Build ran `(cd src-tauri && cargo build --release -p monarch-helper)` **without** `CARGO_TARGET_DIR`. If someone re-added `target-dir` in `.cargo/config.toml`, the helper could again be built to a different directory than the one `package()` uses.
- **Fix:** Pre-step now sets `CARGO_TARGET_DIR="$srcdir/$pkgname/src-tauri/target"` when building the helper so the release helper is always in `src-tauri/target/release/`, matching `npm run tauri build` and `package()`.

**File changed:** `PKGBUILD`

---

## Part 6: Polkit and Manual Testing

### 6.1 pkexec Running the Installed Helper

- **Observation:** When running `pkexec /path/to/dev/monarch-helper` manually, the error “unknown variant AlpmInstall” still appeared.
- **Cause:** The Polkit policy file has `org.freedesktop.policykit.exec.path` set to `/usr/lib/monarch-store/monarch-helper`. Polkit can run that path instead of the one passed to `pkexec`, so the **installed** (old) helper was executed.
- **Resolution:** For **manual** testing of the dev helper, bypass pkexec and use **sudo** with the **full path** to the dev binary. No code change; documented in conversation and TROUBLESHOOTING.

### 6.2 Which Binary After “cargo build -p monarch-helper”

- **Observation:** After removing `target-dir`, building from `src-tauri` with **no** `CARGO_TARGET_DIR` uses Cargo’s default (workspace root = `src-tauri`, so target = `src-tauri/target`). With **`npm run tauri dev`**, `CARGO_TARGET_DIR` is set to `src-tauri/target`, so the pre-step and app agree. For manual runs of `cargo build -p monarch-helper` from `src-tauri` without the env, the binary is still at `src-tauri/target/debug/monarch-helper`. No second layout.

---

## Part 7: Documentation Updates

- **release-checklist.md** — Helper bullet updated: PKGBUILD uses `CARGO_TARGET_DIR=.../src-tauri/target` and installs `src-tauri/target/release/monarch-helper`.
- **docs/TROUBLESHOOTING.md** — “Quick fix (source only)” for updating the helper now uses the same target dir and path (`src-tauri/target/release/monarch-helper`).
- **docs/DEVELOPER.md** — Added a short section explaining why we must **not** set `target-dir` in `.cargo/config.toml` and that the app/onboarding use `utils::get_dev_helper_path()` as the single source of truth.

---

## Summary Table: All Code/Config Changes

| Area | File(s) | Change |
|------|---------|--------|
| Target dir | `src-tauri/.cargo/config.toml` | Removed `target-dir = "../target"` so pre-step and app share `src-tauri/target` |
| Progress I/O | `src-tauri/monarch-helper/src/progress.rs` | New: single writer thread, `send_progress_line()` |
| Progress usage | `transactions.rs`, `main.rs` | All progress via `progress::send_progress_line()`; no direct stdout in callbacks |
| Download throttle | `transactions.rs` | Shared `Mutex` state for “0%” and percent; no thread_local |
| Keyring | `transactions.rs` | Keyring pre-flight non-fatal; log and continue on failure |
| Helper path | `utils.rs` | Added `get_dev_helper_path()`; `monarch_helper_available()` uses it |
| Helper path | `helper_client.rs` | Use `utils::get_dev_helper_path()` only |
| Helper path | `repo_setup.rs` | Use `utils::get_dev_helper_path()` for deployment source when not from `/usr` |
| Test script | `monarch-helper/tests/test_install_flow.sh` | Resolve workspace root; use `$WORKSPACE_ROOT/target/debug/monarch-helper` |
| Package build | `PKGBUILD` | Set `CARGO_TARGET_DIR=.../src-tauri/target` when building helper |
| Docs | `release-checklist.md`, `TROUBLESHOOTING.md`, `DEVELOPER.md` | Updated helper path and target-dir explanation |

---

## How to Verify

1. **Dev workflow**
   - From repo root: `npm run tauri dev`.
   - Trigger an install (e.g. a small repo package).
   - In install output, look for: `Seeking helper at: .../src-tauri/target/debug/monarch-helper`.
   - Install should complete; progress should move past 0% and not spam “0%”.

2. **No stale helper**
   - After changing helper code, run `npm run tauri dev` again (pre-step rebuilds helper into `src-tauri/target/`).
   - No need to manually point at another path; the app uses that binary.

3. **Package build**
   - Run the PKGBUILD build; after `package()`, `src-tauri/target/release/monarch-helper` should exist and be the one installed under `/usr/lib/monarch-store/monarch-helper`.

---

## Conclusion

The main failure was **one** Cargo config line causing the helper to be built in a different directory than the one the app used, so installs and updates were running an old or wrong helper. Fixing that, plus unifying progress output, download throttling, keyring behavior, and helper path resolution, ensures that:

- **`npm run tauri dev`** builds and runs the **same** helper binary.
- Downloads no longer block or spam 0%.
- Onboarding and the app use a **single** definition of the dev helper path.
- PKGBUILD and docs align with the same target layout.

All identified helper-related issues have been addressed in code and config as described above.

---

## Part 8: The "No Sudo" Rule (Jan 31, 2026)

### 8.1 Incident

An install failed with `sudo: no password provided` because the **GUI** (`repair.rs`) used `run_privileged` to run `sudo -S rm -f /var/lib/pacman/db.lck` when a password was supplied. The GUI has no TTY, so sudo could not accept input and failed.

### 8.2 Resolution

1. **Pre-flight unlock** (in `commands/package.rs`) now always calls `repair_unlock_pacman(app, None)` so the unlock step uses the **Helper** (`RemoveLock`) via Polkit, not sudo.
2. **Empty password** in `repair.rs`: `password.filter(|s| !s.trim().is_empty())` so we use the Helper path instead of sudo when the frontend sends an empty string.

### 8.3 Rule

**The GUI must NEVER run `sudo` for system tasks** (lock removal, cache clean, repairs). Always delegate to the Helper via `invoke_helper` with the appropriate `HelperCommand`. Do not re-introduce `Command::new("sudo")` in `src-tauri/monarch-gui/` for those operations. See `.cursor/CONTEXT.md` and `.cursorrules`.

---

## Part 9: Final Polish — "Airbags and Wipers" (v0.3.5)

Before shipping v0.3.5, four critical "blind spots" that often break Arch GUI apps were audited and addressed where applicable.

### 9.1 Cache Bloat

| Item | Status |
|------|--------|
| **Risk** | `/var/cache/pacman/pkg/` grows to 50GB+ over time. |
| **Smell** | Does the app run `paccache -r` or `pacman -Sc`? |
| **Audit** | Settings → Maintenance has **Clear Package Cache**; Helper command `ClearCache` wipes cache dir (full clean). No `pacman -Sc` or `paccache -r` (keeps N recent) in code today. |
| **Action** | No change for alpha. Optional later: add "Clean cache (keep last 3)" using `paccache -rk3` or `pacman -Sc --noconfirm` and toast after Update All. |

### 9.2 .pacnew Silent Killer

| Item | Status |
|------|--------|
| **Risk** | Config files get `.pacnew`; user never merges; services break. |
| **Smell** | Does the app warn about `.pacnew`? |
| **Audit** | UpdatesPage / SystemHealth already scan for `*.pacnew` under `/etc` and show a warning. |
| **Action** | **Ignored for v0.3.5** (per recommendation). No merge UI; warning is sufficient for alpha. |

### 9.3 Orphan Build Dependencies (AUR)

| Item | Status |
|------|--------|
| **Risk** | AUR build installs make/gcc/fakeroot; if not removed, system fills with dev libs. |
| **Smell** | Does makepkg use `-r` (remove make-deps after build)? |
| **Audit** | Initial makepkg in `commands/package.rs` already had `-s -r --noconfirm --needed`. **Retry** makepkg path lacked `-r`. |
| **Action** | **Done.** Retry makepkg now uses `.args(["-s", "-r", "--noconfirm", "--needed"])` so both initial and retry remove make-deps after build. |

### 9.4 Mirrorlist Rot

| Item | Status |
|------|--------|
| **Risk** | Dead/slow mirrors cause long timeouts; users think app is frozen. |
| **Smell** | Is there a "mirror check" or Reflector prompt? |
| **Audit** | `commands/system.rs` has `test_mirrors(repo_key)`; Settings → Repositories exposes **Test Mirrors** per repo (rate-mirrors/reflector; top mirrors with latency). No automatic "if slow, suggest Reflector" in Keyring flow. |
| **Action** | No change for alpha. Manual Test Mirrors in Settings is sufficient. |

### 9.5 Code Freeze Checklist (Verify Only)

| Scenario | What to verify | Notes |
|----------|----------------|-------|
| **No Internet** | Disconnect WiFi → Refresh. App should not crash; show network/refresh error. | Frontend/backend handle failed sync gracefully. |
| **Disk Full** | Code audit: does `transactions.rs` check free space before download? | No pre-download free-space check in current code; ALPM will fail with disk-full errors. Optional v1.0. |
| **Partial Install** | Kill app during download; re-open. Should resume or show clean state, not "Database Locked" forever. | Helper removes lock on startup/retry; GUI uses Helper for unlock (No Sudo). |
| **Bad Config** | Corrupt `/etc/pacman.conf` (typo) → Open app. Should show helpful error, not crash. | ALPM init and config parsing surface errors; frontend shows friendly message. |

### 9.6 Summary

- **Cache:** Clear Cache exists; optional "keep last N" and post–Update All toast deferred.
- **.pacnew:** Warning in place; no merge for alpha.
- **makepkg -r:** Verified and fixed on **retry** path; both paths now use `-r`.
- **Mirrors:** Test Mirrors in Settings; no auto Reflector in keyring flow for alpha.

**Recommendation:** Ship v0.3.5 alpha. "No Sudo" and Keyring-First fixes put the app ahead of most custom Linux launchers; perfect is the enemy of good.

---

## Part 10: Distro-Aware Repository & Selection Audit (v0.3.5)

Audit of the full "Repository Management" pipeline — Settings UI to pacman.conf and install handoff — to ensure safety, accuracy, and distro-compliance. **Critical constraint:** The app must never overwrite distro-specific configurations (e.g. Manjaro mirrorlist, CachyOS optimization flags) with generic Arch defaults.

### 10.1 Atomic Toggle (Key Import During Enable)

| Requirement | Status |
|-------------|--------|
| When user enables Chaotic-AUR or CachyOS: (1) Add lines to pacman.conf, (2) Import key, (3) Sync DB | **Done.** |
| **Audit:** Toggle previously only wrote conf and synced; key import was only in onboarding. Next update could hit Unknown Trust. | **Fix:** When enabling a repo (single or family), we now run key import **before** writing config. |
| **Implementation:** `toggle_repo` and `toggle_repo_family` accept optional `password`. When `enabled` is true, we call `enable_repo` / `enable_repos_batch` (key + mirrorlist scripts) first, then `set_repo_state` / `set_repo_family_state` (which runs `apply_os_config` → write monarch conf + ForceRefreshDb). |
| **Frontend:** Settings passes password when enabling (from session password if reduce prompts). |

### 10.2 Distro-Specific Mirror Protection (Manjaro / Endeavour)

| Requirement | Status |
|-------------|--------|
| Never run reflector on Manjaro; use pacman-mirrors there. | **Done.** |
| **Audit:** `rank_mirrors` in `commands/system.rs` already uses a script that checks `/etc/manjaro-release` and `command -v pacman-mirrors` and runs `pacman-mirrors -f 5` instead of reflector. |
| **UI:** Added `get_mirror_rank_tool()` (returns `"pacman-mirrors"` \| `"reflector"` \| `"rate-mirrors"` \| null). Settings → Performance & Hardware shows "Rank Mirrors (Manjaro)" or "Rank Mirrors (reflector)" so the user sees the correct tool. |

### 10.3 Repo Availability (Select Source Dropdown)

| Requirement | Status |
|-------------|--------|
| Dropdown must only show repos where the package **actually exists** (not blindly list Extra, Core, Chaotic). | **Done.** |
| **Audit:** `get_package_variants` already returns only packages found in ALPM sync DBs, Chaotic API, or AUR. |
| **Frontend:** PackageDetailsFresh filters variants to those with a valid `version` before setting state and passing to RepoSelector, so we never show a source with no version. |

### 10.4 Target Repo Handoff (Ghost Fix)

| Requirement | Status |
|-------------|--------|
| Install must pass the **selected** repo as `target_repo` in the helper payload so the backend installs from that repo only. | **Done.** |
| **Audit:** `install_package` received `repoName` but passed `target_repo: None` to the helper. |
| **Fix:** For non-AUR installs, we now pass `_repo_name` as `target_repo` to `AlpmInstall` (main path and retry path). AUR installs keep `target_repo: None`. |

### 10.5 CachyOS / -v3 / -v4 Priority

| Requirement | Status |
|-------------|--------|
| Backend should prefer -v3/-v4/znver4 DBs when present and CPU-compatible. | **Done.** |
| **Audit:** `build_priority_order` in `transactions.rs` already orders repos by `cpu_optimization` (znver4, v4, v3) and pushes CachyOS optimized repos first when enabled. `force_refresh_sync_dbs` does **not** touch `/etc/pacman.d/mirrorlist`; it only refreshes sync DBs from config. |

### 10.6 Summary

- **Atomic toggle:** Key import runs synchronously when enabling a repo/family; then conf write + sync.
- **Manjaro:** rank_mirrors script uses pacman-mirrors when present; UI shows "Rank Mirrors (Manjaro)" via `get_mirror_rank_tool`.
- **Dropdown:** Only variants with a valid version are shown; backend only returns packages that exist.
- **target_repo:** Selected repo is passed to the helper so install uses that repo only.
- **CachyOS:** Priority order and `force_refresh_sync_dbs` already distro-safe; no mirrorlist overwrite.

---

## Part 11: First Impression — Onboarding & Settings Audit (v0.3.5)

Audit of the Onboarding flow and Settings page to ensure the handshake between user, GUI, and system is safe and correct. If a user enables a feature, it must work immediately without breaking the OS.

### 11.1 Helper Deployment Verification

| Item | Status |
|------|--------|
| **Target** | `repo_setup.rs` — helper deployment is inside `bootstrap_system` (no separate `install_monarch_helper`). |
| **Source path** | **Uses `utils::get_dev_helper_path()`** when not running from `/usr`. Dev: same resolution as `helper_client`; installed: `/usr/lib/monarch-store/monarch-helper`. |
| **Copy** | Done in shell: `cp "{{HELPER_SOURCE}}" /usr/lib/monarch-store/monarch-helper`; no hardcoded `../target/debug`. |
| **Risk** | None. No hardcoded paths; dynamic resolution. |

### 11.2 Policy Installation

| Item | Status |
|------|--------|
| **Target** | Policy written in `repo_setup.rs` (bootstrap, set_one_click_control), `repair.rs`, `commands/system.rs` (install_monarch_policy). |
| **Path** | **`/usr/share/polkit-1/actions/com.monarch.store.policy`** (identifier is `com.monarch.store`, not `org.monarch.store`). |
| **Rules** | `install_monarch_policy` also writes `/usr/share/polkit-1/rules.d/10-monarch-store.rules`. |
| **Reload** | No `systemctl restart polkit` in code. Polkit picks up new/updated policy files automatically; no daemon restart required. |

### 11.3 Repo Selection (Onboarding) — Key Import

| Item | Status |
|------|--------|
| **Chaotic step** | OnboardingModal calls `enable_repo("chaotic-aur", pwd)` — runs key import (pacman-key --recv-key, etc.) before repo config. |
| **Finish** | `handleFinish` calls `toggle_repo_family(family, enabled, skipOsSync: true, password)` for each family. When enabling, backend runs `enable_repos_batch` (key import) then `set_repo_family_state` (apply_os_config). |
| **Risk** | None. Key import runs during onboarding (and when enabling in Settings). |

### 11.4 Settings & Persistence

| Item | Status |
|------|--------|
| **Parallel Downloads** | `handleParallelDownloads` calls `invoke('set_parallel_downloads', { count })` and `localStorage.setItem('parallel-downloads', ...)`. Backend writes to **/etc/pacman.conf** via privileged script (Polkit when no password). **No** `save_settings` / monarch.json for this; pacman.conf is source of truth. |
| **Initial load** | SettingsPage loads `parallelDownloads` from **localStorage** in useEffect (not from backend/pacman.conf). If user edits pacman.conf externally, UI can show a stale value until they change it again in UI. Optional improvement: read ParallelDownloads from pacman.conf on Settings load. |
| **Backup before edit** | **Fixed.** `set_parallel_downloads` now creates `/etc/pacman.conf.bak.parallel.$(date +%s)` before modifying pacman.conf. |
| **backup_pacman_conf** | No dedicated `backup_pacman_conf` in `commands/system.rs`. Backup is inline in scripts: `reset_pacman_conf` uses `.bak.reset.$(date +%s)`; `set_parallel_downloads` now uses `.bak.parallel.$(date +%s)`. |

### 11.5 Maintenance Tab Safety (No Sudo Rule)

| Item | Status |
|------|--------|
| **Clear Cache (Settings)** | Invokes `clear_cache` — clears **in-memory** caches (metadata, chaotic, flathub, scm, repo sync). Does **not** wipe `/var/cache/pacman/pkg`. No privilege; no sudo. |
| **Clean Package Cache (disk)** | Helper has `ClearCache { keep }`; GUI does not currently expose a "Clean Package Cache" that wipes `/var/cache/pacman`. If added, it **must** use `invoke_helper(HelperCommand::ClearCache)`, not sudo. |
| **Remove Orphans** | Uses `remove_orphans` → `run_pacman_command_transparent(app, args, None)`. That uses **Polkit** (pkexec) when password is None, not helper. For strict "No Sudo" compliance, consider delegating to helper `RemoveOrphans` so all privileged pacman ops go through the helper. |

### 11.6 Theme & First Paint

| Item | Status |
|------|--------|
| **Theme persistence** | `useTheme` reads from `localStorage` in `useState` initializer; applies class in `useEffect`. |
| **Flash** | First paint can show default (e.g. light) before `useEffect` runs; possible brief white flash on restart if user had dark. Mitigation: add a small inline script in `index.html` that reads `theme-mode` from localStorage and sets `document.documentElement.classList` before React loads (optional). |

### 11.7 Distro-Respect & Backup

| Item | Status |
|------|--------|
| **Backup before writing pacman.conf** | **Done.** `reset_pacman_conf` creates `pacman.conf.bak.reset.$(date +%s)`. `set_parallel_downloads` now creates `pacman.conf.bak.parallel.$(date +%s)`. |
| **apply_os_config** | Writes only to `/etc/pacman.d/monarch/*.conf`, not to `/etc/pacman.conf`. No backup needed for monarch/*.conf (modular; can be recreated). |
| **Duplicate [chaotic-aur]** | We do **not** append to pacman.conf. We write `50-chaotic-aur.conf` in monarch/; one file per repo. No duplicate-section risk. |

### 11.8 Execution Checklist Summary

| Check | Result |
|-------|--------|
| repo_setup.rs `std::fs::copy` | No Rust `std::fs::copy`; copy is in shell script using `{{HELPER_SOURCE}}` from `get_dev_helper_path()`. |
| Settings.tsx useEffect initial load | Settings load repo state and localStorage (parallel downloads, etc.) in useEffect; parallel downloads initial value is from localStorage, not pacman.conf. |
| backup_pacman_conf in system.rs | No standalone function; backup is inline in scripts. `set_parallel_downloads` now includes backup step. |

---

## Part 12: App Store–Style Cancel Flow (v0.3.5)

Users can cancel an in-progress install and close the modal without leaving a stale lock that blocks the next install.

### 12.1 Helper: PID File and Cancel Watcher

| Item | Status |
|------|--------|
| **PID file** | On startup, the helper writes its PID to `/var/tmp/monarch-helper.pid` and removes it on normal exit (via `PidFileGuard`). |
| **Cancel file** | A background thread watches for `/var/tmp/monarch-cancel`. If present, the helper removes the cancel file and PID file and exits (`std::process::exit(0)`), stopping the transaction. |
| **Location** | `monarch-helper/src/main.rs`: PID write on start, watcher thread, guard cleanup. |

### 12.2 GUI: cancel_install Command

| Item | Status |
|------|--------|
| **Command** | `repair::cancel_install(app)` in `repair.rs`: creates `/var/tmp/monarch-cancel`, waits ~1.5 s for the helper to exit, then calls `repair_unlock_pacman(app, None)` so any leftover `db.lck` is cleared via Helper `RemoveLock`. |
| **No Sudo** | Unlock is always done via Helper (Polkit), not sudo. |

### 12.3 Frontend: InstallMonitor

| Item | Status |
|------|--------|
| **Cancel button** | While an install is running, a Cancel button calls `invoke('cancel_install')`, sets status to error, and auto-closes the modal after a short delay. |
| **Close (X) warning** | If the user clicks the modal close button while install is running, a confirmation dialog asks whether to cancel the installation instead; on Yes, calls `cancel_install` and closes. |

**Files:** `monarch-helper/src/main.rs`, `monarch-gui/src/repair.rs`, `src/components/InstallMonitor.tsx`.

---

## Part 13: Startup Unlock (Stale Lock)

So the workflow is not broken after a previous cancel or crash, the app clears a stale Pacman DB lock at startup before health check and sync.

### 13.1 Behavior

| Item | Status |
|------|--------|
| **When** | At app launch, in `App.tsx` `initializeStartup()`, before health check and sync. |
| **Check** | Frontend calls `needs_startup_unlock()` first; if false (no db.lck or pacman running), skip unlock. |
| **Command** | `unlock_pacman_if_stale(app, password: Option<String>)` in `repair.rs`: if db.lck does not exist or pacman is running, returns immediately. Otherwise invokes Helper `RemoveLock`. When **password** is provided (e.g. **Reduce password prompts** is on), the GUI passes it to `invoke_helper(RemoveLock, password)` so the helper is run via sudo -S and the system prompt does not appear at launch; when password is None, Polkit (pkexec) is used. |
| **Silent** | No repair-log or toast; lock is cleared so subsequent sync and install work. |

### 13.2 Rationale

- After **Cancel** or a **crashed helper**, `db.lck` can be left behind. Without startup unlock, the next sync or install would hit "Database Locked."
- Running unlock at startup (when safe: no pacman process) ensures the user never has to manually run "Unlock Database" or `sudo rm db.lck` just because they cancelled or the app crashed earlier.
- When **Reduce password prompts** is enabled (Settings → Workflow & Interface), the in-app password is used for startup unlock so the user does not see a system prompt at launch.

**Files:** `monarch-gui/src/repair.rs` (`needs_startup_unlock`, `unlock_pacman_if_stale`), `src/App.tsx` (await `invoke('needs_startup_unlock')` then, if true, optional `requestSessionPassword()` and `invoke('unlock_pacman_if_stale', { password })` at start of `initializeStartup`).
## Part 14: The Iron Core — SafeUpdateTransaction (v0.3.6)

### 14.1 Issue
Previous versions relied on complex manual logic in `transactions.rs` to enforce full upgrades during individual package installs. This led to potential borrow checker issues and brittle code.

### 14.2 Resolution
Implemented `SafeUpdateTransaction` in `safe_transaction.rs`. This new "Iron Core" logic:
- Aborts early if `/var/lib/pacman/db.lck` is found.
- Forces a manual iteration of all local packages to resolve updates from sync DBs.
- Enforces a single `alpm.trans_commit()` call to ensure atomicity.
- Returns clean, owned `String` errors to avoid lifetime conflicts with the GUI's error service.

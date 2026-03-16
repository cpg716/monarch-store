> Historical note: This review document describes a legacy or point-in-time parity snapshot. GTK current-state and release-gate status now live in `docs/GTK_TAURI_PARITY_MATRIX.md` and the GTK-first root docs.

# Full Review: MonARCH Store Update Systems

**Date:** 2026-02-05  
**Scope:** Discovery, execution, UI, safety, and integration of repo, AUR, and Flatpak updates.

---

## 1. Architecture Overview

Updates flow through three layers:

| Layer | Role |
|-------|------|
| **Frontend** | Updates page (list, "Update All", progress), global listeners in `App.tsx`, Zustand store (`pendingUpdates`, `updateLogs`, `isUpdating`, etc.), background checker (`useUpdateChecker`) |
| **GUI (monarch-gui)** | `check_updates` (unified discovery), `perform_system_update` (fire-and-forget), `apply_updates` (selective; not used by current UI). Runs repo check (ALPM read), AUR/Flatpak checks, and orchestrates Helper + Flatpak CLI |
| **Helper (monarch-helper)** | Single writer for ALPM: `ExecuteBatch` (refresh_db + update_system + install/remove/local_paths), `execute_alpm_upgrade`, lock handling, progress emission |

**Order of operations (Iron Law):** Official repo sync + full upgrade → AUR builds + install → Flatpak updates. No `pacman -Sy` alone; full upgrade is enforced.

---

## 2. Discovery: "What needs updating?"

### 2.1 Unified check: `check_updates`

- **Location:** `monarch-gui/src/commands/update.rs`
- **Behavior:** Runs three tasks in parallel and merges results:
  - **Repo:** `alpm_read::get_host_updates()` — local ALPM + syncdbs from pacman.conf; compares local vs sync DB versions. **Assumes sync DBs are already present** (no refresh in this path). If user never refreshed, counts can be stale.
  - **AUR:** `aur_api::get_candidate_updates()` — foreign packages from ALPM, then `get_multi_info` for current AUR versions; compares with local. Uses string inequality for version (no `vercmp`); may treat downgrades as "updates."
  - **Flatpak:** `flathub_api::get_updates()` — `flatpak remote-ls --updates`; returns app IDs and versions.
- **Used by:** Updates page on load (`checkForUpdates`), sidebar badge (`refreshPendingUpdates` → `check_updates`), background checker (`useUpdateChecker`).

### 2.2 Repo updates: `get_host_updates`

- **Location:** `alpm_read.rs`
- **Behavior:** `Alpm::new("/", "/var/lib/pacman")`, `register_syncdbs_from_conf`, then iterates syncdbs and localdb; pushes `UpdateItem` when sync version > local (using `alpm::vercmp`). Read-only; no lock.
- **Caveat:** If sync DBs are old (e.g. user hasn’t run refresh in a while), repo update list can be incomplete until the next full upgrade (which does refresh).

### 2.3 AUR updates: `get_candidate_updates`

- **Location:** `aur_api.rs`
- **Behavior:** Foreign packages → AUR multi-info → version comparison. No `vercmp` (string compare); AUR version strings can make this fuzzy (e.g. `1.0.0-1` vs `1.0.0`).
- **Filtering in run:** During *execution*, `check_aur_updates()` in `update.rs` filters to "truly AUR only" via `is_in_sync_repos` so packages that moved to Chaotic/CachyOS are not built from AUR.

### 2.4 Flatpak updates: `flathub_api::get_updates`

- **Location:** `flathub_api.rs`
- **Behavior:** `flatpak remote-ls --updates`; parses tab-separated output; fills `UpdateItem` with app ID as name. Non-blocking CLI call.

---

## 3. Execution: "Applying updates"

### 3.1 Update All (Updates page)

- **Entry:** User clicks "Update All" → optional critical-news blocker → confirmation → `perform_system_update({ password: null })`.
- **Backend:** `perform_system_update` returns `Ok("started")` immediately and spawns a background task that runs `run_system_update_impl`.
- **No blocking:** UI does not await the Tauri command; completion is signaled only via `update-complete` event.

**Phases in `run_system_update_impl`:**

1. **Connectivity:** `ping -c 1 -W 2 archlinux.org`. On failure, returns error and no further work.
2. **Lock:** `PRIVILEGED_LOCK` in GUI prevents concurrent pacman-triggering operations.
3. **Repo (Iron Core):** `invoke_helper(ExecuteBatch { update_system: true, refresh_db: true })`. Helper does:
   - Optional `remove_lock` if requested (not set in this path).
   - `force_refresh_sync_dbs` then `execute_alpm_upgrade(None, alpm)`.
   - Progress messages streamed back; GUI emits `update-status`, `install-output`, `update-progress`.
4. **Failure gate:** If any message indicates transaction/prepare/404 failure, `sysupgrade_failed` is set; AUR/Flatpak are skipped and error is returned.
5. **AUR:** `check_aur_updates()` (AUR-only filter) → for each package, `build_aur_package` (makepkg in GUI) → `copy_paths_to_monarch_install` → `invoke_helper(AlpmInstallFiles { paths })`. Build failures are logged and skipped; install failures can abort.
6. **Flatpak:** `flathub_api::get_updates()` then for each app `update_flatpak(app, name)`. Failures are logged; rest continue.
7. **Done:** `Ok("System fully updated")` and spawn block emits `update-complete { success: true, message }` (or false on error).

### 3.2 Helper: ExecuteBatch

- **Location:** `monarch-helper/src/main.rs`
- **Steps (in order):**  
  - 0a. `remove_lock` if `manifest.remove_lock`.  
  - 0b. `clear_cache` if requested.  
  - 1. `force_refresh_sync_dbs` if `refresh_db`.  
  - 2. `execute_alpm_upgrade(None, alpm)` if `update_system`.  
  - 3. Uninstall targets if any.  
  - 4. Install repo targets; then install local paths (AUR built packages).
- **Lock:** ExecuteBatch does **not** call `ensure_db_ready()` before starting. ALPM is created earlier in the helper process; if `db.lck` is held by another process, ALPM init may block or fail. Other commands (AlpmInstall, AlpmUninstall, etc.) do call `ensure_db_ready()`.

### 3.3 Safe upgrade: `execute_alpm_upgrade`

- **Location:** `transactions.rs`
- **Behavior:** Refresh keyrings, sync DBs (`syncdbs_mut().update(false)`), `trans_init(ALL_DEPS)`, `sync_sysupgrade(false)`, prepare, commit. On corrupt-DB errors, retry with `force_refresh_sync_dbs` and second attempt. Progress and errors emitted via progress channel.

### 3.4 Selective updates: `apply_updates`

- **Location:** `update.rs`
- **Behavior:** Takes a list of `UpdateItem`. **Arch does not support selective repo upgrades** (partial upgrades are unsupported and dangerous). The code respects this:
  - If **any** target is repo (`has_official`), it runs a **full** system upgrade (ExecuteBatch with `refresh_db` + `update_system`) — i.e. the whole system is upgraded, not "only these repo packages."
  - Only **AUR** and **Flatpak** are selective: we build/install only the chosen AUR packages and update only the chosen Flatpak apps. That is safe (user/build scope).
- So "selective" here means "run full -Syu if any repo is selected, then update only the selected AUR/Flatpak items." There is no partial repo upgrade.
- **UI:** The current Updates page does **not** use `apply_updates`; it only has "Update All" which calls `perform_system_update`. A future "Update selected" could call `apply_updates(selected)` and would still do full -Syu whenever any selected item is from repo.

---

## 4. UI & State

### 4.1 Updates page

- **Data:** `check_updates()` on mount; list stored in local state; deduplicated by `name:source_type:id`.
- **Actions:** "Check Now" (re-run `check_updates`), "Update in terminal" (copy `sudo pacman -Syu`), "Update All" (with optional critical news blocker).
- **During update:** Stepper (Synchronizing → Upgrading → Community → Flatpaks) driven by `currentStep` derived from `statusMessage` (keywords: database/sync, upgrade, aur/community, flatpak). Progress bar and "Show Process Details" show `updateLogs` (from `install-output`).
- **Completion:** Listener for `update-complete` stops spinner, sets result message, runs `checkForUpdates()`, fetches pacnew warnings and (on success) orphans. Shows reboot/pacnew/service-restart banners and orphan cleanup when applicable.

### 4.2 Global listeners (App.tsx)

- **update-progress:** Updates `updateProgress`, `updateStatus`, `updatePhase`; on `phase === 'complete'` schedules `setUpdating(false)` and fetches reboot/pacnew; on `phase === 'error'` schedules stop.
- **install-output:** Appends to `updateLogs` (cap 500), sets `updateStatus`.
- **update-status:** Sets `updateStatus`.

So both `update-complete` (Updates page) and `update-progress` with phase `complete` can trigger "update finished" behavior; the timer in the progress listener may race slightly with `update-complete`.

### 4.3 Sidebar badge

- **Source:** `pendingUpdates` from store (`repo`, `aur`, `flatpak`, `total`).
- **Population:** `refreshPendingUpdates()` calls `check_updates()` and buckets by `source_type`; also fetches reboot, pacnew, service restarts.

### 4.4 Background checker

- **Hook:** `useUpdateChecker` (mounted in App).
- **Schedule:** First run after 10 s; then every 30 minutes (skips when `isUpdating`).
- **Notifications:** If `updateNotificationsEnabled` and `pendingUpdates.total` increased and last notify > 2 hours, calls `notifyUpdatesAvailable(...)`.

---

## 5. Safety & Correctness

### 5.1 Partial upgrade prevention

- **Rule:** No standalone `pacman -Sy`. Full upgrade is done via `refresh_db` + `update_system` in one batch; helper uses `sync_sysupgrade` after sync.
- **AUR after repo:** If repo phase fails, AUR and Flatpak are not run, avoiding mixed states.

### 5.2 Lock handling

- **GUI:** Startup can call `unlock_pacman_if_stale`; repair flow and InstallMonitor use `repair_unlock_pacman` when appropriate.
- **Helper:** `ensure_db_ready()` checks `db.lck`, removes if stale (no pacman running). Used by AlpmInstall, AlpmUninstall, AlpmInstallFiles, AlpmUpgrade; **not** by ExecuteBatch. ExecuteBatch can remove lock only if `manifest.remove_lock` is true (e.g. from repair).

### 5.3 Flatpak

- **Install/Uninstall:** Handled in GUI (Flatpak CLI); no Helper. We recently fixed success reporting (install no longer fails due to ALPM verification; uninstall now emits `install-complete`).
- **Update:** Same: `update_flatpak` in GUI; output now also emitted to `install-output` for the transaction log.

### 5.4 AUR version comparison

- Discovery uses string inequality for AUR versions; no `vercmp`. Could mark downgrades as updates in rare cases. Execution uses the same list; actual build/install can still fail or succeed independently.

---

## 6. Gaps & Recommendations

### 6.1 ExecuteBatch and db.lck

- **Gap:** ExecuteBatch does not call `ensure_db_ready()` before refresh/upgrade. If another process holds the lock, behavior depends on when ALPM was created (may block or fail earlier).
- **Recommendation:** At the start of the ExecuteBatch branch, call `ensure_db_ready()` and on error emit progress and return, for consistency with other commands.

### 6.2 Repo discovery freshness

- **Gap:** `get_host_updates()` uses existing sync DBs. If the user hasn’t refreshed in a long time, the "X updates available" count can be low or zero until they run "Update All" (which refreshes).
- **Recommendation:** Either document that "Check Now" reflects last sync state, or add an optional "Refresh databases then check" (e.g. read-only sync or a dedicated refresh step) so the count can be updated without a full upgrade.

### 6.3 AUR version comparison

- **Gap:** AUR update detection uses string comparison; no `vercmp`. Minor risk of wrong "update" or "no update" for odd version strings.
- **Recommendation:** Use ALPM-style version comparison (e.g. expose a vercmp helper or use a small version-compare crate) for AUR candidate updates.

### 6.4 Two "update finished" paths

- **Gap:** Both `update-complete` (Updates page) and `update-progress` with `phase === 'complete'` (App.tsx) can set "update finished" and fetch reboot/pacnew. Slight duplication and possible race.
- **Recommendation:** Prefer a single source of truth: e.g. only `update-complete` for "update run finished," and have the progress listener only update progress/phase, not final state. Or document that both are intentional (progress for phase UI, complete for final state).

### 6.5 Selective updates not in UI

- **Gap:** `apply_updates(targets)` exists but the Updates page only has "Update All." Users cannot select a subset (e.g. "update only these 3 AUR/Flatpak items" after a full -Syu).
- **Note:** We do **not** support selective *repo* updates (Arch forbids partial upgrades). Any "Update selected" that includes a repo package would still run full -Syu; only AUR and Flatpak in the selection would be limited to the chosen items.
- **Recommendation:** Either add "Update selected" (and pass selected items to `apply_updates`) or document that the API is for future use / other callers.

### 6.6 Update size estimate

- **Gap:** Updates page shows `(updates.length * 1.5).toFixed(1) MB` as a rough size. Repo items have `size` from ALPM; AUR/Flatpak often do not. So the estimate is heuristic.
- **Recommendation:** Sum repo `size` where available; keep heuristic for the rest or show "Size: —" for non-repo.

---

## 7. Summary Table

| Component | Status | Notes |
|-----------|--------|--------|
| Discovery (repo/AUR/Flatpak) | ✅ | Parallel, unified; repo uses existing DBs |
| Perform full update | ✅ | Non-blocking, correct order, failure gate |
| Helper ExecuteBatch | ✅ | Refresh → upgrade → remove → install; no explicit lock check |
| Safe upgrade (no -Sy alone) | ✅ | Enforced in helper and GUI flow |
| Updates page UI | ✅ | Stepper, logs, reboot/pacnew/orphans |
| Global listeners | ✅ | Progress + logs + status; minor overlap with update-complete |
| Background checker | ✅ | 30 min, notifications with throttle |
| Flatpak install/uninstall | ✅ | Fixed (success/install-complete + log output) |
| Selective apply_updates (AUR/Flatpak only; repo → full -Syu) | ⚠️ | Implemented but not used by UI |
| Lock check in ExecuteBatch | ⚠️ | Not called; could add ensure_db_ready |
| Repo count freshness | ⚠️ | Depends on last sync |
| AUR version comparison | ⚠️ | String-based |

Overall, the update system is coherent, respects the Iron Law (no partial upgrades), and integrates repo, AUR, and Flatpak.

### AUR and Flatpak management (post-review implementation)

- **Discovery:** `check_updates(include_aur?, include_flatpak?)` — when `false`, that source is skipped so the list matches the user’s Sources settings (Settings → Sources: AUR / Flatpak toggles).
- **Execution:** `perform_system_update(..., include_aur?, include_flatpak?)` — when `false`, the AUR or Flatpak phase is skipped so "Update All" only runs what was shown.
- **Frontend:** Updates page and background checker pass `isAurEnabled` and `isFlatpakEnabled` from `useSettings()` into both `check_updates` and `perform_system_update`. The updates list is grouped by source (System repos / AUR / Flatpak) with section headers.
- **Sidebar badge:** `refreshPendingUpdates(includeAur?, includeFlatpak?)` is called from `useUpdateChecker(isAurEnabled, isFlatpakEnabled)` so the count reflects the same scope as "Update All."

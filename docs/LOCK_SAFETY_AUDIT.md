# Lock Safety Audit — MonARCH Store

**Date:** 2025-01-31 · **Release:** v0.3.5-alpha  
**Objective:** Verify GUI (monarch-gui) adheres to Lock Safety so the Helper has exclusive ALPM access during transactions.

---

## Task 1: Handle Leak Scan

**Target:** `src-tauri/monarch-gui/src/`

**Findings:**

- **No long-lived ALPM in Tauri state.** There is no `Arc<Mutex<Alpm>>` or any managed Alpm handle in app state.
- **ALPM usage is confined to `alpm_read.rs`.** Four functions use ALPM:
  - `search_local_dbs(query)` — creates `Alpm::new("/", "/var/lib/pacman")`, uses it, returns (drops).
  - `get_package_native(name)` — same: create, use, return (drop).
  - `get_installed_packages_native()` — same.
  - `get_packages_batch(names, enabled_repos)` — same.
- **Create/drop per request.** Every ALPM use is a local variable, used synchronously, then dropped when the function returns. No handle is held across an `await` or across an `invoke_helper` call.
- **Bad pattern not present.** There is no `let db = state.alpm.lock(); …; helper.install();` — the GUI never holds an ALPM lock while calling the helper.

**Caveat (concurrent read + helper):**  
`search_local_dbs` and `get_packages_batch` are invoked from `commands/search.rs` inside `tokio::task::spawn_blocking`. If a user runs a search (blocking thread holds ALPM for the duration of the search) and at the same time starts an install (invoke_helper), the Helper process will try to acquire `db.lck` and may block until the GUI’s blocking task finishes and drops its Alpm handle. This is a theoretical race; in practice reads are short and the Helper would wait. No code change recommended unless lock contention is observed.

---

## Task 2: Helper Handshake Verification

**Target:** `src-tauri/monarch-gui/src/commands/` and startup flow.

**Findings:**

- **No active DB read before helper in install/uninstall/sync paths.**  
  In `package.rs`, before `invoke_helper` we acquire `PRIVILEGED_LOCK`, then either build AUR + invoke_helper, or apply_os_config (which itself invokes helper for WriteFiles/ForceRefreshDb) then invoke_helper for AlpmInstall. We do **not** call `alpm_read::*` in the same flow immediately before invoke_helper.  
  In `update.rs`, `system.rs`, `repo_manager.rs`, helper is invoked without a preceding ALPM read in the same request.
- **Startup:** In `src/App.tsx`, `initializeStartup` runs **first** `needs_startup_unlock` then `unlock_pacman_if_stale` (if needed) as step 0, before `get_repo_states`, `check_initialization_status`, or any sync. So **unlock_pacman_if_stale is called exactly once at startup and before other ALPM-related operations.** When **Reduce password prompts** is on, the frontend passes the session password to `unlock_pacman_if_stale(app, { password })`; the GUI then invokes the helper with that password (sudo -S) instead of Polkit, so the system prompt does not appear at launch.

---

## Task 3: Documentation

**Target:** `AGENTS.md`

**Action:** Appended section **Lock Safety / Split-Brain Architecture** with:

- Rule 1: monarch-helper is the only binary that writes to `/var/lib/pacman`.
- Rule 2: GUI does AUR build in user space and hands off `.pkg.tar.zst` to Helper.
- Rule 3: No sudo in GUI for package ops; only pkexec via Helper.
- Summary of GUI ALPM use (short-lived, per-request, no state) and the concurrent-read caveat.

---

## Deliverable

**GUI handles are safely scoped.**

- No long-lived ALPM handle in the GUI; no Arc/Mutex<Alpm> in state.
- ALPM is used only in `alpm_read.rs`, create/drop per request; no handle held across `invoke_helper`.
- Startup runs `unlock_pacman_if_stale` once before other ALPM-related work.
- No potential contention **in the same request** (we never lock ALPM then call the helper).  
- **Theoretical only:** concurrent search (spawn_blocking) + install could cause the Helper to block on `db.lck` until the search finishes; acceptable unless contention is observed.

No code changes required. Documentation updated in `AGENTS.md` so future agents respect the Split-Brain model and do not suggest a rewrite.

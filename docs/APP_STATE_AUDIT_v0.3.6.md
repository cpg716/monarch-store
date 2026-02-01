# MonARCH Store — App State Audit (v0.3.6-alpha)

**Date:** 2026-02-01  
**Scope:** Build, tests, Iron Core vs docs, CheckUpdatesSafe, frontend/backend wiring, doc/code consistency.

---

## 1. Build & Tests

| Check | Result | Notes |
|-------|--------|--------|
| Frontend (`npm run build`) | ✅ Pass | TS check + Vite build succeed. Warnings: dynamic imports (Sidebar, LoadingScreen, etc.) and `/grid.svg` unresolved at build time. |
| monarch-helper (`cargo test`) | ✅ Pass | 4 unit + 5 integration tests pass. |
| monarch-helper (`cargo check`) | ⚠️ Warnings | **2 dead_code warnings:** `SafeUpdateTransaction` and its methods (`new`, `with_targets`, `execute`) are never used. |
| monarch-gui full build | — | Not run (long compile); frontend build confirms TS/Vite path. |

**Verdict:** App builds and tests pass. Iron Core code exists but is unused (see §2).

---

## 2. Iron Core (SafeUpdateTransaction) vs Documentation

| Doc claim | Code reality |
|-----------|----------------|
| "All sync operations MUST use SafeUpdateTransaction" (AGENTS, CONTRIBUTING) | **Not true.** `SafeUpdateTransaction` in `safe_transaction.rs` is **never constructed or called** anywhere in monarch-helper. |
| "Helper uses SafeUpdateTransaction for atomic reliability" (ARCHITECTURE, INSTALL_UPDATE_AUDIT) | Install/sysupgrade use `transactions.rs`: `execute_alpm_install`, `execute_alpm_sysupgrade`, etc. |
| "Iron Core ensures exclusive ALPM access" (LOCK_SAFETY_AUDIT) | Lock is enforced by **`ensure_db_ready()`** in `main.rs` (checks `db.lck` + stale removal). SafeUpdateTransaction’s lock check is redundant and unused. |

**Actual safety today:**

- **Lock guard:** `ensure_db_ready()` runs before AlpmInstall, AlpmUninstall, AlpmUpgrade, AlpmSync, Sysupgrade, AlpmInstallFiles, etc. CheckUpdatesSafe correctly **skips** it (uses temp DB).
- **Full-upgrade logic:** In `transactions.rs`, `execute_alpm_install` when `sync_first` is true adds all upgradable packages (lines 229–247), then single `trans_commit()`. Sysupgrade path uses full upgrade. So “no partial upgrade” is enforced in **transactions.rs**, not in SafeUpdateTransaction.

**Recommendation:** Either (a) wire SafeUpdateTransaction into AlpmInstall/Sysupgrade and remove duplicate logic from transactions.rs, or (b) soften docs to say “atomic -Syu logic is enforced in transactions.rs; SafeUpdateTransaction is the formal protocol implementation available for future use.”

---

## 3. CheckUpdatesSafe & pacman -Sy

- **Implementation:** `execute_alpm_check_updates_safe` uses a **temp directory** (tempfile), symlinks `/var/lib/pacman/local` into it, then runs:
  - `pacman -Sy --dbpath <temp>` (sync into temp only)
  - `pacman -Qu --dbpath <temp>` (query upgradable from temp)
- **Real DB:** Never locked; real `/var/lib/pacman` is not written. So **-Sy on temp dbpath is intentional and safe** (read-only check for “updates available”).
- **Verdict:** ✅ No violation of “never -Sy alone on the real DB.” Doc note: “Safe check: does not require ensure_db_ready() because it uses a temp DB path” in main.rs is correct.

---

## 4. Frontend ↔ Backend Wiring

- **Tauri commands:** All frontend `invoke(...)` targets checked (e.g. `repair_unlock_pacman`, `cancel_install`, `get_app_reviews`, `get_app_rating`, `apply_os_config`, `bootstrap_system`, etc.) are registered in `lib.rs` `invoke_handler`.
- **Verdict:** ✅ No missing commands; frontend invokes match backend handlers.

---

## 5. Documentation & Code Consistency

| Issue | Severity | Detail |
|-------|----------|--------|
| TESTING.md | Medium | Says “Test SafeUpdateTransaction state transitions” and “cargo test safe_transaction::tests”. **No tests exist** in `safe_transaction.rs` (no `#[cfg(test)]` or `#[test]`). Command fails. |
| PROGRESS.md header | Low | Still “Last updated: 2025-01-31 (v0.3.5-alpha)” while body has v0.3.6 bullets. |
| package.json version | Low | `"version": "0.3.5-alpha"`; docs refer to v0.3.6-alpha. |
| Native dialogs (rfd) | Medium | Docs (README, TROUBLESHOOTING, PROGRESS) say file pickers use **rfd** (Portal-based). **rfd is in Cargo.toml but not used** in any .rs/.ts/.tsx. File pickers likely still use Tauri/default. |
| Iron Core “MUST use” | High | CONTRIBUTING/AGENTS say all sync ops MUST use SafeUpdateTransaction; code does not use it (see §2). |

---

## 6. Security & Lock Safety (Summary)

- **GUI:** No long-lived ALPM; short-lived handles in `alpm_read.rs` only. No ALPM held across `invoke_helper`. ✅  
- **Helper:** `ensure_db_ready()` before all write paths; CheckUpdatesSafe explicitly skips it and uses temp DB. ✅  
- **Partial upgrades:** Prevented in `transactions.rs` (sync_first + add all upgrades + single commit). ✅  
- **SafeUpdateTransaction:** Implements same idea (lock check + full upgrade + single commit) but is dead code; safety currently comes from transactions.rs + ensure_db_ready().

---

## 7. Recommendations (Prioritized)

1. **Doc/code alignment (Iron Core):** Either wire `SafeUpdateTransaction` into the helper’s install/sysupgrade paths and use it, or update ARCHITECTURE, AGENTS, CONTRIBUTING, INSTALL_UPDATE_AUDIT, and Fort Knox to state that atomic -Syu is enforced in **transactions.rs** and that SafeUpdateTransaction is the reserved/protocol implementation.
2. **TESTING.md:** Remove or fix “cargo test safe_transaction::tests” (e.g. add real unit tests in `safe_transaction.rs` or delete the line and mention “SafeUpdateTransaction is not yet exercised by tests”).
3. **rfd:** Either implement Portal-based file pickers using `rfd` or remove `rfd` from Cargo.toml and adjust README/TROUBLESHOOTING/PROGRESS to avoid claiming native rfd dialogs.
4. **Version/date:** Set package.json to `0.3.6-alpha` and update PROGRESS “Last updated” to 2026-02-01 (v0.3.6-alpha) for consistency.
5. **Dead code:** If SafeUpdateTransaction remains unused, add `#[allow(dead_code)]` to the module or types to silence warnings until it’s wired or removed.

---

## 8. Quick Reference

| Area | Status |
|------|--------|
| Frontend build | ✅ |
| Helper tests | ✅ (9 tests) |
| Iron Core used in production path | ❌ (dead code) |
| Lock safety (ensure_db_ready) | ✅ |
| Full-upgrade logic (transactions.rs) | ✅ |
| CheckUpdatesSafe (-Sy on temp only) | ✅ Safe |
| Frontend invoke → backend | ✅ All registered |
| TESTING.md safe_transaction tests | ❌ Missing |
| rfd usage | ❌ Dependency only |
| package.json / PROGRESS version | ⚠️ Out of date |

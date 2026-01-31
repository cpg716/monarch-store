# Omegascope Pre-Flight Report — MonARCH Store v0.3.5-alpha

**Date:** 2025-01-31  
**Scope:** Total codebase audit (UX, Rust safety, security, Arch integration).  
**Verdict:** See final table.

---

## Pillar 1: User Experience (Frontend & Polish)

### Console hygiene

| Location | Count | Type | Action |
|----------|-------|------|--------|
| `src/App.tsx` | 1 | `console.warn` | Optional: route to errorService.reportWarning |
| `src/pages/SettingsPage.tsx` | 2 | `console.warn` | Fallback for missing commands; acceptable for dev |
| `src/hooks/useSettings.ts` | 4 | `console.warn` / `console.error` | Optional: use getErrorService() |
| `src/context/ErrorContext.tsx` | 2 | `console.error` | Intentional (raw backend log); keep or gate behind dev |
| `src/store/internal_store.ts` | 3 | `console.error` | Optional: use getErrorService() |
| `src/components/TrendingSection.tsx` | 1 | `console.error` | Optional: errorService.reportError |
| `src/components/ErrorBoundary.tsx` | 1 | `console.error` | Standard React boundary; acceptable |
| `src/hooks/useSmartEssentials.ts` | 1 | `console.error` | Optional: errorService |
| `src/hooks/useDistro.ts` | 1 | `console.error` | Optional: errorService |
| `src/services/reviewService.ts` | 1 | `console.error` | Optional: errorService |
| `src/hooks/useRatings.ts` | 1 | `console.warn` | Optional |
| `src/hooks/useFavorites.ts` | 2 | `console.error` | Optional |
| `src/hooks/useSearchHistory.ts` | 1 | `console.error` | Optional |
| `src/hooks/usePackageMetadata.ts` | 1 | `console.warn` | Optional |

**Summary:** 22 console usages; no `debugger`. Most are in catch paths; production builds can strip or replace with structured logging. **Does not block release** if build does not strip console.

### TODO / FIXME / HACK

- **Scan:** All `.ts`, `.tsx`, `.rs` (literal "TODO", "FIXME", "HACK", "XXX").
- **Result:** No stability-blocking TODOs found. One comment "XXXXXX" in package.rs (mktemp placeholder) and path comment in utils.rs; neither are TODO/FIXME.
- **Action:** None required.

### Error handling (UI)

- **Catch blocks:** Consistently use `errorService.reportError(e)` or `reportWarning(e)`; ErrorContext + friendlyError used for user-facing messages.
- **InstallMonitor / SystemHealthSection:** Some log lines append raw `e` or `e.message` to **transaction logs** (verbose, for experts). User-facing toasts/modals go through ErrorContext; UpdatesPage uses `friendlyError(raw).description` for update result.
- **Verdict:** User-facing errors are friendly; raw content appears only in detailed logs. **Pass.**

### Input sanitization (search / forms)

- **Search:** `search_packages` receives `query` as string; backend uses it in ALPM search and AUR API only (no shell). `alpm_read::search_local_dbs(&query)` passes to ALPM; no command injection.
- **InstalledPage filter:** Client-side `name.toLowerCase().includes(searchQuery)` — string match only.
- **Orphan remove:** `validate_package_name(name)` used for every orphan before `-Rns`.
- **Verdict:** No injection risk from search or form inputs in audited paths. **Pass.**

---

## Pillar 2: Rust Engine (Safety & Performance)

### Unwrap / expect audit

| Area | Count | Risk | Action |
|------|-------|------|--------|
| **monarch-helper/src/main.rs** | 8 | High | Temp file + JSON parse in MONARCH_CMD_FILE path use `.expect()`; IO or bad JSON can panic. Replace with error emission to GUI. |
| **monarch-gui/src/helper_client.rs** | 7 | Medium | Same pattern (serialize, temp file, read). GUI side; failure can bubble as Err. Prefer `?` / map_err. |
| **monarch-gui/src/lib.rs** | 1 | Low | `.expect("error while running tauri application")` at app entry — acceptable. |
| **monarch-gui/src/utils.rs** | 2 | Low | Regex::new().expect (static); test assert. Acceptable. |
| **monarch-gui/src/repo_db.rs** | 4 | Medium | tempdir().unwrap(), result.unwrap() in logic paths. Replace with ? or explicit error. |
| **monarch-gui/src/metadata.rs** | 6 | Low | Regex expect (static); Mutex lock .expect("poisoned"). Poison is rare; acceptable. |
| **monarch-gui/src/metadata_fix.rs** | 2 | Low | lock().unwrap(); same as above. |
| **monarch-gui/src/error_classifier.rs** | 4 | Low | In tests only (from_output). OK. |
| **monarch-gui/src/reviews.rs, commands/reviews.rs** | 2 | Low | .unwrap() on optional; verify context. |
| **monarch-gui/src/scm_api.rs** | 1 | Low | Regex::new().unwrap() (static). OK. |
| **monarch-gui/src/mocks.rs, repo_db_tests.rs** | 6+ | — | Test code. OK. |
| **tauri-plugin-aptabase** | 8 | Low | Third-party; URL parse, lock. Acceptable for plugin. |
| **monarch-helper/tests** | 12+ | — | Test code. OK. |

**Summary:** ~74 total. **Critical:** Helper main.rs command-file read path (expect on temp file and JSON). **Recommended:** Replace expect with error emission in helper + helper_client for command submission; optionally harden repo_db unwraps. **Release:** GO with documented debt; fix helper command-path expects in follow-up.

### Hardcoded paths

- **Scan:** `/home/`, `/usr/`, `C:\` in `src-tauri/**/*.rs`.
- **Result:** Paths are almost all **intentional**: `/usr/lib/monarch-store/monarch-helper`, `/usr/share/polkit-1/actions/...`, `/usr/bin/pkexec`, `/usr/bin/pacman`, `/usr/bin/pacman-mirrors`, `/usr/share/app-info/`, `/usr/share/icons/...`, `/usr/share/applications`, `/var/cache/pacman/pkg`, `/var/lib/pacman`, etc. These are standard FHS and policy locations; no arbitrary `/home` or Windows paths in production logic.
- **Verdict:** **Clean** for release; hardcoded paths are installation/policy/ALPM standards only.

### Resource leaks

- File handles: Temp files use `NamedTempFile` (auto-remove on drop). Normal I/O uses scope-bound handles.
- Threads: Progress writer thread in helper; channel-based; no unbounded spawn found. **No action.**

### Command injection

- **Command::new()** usages: Arguments passed as `.arg(...)`; no `sh -c` with user input in audited paths. Scripts in repo_setup/repair use templated strings (e.g. `{{HELPER_SOURCE}}`); user input is validated (e.g. `validate_package_name`) before being passed to pacman/orphans. **Pass.**

---

## Pillar 3: Security & Permissions

### Tauri capabilities

- **File:** `src-tauri/monarch-gui/capabilities/default.json`.
- **Scope:** `fs:allow-read` restricted to `$CACHE/monarch-store/**` only. No `**` or broad filesystem.
- **Verdict:** **Restricted;** appropriate for release.

### Helper isolation

- **Interaction:** All privileged commands go through `HelperCommand` enum; parsed from JSON (file or stdin). No raw string execution.
- **Validation:** Helper rejects non-JSON stdin; invalid JSON returns structured error to GUI. WriteFile/WriteFiles restricted to `/etc/pacman.d/monarch/` in helper.
- **Verdict:** **HelperCommand-only;** no open door found.

---

## Pillar 4: Arch Linux Integration

### Cache management

| Item | Status | Notes |
|------|--------|--------|
| **db.lck** | OK | At startup the app calls `needs_startup_unlock()` then `unlock_pacman_if_stale` (RemoveLock via Helper); when **Reduce password prompts** is on, the in-app password is used so the system prompt does not appear at launch; cancel flow clears lock. |
| **Pacman package cache** | Future debt | Helper has `ClearCache { keep }` and clears `/var/cache/pacman/pkg`. GUI `clear_cache` command clears **in-memory** caches only (metadata, chaotic, flathub, scm, repo sync). Settings "Clear Cache" does **not** call Helper ClearCache. **Action:** Document as future: "Expose Helper ClearCache (disk) in Settings or Maintenance." |

### Orphan handling

- **remove_orphans:** Uses `-Rns` (recursive, no save of reason, --noconfirm). Package names validated with `validate_package_name` before invocation. Aligns with Arch practice (remove with unused deps). **Pass.**

### Repo conflicts

- No explicit check for "chaotic-aur vs existing user repo" in repo_manager. Enabling chaotic does not overwrite `/etc/pacman.conf`; modular config in `/etc/pacman.d/monarch/`. **No blocking issue;** optional improvement to document.

---

## Pre-Flight Summary Table

| Category | Item | Status | Action Required |
|----------|------|--------|-----------------|
| **UX** | Console logs | 22 (warn/error) | Optional: replace with ErrorService; does not block. |
| **UX** | TODO/FIXME | 0 blocking | None. |
| **UX** | Error handling (UI) | OK | User-facing uses friendlyError/ErrorContext. |
| **UX** | Input sanitization | OK | Search/orphans validated; no shell injection. |
| **Rust Safety** | .unwrap() / .expect() | ~74 total | **Critical path:** Helper main.rs + helper_client.rs command-file/JSON path. Replace with error emission / `?`. Others in tests, static init, or low probability (Mutex poison). |
| **Rust Safety** | Hardcoded paths | Intentional only | None; FHS/policy paths only. |
| **Rust Safety** | Resource leaks | OK | Temp files and threads scoped. |
| **Rust Safety** | Command injection | OK | .arg(); validate_package_name; no sh -c user input. |
| **Security** | Tauri capabilities | Restricted | fs: $CACHE/monarch-store/** only. |
| **Security** | Helper isolation | HelperCommand only | No raw execution. |
| **Arch** | db.lck / lock | OK | Startup unlock; cancel + RemoveLock. |
| **Arch** | Pacman cache clean | Future debt | Helper has ClearCache; Settings "Clear Cache" is in-memory only. Document expose in UI. |
| **Arch** | Orphan remove | OK | -Rns; validate_package_name. |
| **Arch** | Repo conflicts | No explicit check | Optional doc improvement. |

---

## Final Verdict

**GO** for Release v0.3.5-alpha, with the following conditions:

1. **Known debt — verified resolved:**
   - **Helper / helper_client command path:** Production code in `monarch-helper/src/main.rs` and `monarch-gui/src/helper_client.rs` already uses `Result`/`match`/`if let Ok` and emits errors to the GUI (e.g. `emit_progress(0, "Error: ...")`) instead of panicking. All `.expect()` usages are inside `#[cfg(test)]` only; test code may panic. **No code change required.**

2. **Optional (non-blocking):**
   - Console: optionally migrate remaining `console.warn`/`console.error` to ErrorService for production consistency.

3. **Expose Helper ClearCache — done.** Settings → Maintenance "Clear Cache" now runs (1) in-memory app caches via `clear_cache`, then (2) disk pacman cache via `clear_pacman_package_cache` (Helper `ClearCache { keep }`). One click clears both.

4. **No blocking issues:** No unsafe input to shell, no overbroad capabilities, no sudo in GUI, orphan removal and lock handling are correct.

**Signed:** Omegascope Audit — Pre-Flight Complete.

---

## Future work

| Item | Description | Status |
|------|-------------|--------|
| **Expose Helper ClearCache in Settings** | Users can clear pacman package cache from the app. | **Done.** Settings → Maintenance "Clear Cache" runs `clear_cache` (in-memory) then `clear_pacman_package_cache` (Helper `ClearCache { keep: 0 }` for `/var/cache/pacman/pkg`). |

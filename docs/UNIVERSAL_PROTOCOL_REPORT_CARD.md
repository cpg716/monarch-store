# Universal Protocol Report Card
**Audit Date:** 2026-02-02  
**Codebase:** MonARCH Store v0.4.0-alpha  
**Scope:** Backend (Rust), Frontend (React/TS), Configuration

---

## Summary

| Checkpoint | Result | Notes |
|------------|--------|-------|
| 1. Repo Safety | PASS (with caveats) | No repo injection; system config writes limited to explicit Settings actions |
| 2. Native Builder | PASS | makepkg runs as user; explicit root guard; PGP key auto-import |
| 3. Silent Guard | PASS | Updates use ExecuteBatch; installs are one-at-a-time by design |
| 4. Unified State | PASS | Search deduplication, friendly labels, distro-aware |
| 5. Zombie Code | PASS | Wrapper removed; pkexec pacman used directly |

---

## PASS: Protocols Correctly Implemented

### Checkpoint 1: Repo Safety Protocol
- **No repo injection:** `repo_manager.rs` explicitly states: "We no longer modify pacman.conf or manage .conf files directly via HelperCommand::{WriteFiles, RemoveFiles}."
- **apply_os_config** only triggers `ExecuteBatch { refresh_db: true }` — no drop-in file writes.
- **Manjaro guard:** `set_repo_state` blocks enabling chaotic-aur on Manjaro (`ChaoticSupport::Blocked`).
- **Install guard:** `package.rs` blocks installing chaotic-aur/cachyos packages on Manjaro.
- **Host-adaptive discovery:** Repos are read from ALPM; `chaotic_enabled` is checked via `std::fs::read_to_string("/etc/pacman.conf")` (read-only).
- **SourcesTab:** Disables chaotic toggle when not in pacman.conf; tooltip: "Not available in /etc/pacman.conf."

### Checkpoint 2: Native Builder Protocol
- **makepkg never as root:** Explicit check in `package.rs` (lines 740–756): `if is_root { return Err("Security Violation: Attempted to run makepkg as root. This is forbidden.") }`
- **Build directory:** Uses `tempfile::tempdir()` — ephemeral temp, not system folders.
- **PGP keys:** Auto-import before retry when build fails with missing keys; keyservers tried in order.
- **stdin closed:** When no password, `Stdio::null()` so makepkg never blocks on read.

### Checkpoint 3: Silent Guard Protocol
- **Updates:** `perform_system_update` → single `ExecuteBatch { update_system: true, refresh_db: true }`.
- **apply_updates:** Single `ExecuteBatch` with full manifest.
- **apply_os_config:** Single `ExecuteBatch { refresh_db: true }`.
- **repair_unlock_pacman:** Uses `ExecuteBatch { remove_lock: true }`.
- **clear_pacman_package_cache:** Uses `ExecuteBatch { clear_cache: true }`.
- **Install flow:** One `install_package` per user action; no frontend loop of installs.

### Checkpoint 4: Unified State Protocol
- **Search deduplication:** `merge_and_deduplicate` in `utils.rs`; `package_map` in `search.rs` merges Official > Flatpak > AUR.
- **available_sources:** Populated for merged results; UI can show source selector.
- **Friendly labels:** `get_friendly_label` maps repo + distro to "Manjaro Official", "CachyOS (Optimized)", etc.
- **Distro context:** `DistroContext` drives label selection and Manjaro guard.

### Checkpoint 5: Zombie Code (Partial)
- **AUR:** Uses `raur` (native), not heavy HTTP wrappers.
- **ALPM:** Uses `alpm` crate for read/write.
- **monarch-permission-sanitizer.sh:** Complements Rust backend (cleans /tmp/monarch-install, cache); does not duplicate keyring/DB repair.

---

## WARNING: Suspicious or Partially Implemented

### 1. System Config Writes (Explicit User Actions)
- **set_parallel_downloads** (`system.rs` 611–617): Runs `sed -i` on `/etc/pacman.conf` to set ParallelDownloads. User-initiated from Settings.
- **rank_mirrors** (`system.rs` 774–781): Writes to `/etc/pacman.d/mirrorlist` via `reflector --save` or `rate-mirrors | sudo tee`. User-initiated.
- **optimize_system** (`system.rs` 278–288): Modifies `/etc/makepkg.conf` (COMPRESSZST, MAKEFLAGS). User-initiated.

**Interpretation:** These are documented Settings features, not silent repo injection. They do write to system configs. If the protocol is strictly "only read or manage strictly local config," these are violations. If the protocol allows explicit user-initiated tuning, they are acceptable. **Recommendation:** Document in SECURITY.md that these are privileged, user-initiated actions.

### 2. Stale Comment
- **apply_os_config** docstring: "Apply OS configuration (write repo configs to /etc/pacman.d/monarch/)" — implementation does not write; it only triggers refresh.
- **package.rs** (237): "Ensure monarch repo configs (e.g. 50-chaotic-aur.conf) are on disk" — no code writes those files anymore.

### 3. pacman-helper.sh Wrapper Uses RunCommand
- **Location:** `package.rs` 791–806.
- **Behavior:** When no password and helper exists, makepkg's PACMAN env is set to a wrapper script that generates `RunCommand` JSON and invokes `pkexec monarch-helper`.
- **HelperCommand:** Does **not** include `RunCommand`. Helper will fail to parse with "unknown variant `RunCommand`".
- **Impact:** AUR builds that need pacman during makepkg (e.g. -Si, -S for deps) would fail when using the wrapper. When helper doesn't exist, `PACMAN="pkexec pacman"` is used, which works. When password is provided, `PACMAN="sudo -A pacman"` is used, which works.
- **Recommendation:** Either add RunCommand support to helper (with strict validation) or remove the wrapper and always use `pkexec pacman` when no password. The wrapper is effectively dead code that breaks AUR builds in the "helper exists, no password" path.

---

## CRITICAL FAIL: Core Rule Violations

### 1. ~~AUR Build Path With Wrapper Sends Unsupported RunCommand~~ (FIXED)
- **Rule:** Build process must work without running as root.
- **Violation:** The pacman-helper.sh wrapper sends `RunCommand` JSON to monarch-helper. Helper does not define `RunCommand`; `serde_json::from_str::<HelperCommand>` will error with "unknown variant `RunCommand`".
- **Effect:** AUR builds that require pacman during makepkg (dependency install) fail when: (a) monarch-helper is installed, and (b) user does not provide password.
- **Fix Applied:** Removed the pacman-helper.sh wrapper; we now use `PACMAN="pkexec pacman"` when no password.

---

## Remediation Plan

### Critical
1. ~~**Fix AUR pacman wrapper**~~ **DONE:** Removed pacman-helper.sh wrapper; use `pkexec pacman` when no password.

### Warnings
2. ~~**Update comments**~~ **DONE:** Updated `apply_os_config` docstring and package.rs comment.
3. ~~**Document system config writes**~~ **DONE:** Added "Privileged Settings Actions" section to SECURITY.md.

### Optional
4. **Log helper parse errors:** When "unknown variant" occurs for RunCommand specifically, emit a clearer message. (Low priority; wrapper removed.)

---

## File Reference

| Area | Key Files |
|------|-----------|
| Repo / Config | `repo_manager.rs`, `commands/system.rs`, `repair.rs` |
| AUR Build | `commands/package.rs`, `aur_api.rs` |
| Helper | `monarch-helper/main.rs`, `transactions.rs` |
| Search | `commands/search.rs`, `utils.rs` (merge_and_deduplicate) |
| Frontend Install | `InstallMonitor.tsx`, `UpdatesPage.tsx` |

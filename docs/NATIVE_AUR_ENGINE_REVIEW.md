> Historical note: This review document describes a legacy or point-in-time parity snapshot. GTK current-state and release-gate status now live in `docs/GTK_TAURI_PARITY_MATRIX.md` and the GTK-first root docs.

# Native AUR Engine — Full Review

**Date:** 2026-02-05  
**Scope:** AUR discovery, dependency resolution, build pipeline (makepkg), install handoff, security, and integration with repo/Flatpak.

---

## 1. Architecture Overview

| Layer | Responsibility |
|-------|----------------|
| **aur_api.rs** | AUR RPC via `raur` crate (search, info, get_candidate_updates). Single shared `Handle`. No direct git/pacman. |
| **package.rs** | Build pipeline: `build_aur_package` → dependency resolution → `build_aur_package_single` (clone, makepkg, PGP recovery, artifact discovery). Copy to `/tmp/monarch-install`, then invoke Helper `AlpmInstallFiles`. |
| **Helper** | Only installs built `.pkg.tar.zst` from allowed prefix `/tmp/monarch-install`. No makepkg, no clone. Auth: `AlpmInstallFiles` is invoked with `one_click` from RepoManager (branded prompt or Polkit). See `docs/RECENT_CHANGES.md` §8. |
| **update.rs** | Batch AUR updates: `check_aur_updates()` (truly AUR-only filter), build each, `install_built_packages` (copy + AlpmInstallFiles). |

**Iron rule:** makepkg runs only in the GUI process, as the unprivileged user. The Helper never runs makepkg or git.

---

## 2. Discovery & Eligibility

### 2.1 AUR search and info

- **aur_api.rs:** `search_aur` (raur), `get_multi_info(names)` for metadata and dependency lists. Uses a static `AUR_HANDLE` (raur).
- **Package name validation:** Before install, `validate_package_name(name)` is called (utils.rs): regex `^[a-zA-Z0-9@._+\-]+$`. Rejects shell metacharacters and path traversal.

### 2.2 Update eligibility (“truly AUR only”)

- **package.rs `is_in_sync_repos(name)`:** Uses `alpm_read::is_package_in_syncdb`. If the package exists in any sync DB (core, extra, chaotic-aur, cachyos, etc.), it is not built from AUR.
- **update.rs:** After `check_aur_updates()`, filters to `truly_aur_only` via `is_in_sync_repos` so packages that moved to Chaotic/CachyOS are not built from AUR (avoids makepkg vs repo mismatch).

### 2.3 Version comparison for updates

- **get_candidate_updates:** Compares local (foreign) version to AUR `pkg.version` with **string inequality**. No `vercmp`; downgrades can appear as “updates.” Documented in UPDATE_SYSTEM_REVIEW as a known limitation.

---

## 3. Build Pipeline (build_aur_package)

### 3.1 Pre-flight

- **audit_aur_builder_deps:** Requires `base-devel` and `git` installed (alpm_read). Emits a clear error and returns if missing.
- **resolve_aur_dependencies:** Recursive resolution for the requested package and its AUR-only deps (depends + make_depends). Version constraints stripped (e.g. `libfoo>=1.0` → `libfoo`). Skips dep if `is_dep_satisfied` (already installed) or `is_in_sync_repos` (available from repo). Build order: deps first, then the requested package. No cycle detection beyond “visited” set (repeated name skips).

### 3.2 Per-package build (build_aur_package_single)

1. **Temp dir:** `tempfile::tempdir()` — process-private, cleaned on drop.
2. **Clone:** `git clone --depth 1 https://aur.archlinux.org/{name}.git` in that temp dir. No user-controlled URL; name comes from validated package name.
3. **Privilege priming:** If password is provided, `sudo -S -v` to refresh timestamp; optional `SUDO_ASKPASS` script written under the temp dir with the password and 0o700. Used so makepkg’s pacman (build deps) can run without an interactive prompt.
4. **Security check:** On Linux, explicit check that effective UID is not 0. If root, returns error: "Security Violation: Attempted to run makepkg as root."
5. **makepkg:**  
   - Args: `-s -r --noconfirm --needed` (-s sync deps, -r remove make deps after build).  
   - Env: `MAKEFLAGS=-j{N}`, `PKGEXT=.pkg.tar.zst`, `PACMAN=pkexec pacman` (or `sudo -A pacman` when askpass is set).  
   - Cwd: temp dir’s `{name}` (clone output).  
   - Stdout/stderr streamed to `install-output`; stderr lines prefixed with `MAKEPKG:`.  
   - Progress: percentage-like tokens in stderr parsed and emitted as `update-progress` (download).
6. **PGP recovery:** If makepkg fails and missing key IDs are detected in stderr (unknown public key, not found in keychain, etc.), keys are imported via `gpg --keyserver ... --recv-keys` (keyserver.ubuntu.com, keys.openpgp.org, pgp.mit.edu). Then `rm -rf src pkg` in pkg_dir and makepkg is re-run once. The native engine in **monarch-core** implements this same PGP recovery flow (key detection from stderr, multi-keyserver import, single retry). If retry still fails or no keys could be imported, a clear error is returned (e.g. “PGP verification failed. Could not import required keys: …”).
7. **Non-PGP failure:** If build failed and no missing keys were found, last “ERROR:” line is surfaced; if it’s “unknown error has occurred,” the message suggests base-devel, git, and `scripts/monarch-permission-sanitizer.sh`.
8. **Artifact:** Directory is scanned for `*.pkg.tar.zst`; the first match is returned as the built package path. No multi-package handling (split packages yield one path; that’s acceptable for install).

### 3.3 Handoff to Helper

- **copy_paths_to_monarch_install(built_paths):** Creates `/tmp/monarch-install`, copies each built file into it by filename, returns the list of destination paths.
- **invoke_helper(AlpmInstallFiles { paths }):** Helper receives only paths under `/tmp/monarch-install`. It canonicalizes each path and rejects any path not under that prefix. Then `execute_alpm_install_files` loads each as a local package and runs a single transaction (prepare + commit). No network; no makepkg.

---

## 4. Security

### 4.1 Makepkg never as root

- Explicit `id -u` check in the GUI; build is aborted with a clear error if run as root.
- Helper has no code path that runs makepkg or git.

### 4.2 Package name and paths

- **Package name:** Validated with `validate_package_name` (regex) before install. AUR clone URL is built from that name only (no user-controlled URL).
- **Helper:** Only installs files under `/tmp/monarch-install`; canonicalization and prefix check prevent escape.

### 4.3 Password and askpass

- When “reduce password prompts” is used, the password is written into a temporary askpass script under the build temp dir (mode 0o700). The script is short-lived (temp dir is process-private and removed when the function returns). Risk: password on disk for the duration of the build; acceptable for the documented one-click flow. No password is sent to the Helper for the AUR path; Helper uses Polkit for AlpmInstallFiles.

### 4.4 Dependency resolution

- Dep names are taken from AUR metadata (depends / make_depends). Version constraints are stripped; only the name is used for “satisfied” and “in sync repos” checks. No arbitrary commands or URLs.

---

## 5. Integration Points

### 5.1 Install flow (single package)

- **install_package_core** with `source_type == "aur"`: validates name, calls `build_aur_package`, `copy_paths_to_monarch_install`, then `invoke_helper(AlpmInstallFiles)`. Streams helper progress to `install-output`. Does not emit `install-complete` (that is left to the generic success path after verification). For AUR, verification is ALPM-based (package must be in localdb); AUR packages are in localdb after install, so this is correct.

### 5.2 Update flow (batch)

- **run_system_update_impl** (when include_aur): `check_aur_updates()` returns only packages not in sync repos. For each, `build_aur_package`; build failures are logged and the package is skipped. All built paths are collected and installed in one `install_built_packages` (copy + AlpmInstallFiles). So one transaction for all built AUR packages.

### 5.3 apply_updates (selective)

- When targets include AUR items, each is built and paths accumulated; then one `install_built_packages` call. Same pattern as the full update.

---

## 6. Cancel and Cleanup

### 6.1 cancel_install (InstallMonitor Cancel button)

- **repair.rs:** Writes `/var/tmp/monarch-cancel`, waits 1.5s, then calls `repair_unlock_pacman`. This is intended for the **Helper** (which watches the cancel file and exits). It does **not** kill the GUI-side makepkg process.
- **ACTIVE_INSTALL_PROCESS** in package.rs is only used in `abort_installation`. The AUR build never registers the makepkg `Child` in that mutex. So when the user clicks Cancel during an AUR build, the Helper (if it were running) could be signaled, but the running makepkg in the GUI is **not** terminated. The build continues until it finishes or fails; then the GUI may try to invoke the helper with the built paths. **Gap:** Cancel during the AUR build phase does not stop makepkg.

### 6.2 Temp dir and /tmp/monarch-install

- Each build uses a new temp dir; it is dropped at the end of `build_aur_package_single`, so clone and build artifacts are removed. `/tmp/monarch-install` is only written by the GUI and read by the Helper; the permission sanitizer script can reset it if needed.

---

## 7. Error Handling and UX

- **Missing base-devel/git:** Clear message and early return.
- **Clone failure:** “Failed to clone {name} from AUR.”
- **PGP:** Automatic key import and one retry; if that fails, message lists keys and suggests manual import.
- **Unknown error:** Suggests base-devel, git, and `scripts/monarch-permission-sanitizer.sh`.
- **No .pkg.tar.zst found:** “Could not find built package in …”
- All build and helper output is streamed to `install-output` (and progress heuristics in InstallMonitor).

---

## 8. Gaps and Recommendations (Addressed)

### 8.1 Cancel during AUR build — **Fixed**

- **Gap:** Cancel only affects the Helper. The makepkg process in the GUI is not tracked in `ACTIVE_INSTALL_PROCESS` (or equivalent), so it is not killed when the user cancels.
- **Recommendation:** Register the makepkg `Child` (or the current build task handle) in a shared place (e.g. a dedicated “AUR build process” mutex or a cancel token) so that `cancel_install` or a dedicated “abort AUR build” path can kill it. Alternatively, have the frontend call `abort_installation` when canceling during install and ensure the AUR build stores its child in `ACTIVE_INSTALL_PROCESS` for the duration of the build.

### 8.2 AUR version comparison

- **Gap:** Update eligibility uses string inequality; no `vercmp`. Can show “update” for downgrades or odd version strings.
- **Recommendation:** Use ALPM-style version comparison (e.g. expose vercmp from alpm_read or use a small version crate) when deciding if an AUR package is an upgrade.

### 8.3 Dependency cycle

- **Gap:** Resolution uses a “visited” set to avoid infinite recursion but does not detect cycles that span multiple packages (e.g. A→B→C→A). In practice AUR metadata rarely has such cycles; if it did, we could loop until a max depth.
- **Recommendation:** Add a max depth or explicit cycle detection if we see real-world cycles.

### 8.4 Askpass script and password lifetime

- Password is written to a file in the temp dir. Temp dir is unlinked when the function returns; the script is only used during makepkg. Acceptable for the current design; document that “reduce password prompts” keeps the password in a short-lived script for the duration of the build.

### 8.5 Split packages

- Only one `.pkg.tar.zst` is taken per package directory. If a PKGBUILD produces multiple packages (split packages), only the first found is installed. Some AUR packages ship multiple .pkg.tar.zst files.
- **Recommendation:** Collect all `.pkg.tar.zst` in the pkg_dir and return a list; then copy and install all of them. This may require changing the return type of `build_aur_package_single` to `Vec<String>` and flattening in `build_aur_package`.

---

## 9. Summary Table

| Area | Status | Notes |
|------|--------|--------|
| makepkg as user only | ✅ | Explicit root check; Helper has no makepkg |
| Package name validation | ✅ | Regex before install |
| Helper path restriction | ✅ | Only /tmp/monarch-install |
| Dependency resolution | ✅ | Recursive; skip satisfied and sync-repo; cycle + max depth |
| Truly AUR only (updates) | ✅ | is_in_sync_repos filter |
| PGP auto-import + retry | ✅ | Keyservers + one retry |
| Build deps (base-devel, git) | ✅ | Pre-flight audit |
| Cancel during AUR build | ✅ | Child in ACTIVE_INSTALL_PROCESS; frontend calls abort_installation |
| AUR version comparison | ✅ | alpm_read::vercmp_greater in get_candidate_updates |
| Split packages | ✅ | All .pkg.tar.zst collected and installed |
| Dependency cycle | ✅ | Stack-based cycle detection; max depth 64 |

Overall, the Native AUR engine is well aligned with Arch and project rules: makepkg runs as the user, build is in the GUI, and the Helper only installs from a restricted directory. The main improvements are cancel behavior during build, version comparison for updates, and handling of split packages and dependency cycles where needed.

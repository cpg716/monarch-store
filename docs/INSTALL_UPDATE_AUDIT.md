# Install & Update Audit: Why It Fails and Passwordless Polkit

**Last updated:** 2026-02-01 (v0.3.6-alpha)

This document audits the install and update flows, root causes of failures, and how to make installs/updates work **without** a password prompt when Polkit is configured. For a comparison with Apdatifier, Octopi, and CachyOS packageinstaller, see [Install & Update Comparison](INSTALL_UPDATE_COMPARISON.md).

> [!CAUTION]
> **ALPHA RELEASE NOTICE**: The installation and update engine described herein is in an **Experimental Alpha** state. While the architecture is designed for robustness, users may encounter synchronization failures or transaction errors. Please use with caution on production systems.

---

## 1. Architecture Overview

| Layer | Role |
|-------|------|
| **Frontend** | `InstallMonitor.tsx` → `invoke('install_package', …)` or `invoke('perform_system_update', …)` |
| **Backend (GUI)** | `commands/package.rs` (install), `commands/update.rs` (system update), `commands/system.rs` (update_and_install) |
| **Helper client** | `helper_client.rs` → writes JSON to temp file in `/var/tmp`, spawns `pkexec <helper_bin> <cmd_file_path>` or `sudo -S <helper_bin> <cmd_file_path>`. **Command is always delivered via file path** (pkexec does not reliably forward stdin). |
| **Privilege** | `pkexec` (no password) or `sudo -S` (password on stdin) |
| **Helper (root)** | `monarch-helper`: Uses **SafeUpdateTransaction** (Iron Core) to run `-Syu` logic natively. |

Polkit matches the **first argument** to pkexec (the program path) against `org.freedesktop.policykit.exec.path` in the policy. Only when the path **exactly** matches does the action (e.g. `com.monarch.store.package-manage`) apply, including passwordless rules.

---

## 2. Why Installing Fails

### 2.1 Command Delivery: Always Use Temp File (v0.3.6)

- **Symptom (previous):** Install/update appeared to start but nothing happened, or "Invalid input on stdin".
- **Cause:** When using pkexec (no password), the GUI sent the command on **stdin**. Many systems do **not** forward stdin to the pkexec child (Polkit/security), so the helper never received the command.
- **Fix (current):** Command is **always** written to a temp file in `/var/tmp` and the **file path** is passed as the helper’s first argument for both pkexec and sudo -S. The helper reads JSON from the file, parses it, then deletes the file. Install and update now work reliably with pkexec.

### 2.2 Polkit Path Mismatch (Dev vs Production)

- **Policy says:** `org.freedesktop.policykit.exec.path` = `/usr/lib/monarch-store/monarch-helper`
- **Fix (v0.3.6):** `helper_client` **prefers the production helper** when it exists. So when the package is installed, we always run `pkexec /usr/lib/monarch-store/monarch-helper <cmd_file_path>` → path matches policy → correct action and passwordless rules apply.
- **Pure dev (no package installed):** We use the dev helper (`target/debug/monarch-helper`). Its path does **not** match the policy, so Polkit may prompt or deny. Use **Reduce password prompts** (Settings) and enter your password once so we use `sudo -S` and the same file-based command delivery.

### 2.3 Double Invocation (Fixed Previously)

- **Symptom:** Password asked twice; "Starting ALPM Transaction" and errors appear twice.
- **Cause:** React 18 Strict Mode runs effects twice; the auto-start effect called `handleAction()` twice → two `install_package` calls.
- **Fix (current):** `InstallMonitor` uses a ref (`actionStartedForRef`) so we only start the install once per package; ref is reset when `pkg` is cleared.

### 2.4 Verification After Helper Exits

- **Symptom:** `Installation reported success but package 'X' is not installed`.
- **Cause:** Helper stream is drained; then we run a **post-install check** (`pacman -Q <name>`). If the helper failed (e.g. Invalid JSON earlier) or didn’t actually install, verification fails and we show this message. So this is a consequence of the helper failing, not a separate bug.

---

## 3. Why Updating Fails

### 3.1 System Update (`perform_system_update`)

- **Flow:** `update.rs` runs in two phases:
  1. **Phase 2 (repos):** `invoke_helper(HelperCommand::Sysupgrade)` → same temp-file + pkexec as install. Updates all packages from enabled sync repos (Arch, Chaotic, CachyOS, etc.).
  2. **Phase 3 (AUR-only):** `check_aur_updates()` gets foreign packages (`pacman -Qm`) with a newer AUR version, then **filters out** any package that exists in a sync repo (`pacman -Si`). Only the remaining (truly AUR-only) packages are built with makepkg and installed via `AlpmInstallFiles`. This avoids building from AUR packages that are available as pre-built in Chaotic/CachyOS.
- **Same issues as install:** Path must match policy for Polkit; temp file must be readable by root. With the current temp-file approach and correct helper path, this should work when install does.

### 3.2 “Update and Install” (`update_and_install_package`)

- **Previous bug:** Only ran `HelperCommand::Sysupgrade` (full system upgrade) and did **not** install the requested package by name.
- **Fix (current):** After Sysupgrade completes, we now run `AlpmInstall { packages: vec![name], sync_first: false, enabled_repos, cpu_optimization }` so the named package is actually installed or upgraded.

---

## 4. Password Prompt: Why It Asks and How to Remove It

### 4.1 How Polkit Is Used

- **Install/update privilege:** `helper_client::invoke_helper` runs:
  - `pkexec <helper_bin> <cmd_path>` (no password), or
  - `sudo -S <helper_bin> <cmd_path>` (password on stdin if provided).
- **Policy:** `com.monarch.store.policy` defines:
  - `com.monarch.store.script` → path = `monarch-wrapper` (for script-based operations).
  - `com.monarch.store.package-manage` → path = `/usr/lib/monarch-store/monarch-helper` (for direct helper).
- **Defaults in policy:** `allow_active` = `auth_admin_keep` (prompt once, then keep for a while). So by default Polkit **does** ask for a password for the active session unless overridden.

### 4.2 Making It Passwordless (No Prompt)

Two mechanisms:

**A) Policy default**

- In `install_monarch_policy` (system.rs), when “one click” is enabled we write `<allow_active>yes</allow_active>` so the **default** for the active session is “allow without auth”.
- That only applies after the user has run “Install policy” (or onboarding) with “one click” enabled. The **bundled** policy file in the repo still has `auth_admin_keep`.

**B) Polkit rule (recommended)**

- Rules in `/usr/share/polkit-1/rules.d/` override policy defaults.
- Current `10-monarch-store.rules` only handles `com.monarch.store.script` (wrapper), not `com.monarch.store.package-manage` (helper).
- **Fix:** Add a rule for `com.monarch.store.package-manage`: for subjects in the `wheel` group (or active session), return `polkit.Result.YES`. Then running `pkexec /usr/lib/monarch-store/monarch-helper …` will not ask for a password for those users.

**Requirements for passwordless install/update:**

1. **Installed helper** at `/usr/lib/monarch-store/monarch-helper` (so pkexec path matches the policy).
2. **Policy** installed at `/usr/share/polkit-1/actions/com.monarch.store.policy` with action `com.monarch.store.package-manage` and `exec.path` = `/usr/lib/monarch-store/monarch-helper`.
3. **Rule** in `/usr/share/polkit-1/rules.d/` that returns `YES` for `com.monarch.store.package-manage` for the desired users (e.g. wheel).
4. **GUI** uses the installed helper path when it exists (so we don’t invoke a dev path that doesn’t match the policy).

### 4.3 Reduce password prompts & startup unlock

- **Reduce password prompts** (Settings → Workflow & Interface): When enabled, the user can enter their password once in a MonARCH dialog; it is used for installs, repairs, and **startup unlock** for the session (~15 min), not persisted. The password is sent to the app and used with `sudo -S` when invoking the helper; it is less secure than using the system (Polkit) prompt each time.
- **Startup unlock**: At launch the app calls `needs_startup_unlock()`. v0.3.6 features an atomic lock check within `SafeUpdateTransaction` to ensure transactions never conflict with existing pacman instances.

---

### 4.4 Repo toggle and sync reliability

- **Repo toggle:** When enabling or disabling a repo (Settings or onboarding), the backend runs `apply_os_config` (writes monarch confs, then invokes Helper `ForceRefreshDb`). Errors from `apply_os_config` are now propagated to the frontend; e.g. `useSettings` `toggleRepo` / `toggleRepoFamily` use `getErrorService()?.reportError(e)` so the user sees a toast when sync fails after a repo change.
- **Sync retry:** The helper’s `execute_alpm_sync` retries once on transient network failures (e.g. "failed to retrieve", "connection", "timeout") with a short delay; `repo_manager::apply_os_config` retries `ForceRefreshDb` once on failure before returning an error.

---

## 5. Summary of Fixes (Done or Recommended)

| Issue | Status | Action |
|-------|--------|--------|
| Invalid JSON (long argv) | Done | Command via temp file; helper reads file. |
| Double install start | Done | Ref guard in InstallMonitor. |
| Polkit path mismatch in dev | Recommended | Prefer `MONARCH_PK_HELPER` when it exists. |
| Password prompt | Recommended | Polkit rule for `package-manage` → `YES` for wheel. |
| Update-and-install doesn’t install package | Done | After Sysupgrade, run AlpmInstall for the named package. |

---

## 6. File Reference

| File | Purpose |
|------|---------|
| `src-tauri/monarch-gui/src/helper_client.rs` | Builds command, writes temp file, spawns pkexec/sudo with helper path. |
| `src-tauri/monarch-gui/src/commands/package.rs` | `install_package`, `install_package_core`; calls `invoke_helper(AlpmInstall)` or `AlpmInstallFiles`. |
| `src-tauri/monarch-gui/src/commands/update.rs` | `perform_system_update` → `invoke_helper(Sysupgrade)`; then `check_aur_updates()` (filter by `is_in_sync_repos`) and AUR build/install for AUR-only packages. |
| `src-tauri/monarch-gui/src/commands/system.rs` | `update_and_install_package` (Sysupgrade only), `install_monarch_policy`, `check_security_policy`. |
| `src-tauri/monarch-helper/src/main.rs` | Parses args[1] as file path or inline JSON; runs ALPM. |
| `src-tauri/monarch-gui/com.monarch.store.policy` | Polkit action for monarch-helper (package-manage). |
| `src-tauri/rules/10-monarch-store.rules` | Polkit JS rule; currently only script action. |
| `PKGBUILD` | Installs policy, rules, helper, wrapper. |

---

## 8. Verification Checklist

After applying the recommended changes:

1. **Installed system:** Helper at `/usr/lib/monarch-store/monarch-helper`, policy and rules installed. Run install from the GUI → **one** pkexec, **no** password if rule is in place, install completes and `pacman -Q` shows the package.
2. **Dev (with installed helper):** Prefer installed helper when present → same behavior as above.
3. **Dev (no installed helper):** Use dev helper; password may still be required (path doesn’t match policy); install should at least succeed after entering password once.
4. **System update:** “Update All” runs Sysupgrade (repos) then AUR updates only for packages not in any sync repo; no freeze, success or clear error.
5. **Update and install:** After Sysupgrade, run AlpmInstall for the requested package so the named app is actually installed/upgraded.

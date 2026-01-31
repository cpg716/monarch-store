# Troubleshooting Guide 🛟

**Last updated:** 2025-01-31 (v0.3.5-alpha)

Common issues users encounter when using MonARCH Store.

**Install/Update not working or password prompts:** See [Install & Update Audit](INSTALL_UPDATE_AUDIT.md) for the full flow, Polkit setup, and passwordless configuration (Polkit rules, helper path, policy).

## 🖥️ "I don't see the latest Settings / UI changes"

**Cause:** The app you're running is using a **bundled frontend** from when it was built. If you're launching the **installed** MonARCH Store (e.g. from the desktop or `monarch-store` in PATH), that binary was built at **package install time** and has the frontend baked in. New UI (e.g. Performance & Hardware, Parallel Downloads, Mirror Ranking) only appears after that bundle is updated.

**Fix:**
- **From source (see latest UI immediately):** Run from the repo root: `npm run tauri dev`. This starts the Vite dev server and Tauri loads the app from it, so you see the current source including all Settings changes.
- **Installed package:** Rebuild and reinstall so the new frontend is bundled: `npm run tauri build` (or `makepkg -si` for the full package), then install the resulting binary/package.

## 🔑 GPG / Signature Errors

**Error:** `signature from "User <email>" is unknown trust` or `invalid or corrupted package (PGP signature)`.

**Cause:** The Arch Linux keyring is out of date, or the package signer's key isn't in your keyring.

**Fix:**
1. **Preferred**: Run a full system update (this refreshes the keyring as part of the transaction):
   ```bash
   sudo pacman -Syu
   ```
2. **Keyring-only** (if you must fix keys without a full upgrade):
   ```bash
   sudo pacman -Syu archlinux-keyring
   sudo pacman-key --init
   sudo pacman-key --populate archlinux
   sudo pacman-key --refresh-keys
   ```
   Note: MonARCH never runs `pacman -Sy` alone; use `-Syu` for any sync+install.

## 🔒 Database Locked

**Error:** `status: /var/lib/pacman/db.lck exists` or ALPM_ERR_DB_WRITE / "Database Locked".

**Cause:** Another package manager (pacman, yay, CachyOS updater, Octopi, etc.) is currently running, or a previous process crashed without cleaning up.

**In-app:** As of v0.3.5-alpha, MonARCH **auto-unlocks** when it detects a locked DB during install: it shows "Auto-unlocking…", removes the lock (via repair), and retries once. **At startup**, MonARCH checks `needs_startup_unlock()`; if a stale lock exists it runs `unlock_pacman_if_stale` so the next install/sync works. If **Reduce password prompts** is enabled (Settings → Workflow & Interface), MonARCH shows its own password box at launch when a stale lock is detected, instead of the system prompt. If you cancelled an install or the app crashed, reopening the app will clear the stale lock. Experts can see the raw ALPM message in the detailed log (Settings → General → Show Detailed Transaction Logs). You can also use **Settings → Maintenance → Advanced Repair → Unlock Database** to clear the lock manually.

**Manual fix:**
1.  **Check for running processes:**
    ```bash
    ps aux | grep pacman
    ```
2.  **If safe (no process running), remove the lock:**
    ```bash
    sudo rm /var/lib/pacman/db.lck
    ```

## ⚠️ "Target not found" (Multi-Repo)

**Error:** Search finds the app, but install fails with `target not found`.

**Cause:** You may have disabled the specific repository (e.g., CachyOS or Chaotic-AUR) that contains the package, or your local database is stale.

**Fix:**
1.  Go to **Settings > Repositories**.
2.  Ensure the repository is **Enabled**.
3.  Click **"Sync Repositories"** to refresh your local database.

## 🔧 AUR Build: "An unknown error has occurred"

**Error:** When installing or updating an AUR package, makepkg fails with `==> ERROR: An unknown error has occurred. Exiting...`.

**Cause:** Often due to: (1) build cache or `/tmp/monarch-install` owned by root from a previous run, (2) missing or incomplete `base-devel`/`git`, or (3) makepkg run as root (MonARCH forbids this).

**Fix:**
1. **Run the Permission Sanitizer** (as your normal user, not root):
   ```bash
   ./scripts/monarch-permission-sanitizer.sh
   ```
2. **Ensure the AUR toolchain is installed:**
   ```bash
   sudo pacman -S --needed base-devel git
   ```
3. Retry the install or update. If it still fails, check the install output log for PGP or dependency errors.

## 📡 "Error: Invalid JSON command" or "Command file not found"

**Error:** Update or install shows `[0] Error: Invalid JSON command in arguments` or `Error: Command file not found: /tmp/monarch-cmd-*.json`.

**Cause:** The GUI passes commands to the helper via a temp file in **`/var/tmp`** (not `/tmp`) so that when the app runs with systemd PrivateTmp, root (pkexec) can still read the same path. If the file is missing or the path is wrong, the helper reports "Command file not found" or "Invalid JSON command". When building AUR packages, the pacman wrapper also uses `/var/tmp` for its command file.

**Fix:**
1. Ensure the helper is the production binary: `/usr/lib/monarch-store/monarch-helper` (so Polkit policy and path match).
2. Run the Permission Sanitizer to clear stale command files in `/var/tmp` and `/tmp`: `./scripts/monarch-permission-sanitizer.sh`.
3. Retry; avoid running multiple updates at once. If you still see "Invalid JSON" with a preview in the message, the helper received bad or empty input—ensure no other process is removing command files and that `/var/tmp` is writable by your user.

## 📦 Sync databases corrupt ("Unrecognized archive format")

**Error:** Install or update shows `could not open file /var/lib/pacman/sync/core.db: Unrecognized archive format` or `could not open database`, then "Package not found" or "Sync databases are corrupt (Unrecognized archive format). Run 'sudo pacman -Syy' in a terminal to fix."

**Cause:** The sync databases in `/var/lib/pacman/sync/` (core.db, extra.db, multilib.db, etc.) are corrupted or incomplete—often from an interrupted download, disk full, or partial upgrade. Pacman cannot read them until they are re-downloaded.

**Fix (required before installs will work):**
1. In a terminal, run a **force** sync (re-downloads all sync DBs):
   ```bash
   sudo pacman -Syy
   ```
2. Optionally do a full upgrade:
   ```bash
   sudo pacman -Syu
   ```
3. Retry the install in MonARCH. After fixing DBs, installs should complete in normal time (seconds to a minute, not minutes).

**Prevention:** MonARCH (with the **updated** monarch-helper) uses force refresh when syncing, so corrupt DBs are replaced automatically. Ensure you have the latest monarch-store package so the helper is up to date (see "unknown variant AlpmInstall" below).

**Self-heal:** As of v0.3.5-alpha, during install MonARCH **silently** detects "Unrecognized archive format" or "could not open database", shows "Repairing databases…", runs force refresh (helper reads `/etc/pacman.conf` directly so recovery works even when ALPM is blind), then retries—no error pop-up. You can also trigger refresh manually in **Settings → Maintenance → Advanced Repair → Refresh Databases** (or the simple "Fix My System" flow). **Test Mirrors** (Settings → Repositories, per repo) shows top 3 mirrors with latency without changing system config.

## 🔄 "unknown variant `AlpmInstall`" or "helper is outdated"

**Error:** Install or update shows `unknown variant 'AlpmInstall', expected one of 'InstallTargets', 'InstallFiles', 'Sysupgrade', ...` or the helper says it does not support ALPM install/update. You may also see "Installation reported success but package 'X' is not installed."

**Cause:** The **monarch-helper** binary installed on your system (`/usr/lib/monarch-store/monarch-helper`) is from an older build and does not include the ALPM commands (AlpmInstall, AlpmUpgrade, etc.). The app (GUI) was updated but the helper was not, so they are out of sync.

**Automatic fallback:** As of v0.3.5-alpha, the app **automatically retries** with the legacy path (Refresh + InstallTargets) when it detects an outdated helper or when verification fails after a repo install. You may see "Installed helper is outdated; syncing and installing with legacy path" or "Package not found after install; retrying with legacy helper path" — installs can still succeed. For the best experience (sync+install in one transaction, CPU optimization), update the helper.

**Dev mode (tauri dev):** If you only run `npm run tauri dev` and still see `unknown variant AlpmInstall`, it means the GUI is accidentally using the **system** helper instead of the dev-built one. Ensure the dev helper exists and is selected:
- Run `npm run tauri dev` (it now builds `src-tauri/target/debug/monarch-helper` and forces dev helper use).
- Look for the log line: `Seeking helper at: .../src-tauri/target/debug/monarch-helper` in the install output.
- If you still see `/usr/lib/monarch-store/monarch-helper`, your shell has `MONARCH_USE_PRODUCTION_HELPER=1` set — unset it or run the script above.

**Fix (optional, recommended):** Update monarch-store so **both** the app and the helper are from the same version:
- **If you installed from the AUR or a package:** run `pacman -Syu monarch-store` (or your distro’s equivalent) so the package reinstalls/updates the helper.
- **If you built from source:** rebuild and reinstall so the helper binary is updated: `npm run tauri build` then install the resulting package, or copy the new `monarch-helper` to `/usr/lib/monarch-store/` if you install manually.
- **Quick fix (source only):** build and install just the helper (package name is `monarch-helper`, no trailing *s*): from repo root run `npm run tauri build` (or `cd src-tauri && CARGO_TARGET_DIR="${PWD}/src-tauri/target" cargo build --release -p monarch-helper` then copy), then `sudo cp src-tauri/target/release/monarch-helper /usr/lib/monarch-store/monarch-helper`

After updating, install and update will use the full ALPM path.

## 🐌 Slow Downloads

**Cause:** Your selected mirror is slow or geographicaly distant.

**Fix:** Update your system mirrorlist.
```bash
sudo reflector --latest 5 --sort rate --save /etc/pacman.d/mirrorlist
```
*(Requires `reflector` package)*

## 🔨 Build / Development

### Build stalls at 711/714 crates (Cargo deadlock)

**Symptom:** `npm run tauri dev` or `cargo build` appears to hang near the end (e.g. 711/714).

**Cause:** Previously, `monarch-gui/build.rs` ran `cargo build -p monarch-helper` during the GUI build. The parent Cargo process already holds the target-directory lock, so the child Cargo blocks forever → deadlock.

**Fix (in tree):** Use **`npm run tauri dev`** from the repo root. The npm script builds `monarch-helper` first in a separate Cargo run, then runs `tauri dev`. Do not run `cargo build` from inside `build.rs`. See `src-tauri/monarch-gui/build.rs` (it only calls `tauri_build::build()`).

### "Command get_chaotic_packages_batch not found"

**Symptom:** On startup or when opening Home/Category view, telemetry or UI reports: `Command get_chaotic_packages_batch not found` (or `get_chaotic_package_info`).

**Cause:** The frontend invokes these Tauri commands; they must be implemented in Rust and registered in `lib.rs` `invoke_handler`. In v0.3.5-alpha these were added.

**Fix:** Ensure you are on a version that includes `get_chaotic_package_info` and `get_chaotic_packages_batch` in `commands::search` and in the handler list in `monarch-gui/src/lib.rs`. If you still see the error after pulling, run `npm run tauri dev` (or rebuild) so the backend is up to date.

# Troubleshooting Guide 🛟

Common issues users encounter when using MonARCH Store.

**Install/Update not working or password prompts:** See [Install & Update Audit](INSTALL_UPDATE_AUDIT.md) for the full flow, Polkit setup, and passwordless configuration (Polkit rules, helper path, policy).

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

**Error:** `status: /var/lib/pacman/db.lck exists`.

**Cause:** Another package manager (pacman, yay, CachyOS updater, Octopi, etc.) is currently running, or a previous process crashed without cleaning up.

**Fix:**
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

## 🐌 Slow Downloads

**Cause:** Your selected mirror is slow or geographicaly distant.

**Fix:** Update your system mirrorlist.
```bash
sudo reflector --latest 5 --sort rate --save /etc/pacman.d/mirrorlist
```
*(Requires `reflector` package)*

# Install & Update: MonARCH vs Apdatifier, Octopi, CachyOS

**Last updated:** 2026-02-01 (v0.3.6-alpha)  
**Purpose:** Compare how MonARCH manages installs and updates to [Apdatifier](https://github.com/exequtic/apdatifier), [CachyOS packageinstaller](https://github.com/CachyOS/packageinstaller), and [Octopi](https://github.com/aarnt/octopi) so we can deliver **rock-solid, user-friendly** package and system management.

---

## 1. How Each App Handles Installs & Updates

### Apdatifier (KDE Plasma widget)

- **Role:** Update **notifier** and **launcher**, not an in-app package manager.
- **Installs/updates:** Does **not** run pacman or ALPM inside the app. It:
  - Checks for updates (Arch, Plasma widgets, Flatpak) via scripts.
  - Notifies the user (badge, list).
  - **Initiates a full system upgrade in the user’s chosen terminal** (e.g. Konsole, Alacritty, Kitty) with a command like `paru -Syu` or `yay -Syu`.
- **AUR:** Optional `paru`, `yay`, or `pikaur`; the actual upgrade runs in the terminal.
- **Takeaway:** “Rock solid” for them = reliable **notification** + **opening the right terminal with the right command**. No helper, no in-process ALPM; maximum transparency because the user sees the real command.

### Octopi (Qt / C++ Pacman front end)

- **Role:** Full GUI package manager (browse, install, remove, **system upgrade**).
- **Installs/updates:**
  - Uses **octphelper** (a separate helper binary) for privileged operations.
  - Uses **qt-sudo** (or similar) for privilege escalation.
  - Runs pacman **via the helper**; some code paths may call the **pacman binary** rather than libALPM everywhere ([FAQ](https://tintaescura.com/projects/octopi/faq/)).
- **System upgrade:** Often documented as **Ctrl+U** or “System upgrade” (sometimes described as `pacman -Su` in older docs). **Arch does not support partial upgrades;** `pacman -Su` without prior `-Sy` can be dangerous. Octopi has had [reports of upgrade issues](https://bbs.archlinux.org/viewtopic.php?id=281451) (e.g. boot/DM problems after upgrade).
- **AUR:** Supports pacaur, paru, pikaur, trizen, yay; can download yay-bin.
- **Takeaway:** Helper + privilege escalation is similar in spirit to MonARCH. MonARCH is **stricter** by enforcing **full -Syu** (sync + upgrade) via **SafeUpdateTransaction** and never running `-Sy` alone.

### CachyOS packageinstaller (code-reviewed)

- **Role:** Curated app installer for CachyOS (install from `pkglist.yaml`); also repo browser and “Upgrade all”.
- **Backend:** Rust (`backend-rustlib`) as a **staticlib** linked into the Qt app. Rust uses **ALPM only for prepare/display** (e.g. `prepare_add_trans`, `display_install_targets`) with `TransFlag::NO_LOCK`; it does **not** call `trans_commit()` — it only runs `trans_prepare()` then `trans_release()`.
- **How install/update actually run:** The Qt app runs **`pkexec pacman`** directly via a shell ([mainwindow.cpp](https://github.com/CachyOS/packageinstaller/blob/develop/src/mainwindow.cpp)):
  - **Install:** `pkexec pacman -S <names>` (no separate helper; command is the argv string).
  - **Uninstall:** `pkexec pacman -R --noconfirm <names>` or `pkexec pacman -R <names>`.
  - **Upgrade all:** `pkexec pacman -Syu`.
- **Why it “just works”:** There is **no helper binary** and **no command delivery** (no JSON, no temp file, no stdin). The process is `QProcess::start("/bin/bash", {"-c", "pkexec pacman -S " + names})`. Polkit sees `pacman` (or a generic exec), so there is no “helper path must match policy” issue. The only failure mode is pkexec/auth.
- **Trade-off:** They get no structured progress (just pacman’s raw stdout). For install they use `pacman -S` without an explicit sync first (stale DB can still cause issues). For upgrade they correctly use `-Syu`.
- **Takeaway:** We use a **helper** for atomic -Syu, structured progress, and lock checks. Our command delivery (temp file + argv) was the main failure point; we fixed it by **always** passing the command via a file. For users who still hit policy/path issues, we document a **CachyOS-style fallback**: run `pkexec pacman -S <pkg>` or “Update in terminal” (`sudo pacman -Syu`) so they can proceed without the helper.

---

## 2. Where MonARCH Is Already Top Tier

| Area | MonARCH | Comparison |
|------|---------|------------|
| **No partial upgrades** | **SafeUpdateTransaction** enforces full `-Syu` (sync + upgrade in one transaction). We **never** run `pacman -Sy` alone. | Octopi’s documented “System upgrade” has been associated with `-Su` in places; we explicitly avoid that. |
| **Lock safety** | **db.lck** checked in helper (`ensure_db_ready`) and again inside **SafeUpdateTransaction** before starting. Stale lock removal via self-healer + startup unlock. | Reduces “database locked” and concurrent-pacman issues. |
| **Privilege** | **monarch-helper** via **pkexec** (Polkit); command from temp file (path only in argv). No `sudo` in GUI for package ops. | Same idea as Octopi’s helper; we avoid arbitrary root execution. |
| **AUR safety** | AUR build in **GUI (user)** with makepkg; only **install** of built `.pkg.tar.zst` in **helper** from `/tmp/monarch-install/`. | Same “never makepkg as root” principle as best practice. |
| **Error recovery** | **execute_with_healing**, **ClassifiedError**, **ErrorService**, Smart Retry (stale DB → sync + full upgrade), corruption detection + Force Refresh. | More structured than “run and show log” in many front ends. |
| **Progress & UX** | Events over stdout; GUI **update-complete** / **update-progress**; 45s “waiting for auth” reminder; InstallMonitor with cancel. | Keeps UI responsive and informs the user. |

---

## 3. Gaps and Improvements (So We Stay “No Exceptions” Top Tier)

### 3.1 Transparency: “Update in terminal” (Apdatifier-style)

- **Gap:** Users who want to **see exactly what runs** (or who hit rare GUI/helper issues) have no built-in way to run the **same** operation in their own terminal.
- **Improvement:** Provide a clear “Update in terminal” path:
  - Show/copy the **exact** command we conceptually run: **`sudo pacman -Syu`** (or `paru -Syu` / `yay -Syu` if we ever offer that as an option).
  - **Copy to clipboard** + short toast: “Command copied. Paste in your terminal to run.”
  - Optional: “Open terminal” that launches the user’s terminal with that command (like Apdatifier’s “full system upgrade in selected terminal”).
- **Result:** Same safety as in-app (full -Syu), with maximum transparency and an escape hatch.

### 3.2 Sysupgrade path: Sync-before-upgrade

- **Current:** `execute_alpm_upgrade` does `syncdbs_mut().update(false)` then `SafeUpdateTransaction::new(alpm).with_targets(vec![]).execute()`.
- **SafeUpdateTransaction** does **not** sync; it assumes the caller has synced. So we’re already correct: sync once, then one atomic upgrade.
- **Action:** No change needed; doc/comment in code to state “sync is done by caller” so future edits don’t drop sync.

### 3.3 Clear user messaging when “Package manager busy”

- **Current:** ErrorService + ClassifiedError for “database locked” / “package manager busy”.
- **Improvement:** Keep messaging in **friendlyError.ts** and recovery modals clear: “Another app or terminal is using the package manager. Close it or wait, then try again.” Link to “Update in terminal” so users can run the command themselves if they prefer.

### 3.4 AUR long-running builds

- **Current:** InstallMonitor shows “Building from source…” and “Large packages can take several minutes. You can cancel to skip the rest.”
- **Improvement:** Already improved; consider adding optional “Run in terminal” for AUR installs (e.g. `paru -S <pkg>`) for power users who want to see build output live.

---

## 4. Summary Table

| Capability | Apdatifier | Octopi | CachyOS PI | MonARCH |
|------------|------------|--------|------------|---------|
| In-app system upgrade | No (launches terminal) | Yes | No (installer only) | Yes |
| Full -Syu enforcement | N/A (user runs command) | Unclear / -Su in docs | N/A | Yes (SafeUpdateTransaction) |
| Helper (root) | No | Yes (octphelper) | No (pkexec pacman directly) | Yes (monarch-helper) |
| Privileged install | N/A | Helper | `pkexec pacman -S` / `-Syu` | Helper + temp file (command path in argv) |
| libALPM in-process | No | Partial (pacman binary in places) | Yes (Rust: prepare/display only) | Yes (GUI read, helper write) |
| Lock check before upgrade | N/A | Unknown | Unknown | Yes (db.lck + stale removal) |
| “Update in terminal” / copy command | Yes (core feature) | No | No | Added in v0.3.6 (copy + optional open) |
| Error classification + recovery | Minimal | Basic | Basic | Yes (ClassifiedError, Smart Retry, Force Refresh) |

---

## 5. References

- [Apdatifier](https://github.com/exequtic/apdatifier) – KDE widget, update notifications, launch upgrade in terminal.
- [CachyOS packageinstaller](https://github.com/CachyOS/packageinstaller) – Curated app installer; Rust backend for prepare/display; install/update via `pkexec pacman -S` / `pkexec pacman -Syu` (no helper). See `src/mainwindow.cpp` (install/uninstall/on_push_upgrade_all) and `backend-rustlib/src/lib.rs`.
- [Octopi](https://github.com/aarnt/octopi) – Qt Pacman/AUR front end, octphelper, [FAQ](https://tintaescura.com/projects/octopi/faq/).
- [Arch partial upgrades](https://wiki.archlinux.org/title/System_maintenance#Partial_upgrades_are_unsupported) – Why we never run `-Sy` alone and always use full -Syu.

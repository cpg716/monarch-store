# MonARCH Store — User Guide 🚀

**Current version:** v0.4.5-alpha · **Last updated:** 2026-02-08

Welcome to MonARCH Store, the host-adaptive software manager for Arch Linux and its derivatives. This guide will help you understand how to use MonARCH and how it works under the hood.

---

## 1. What is MonARCH Store?

MonARCH is **not** just another software store. It is a "Host-Adaptive" manager, meaning it respects your system's existing configuration. Instead of forcing its own repositories or settings, it adapts to what you have already set up in `/etc/pacman.conf`.

It provides a unified interface for:
*   **Official Repositories** (Core, Extra, Multilib, etc.)
*   **AUR** (Arch User Repository)
*   **Flatpaks** (via Flathub)

---

## 2. Getting Started

### 🧭 Onboarding (first run)
On first launch, a short wizard guides you: **Welcome** (your distro and philosophy), **Source Manager** (Flatpak, AUR, Chaotic-AUR toggles—Chaotic is not available on some distros, e.g. Manjaro), **Chaotic-AUR Setup** (if supported—on Garuda/CachyOS it may show “Already enabled”; otherwise install keys and mirrors, then add the repo to pacman.conf), **Security & Privacy** (one prompt per session vs system dialog every time, and telemetry), **Theme** (Light/Dark, accent), and **Confirmation**. You can change these later in Settings.

### 🛸 The Dashboard
When you launch MonARCH, the Dashboard gives you a bird's-eye view of your system:
*   **Quick Search**: Find any app instantly.
*   **Updates status**: See if your system is up to date.
*   **Featured Apps**: Tailored suggestions based on your distribution.

### 🔍 Unified Search
Searching in MonARCH is powerful. When you type a query, MonARCH searches all three sources (Repos, AUR, Flatpak) simultaneously. If an app is available in multiple places (e.g., Firefox in official repos and as a Flatpak), it merges them into a single entry where you can choose your preferred **Source**.

---

## 3. Managing Applications

### 📦 Installing Apps
1.  Search for an app.
2.  Click the package to see details.
3.  Select your preferred **Source** (Official, Flatpak, or AUR).
4.  Click **Install**.
5.  If prompted, enter your password. With **Reduce password prompts** on (Settings), you’ll see MonARCH’s prompt once per session; with it off, you’ll see the system authentication (Polkit) dialog each time.

For **official repository** packages, MonARCH runs a full system upgrade first, then installs your package—this keeps your system consistent and avoids partial upgrades. If an install fails because databases are out of date, you will be prompted to run a system update; we do not upgrade in the background without your confirmation.

If an app is **only** available from Chaotic-AUR and you have not enabled that repo yet, the card shows **Setup Required** and a **Configure Source** button that opens Settings so you can complete the Chaotic-AUR setup (keys, mirrors, then add the repo to pacman.conf).

### 🗑️ Removing Apps
Navigate to your **Library**, find the application, and click **Uninstall**. For repository packages, MonARCH will also offer to remove "orphans" (dependencies that are no longer needed).

---

## 4. Updates: The Iron Core

MonARCH handles updates differently than most stores to ensure your system stays stable.

*   **Unified Updates**: We check all sources in parallel.
*   **The Safety Lock**: If any "Official Repo" package needs an update, MonARCH enforces a **full system upgrade** (`-Syu`). This prevents "partial upgrades," which are the #1 cause of breakage on Arch Linux.
*   **Built from Source**: AUR packages are marked with a special badge. Since these are compiled on your machine, they will take longer and use more CPU than standard updates.

---

## 5. 🛸 Mission Control (Settings)

Mission Control is where you fine-tune your MonARCH experience.

### 🦎 Sources
Enable or disable repositories. MonARCH automatically detects CachyOS, Garuda, or EndeavourOS specific repos.

**Chaotic-AUR:** MonARCH shows Chaotic-AUR as **Active** (in pacman.conf and in use), **Inactive** (not in pacman.conf), or **Blocked** (e.g. on Manjaro). To enable Chaotic-AUR: turn the toggle on, click **Install Keys & Mirrors** (MonARCH installs the keyring and mirrorlist), then add the repo block to `/etc/pacman.conf` as shown in the "Final Step" modal. The modal explains: open the file in a text editor (e.g. `sudo nano /etc/pacman.conf`), add the two lines at the end, save, then click **Check Again**. MonARCH never edits pacman.conf for you—you add the repo manually.

### 🛠️ AUR Builder
Settings for how your machine builds AUR packages. You can clean build directories automatically to save space or enable verbose logging if a build fails.

### 🩺 Maintenance & Repair
If something feels wrong (e.g., "Database locked" or GPG errors), use the **Advanced Repair** tools:
*   **Unlock Database**: Clears stale pacman locks.
*   **Fix Keyring**: Refreshes your system's security keys.
*   **Refresh Databases**: Force-syncs your repository metadata.

---

## 6. How it Works (For the curious)

MonARCH is built with a **Highly-Integrated Backend** and a **Dumb View Frontend**:
1.  **The Brain (Backend)**: The Rust backend handles all the heavy lifting—parallel searches, metadata hydration (icons, descriptions), and security checks. It acts as the single source of truth for all application data.
2.  **The View (Frontend)**: The beautiful interface you see is a "Dumb View." It doesn't guess metadata or manage complex state; it simply renders the enriched data provided by the backend via a type-safe contract (`bindings.ts`).
3.  **The Helper (Root)**: A privileged tool that handles system modifications (ALPM) separately from the user interface, ensuring maximum security and stability.

This architecture ensures that your experience is fast, unified, and inherently stable.

---

*Enjoy a simpler, safer, and faster Arch Linux experience!*

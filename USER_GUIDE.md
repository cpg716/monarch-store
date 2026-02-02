# MonARCH Store — User Guide 🚀

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
5.  If prompted, enter your password. MonARCH uses standard system authentication (Polkit).

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

### 🛠️ AUR Builder
Settings for how your machine builds AUR packages. You can clean build directories automatically to save space or enable verbose logging if a build fails.

### 🩺 Maintenance & Repair
If something feels wrong (e.g., "Database locked" or GPG errors), use the **Advanced Repair** tools:
*   **Unlock Database**: Clears stale pacman locks.
*   **Fix Keyring**: Refreshes your system's security keys.
*   **Refresh Databases**: Force-syncs your repository metadata.

---

## 6. How it Works (For the curious)

MonARCH is built with a **dual-brain** architecture:
1.  **The GUI (User)**: The beautiful interface you see. It runs as your normal user and cannot touch system files directly.
2.  **The Helper (Root)**: A small, high-security background tool that runs as root. When you click "Install," the GUI sends a command to the Helper, which then talks to the system's package manager (`libalpm`).

This ensures that your system remains secure while providing a modern, premium experience.

---

*Enjoy a simpler, safer, and faster Arch Linux experience!*

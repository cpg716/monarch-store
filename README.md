# MonARCH Store: Universal Arch Linux App Manager
> **The first Distro-Aware App Manager for Arch, Manjaro, and CachyOS.**

**Author:** [cpg716](https://github.com/cpg716) — developer and creator of MonARCH Store, with the help of AI coding tools.

**Last updated:** 2025-01-31 (v0.3.5-alpha)

## ⚠️ Alpha Disclaimer

**MonARCH Store is currently in early ALPHA.** 

While the core discovery and browsing experience is stable, the underlying **installation and update engine is experimental.** Users may encounter edge cases or failures depending on their specific system configuration, mirror health, or distribution. 

> [!WARNING]
> Use this software with caution on production systems. Always ensure you have a backup of your important data and be prepared to use the terminal (`pacman`) if a GUI operation fails.

---

A premium, distro-agnostic software center built with Tauri and React. MonARCH automatically detects your distribution and adapts its capabilities to ensure safety and performance.

![MonARCH Store Dashboard](screenshots/home.png)

## 📸 Gallery

<p align="center">
  <img src="screenshots/browse.png" width="45%" alt="Browse Categories">
  <img src="screenshots/library.png" width="45%" alt="Installed Library">
</p>
<p align="center">
  <img src="screenshots/settings.png" width="45%" alt="Settings Dashboard">
  <img src="screenshots/loading.png" width="45%" alt="Repository Sync">
</p>

## ✨ Features

### ⚡ Instant Downloads (Chaotic-First)
We prioritized speed above all else. MonARCH automatically detects if a package has a pre-built binary in **Chaotic-AUR** or **CachyOS** and serves that instead of forcing you to compile from source.
*   **Zero-Compile Updates**: Get AUR packages in seconds, not hours.
*   **Transparent**: You can always choose to "Build from Source" via the dropdown if you prefer.

### 🧠 Distro-Aware Intelligence
MonARCH adapts its behavior based on your specific OS:
*   **Manjaro Guard**: Automatically hides dangerous Arch-native repositories (like `chaotic-aur`) to prevent "Partial Upgrade" breakage on stable systems.
*   **Smart Sync**: Checks database freshness before downloading. If your DB is < 1hr old, we skip the sync for instant results.
*   **Universal UI**: The interface shifts between "Store Mode" (Discovery) and "Manager Mode" (Maintenance) based on context.

### 🚀 Hardware Optimization
MonARCH detects your CPU capabilities (AVX2, AVX-512) and automatically prioritizes **CachyOS v3/v4** repositories if available.
*   **10-20% Faster**: Python, compilers, and rendering apps run significantly faster.
*   **Automatic**: No configuration needed. If your CPU supports it, we use it.

### 🩺 Hardened System Health & Omni-User UX (v0.3.5)
MonARCH includes the "Butterfly" system engine and a **dual-core** experience: simple for beginners, transparent for experts.
*   **Intelligent Startup Probes**: Verifies `pkexec`, `git`, and `polkit` existence before the UI loads.
*   **Distro-Aware Optimization**: Automatically applies safety guards for Manjaro and performance mirrors for CachyOS.
*   **Unified Maintenance Wizard**: A single authorized repair flow for Keyring, Security Policies, and Repository synchronization.
*   **Self-Healing**: If corrupt sync databases or a locked DB are detected during install, the app silently repairs (e.g. "Repairing databases…", "Auto-unlocking…") and retries—no error pop-up for common cases. At startup, the app checks for a stale pacman lock (`needs_startup_unlock`); if one exists it is cleared automatically (via Helper RemoveLock). If **Reduce password prompts** is on (Settings → Workflow & Interface), startup unlock uses the in-app password dialog so the system prompt does not appear at launch. Install can be cancelled (Cancel button or close-with-warning); the lock is cleared after cancel.
*   **Glass Cockpit**: Settings → General offers **Show Detailed Transaction Logs**; Settings → Maintenance offers **Advanced Repair** (Unlock DB, Fix Keys, Refresh DBs, Clear Cache, Clean Orphans) and **Test Mirrors** per repo (latency in ms).

### 🛡️ Smart Repository Management
*   **Soft Disable Architecture**: Disabling a repo hides clutter but keeps system updates secure in the background.
*   **Chaotic Binary Support**: Native integration with Chaotic-AUR and CachyOS.
*   **Zero-Compile Experience**: Prioritizes pre-built binaries to save time and battery.

### 🧠 Intelligent Package Merging
Stop guessing which "firefox" is the right one. MonARCH intelligently merges results from all sources into a single, clean view.
*   **Unified Search**: Official, Chaotic, and AUR results in one card.
*   **De-Duplication**: We show you the *best* version by default.

### 🛡️ Safety First (Iron Core)
*   **Atomic Update Protocol**: All repo installs use a single transaction (`pacman -Syu --needed`) enforced by our **SafeUpdateTransaction** struct. We never run `pacman -Sy` alone.
*   **Lock Guard**: Atomic checks prevent operations when `/var/lib/pacman/db.lck` is present.
*   **GPG Automator**: Missing keys are imported automatically during install.
*   **PKGBUILD Inspector**: Review build scripts before installing from AUR.
*   **Polkit Integration**: Privileged operations use `monarch-helper` via `pkexec`; passwordless installs when Polkit rules are installed.

### 🦎 Native Desktop Integration (v0.3.6)
*   **The Chameleon**: Uses **XDG Portals** to accurately detect your system theme (Dark/Light) across all desktops (GNOME, KDE, Hyprland, Sway) without relying on legacy GTK signals.
*   **Wayland Ghost Protocol**: Automatically detects Wayland sessions and adjusts window rendering to prevent flickering and transparency artifacts (especially on Nvidia/KDE).
*   **Native Dialogs**: Portal-based file pickers (`rfd`) are planned; ensure `xdg-desktop-portal` is installed for theme detection and future native dialogs.
*   **Optional single-password mode**: In Settings → Workflow & Interface, **Reduce password prompts** lets you enter your password once in MonARCH for the session (~15 min).
65: 
66: ### ⭐ Hybrid Reviews & Rich Metadata
67: MonARCH combines the best of the web with the power of Arch:
68: *   **Smart Composition**: Automatically finds high-res icons and screenshots from Flathub even for native packages (without installing Flatpak).
69: *   **Hybrid Ratings**: Merges global ratings from **ODRS** (Gnome/KDE users) with local ratings from **MonARCH** users into a single score.
70: *   **365-Day Currency**: Ratings are strictly filtered to the last year so you always see the *current* state of the software.
71: *   **Source Badges**: Clearly see if a review comes from the global Linux community (Blue Badge) or a fellow MonARCH user (Purple Badge).
72: *   [Learn more about the Review System](docs/REVIEWS.md)

### ⚙️ Repository Configuration
You can personalize your store experience by toggling specific repositories (CachyOS, Manjaro, Chaotic-AUR) in the settings. MonARCH uses a **"Soft Disable"** architecture: disabling a repository hides it from search but keeps it active in the background for system updates, ensuring your installed apps always remain secure and up-to-date. **Test Mirrors** (Settings → Repositories) runs `rate-mirrors` (or reflector) per repo and shows the top 3 mirrors with latency (ms) without changing system config.

## 📘 Documentation
- [**Developer documentation**](docs/DEVELOPER.md) - Setup, architecture, code style, and critical rules for contributors.
- [Full App Audit](docs/APP_AUDIT.md) - Exhaustive UI/UX, frontend, backend, and feature reference.
- [Install & Update Audit](docs/INSTALL_UPDATE_AUDIT.md) - Install/update flow, Polkit, and passwordless setup.
- [Troubleshooting Guide](docs/TROUBLESHOOTING.md) - Fix GPG, lock files, and database issues.
- [Security Policy](SECURITY.md) - How to report vulnerabilities; [Fort Knox Audit](docs/SECURITY_AUDIT_FORT_KNOX.md) - Security & Arch compliance.
- [Architecture](docs/ARCHITECTURE.md) - Technical design.

## 🚀 Installation

### Option 1: Pre-built Binary (Recommended)
Download the latest `monarch-store-0.3.5_alpha-x86_64.pkg.tar.zst` (or current version) from the [Releases Page](https://github.com/cpg716/monarch-store/releases).

```bash
sudo pacman -U monarch-store-0.3.5_alpha-1-x86_64.pkg.tar.zst
```

### Option 2: Build from Source
```bash
git clone https://github.com/cpg716/monarch-store.git
cd monarch-store
npm install
npm run tauri build
```

### 🛟 Troubleshooting

If you encounter build errors on Arch Linux (e.g., `failed to run cargo metadata`), use our included fix script to verify your environment:

```bash
git pull origin main
chmod +x arch_fix.sh
./arch_fix.sh
```
This script installs missing build dependencies (like `webkit2gtk`) and cleans stale build artifacts.


## 🤝 Contributing

We welcome contributions! Please follow the standard fork-and-pull request workflow. See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md) for build commands and code style.

*   **Frontend**: React 19, TypeScript, Tailwind CSS 4, Vite 7, Zustand, Framer Motion.
*   **Backend**: Tauri 2 with Rust workspace (`monarch-gui` + `monarch-helper`).

## 📄 License
MIT License.

## 👤 Author
**cpg716** — developer and creator. This app was built with the help of AI coding tools.

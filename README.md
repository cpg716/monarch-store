# MonARCH Store: Universal Arch Linux App Manager
> **The Host-Adaptive App Manager for Arch, Manjaro, Garuda, and CachyOS.**

**Author:** [cpg716](https://github.com/cpg716) — developer and creator of MonARCH Store, with the help of AI coding tools.

**Current Version:** v0.4.0-alpha

## ⚠️ Alpha Disclaimer

**MonARCH Store is currently in ALPHA.** 
While the browsing and Flatpak features are robust, the **system package management is powerful and should be used with care.**

> [!WARNING]
> Use this software with caution on production systems. Always ensure you have a backup of your important data.

---

A premium, universal software center built with Tauri and React. MonARCH **respects your existing system configuration** (Host-Adaptive) while providing a unified interface for Official, AUR, and Flatpak applications.

![MonARCH Store Dashboard](screenshots/home.png)

## ✨ Key Features (v0.4.0)

### 🦎 Host-Adaptive Architecture
MonARCH no longer "injects" its own opinions into your system.
*   **Respects `pacman.conf`**: We typically only show repositories you have explicitly enabled on your host system.
*   **Manjaro Guard**: Automatically prevents enabling incompatible repositories (like `chaotic-aur`) on Manjaro systems to ensure stability.
*   **Discovery Mode**: Automatically detects CachyOS, Garuda, or EndeavourOS specific repositories and displays them correctly.

### 📦 Unified Search & Aggregation
Stop searching three different websites. MonARCH combines them all:
*   **One Search Bar**: Queries **Official Repos**, **AUR**, and **Flathub** simultaneously.
*   **Source Priority**: Intelligently ranks results (Official > Flatpak > AUR).
*   **Smart Merging**: Duplicate apps are merged into a single card with a "Source" selector.

### 🛠️ Native AUR Builder
A robust, safe implementation of the Arch User Repository.
*   **Built from Source**: Clearly identifies AUR packages that require local compilation.
*   **Native Cloning**: Uses `libgit2` for fast, reliable cloning of AUR packages.
*   **User-Level Builds**: Runs `makepkg` as your user (never root) for security.
*   **Live Logs**: Streams real-time build logs to the UI so you can see exactly what's happening.

### 📦 Full Flatpak Support
The ultimate safety net.
*   **Unified Updates**: Flatpaks are now first-class citizens in the update engine.
*   **Sandboxed**: Perfect for proprietary apps like Discord, Spotify, or Zoom.
*   **Visual Integration**: Flatpaks appear seamlessly alongside native apps.

### 🔄 Unified Update System (The Apdatifier Core)
No more individual updates.
*   **Parallel Aggregation**: Checks for updates from Official Repos, AUR, and Flatpak simultaneously.
*   **Safety Lock**: If any official package is selected, a full system upgrade (`-Syu`) is enforced to prevent partial upgrades.
*   **Built from Source Indicators**: AUR packages are clearly marked with their build status.

### 🛸 Mission Control (Settings Redesign)
A completely overhauled settings experience.
*   **Tabbed Layout**: Dedicated sections for Sources, Builder, and Maintenance.
*   **Advanced AUR Controls**: Fine-tune parallel downloads, build directory cleaning, and verbose logging.
*   **Diagnostics**: Integrated system health checks and repair tools.

### 🩺 System Health & Safety
*   **Legacy Audit**: Entire codebase sanitized of "Ghost Commands" for absolute stability.
*   **Atomic Updates**: Repo installs use safe transaction barriers (`pacman -Syu --needed`).
*   **Lock Guard**: Prevents operations when the Pacman DB is locked.

## 📘 Documentation
- [**User Guide**](USER_GUIDE.md) - How to use MonARCH and how it works.
- [**FAQ**](FAQ.md) - Frequently asked questions.
- [**Roadmap**](ROADMAP.md) - Future plans and upcoming features.
- [**Architecture & Design**](ARCHITECTURE.md) - Deep dive into the Host-Adaptive model.
- [**Developer Guide**](docs/DEVELOPER.md) - Setup and contribution guide.
- [**Security Policy**](SECURITY.md) - Our security commitments.

## 🚀 Installation

### Option 1: Pre-built Binary (Recommended)
Download the latest `.pkg.tar.zst` from the [Releases Page](https://github.com/cpg716/monarch-store/releases).

```bash
sudo pacman -U monarch-store-x.x.x-x86_64.pkg.tar.zst
```

### Option 2: Build from Source
```bash
git clone https://github.com/cpg716/monarch-store.git
cd monarch-store
npm install
npm run tauri build
```

## 🤝 Contributing
We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md).

*   **Frontend**: React 19, TypeScript, Tailwind CSS 4, Vite 7, Zustand.
*   **Backend**: Tauri 2, Rust, Tokio.

## 📄 License
MIT License.

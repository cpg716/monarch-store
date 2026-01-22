# 🦋 MonARCH Store

**A modern, premium software store for Arch Linux, built with Tauri v2, React, and Rust.**

MonARCH Store is designed to make package management on Arch-based systems (Arch, EndeavourOS, CachyOS, Garuda, etc.) beautiful, fast, and accessible. It unifies **Official Distribution Repositories**, **AUR**, and **Chaotic-AUR** into a single, cohesive experience.

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

### 🌪️ Chaotic-AUR: A Game Changer
We don't just "support" Chaotic-AUR — **we prioritize it.**
*   **Zero-Compile Updates**: Automatic preference for pre-built binaries from the Chaotic-AUR infrastructure.
*   **Massive Library**: Access thousands of pre-compiled AUR packages without waiting hours for local compilation.

### 🧠 Intelligent Package Merging
Stop guessing which "firefox" is the right one. MonARCH intelligently merges results from all sources into a single, clean view:
*   **Smart Resolution**: If a package exists in Official Repos, Chaotic-AUR, and AUR, we automatically serve the fastest/safest option (Official/Chaotic) while keeping the AUR version available as a fallback.
*   **De-Duplication**: Clean, unified search results without clutter.

### 🐧 Optimized for Your Distro
*   **CachyOS Optimized**: Fully compatible with CachyOS's `x86-64-v3`/`v4` optimized repositories for maximum performance.
*   **Manjaro Tested**: Verified stable on Manjaro's branch structure.
*   **Universal Arch Support**: Works seamlessly on EndeavourOS, Garuda, and vanilla Arch.

### 🛠️ Graphical "No-Terminal" AUR Builder
Building from the AUR shouldn't require a CLI degree.
*   **One-Click Build**: Click install, and we handle the `makepkg` magic, dependency resolution, and `sudo` handling in the background.
*   **Visual Feedback**: Beautiful, real-time progress bars instead of scrolling text.

### ⭐ Hybrid Community Reviews
*   **ODRS Integration**: See global ratings for official Linux apps.
*   **Community Reviews**: Submit and read reviews for AUR packages (powered by Supabase).

### 📊 Analytics
Privacy-friendly usage stats (install trends, top searches) powered by Aptabase.

### 🛡️ Safety First
*   **PKGBUILD Inspector**: Review build scripts before installing from AUR.
*   **Out-of-Date Warnings**: Visual alerts for flagged packages.

## 🚀 Installation

### Pre-requisites
*   Arch Linux (or derivative).
*   `paru` or `yay` (optional, for AUR helper support, defaults to manual `makepkg` logic if missing, but helper recommended).

### Install via PKGBUILD (Recommended)
Coming soon to AUR!

### Manual Build
```bash
# 1. Clone the repo
git clone https://github.com/cpg716/monarch-store.git
cd monarch-store

# 2. Install dependencies
npm install

# 3. Run in Development Mode
npm run tauri dev

# 4. Build Release
npm run tauri build
```


## 🤝 Contributing

We welcome contributions! Please follow the standard fork-and-pull request workflow.

*   **Frontend**: React, TailwindCSS, Framer Motion.
*   **Backend**: Rust (Tauri commands).

## 📄 License
MIT License.

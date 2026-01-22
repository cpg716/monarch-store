# 🦋 MonARCH Store

**MonARCH Store** is a high-performance, premium software store designed specifically for Arch Linux. 

Built with the speed of **Rust** (Tauri v2) and the flexibility of **React**, it provides a seamless, unified interface for browsing and installing applications from:
- 🚀 **Chaotic-AUR** (Pre-built binaries - *Prioritized for speed*)
- 📦 **Official Repositories** (Core, Extra, Multilib)
- 🛠️ **Arch User Repository (AUR)** (Community contributions)
- 🧪 **CachyOS / Garuda / Endeavour / Manjaro** (Distro-specific repos)

![MonARCH Store Screenshot](https://raw.githubusercontent.com/cpg716/monarch-store/main/public/screenshot.png)

## ✨ Features

- **⚡ Blazing Fast**: 
    - **Binary First**: Prioritizes pre-built binaries (Chaotic-AUR) to save compilation time and system resources.
    - **Smart Caching**: Persistent local caching for Repo DBs and AppStream metadata.
- **⭐ Hybrid Review System**:
    - **ODRS Support**: Global reviews from the Open Desktop Rating System (used by GNOME/KDE).
    - **MonArch Community**: Custom Supabase-powered reviews for AUR packages without official IDs.
- **🔍 Universal Search & Deduplication**:
    - Instantly searches across all enabled repositories.
    - Intelligent **App ID Mapping** merges identical apps (e.g. `brave-bin` and `brave`) into a single high-quality view.
- **🔄 Auto-Updates**:
    - Built-in updater powered by **GitHub Releases**.
- **🎨 Premium Design**:
    - **Glassmorphism**: Modern, translucent UI components.
    - **Rich Media**: High-quality icons, screenshots, and descriptions.

## 🚀 Installation

### 1. From the AUR (Recommended)
Once submitted, you can install `monarch-store` using your favorite AUR helper:

```bash
yay -S monarch-store
```

### 2. Manual Installation (PKGBUILD)
If you want to install the latest release manually using the provided `PKGBUILD`:

```bash
git clone https://github.com/cpg716/monarch-store.git
cd monarch-store
sudo pacman -Sy
makepkg -si
```

### 3. Development Build
To run the application from source or contribute:

1. **Clone the repo:**
   ```bash
   git clone https://github.com/cpg716/monarch-store.git
   cd monarch-store
   ```
2. **Install dependencies:**
   ```bash
   npm install
   ```
3. **Run in dev mode:**
   ```bash
   npm run tauri dev
   ```

## � Running MonARCH Store

Once installed via the AUR or `makepkg`, you can launch MonARCH Store in two ways:

1. **Application Menu**: Search for **"MonARCH Store"** in your desktop environment's app launcher (GNOME, KDE, XFCE, etc.).
2. **Terminal**: Run the following command:
   ```bash
   monarch-store
   ```

## �🏗️ Architecture

Check out our [System Architecture](docs/ARCHITECTURE.md) to learn about the interaction between the Rust backend, React frontend, and our hybrid review pipeline.

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## 📜 License

This project is licensed under the [MIT License](LICENSE).

## ❤️ Credits

Powered by:
- [Tauri](https://tauri.app/)
- [Chaotic-AUR](https://aur.chaotic.cx/)
- [Supabase](https://supabase.com/)
- [ODRS](https://odrs.gnome.org/)

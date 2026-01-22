# 🦋 MonARCH Store

**MonARCH Store** is a high-performance, premium software store designed specifically for Arch Linux. 

Built with the speed of **Rust** (Tauri) and the flexibility of **React**, it provides a seamless, unified interface for browsing and installing applications from:
- 📦 **Official Repositories** (Core, Extra, Multilib)
- 🚀 **Chaotic-AUR** (Pre-built binaries for AUR packages)
- 🛠️ **Arch User Repository (AUR)** (Community contributions)
- 🧪 **CachyOS / Garuda / Endeavour / Manjaro** (Distro-specific repos)

![MonARCH Store Screenshot](https://raw.githubusercontent.com/monarch-store/monarch-store/main/public/screenshot.png)

## ✨ Features

- **⚡ Blazing Fast**: 
    - **Async Core**: Complete asynchronous backend rewrite using `tokio` for non-blocking operations.
    - **Batch Fetching**: Metadata is retrieved in efficient batches, eliminating "pop-in" and reducing IPC calls by 96%.
    - **Smart Caching**: In-memory caching for Chaotic-AUR data and persistent local caching for Repo DBs.
    - **Lazy Loading**: Visual assets load only when needed.
- **🎨 Premium Design**:
    - **Glassmorphism**: Modern, translucent UI components.
    - **Rich Metadata**: High-quality icons, screenshots, and descriptions powered by AppStream.
    - **Responsive**: Adapts perfectly to different window sizes with virtualized grids.
- **🔍 Universal Search**:
    - Instantly searches across all enabled repositories.
    - Sorts results by relevance (Official > Chaotic > AUR).
- **🛡️ Secure & Native**:
    - **Audit Passed**: Exhaustive security implementation audit passed (v0.2.0).
    - Uses standard `pkexec` for privilege escalation (no weird root daemons).
    - Leverages your existing pacman configuration and AUR helpers (`paru`, `yay`).

## 🚀 Getting Started

### Prerequisites

- **Arch Linux** (or derivative: EndeavourOS, Garuda, CachyOS, Manjaro, etc.)
- `pacman`
- `paru` OR `yay` (for AUR support)
- `webkit2gtk` (for Tauri frontend)

### Installation

**From AUR:**
```bash
paru -S monarch-store
# or
yay -S monarch-store
```

**Manual Build:**
1. Clone the repo:
   ```bash
   git clone https://github.com/monarch-store/monarch-store.git
   cd monarch-store
   ```
2. Install dependencies:
   ```bash
   npm install
   ```
3. Run in dev mode:
   ```bash
   npm run tauri dev
   ```

## 🏗️ Architecture

Want to know how it works under the hood? Check out our [System Architecture](docs/ARCHITECTURE.md) document to learn about the interaction between the Rust backend and React frontend.

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to submit pull requests, report bugs, and suggest enhancements.

## 📜 License

This project is licensed under the [MIT License](LICENSE).

## ❤️ Credits

Powered by the giants of the open-source world:
- [Tauri](https://tauri.app/)
- [Chaotic-AUR](https://aur.chaotic.cx/)
- [AppStream](https://www.freedesktop.org/wiki/Distributions/AppStream/)
- [ODRS](https://odrs.gnome.org/)

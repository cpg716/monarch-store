# 📈 MonARCH Progress Report

## 🏆 Recent Achievements

### Install & Update Reliability
- **Temp-file command**: Helper receives command via temp file (path in argv) to avoid "Invalid JSON" and argv truncation.
- **Single invocation**: InstallMonitor uses ref guard so install runs once per package (no double password prompt from React Strict Mode).
- **Production helper path**: GUI prefers `/usr/lib/monarch-store/monarch-helper` when present so Polkit policy path matches; passwordless installs work when rules are installed.
- **Update-and-install**: `update_and_install_package` now runs Sysupgrade then AlpmInstall for the named package (previously only Sysupgrade).
- **Polkit rules**: `10-monarch-store.rules` includes `com.monarch.store.package-manage`; `install_monarch_policy` copies rules to `/usr/share/polkit-1/rules.d/`. See [Install & Update Audit](docs/INSTALL_UPDATE_AUDIT.md).

### 🦋 Butterfly Engine (Backend)
- **Startup Integrity**: Verified runtime environment (`git`, `polkit`, `pkexec`) at launch.
- **Parallel Rating Delivery**: ODRS and metadata fetched in parallel for faster home load.

### 🎨 Frontend & Docs
- **Full App Audit**: [docs/APP_AUDIT.md](docs/APP_AUDIT.md) documents UI/UX, all pages, components, hooks, store, backend, and feature areas.
- **Stack**: React 19, TypeScript, Tailwind CSS 4, Vite 7, Zustand, Framer Motion.

---

## 🚧 Current Work
- [ ] **Flathub metadata**: Flathub API used for icons/descriptions/reviews for AUR and official packages (metadata only; we do not add Flatpak app support).
- [ ] **MonARCH Plugin API**: Designing the interface for community repair scripts.

---

## 🗺️ Future Roadmap
- **v0.4.x**: Theme Engine (MonARCH Accent palettes).
- **v1.0.x**: External Plugin API for community-contributed repair scripts.

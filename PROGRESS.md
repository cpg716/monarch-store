# 📈 MonARCH Progress Report

**Last updated:** 2025-01-29 (v0.3.5-alpha.1)

## 🏆 Recent Achievements

### v0.3.5-alpha.1 (Release readiness)
- **AppStream:** Production `monarch-store.metainfo.xml` with `com.monarch.store`, developer cpg716, OARS content rating.
- **Keyboard sovereignty:** Escape key and focus trap on all modals (including Auth and PKGBUILD).
- **Atomic sync:** Full audit; no naked `pacman -Sy` in repair, repo_setup, or monarch-helper.
- **Author credits:** cpg716 listed as developer/creator (with AI coding tools) in metainfo, package.json, README, PKGBUILD.
- **Release script:** `scripts/release-finalize-pkgbuild.sh` and [RELEASE_PUSH_STEPS](docs/RELEASE_PUSH_STEPS.md) for tarball + checksums after tag push.

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

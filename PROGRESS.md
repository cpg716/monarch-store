# 📈 MonARCH Progress Report

## 🏆 Recent Achievements (v0.2.40)
The **"Zero-Config" Release**. This milestone focused on absolute stability and runtime safety, ensuring the app works "out of the box" without manual intervention.

### 🛑 Zero-Config Reliability
- **Strict Dependency Matrix**: Rewrote `PKGBUILD` to enforce every runtime need (polkit, git, openssl, webkit).
- **Runtime Self-Check**: The app now detects missing tools (`git`, `pkexec`) at startup and alerts the user instead of silent crashing.
- **Seamless Auth**: The Polkit policy is now installed directly from the source tree, guaranteeing that password-less package management works immediately.

### 🌐 ODRS Data Integrity
- **Manual ID Map**: Fixed "Missing Ratings" for popular apps (Discord, VLC, GIMP) by injecting a manual translation layer for the ODRS API (e.g., `discord` -> `com.discordapp.Discord`).
- **Resilient Batching**: The startup sequence now aggressively probes for ratings even if metadata is mismatched.

### 🎨 Visual Polish
- **Global Scrolling Fix**: Resolved the "Cut Off" content on small screens by moving the scroll container to the top level.
- **Adaptive Grids**: Prevented "smushed" cards on resize by implementing `minmax(280px)` responsive grids across all views.

---

## 🚧 Current Work & Active Addressing
The following items are currently being monitored or require minor refinement:
- [ ] **Partial Upgrade Detection**: Strengthening the logic to detect if `pacman -Sy` was run without `-u` recently.
- [ ] **Offline Mode UX**: Improving the "No Internet" landing experience to allow browsing cached apps.
- [ ] **AUR Interaction**: Polishing the PKGBUILD inspector modal to handle huge scripts more gracefully.

---

## 🗺️ Future Roadmap
- **v0.3.x**: Native Flatpak support integration.
- **v0.4.x**: Theme Engine (MonARCH Accent palettes).
- **v1.0.x**: External Plugin API for community-contributed repair scripts.

# Release Notes v0.2.40 - The "Zero-Config" Update

## 🛑 Runtime Safety & Integrity
- **Zero-Config Guarantee**: We audited the entire dependency chain. `PKGBUILD` now strictly enforces all requirements (`openssl`, `git`, `polkit`).
- **Self-Healing Startup**: The app now self-diagnoses missing binary tools (`git`, `pkexec`) at launch to prevent silent failures.
- **Polkit Standardization**: Security policies are now installed from a single "Source of Truth," ensuring password-less package management works out of the box on all distributions.

## 🌐 Data & Network Resilience
- **Ratings Fixed**: Solved the "Missing Stars" issue for popular apps (Discord, VLC, GIMP, Lutris) by implementing a manual ODRS ID translation layer.
- **Offline Safety**: Improved error handling when the ODRS API is down (like during the major outage of Jan 2026).

## 🎨 Visual Refinements
- **Responsive Layouts**: Cards no longer get "smushed" on window resize. We implemented a robust `minmax` grid system.
- **Small Screen Support**: Fixed the "Cut Off" content issue on smaller laptops by moving the main scroll container to the top level.
- **Search Grid**: Search results now respect the same adaptive layout rules as the rest of the app.

---

# Release Notes v0.2.30

# Release Notes v0.2.24
- **Icon Restoration**: Fixed missing icons for Brave, Spotify, and Chrome by restoring the robust fallback chain (checking upstream sources when local metadata fails).
- **Search Accuracy**: "Spotify" now finds the main app first! We improved search sorting to prioritize exact matches over launchers or plugins.
- **Linux Native Power**: Full support for system icons (`/usr/share/pixmaps`) and local AppStream caching on Linux devices.

---

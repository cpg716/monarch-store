# Release Notes v0.3.00-Alpha1 (The "Universal" Update)

> **"The first Distro-Aware App Manager for Arch, Manjaro, and CachyOS."**

## 🚀 Rebranding: Universal Manager
MonARCH is now the **Universal Arch Linux App Manager**. We have transitioned from a simple "Store" to a context-aware system utility that adapts its safety rails based on your specific distribution.

## 🛡️ Distro-Aware Intelligence
*   **Manjaro Stability Guard**: Automatically hides bleeding-edge Arch repos (Chaotic-AUR) on Manjaro systems to prevent library mismatch errors.
*   **CachyOS Performance Mode**: Detects AVX2/AVX-512 CPUs and prioritizes v3/v4 repositories for 10-20% faster apps.
*   **Arch Power Mode**: Unlocks full access to all repos for vanilla Arch users.

## ✨ Luminosity UI Engine
*   **Glassmorphism**: A complete UI rewrite featuring blurred backgrounds, "Ghost Text" headers, and premium topography.
*   **Responsive Stacking**: The "App Details" view now intelligently stacks metadata on mobile while expanding to a 2-column layout on desktop.
*   **Skeleton Loading**: Smoother transitions with shimmer effects replaces jarring spinners.

## ⚡ Performance & Resilience
*   **70% Faster Startup**: Parallel ODRS rating fetches mean the homepage loads instantly.
*   **Smart Sync**: The installer now checks database freshness (<1hr) to skip redundant `pacman -Sy` calls.
*   **Offline Mode**: A new global "Offline Guard" prevents crashes when the internet cuts out, serving cached data gracefully.

---

# Release Notes v0.3.00-Alpha1 - The "Butterfly" Update

## 🦋 Major Architectural Overhaul
- **Hardware-Aware Backend**: Implemented `check_requirements()` to ensure system binaries (`git`, `pkexec`) are healthy at boot.
- **Luminosity UI Engine**: Complete redesign of the App Details experience (`PackageDetailsFresh.tsx`) featuring top-aligned metadata, glassmorphism, and high-density layouts.
- **Parallel ODRS Integration**: Ratings and reviews now fetch concurrently, resulting in a ~70% speed boost on the home page.

## 📱 Responsive & Visual Mastery
- **Horizontal Mobile Header**: App title and logo now stay side-by-side even in the smallest windows.
- **Scroll-to-Reviews**: Clicking the Ratings box instantly smooth-scrolls to the user opinions.
- **Button Unity**: Action buttons now group intelligently to prevent isolated wrapping.

---

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

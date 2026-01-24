# Release Notes v0.2.24

## 🚀 Critical Fixes & Features
- **Icon Restoration**: Fixed missing icons for Brave, Spotify, and Chrome by restoring the robust fallback chain (checking upstream sources when local metadata fails).
- **Search Accuracy**: "Spotify" now finds the main app first! We improved search sorting to prioritize exact matches over launchers or plugins.
- **Linux Native Power**: Full support for system icons (`/usr/share/pixmaps`) and local AppStream caching on Linux devices.

## 🛠️ Under the Hood
- **Performance**: Optimized startup synchronization for smoother app launching.
- **Stability**: Fixed a compilation issue in the sidebar navigation.
- **Cleanup**: Purged debug logs for a cleaner console experience.

---

# Release Notes v0.2.6

## 🚀 Critical Fixes
- **Patched Startup Crash**: Fixed a "Malformed XML" error caused by invalid characters in the AppStream data. The store now auto-sanitizes downloads to prevent crashes.
- **Resolved White Screen**: Implemented a global Error Boundary to catch and report UI failures instead of showing a blank screen.

## ⚡ Improvements
- **Smart Sync**: App metadata (icons, descriptions) now automatically refreshes every 3 hours (or your custom interval), ensuring you never see stale data.
- **Unified Updates**: The "Sync Repositories" action now updates everything—Git mirrors, pacman databases, and app metadata—in one go.
- **Robust Mirrors**: Added automatic mirror rotation for CachyOS, Manjaro, and EndeavourOS. If a mirror is down, MonArch instantly finds another.

## 🦋 UI / UX
- **Thematic Animation**: Restored the signature "flapping butterfly" loading animation.
- **Cleaner Package Cards**: Moved ratings to the footer so app names are never obscured.
- **Branded Alerts**: Notifications now feature clear MonArch branding and status indicators.

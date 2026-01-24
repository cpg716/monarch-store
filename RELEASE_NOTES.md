# Release Notes v0.2.25

## 🛡️ Major System Safety Upgrades
- **Infrastructure 2.0**: The entire repository management system has been rewritten.
- **Fail-Safe Updates**: The "Update All" action is now guaranteed to find updates for *all* your installed packages, even if you have hidden their source repository in the Store UI. This prevents accidental partial upgrades.
- **Keyring Auto-Healing**: We have eliminated "Invalid Signature" errors. The new Onboarding wizard automatically initializes and populates the Pacman keyring with fresh keys from Arch, Chaotic-AUR, and CachyOS.
- **Safe Repos**: Disabling a repository in Settings now performs a "Soft Disable"—it hides the packages from the Store search to reduce clutter but keeps the repository active in the background so your existing apps continue to receive critical security updates.

## ⚡ UX Enhancements
- **Password-Free Control**: Toggling repositories on/off is now instant and no longer requires a `sudo` password prompt.
- **Smart Onboarding**: The setup wizard now configures your entire system infrastructure in one go, so you don't face repeated authentication requests later.
- **Clean Settings**: Removed technical jargon warnings. The interface now clearly explains that disabling a source only affects visibility, not system safety.

## 🐛 Bug Fixes
- **Cache Clearing**: Fixed an issue where disabled repositories would still show up in search results until a restart. Toggling them off now instantly removes them from the active session.
- **Dependency Scan**: Added backend logic to track which repository each installed package came from, enabling smarter logic for future safety checks.

---

# Release Notes v0.2.24
- **Icon Restoration**: Fixed missing icons for Brave, Spotify, and Chrome by restoring the robust fallback chain (checking upstream sources when local metadata fails).
- **Search Accuracy**: "Spotify" now finds the main app first! We improved search sorting to prioritize exact matches over launchers or plugins.
- **Linux Native Power**: Full support for system icons (`/usr/share/pixmaps`) and local AppStream caching on Linux devices.

---

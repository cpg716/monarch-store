# 🏗️ MonARCH Store Architecture

## Core Philosophy: "Safe by Default, Powerful by Choice"

MonARCH Store is designed to solve the "Split-Brain" problem common in Arch Linux GUI store wrappers, where the GUI database falls out of sync with the system database, causing partial upgrade failures.

## 1. The "Soft Disable" Repository Model

Unlike standard managers that edit `/etc/pacman.conf` to remove repositories, MonARCH uses a **Soft Disable** approach:

*   **System State (True)**: All supported repositories (`chaotic-aur`, `cachyos`, `garuda`, `multilib`) are **permanently enabled** in `/etc/pacman.d/monarch_repos.conf` upon onboarding.
*   **User View (Virtual)**: When a user "disables" a repo in the UI, MonARCH simply filters those packages from search results and browsing.
*   **Benefit**: System updates (`pacman -Syu`) see *all* repositories, ensuring shared libraries (`glibc`, `openssl`) are updated atomically across the entire system, preventing breakage.

## 2. "Chaotic-First" Installation Pipeline

To provide a "Store-like" instant experience, MonARCH prioritizes pre-built binaries over source compilations:

1.  **Repo 0: Hardware Optimized** (CachyOS-v3/v4). If the user's CPU supports AVX2/AVX-512, these packages are ranked #0.
2.  **Repo 1: Chaotic-AUR**. The largest pre-compiled binary repo for AUR packages. Ranked #1.
3.  **Repo 2: Official Arch**. Standard stable packages. Ranked #2.
4.  **Repo 3: AUR**. Source builds. Ranked #3 (Last Resort).

This ensures that clicking "Install" almost always results in a fast, binary download rather than a slow `makepkg` compilation.

## 3. Intelligent "Split-Brain" Prevention

Before installing any package, the backend (`lib.rs`) performs a live check:

1.  **Check**: Does `pacman -Si <package>` return success?
2.  **Fast Path**: If yes, run `pacman -S <package>`.
3.  **Stale Path (Safe Sync)**: If no (meaning our cache is newer than the system DB), run `pacman -Sy <package>`.
    *   **Crucial**: We use `Targeted Sync` (`-Sy repo/pkg`) or package-specific sync to avoid refreshing the *entire* database without an update, minimizing partial upgrade risks while ensuring the target package installs successfully.

## 4. Hardened AUR Builder

The built-in AUR helper is rewritten in Rust to avoid common pitfalls:

*   **Dependency Resolution**: Parses `.SRCINFO` / `PKGBUILD` and installs dependencies via the system package manager *before* starting the build.
*   **Privilege Separation**: Runs `makepkg` as a standard user, but escalates to `pkexec` only for the final `pacman -U` install step.
*   **Invisible Build**: Hides raw terminal logs behind a friendly UI progress bar, but maintains full log files for debugging.

## 5. Linux Native Integration

*   **Icons**: Uses standard XDG paths (`/usr/share/icons/...`) resolved via `file://` protocol.
*   **Polkit**: Uses `pkexec` for granular permission escalation (no global sudo usage).
*   **AppStream**: Integrates native metadata for rich descriptions and screenshots.

# 🏗️ MonARCH Store Architecture

**Last updated:** 2025-01-31 (v0.3.5-alpha)

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

## 2. The "Butterfly" Engine (Distro-Awareness)

MonARCH is **Context-Aware** thanks to the `distro_detect.rs` module. It probes `/etc/os-release` at startup to build an `IdentityMatrix`:

*   **IS_MANJARO**: Activates "Stability Guard" (Hide Chaotic-AUR, Warn on AUR).
*   **IS_ARCH**: Activates "Power User Mode" (Enable all repos, assume base-devel).
*   **IS_CACHYOS**: Activates "Speed Mode" (Prioritize v3/v4 repos).

## 3. The Installer Pipeline

The installation flow uses the **monarch-helper** binary (invoked via `pkexec`) so that all ALPM write operations run in one privileged process. This prevents partial upgrades and split-brain states.

*   **GUI (user)**: Validates package name, distro safety, and repo selection; builds a JSON command and writes it to a temp file.
*   **Helper (root)**: Reads the command from the temp file, runs ALPM transactions. 
    *   **Config Parsing**: Uses `pacman-conf` to accurately load all repositories and servers, supporting complex `Include` files.
    *   **The Iron Core (v0.3.6)**: Uses `SafeUpdateTransaction` for atomic reliability.
    *   **Lock Guard**: Checks `/var/lib/pacman/db.lck` before actions.
    *   **Strict Safety**: Enforces manual `pacman -Syu` logic (iterating local packages and checking sync DBs for updates) to prevent partial upgrades.
*   **Safety Rule**: We **never** run `pacman -Sy` alone. Repo installs use `pacman -Syu --needed` in a single transaction; system updates use one full upgrade. See [Install & Update Audit](docs/INSTALL_UPDATE_AUDIT.md).

### 4. Butterfly System Health & Omni-User (v0.3.5)
MonARCH includes a permission-aware health monitoring ecosystem and a dual-core UX (simplicity by default, power by choice):
*   **Butterfly Probes**: Verifies `pkexec`, `git`, and `polkit` health at startup to prevent silent failures.
*   **Parallel ODRS Integration**: Ratings are fetched concurrently during onboarding/home view for faster load.
*   **Permission-Safe Sensors**: Health checks are non-privileged, preventing false "Corrupted Keyring" warnings.
*   **Unified Repair Wizard**: A single authorized maintenance flow for Keyring, Security Policies, and Repo sync.
*   **Self-Healing**: During install, corrupt sync DBs or locked DB trigger silent repair (force refresh or unlock) and retry—no error pop-up. Helper `force_refresh_sync_dbs` reads `/etc/pacman.conf` directly so recovery works when ALPM is blind. At startup the app calls `needs_startup_unlock()`; if a stale lock exists and **Reduce password prompts** is on, the in-app password is used for unlock so the system prompt does not appear at launch.
*   **Glass Cockpit**: Settings → General: **Show Detailed Transaction Logs**. Maintenance: **Advanced Repair** (Unlock DB, Fix Keys, Refresh DBs, Clear Cache, Clean Orphans). Repositories: **Test Mirrors** per repo (top 3 mirrors with latency; rate-mirrors/reflector).

### 5. Frontend Stack
*   **Stack**: React 19, TypeScript, Tailwind CSS 4, Vite 7, Zustand (state), Framer Motion. Key dirs: `src/components/`, `src/pages/`, `src/hooks/`, `src/store/`.
*   **Layout**: `PackageDetailsFresh.tsx` and other pages use responsive Grid/Flexbox; glassmorphic styling with `backdrop-blur` and semi-transparent layers.
*   **Components**: Atomic design with `PackageCard.tsx`, `InstallMonitor.tsx`, and shared hooks for metadata, ratings, and favorites.

## 6. Linux Native Integration

*   **Icons**: Uses standard XDG paths and metadata from AppStream and the Flathub API (for icons/descriptions only; we do not ship or install Flatpak apps).
*   **Polkit**: Privileged operations use `pkexec` with **monarch-helper** at `/usr/lib/monarch-store/monarch-helper` when installed; Polkit policy and rules allow passwordless installs for authorized users. See [Install & Update Audit](docs/INSTALL_UPDATE_AUDIT.md).
*   **Helper command**: The GUI writes the JSON command to a temp file and passes only the file path to the helper to avoid argv truncation and ensure reliable installs.

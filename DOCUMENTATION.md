# 📘 MonARCH Store - Technical Documentation

**Last updated:** 2025-01-29 (v0.3.5-alpha.1)

## Architecture Overview: Infrastructure 2.0

MonARCH Store uses a **"Soft Disable"** architecture to balance User Experience with System Safety.

### 🛡️ System Layer (The Rock)
*   **Always On**: During Onboarding, the app configures `/etc/pacman.conf` to enable **all** supported repositories (`cachyos`, `garuda`, `chaotic`, etc.).
*   **Fail Safe**: Because `pacman` sees everything, running `pacman -Syu` (System Update) will **always** find updates for your installed apps, even if you "Hid" the source in the Store.
*   **No Password Fatigue**: Since the system is pre-configured once, toggling repos in the UI does not require root/checksum triggers.
*   **GPG Resilience**: Automatically syncs keys to both system and user keyrings, fixing "Invalid Signature" errors for both the app and manual terminal builds.

### 🖥️ Frontend Layer (The View)
*   **Soft Toggles**: Disabling a repo in `Settings` adds it to a "Hidden" list in `repos.json` and instantly clears it from the search cache.
*   **Result**: The Store stops showing *new* packages from that source, but your *existing* packages remain safe.

## Key Features & Logic

### 1. Unified Search (Chaotic-First)
When you search for "firefox", MonARCH aggregates results from all enabled sources but prioritizes instant binaries:
1.  **Hardware Optimized** (Priority #0): `cachyos-v3` / `v4` (if CPU supported).
2.  **Chaotic-AUR** (Priority #1): Pre-built binary. Fastest install.
3.  **Official Repos** (Priority #2): Standard Arch package.
4.  **AUR** (Priority #3): Source build (fallback).

*Users can manually override this choice using the "Download Source" dropdown in the package details.*

### 2. Update Consistency
We strictly enforce **"Update All"** via `perform_system_update` (Helper command `Sysupgrade`).
*   **Why?**: Arch Linux does not support partial upgrades. Updating one app without system libraries (`glibc`) can break the OS.
*   **Mechanism**: The GUI invokes the Helper with `Sysupgrade`; the Helper runs a single full upgrade (sync + transaction). We **never** run `pacman -Sy` alone. See [Install & Update Audit](docs/INSTALL_UPDATE_AUDIT.md).

### 3. Hybrid Review System
We use a composite rating strategy:
*   **Step 1:** Check **ODRS** (Open Desktop Rating Service).
*   **Step 2:** Fallback to **Supabase** community reviews.
*   **Display:** Merged 5-star rating.

### 4. Butterfly System Health
MonARCH includes a permission-aware health monitoring ecosystem:
*   **Butterfly Probes**: Verifies environmental requirements (`git`, `pkexec`, `polkit`) at startup.
*   **Parallel Rating Fetches**: ODRS and metadata fetched in parallel for faster home screen load.
*   **Permission-Safe Sensors**: Health checks are non-privileged, preventing false "Corrupted Keyring" warnings.
*   **Unified Repair Wizard**: A single authorized maintenance flow for Keyring, Security Policies, and Repo sync.

### 5. Frontend (Luminosity Visual Engine)
*   **Stack**: React 19, TypeScript, Tailwind CSS 4, Vite 7, Zustand, Framer Motion. See [APP_AUDIT](docs/APP_AUDIT.md) for full UI/UX and feature reference.
*   **Layout**: Glassmorphic backdrops, responsive grids, scroll-to-reviews, repo selector in package details.

## 🛠️ Build & Release

To cut a new release:
1.  Update `version` in `package.json`, `src-tauri/monarch-gui/tauri.conf.json`, and both `src-tauri/monarch-gui/Cargo.toml` and `src-tauri/monarch-helper/Cargo.toml` (e.g. `0.3.5-alpha.1`).
2.  Clean build: `npm run tauri build` (from repo root). Rust: `cd src-tauri && cargo check` for backend check.
3.  Tag and push: `git tag -a v0.3.5_alpha.1 -m "Release message"` then `git push origin main && git push origin v0.3.5_alpha.1`.

## ☁️ Backend Configuration (Self-Hosting Community Reviews)

To enable **Community Reviews** with your own backend:
1.  Create a **Supabase** project.
2.  Run the provided SQL setup script (see `src/services/reviewService.ts`).
3.  Update env vars with your **Project URL** and **Anon Key**.

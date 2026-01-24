# 📘 MonARCH Store - Technical Documentation

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

### 1. Unified Search
When you search for "firefox", MonARCH aggregates results from:
1.  **Chaotic-AUR** (Priority #1): Pre-built binary. Fastest install.
2.  **Official Repos** (Priority #2): Standard Arch package.
3.  **AppStream** (Priority #3): Metadata-rich results (icons/screenshots).
4.  **AUR** (Priority #4): Source build (fallback).

### 2. Update Consistency
We strictly enforce **"Update All"** via `perform_system_update`.
*   **Why?**: Arch Linux does not support partial upgrades. Allowing a user to update just one app (e.g., Firefox) without updating system libraries (`glibc`) can break the OS.
*   **Mechanism**: The app calls the system's `checkupdates` tool, which respects the "Always On" system config, ensuring 100% update coverage.

### 3. Hybrid Review System
We use a composite rating strategy:
*   **Step 1:** Check **ODRS** (Open Desktop Rating Service).
*   **Step 2:** Fallback to **Supabase** community reviews.
*   **Display:** Merged 5-star rating.

## 🛠️ Build & Release

To cut a new release:
1.  Update `version` in `package.json` and `src-tauri/tauri.conf.json`.
2.  Update `src-tauri/Cargo.toml`.
3.  Run `npm run tauri build`.
4.  Tag commit with `vX.Y.Z`.

## ☁️ Backend Configuration (Self-Hosting Community Reviews)

To enable **Community Reviews** with your own backend:
1.  Create a **Supabase** project.
2.  Run the provided SQL setup script (see `src/services/reviewService.ts`).
3.  Update env vars with your **Project URL** and **Anon Key**.

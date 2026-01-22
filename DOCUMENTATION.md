# 📘 MonARCH Store - Technical Documentation

## Architecture Overview

MonARCH Store uses the **Tauri v2** framework, combining a web-based frontend with a high-performance Rust backend.

### 🖥️ Frontend (UI)
*   **Framework**: React (Vite) + TypeScript.
*   **Styling**: TailwindCSS (v4) with custom `App.css` for glassmorphism effects.
*   **State Management**: Zustand (local store) + React Hooks.
*   **Charts/Icons**: Lucide-React.

src/
├── components/   # Reusable UI widgets (PackageCard, SearchBar, etc.)
├── pages/        # Main Views (PackageDetails, Installed, etc.)
├── services/     # API Layers (reviewService.ts - Supabase/ODRS)
└── hooks/        # Logic extraction (useFavorites, useInfiniteScroll)
```

### 🦀 Backend (Rust)
*   **Core**: `src-tauri/src/lib.rs` - Main entry point and command registration.
*   **Modules**:
    *   `aur_api.rs`: Async client for `aur.archlinux.org` RPC.
    *   `chaotic_api.rs`: Fetches and caches the massive `chaotic-aur` package list.
    *   `repo_manager.rs`: Handles `pacman` database syncing and local repo management.
    *   `review.rs` / `odrs_api.rs`: Interfaces for fetching ODRS ratings.

## 🧠 Key Features & Logic

### 1. Smart Package Resolution
When you search for "firefox", MonARCH doesn't just show one result. It aggregates from multiple sources and prioritizes them:
1.  **Chaotic-AUR** (Priority #1): Pre-built binary. Fastest install.
2.  **Official Repos** (Priority #2): Standard Arch package.
3.  **AppStream** (Priority #3): Metadata-rich results (icons/screenshots).
4.  **AUR** (Priority #4): Source build (fallback).

### 2. Hybrid Review System
We use a composite rating strategy to ensure every app has data:
*   **Step 1:** Check **ODRS** (Open Desktop Rating Service) using the AppStream ID (e.g., `org.mozilla.firefox`).
*   **Step 2:** If ODRS fails, check ODRS again using the package name.
*   **Step 3:** If still no data (common for AUR apps), query our **Supabase** `reviews` table.
*   **Display:** The UI merges this into a single 5-star rating component.

### 3. Analytics (Aptabase)
We track minimal, privacy-centric events:
*   `app_launch`: Version & OS tracking.
*   `install_clicked`: Which source (AUR vs Official) is preferred.
*   `search`: Query keywords (to identify missing packages).

## 🛠️ Build & Release

To cut a new release:
1.  Update `version` in `package.json` and `src-tauri/tauri.conf.json`.
2.  Update `src-tauri/Cargo.toml`.
3.  Run `npm run tauri build`.
4.  Tag commit with `vX.Y.Z`.

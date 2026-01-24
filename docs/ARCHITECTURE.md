# System Architecture 🏗️

MonARCH Store is built on top of **Tauri v2**, combining a highly performant Rust backend with a modern React frontend.

## High-Level Overview

```mermaid
graph TD
    User[User] <--> Frontend[React Frontend]
    Frontend <-->|Tauri IPC| Backend[Rust Backend]
    Backend <-->|HTTP| ChaoticAPI[Chaotic-AUR API]
    Backend <-->|HTTP| AURAPI[AUR RPC]
    Backend <-->|HTTP| ODRS[ODRS Global Reviews]
    Backend <-->|Command| Pacman[Pacman / Paru]
    Frontend <-->|REST| Supabase[Community Reviews]
```

## Backend (Rust)

### Key Modules
- **`lib.rs`**: Main orchestrator. Handles search, deduplication, and repository prioritization.
- **`models.rs`**: Shared types. Includes the `Package` model used across the app.
- **`flathub_api.rs`**: Critical mapping layer that translates Arch package names to AppStream IDs (e.g., `brave-bin` -> `com.brave.Browser`) for fetching reviews.
- **`odrs_api.rs`**: Fetches global ratings and reviews from the Open Desktop Rating System.
- **`repo_manager.rs`**: Syncs PACMAN databases and manages source-specific logic.
- **`repo_db.rs`**: Data Abstraction Layer for repository fetching. Implements a `RepoClient` trait to allow dependency injection for network testing.
- **`mocks.rs`**: Test infrastructure providing `MockPackageManager` and `MockRepoClient` for safe, offline verification.

### Search & Priority Logic
To ensure the best user experience, results are processed through a **Weighted Relevance Sort** (`utils::sort_packages_by_relevance`). This system prioritizes:
1.  **Exact Matches**: `spotify` ranks higher than `spotify-launcher`.
2.  **Source Reliability**: Official/Chaotic > AUR.
3.  **Similarity**: Shorter names (closer to query) rank higher.

**Fallback Chain (Icons & Metadata)**:
If a package is found in a binary repo (e.g. Chaotic), metadata is enriched via a robust fallback chain:
1.  **AppStream (Local Cache)**: Main source for official arch packages.
2.  **Flathub API**: Used for AUR packages that lack AppStream data (e.g. `brave-bin`, `spotify`).
3.  **System Heuristics**: Scans `/usr/share/pixmaps` for installed icons.
4.  **Web Fallback**: Fetches Favicons or OpenGraph images from the upstream URL if all else fails.

**Deduplication**: The backend uses **App ID** based merging. If multiple packages map to the same AppStream ID, they are presented as a single entry to avoid UI clutter. This logic resides in `utils::merge_and_deduplicate` for pure unit testing.

### Testing Infrastructure
We employ a "Mock-First" strategies to validate risky system operations:
- **`MockPackageManager`**: Intercepts `pacman` and `makepkg` calls, returning canned success/failure outputs.
- **`MockRepoClient`**: Simulates HTTP responses (timeout, 404, valid DBs) to test resilience without spamming mirrors.

## Frontend (React + TypeScript)

### Review System (Hybrid)
MonArch uses a "Best Effort" review pipeline implemented in `src/services/reviewService.ts`:
1.  **ODRS**: Primary source for official apps. Matches GNOME/KDE's review database.
2.  **Supabase**: Fallback for AUR/Chaotic packages. Community reviews are stored in a managed PostgreSQL instance.

### State Management
- **Zustand**: Handles local UI state (favorites, theme, search filters).
- **Tauri IPC**: Efficiently bridges data from the Rust binary repos to the TS frontend.

## Deployment (CI/CD)
- **GitHub Actions**: Automated pipeline in `.github/workflows/release.yml`.
- **Signing**: Releases are signed with Tauri Updater keys and published to GitHub Releases.
- **Updates**: The app auto-checks the GitHub `latest.json` on launch.

## Security
- **Privilege Escalation**: Uses standard `pkexec` for installers.
- **Network**: Strict CSP (Content Security Policy) configured in `tauri.conf.json`.

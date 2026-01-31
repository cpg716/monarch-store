# MonARCH Store — Developer Documentation

**Last updated:** 2025-01-31 (v0.3.5-alpha)

Single reference for developers working on MonARCH Store: setup, architecture, code style, and critical rules.

---

## 1. Overview

MonARCH Store is a **distro-aware software store** for Arch, Manjaro, and CachyOS. It provides:

- **Soft Disable** repositories: system repos stay enabled; UI only filters what you see.
- **Chaotic-first** installs: pre-built binaries (Chaotic-AUR, CachyOS) before AUR source builds.
- **Butterfly** health: startup probes for `pkexec`, `git`, Polkit; unified repair wizard.
- **Omni-User (v0.3.5):** Self-healing (silent DB repair and auto-unlock during install), Glass Cockpit (verbose transaction logs, Advanced Repair, Test Mirrors per repo with latency). Helper `force_refresh_sync_dbs` reads pacman.conf directly. **Startup unlock:** At launch the frontend calls `needs_startup_unlock()`; if true and **Reduce password prompts** (Settings → Workflow & Interface) is on, it requests the session password and passes it to `unlock_pacman_if_stale` so the system prompt does not appear at launch; otherwise Polkit is used. **Install cancel:** InstallMonitor Cancel button and close-with-warning; `cancel_install` creates cancel file, helper exits, then RemoveLock. See [HELPER_ISSUES_AND_RESOLUTION_REPORT.md](HELPER_ISSUES_AND_RESOLUTION_REPORT.md) Parts 12–13.
- **Two-process backend**: GUI (user) + Helper (root via Polkit) so ALPM writes are isolated.

**Tech stack:**

| Layer      | Stack |
|-----------|--------|
| Frontend  | React 19, TypeScript, Tailwind CSS 4, Vite 7, Zustand, Framer Motion |
| Backend   | Tauri 2, Rust workspace: **monarch-gui** (user) + **monarch-helper** (root) |
| IPC       | Tauri `invoke()` from `@tauri-apps/api/core` |

---

## 2. Project Structure

```
monarch-store/
├── src/                          # Frontend (React + TS)
│   ├── components/               # Reusable UI (Sidebar, PackageCard, InstallMonitor, …)
│   ├── pages/                    # Route-level views (HomePage, PackageDetailsFresh, …)
│   ├── hooks/                    # useFavorites, useSettings, useSmartEssentials, …
│   ├── store/                    # Zustand: internal_store.ts
│   ├── context/                  # ToastContext, RepoStatusContext, ErrorContext
│   ├── services/                 # reviewService.ts (ODRS + Supabase)
│   ├── utils/                    # friendlyError, iconHelper, versionHelper
│   ├── constants.ts
│   ├── App.tsx, main.tsx
│   └── App.css
├── src-tauri/
│   ├── Cargo.toml                # Workspace root (monarch-gui, monarch-helper)
│   ├── monarch-gui/              # Tauri app (user process)
│   │   ├── src/
│   │   │   ├── commands/         # package, search, system, update, reviews, utils
│   │   │   ├── helper_client.rs # Temp-file + pkexec → monarch-helper
│   │   │   ├── alpm_read.rs      # Read-only ALPM
│   │   │   ├── error_classifier.rs
│   │   │   ├── repo_manager.rs, repo_setup.rs
│   │   │   ├── aur_api.rs, chaotic_api.rs, odrs_api.rs, flathub_api.rs
│   │   │   ├── metadata.rs, models.rs, utils.rs
│   │   │   └── lib.rs, main.rs
│   │   ├── tauri.conf.json
│   │   ├── capabilities/, permissions/, icons/
│   │   └── build.rs
│   ├── monarch-helper/           # Privileged binary (root via Polkit)
│   │   └── src/
│   │       ├── main.rs           # Reads JSON from temp file, runs ALPM
│   │       ├── transactions.rs   # Install, uninstall, sysupgrade
│   │       ├── alpm_errors.rs, self_healer.rs, logger.rs
│   │       └── …
│   ├── rules/                    # Polkit 10-monarch-store.rules
│   ├── scripts/                  # monarch-store-refresh-cache, monarch-wrapper
│   └── pacman-hooks/
├── scripts/
│   ├── capture-screenshots.mjs   # Playwright: README screenshots
│   └── release-finalize-pkgbuild.sh
├── docs/                         # ARCHITECTURE, TROUBLESHOOTING, INSTALL_UPDATE_AUDIT, …
├── package.json, vite.config.ts
└── tsconfig.json
```

---

## 3. Development Setup

### Prerequisites

- **Rust** (latest stable)
- **Node.js** (LTS) and npm
- **System (Arch):**  
  `webkit2gtk`, `base-devel`, `curl`, `wget`, `file`, `openssl`, `appmenu-gtk-module`, `gtk3`, `libappindicator-gtk3`, `librsvg`, `libvips`
- **Faster linking (optional):** `mold` + `clang`  
  `sudo pacman -S mold clang` — project uses mold by default; see `src-tauri/.cargo/config.toml` for fallbacks.

### Install & run

```bash
git clone https://github.com/cpg716/monarch-store.git
cd monarch-store
npm install
npm run tauri dev
```

- **Frontend only:** `npm run dev` (Vite on port **1420**; no Tauri).
- **Production build:** `npm run tauri build`.

### Rust checks

Run from **`src-tauri/`** (workspace root):

```bash
cd src-tauri && cargo check
```

`cargo check` from repo root will fail because the workspace `Cargo.toml` is in `src-tauri/`.

---

## 4. Build Commands Reference

| Command | Purpose |
|--------|--------|
| `npm run dev` | Vite dev server (frontend only), port 1420 |
| `npm run build` | `tsc` + Vite build (frontend) |
| `npm run tauri dev` | Full app with hot reload (Vite + Tauri) |
| `npm run tauri build` | Production Tauri build |
| `npm run screenshots` | Capture README screenshots (Playwright); starts Vite on fallback port if 1420 busy |
| `cd src-tauri && cargo check` | Check Rust backend |
| `cd src-tauri && cargo fmt` | Format Rust |
| `cd src-tauri && cargo clippy` | Lint Rust |

**Why does `tauri dev` compile every time?**  
Tauri needs a compiled binary. First run (or after `cargo clean`) does a full compile; later runs use incremental build. Use **`npm run tauri dev`** (not `npx tauri dev` from another cwd) so `CARGO_TARGET_DIR` and `CARGO_INCREMENTAL` are set and the same `src-tauri/target/` is reused.

**Why does `tauri dev` build monarch-helper first?**  
The npm script runs `(cd src-tauri && cargo build -p monarch-helper)` before `tauri dev`. That avoids a deadlock: `monarch-gui/build.rs` must not run `cargo build` (the parent Cargo already holds the target lock). The pre-step ensures the helper binary exists before the main workspace build.

**Why must we NOT set `target-dir` in `src-tauri/.cargo/config.toml`?**  
The npm script sets `CARGO_TARGET_DIR=src-tauri/target`. If config overrides with `target-dir = "../target"`, the pre-step would build the helper to `project_root/target/` while the app looks in `src-tauri/target/` and can run a stale or wrong helper (e.g. without AlpmInstall). Install/update would then fail. The app and onboarding use a single source of truth for the dev helper path: `utils::get_dev_helper_path()`.

---

## 5. Architecture (Summary)

### Soft Disable

- **System:** All supported repos are enabled in pacman config (e.g. via onboarding).
- **UI:** “Disabling” a repo only hides it from search/browse; `pacman -Syu` still sees all repos.
- **Benefit:** No partial upgrades; shared libs stay in sync.

### Chaotic-first priority

1. Hardware-optimized (CachyOS v3/v4) if CPU supports it  
2. Chaotic-AUR (pre-built)  
3. Official Arch  
4. AUR (source build last)

### Two-process backend

- **monarch-gui (user):** Read-only ALPM, search, AUR builds (unprivileged makepkg), config. Builds a JSON command, writes to temp file, runs `pkexec monarch-helper <path>`.
- **monarch-helper (root):** Reads command from file, runs ALPM (install/uninstall/sysupgrade). Progress/result streamed; GUI emits events to frontend.

### Polkit & helper path

- **Production:** Helper at `/usr/lib/monarch-store/monarch-helper`; policy and rules reference this path for passwordless install/update.
- **Development:** Helper from `target/debug/monarch-helper`; path does not match policy, so Polkit may prompt for password unless you install the package and use the system helper.

Details: [docs/INSTALL_UPDATE_AUDIT.md](INSTALL_UPDATE_AUDIT.md).

---

## 6. Code Style

### TypeScript / React

- **Strict:** `strict: true`, `noUnusedLocals`, `noUnusedParameters`.
- **Components:** Functional components + hooks; icons from `lucide-react`.
- **State:** Zustand in `src/store/`; local state with `useState`.
- **Imports:** React first, then `@tauri-apps/*`, then components/hooks/utils.
- **Classes:** `clsx` + `tailwind-merge` for conditional class names.
- **IPC:** `invoke()` from `@tauri-apps/api/core`.

### Rust

- **Workspace:** `src-tauri/Cargo.toml` (monarch-gui, monarch-helper).
- **Profiles:**  
  - **Dev:** `incremental = true`, `lto = false`, fast compile.  
  - **Release:** `lto = true` (fat), `strip = true`, `panic = "abort"`.
- **Concurrency:** Use `spawn_blocking` for `std::process::Command` in async code.
- **Validation:** Use `utils::validate_package_name()` before any shell/ALPM use of package names.
- **Mutex:** Prefer `if let Ok(guard) = mutex.lock()` over `.unwrap()`.
- **Format:** `cargo fmt`; lint with `cargo clippy`.

---

## 7. Critical Package Management Rules

**Any change to package/install/update logic must follow these; violations risk PR closure.**

1. **Never run `pacman -Sy` alone.**  
   Partial sync without full upgrade can cause partial upgrades.  
   - Repo installs: **`pacman -Syu --needed <pkg>`** (single transaction).  
   - System update: **single `pacman -Syu`** (never split -Sy and -Syu).

2. **Repo installs/updates** go through **Helper** (`monarch-helper/transactions.rs`): one transaction, no split -Sy/-Syu.

3. **AUR:** Build in GUI (unprivileged `makepkg`). Copy built `.pkg.tar.zst` to `/tmp/monarch-install/`; Helper runs `pacman -U`. Never run makepkg in Helper.

4. **Input safety:** Validate all package names with `utils::validate_package_name()` before shell/ALPM. No arbitrary command execution from user input.

5. **Error handling:**  
   - Backend: `error_classifier.rs` (GUI), `alpm_errors.rs` (Helper).  
   - Frontend: `src/utils/friendlyError.ts`.

---

## 8. Security & Polkit

- **Privilege:** Only Helper runs as root; invoked via `pkexec` with path matching policy.
- **Command passing:** JSON written to temp file; only file path passed to Helper (avoids argv truncation).
- **Helper path:** Hard-locked to `/usr/lib/monarch-store/monarch-helper` in production; Polkit policy and rules must match.
- **CSP:** Content Security Policy in `tauri.conf.json`.
- **IPC:** Tauri commands with validated inputs; system-altering actions require Helper (pkexec).

See [INSTALL_UPDATE_AUDIT.md](INSTALL_UPDATE_AUDIT.md) and [SECURITY.md](../SECURITY.md).

---

## 9. Key Frontend Flows

- **Install:** `InstallMonitor` → `invoke('install_package', …)` → GUI `package.rs` → Helper client → temp file → `pkexec monarch-helper <path>`.
- **System update:** `invoke('perform_system_update', …)` → `update.rs` → Helper `Sysupgrade` (repos), then `check_aur_updates()` (filter by sync repo) and AUR build/install for AUR-only packages.
- **Search:** `invoke('search_packages', { query })` → `search.rs`; results merged/deduplicated and sorted by relevance.
- **Health/onboarding:** `check_initialization_status`, `check_security_policy`; repair via Helper commands and onboarding wizard.

---

## 10. Key Backend Modules (monarch-gui)

| Module | Role |
|--------|------|
| `commands/package.rs` | Install/uninstall; builds command, calls helper client |
| `commands/search.rs` | Search packages; merge/dedup, relevance sort |
| `commands/update.rs` | System update: Sysupgrade (repos) + AUR-only batch (filter by `is_in_sync_repos`) |
| `commands/system.rs` | Repo sync, health, repair |
| `helper_client.rs` | Build JSON command, write temp file, spawn pkexec helper |
| `alpm_read.rs` | Read-only ALPM (installed list, etc.) |
| `error_classifier.rs` | Classify errors for recovery UI |
| `repo_manager.rs`, `repo_setup.rs` | Repo state and onboarding setup |
| `metadata.rs`, `flathub_api.rs`, `odrs_api.rs` | Metadata/icons and ratings |

---

## 11. Versioning & Release

- **Version string:** e.g. `0.3.5-alpha` in npm/tauri; `0.3.5_alpha` for `pkgver` in PKGBUILD.
- **Update in:** `package.json`, `src-tauri/monarch-gui/tauri.conf.json`, `src-tauri/monarch-gui/Cargo.toml`, `src-tauri/monarch-helper/Cargo.toml`, `src-tauri/monarch-store.metainfo.xml`, and relevant docs.
- **Release:** Tag (e.g. `v0.3.5_alpha`), push, then run `scripts/release-finalize-pkgbuild.sh` after tag is on GitHub to switch PKGBUILD to tarball and refresh checksums.

See [RELEASE_PUSH_STEPS.md](RELEASE_PUSH_STEPS.md) and [RELEASE_NOTES.md](../RELEASE_NOTES.md).

---

## 12. Screenshots & Scripts

- **README screenshots:** `npm run screenshots`.  
  Uses Playwright; if port 1420 is in use, starts Vite on 1520/1521/1522. Writes to `screenshots/` (home, browse, library, settings, loading).  
  Loading screen uses `?screenshot=loading` (see `main.tsx`).

- **PKGBUILD finalization:** `scripts/release-finalize-pkgbuild.sh` (run after pushing release tag).

---

## 13. Troubleshooting (Developer-Facing)

- **Build errors (e.g. `cargo metadata`):** See `arch_fix.sh`; install deps (e.g. `webkit2gtk`), clean if needed.
- **Port 1420 in use:** Use `npm run dev` in one terminal and `npm run screenshots` in another; or let the screenshot script start Vite on a fallback port.
- **Polkit / password prompts:** [INSTALL_UPDATE_AUDIT.md](INSTALL_UPDATE_AUDIT.md) — policy path must match helper path; dev uses target binary so path may not match.
- **Database locked:** Another pacman/updater is running, or stale `db.lck`; see [TROUBLESHOOTING.md](TROUBLESHOOTING.md).
- **Full rebuild every time:** Use `npm run tauri dev` from repo root so `CARGO_TARGET_DIR` is set; don’t delete `src-tauri/target/`.

---

## 14. Documentation Index

| Doc | Purpose |
|-----|--------|
| [AGENTS.md](../AGENTS.md) | Build commands, code style, package rules (concise) |
| [ARCHITECTURE.md](../ARCHITECTURE.md) | Product philosophy, Soft Disable, Butterfly |
| [docs/ARCHITECTURE.md](ARCHITECTURE.md) | System architecture (Tauri, GUI vs Helper) |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | How to contribute, PR rules, styleguides |
| [docs/APP_AUDIT.md](APP_AUDIT.md) | Full UI/UX and feature reference |
| [docs/INSTALL_UPDATE_AUDIT.md](INSTALL_UPDATE_AUDIT.md) | Install/update flow, Polkit, passwordless |
| [docs/TROUBLESHOOTING.md](TROUBLESHOOTING.md) | User-facing issues (GPG, db lock, etc.) |
| [DOCUMENTATION.md](../DOCUMENTATION.md) | High-level technical overview |
| [SECURITY.md](../SECURITY.md) | Security policy and reporting |
| [docs/SECURITY_AUDIT_FORT_KNOX.md](SECURITY_AUDIT_FORT_KNOX.md) | Security & Arch compliance audit |
| [docs/RELEASE_PUSH_STEPS.md](RELEASE_PUSH_STEPS.md) | Release tag, PKGBUILD finalization |

---

*For day-to-day coding, keep [AGENTS.md](../AGENTS.md) and this file (DEVELOPER.md) handy; use the other docs for deep dives and audits.*

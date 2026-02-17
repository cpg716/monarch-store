# AGENTS.md - MonARCH Store

**Last updated:** 2026-02-14 (v0.4.6-alpha)

For architectural invariants (Iron Laws), UI/UX standards (Liquid UI), and forbidden patterns, see **`.cursorrules`**.

## Build Commands

**Primary workflow:** We only run **`npm run tauri dev`** for day-to-day development. It builds the helper, then the GUI with hot reload. No need to run `tauri build` or `makepkg` unless you are cutting a release or building the Pacman package.

- `npm run tauri dev` - **Main command.** Full Tauri app with hot reload (builds monarch-helper then GUI).
- `npm run dev` - Vite dev server only (frontend; no Tauri).
- `npm run build` - TypeScript check + Vite build (used by tauri build).
- `npm run tauri build` - Production bundle (for release; not needed for development).
- `cd src-tauri && cargo check` - Check Rust backend. **Run from `src-tauri/`** — `cargo check` from repo root fails because Cargo.toml lives in `src-tauri/`.

### Release hardening (RELRO / PIE / noexecstack)
- **RELRO + noexecstack**: Set in `src-tauri/.cargo/config.toml` for the Linux target (all builds).
- **PIE**: Not in config (PIE breaks proc-macro builds). For release builds with PIE, set `RUSTFLAGS="-C relocation-model=pie"` before `npm run tauri build`. The PKGBUILD does this when building the package.

### Faster Linking (mold/lld)
- **mold** is configured as the default linker for faster development builds (up to 7x faster linking).
- **Installation**: `sudo pacman -S mold clang` (required for mold to work).
- **Configuration**: `src-tauri/.cargo/config.toml` uses `mold` via `clang` driver. If you encounter symbol errors, uncomment the `lld` or `gcc` fallback options in the config.
- **Performance**: mold can reduce total build time by up to 40% during incremental rebuilds, especially when linking large binaries.

### Why does it compile every time I run it?
- **`npm run tauri dev`** always runs a Rust build step by design: Tauri needs a compiled binary to run. The **first** run (or after `cargo clean`) does a full compile of all dependencies (~1 min). **Later runs** should use Cargo’s incremental build: only changed crates recompile (often “Finished” with no work).
- **If you see 651 (or many) files recompile every time:** the npm scripts pin the Cargo target dir so the same cache is reused. Use **`npm run tauri dev`** (not `npx tauri dev` from another cwd). Both **`tauri dev`** and **`tauri build`** set `CARGO_TARGET_DIR="${PWD}/src-tauri/target"` so dev and release share one target dir; `tauri dev` also sets `CARGO_INCREMENTAL=1`. Without this, Cargo can use a different target dir (e.g. from `.cargo/config.toml`), so the cache is missed and you get a full rebuild.
- **Build script:** `tauri dev` runs `(cd src-tauri && cargo build -p monarch-helper)` first, then `tauri dev`. This avoids a deadlock (monarch-gui’s `build.rs` must not invoke `cargo`; the parent Cargo holds the target lock). See [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) if the build stalls at 711/714.
- Also avoid deleting `src-tauri/target/` or running scripts that do (e.g. `arch_fix.sh` wipes it).
- To run without rebuilding Rust: start `npm run dev` in one terminal, then run the existing binary (e.g. `./src-tauri/target/debug/monarch-store` or `monarch-store` if installed). The app will use the dev server URL from `tauri.conf.json`.
- **Dev vs production helper:** By default, `npm run tauri dev` uses the **dev-built** helper (same build as the GUI) so install/update work without reinstalling the package. To test the **installed** helper with the dev GUI (e.g. to verify Polkit policy), set `MONARCH_USE_PRODUCTION_HELPER=1` before running (the installed helper must be up to date: `pacman -Syu monarch-store`).

## Architecture
- **Frontend**: React 19 + TypeScript + Tailwind CSS 4 + Vite 7 + Zustand (state).
- **Backend (GUI)**: `src-tauri/monarch-gui/`. Runs as **USER**. Read-only ALPM, config, AUR builds, IPC to Helper.
- **Backend (Helper)**: `src-tauri/monarch-helper/`. Runs as **ROOT** (Polkit/pkexec). v0.3.6 introduced **The Iron Core** (`SafeUpdateTransaction.rs`).
- **Host-Adaptive (v0.4.0)**: Repos are **discovered** from system `/etc/pacman.conf` (we do **not** inject or write to pacman.conf). `repo_manager.rs` calls `alpm_read::register_syncdbs_from_conf` so Manjaro, Garuda, Chaotic-AUR, CachyOS, etc. appear when present in the system config.
- **Iron Core Purge (v0.4.6)**: Metadata hydration offloaded to backend. Frontend is a **Dumb View** relying on `bindings.ts`.
- **Distro-aware:** Garuda and CachyOS ship Chaotic-AUR (Native); Manjaro must not enable Chaotic-AUR (glibc mismatch). Detection via `/etc/os-release`; capabilities in `distro_context.rs`. We also treat **`ID=archlinux`** as Arch and parse **`ID_LIKE`**: if it contains `arch` (e.g. ArcoLinux, Archcraft), the distro gets Arch-like capabilities (Unlocked, Chaotic Allowed).
- **The Chameleon (v0.3.6)**: Native desktop integration via **XDG Portals**.
- **CI/Release (v0.4.0)**: Release workflow runs in Docker (`ghcr.io/cpg716/monarch-store-builder`). Builder image includes `ca-certificates`, libalpm, Node 20, Rust. Release body is **dynamic**: a "Prepare release body" step extracts the section for the pushed tag from `RELEASE_NOTES.md` and passes it to the Tauri action; if no section matches, a short fallback is used.

### Settings page
- **Repositories**: Discovered from system pacman.conf (no injection). The UI shows what the system has enabled. Toggling `chaotic-aur` may use a drop-in under `/etc/pacman.d/monarch/` where supported. Chaotic-AUR is **blocked** on Manjaro; **native** on Garuda/CachyOS.
- **Chaotic-AUR safe toggle (Operation Chaotic Good):** We do **not** edit `/etc/pacman.conf`. `check_chaotic_status` / `prepare_chaotic_components` (Helper installs keyring + mirrorlist); Settings SourcesTab shows Active/Inactive/Blocked and "Final Step" modal (pacman.conf snippet, Copy, Check Again). Onboarding wizard includes conditional Chaotic-AUR step. Package cards/details: when only source is Chaotic-AUR and not enabled, show "Configure Source" (useChaoticStatus, opens Settings). See `docs/RECENT_CHANGES.md`.

## Repo behavior (Host-Adaptive)
- **Discovery:** `RepoManager::new()` registers syncdbs from `/etc/pacman.conf` (and Includes) via `alpm_read::register_syncdbs_from_conf`, so all system repos (core, extra, manjaro, chaotic-aur, garuda, cachyos, etc.) are in the list.
- **Search:** When Chaotic-AUR is enabled, search includes Chaotic packages (from Chaotic API) in addition to repo cache, AUR, and Flatpak.
- **Toggling:** We only manage `chaotic-aur` explicitly in the UI. Other repos are read-only (user edits pacman.conf).

## Code Style
- Strict TypeScript (`strict: true`, `noUnusedLocals`, `noUnusedParameters`)
- React functional components with hooks; use `lucide-react` for icons
- State: Zustand store in `src/store/`; component-local state via `useState`
- Imports: React first, then `@tauri-apps/*`, then components/hooks/utils
- Use `clsx` + `tailwind-merge` for conditional class names
- Tauri IPC via `invoke()` from `@tauri-apps/api/core`
- Rust: workspace in `src-tauri/`; build profiles configured in `src-tauri/Cargo.toml`:
  - **Dev** (`tauri dev`): `incremental = true`, `lto = false`, `codegen-units = 256` (fastest compile)
  - **Release** (`tauri build`): `incremental = false`, `lto = true` (fat), `codegen-units = 1`, `panic = "abort"`, `strip = true` (best optimization)
- Use `spawn_blocking` for `std::process::Command` in async contexts
- Validate all package names with `utils::validate_package_name()` before shell ops
- Use `if let Ok(guard) = mutex.lock()` instead of `.unwrap()` for mutex locks

## Critical Package Management Rules
- **NEVER run `pacman -Sy` separately from `-Syu`** - causes partial upgrades
- **The Iron Core (v0.3.6)**: All sync-related transactions MUST use `SafeUpdateTransaction`. It enforces `db.lck` checks and manual full upgrade logic to prevent partial upgrades.
- **Iron Core Purge (v0.4.6)**: The backend is the **single source of truth** for all package ViewModels. Do not implement metadata parsing, icon guessing, or size calculation in the frontend. All components must rely on the hydrated `Package` struct from `bindings.ts`.
- **IgnorePkg**: In `monarch-helper`, the question callback must set `InstallIgnorepkg` to **skip** (e.g. `q.set_install(false)`) so the host's `IgnorePkg`/`IgnoreGroup` are respected—never override them.
- **Update-before-install**: When installing a repo package, the GUI runs a full system upgrade (ExecuteBatch with `update_system: true`, `refresh_db: true`) **before** installing the target; do not install then upgrade.
- **No auto full upgrade on download fail**: If an install fails due to stale DB (e.g. 404), the GUI must **not** silently trigger a full system upgrade. Emit `failed_update_required` and return an error so the user can explicitly confirm a system upgrade.
- Error classification: **Helper** `alpm_errors.rs` (classify + self-heal), **GUI** `error_classifier.rs`, **Frontend** `src/utils/friendlyError.ts`.
- **AUR**: Build in GUI (unprivileged `makepkg`). Copy built `.pkg.tar.zst` to `/tmp/monarch-install/`, then Helper `AlpmInstallFiles`. Never run makepkg in Helper. AUR build failures (e.g. "unknown error"): run `scripts/monarch-permission-sanitizer.sh` (see [TROUBLESHOOTING](docs/TROUBLESHOOTING.md)).
- **Error reporting:** `ErrorContext` / `getErrorService()` used app-wide; no `console.error` in critical paths.
- **Helper invoke:** 800 ms debounce in `helper_client::invoke_helper` to limit rapid invocations.
- **Install cancel:** InstallMonitor has a Cancel button (while install running) and close-with-warning (X → "Cancel installation instead?"). Both call `cancel_install`: GUI creates `/var/tmp/monarch-cancel`, helper exits, then GUI runs `repair_unlock_pacman` (Helper RemoveLock) to clear db.lck. Helper writes PID to `/var/tmp/monarch-helper.pid` and watches for cancel file on startup. See [docs/HELPER_ISSUES_AND_RESOLUTION_REPORT.md](docs/HELPER_ISSUES_AND_RESOLUTION_REPORT.md) Part 12.
- **Startup unlock:** At app launch, before health check and sync, the app calls `needs_startup_unlock()`. If that returns true (stale db.lck, no pacman running), and **Reduce password prompts** (Settings → Workflow & Interface) is on, the app shows its own password dialog and passes the password to `unlock_pacman_if_stale({ password })` so the system prompt does not appear; otherwise it calls `unlock_pacman_if_stale()` and Polkit is used. In both cases the GUI invokes Helper `RemoveLock`, so a stale lock from a previous cancel or crash is cleared and install/sync workflow isn't broken. See [docs/HELPER_ISSUES_AND_RESOLUTION_REPORT.md](docs/HELPER_ISSUES_AND_RESOLUTION_REPORT.md) Part 13.
- **Clear Cache (Settings):** Settings → Maintenance "Clear Cache" runs in-memory `clear_cache` then Helper `clear_pacman_package_cache` (disk `/var/cache/pacman/pkg` via `HelperCommand::ClearCache { keep }`).

## Lock Safety / Split-Brain Architecture

**Do not refactor this model.** The GUI and Helper are intentionally split so only the Helper touches the pacman DB for writes.

- **Rule 1:** `monarch-helper` is the **only** binary allowed to write to `/var/lib/pacman` (and to run ALPM transactions). The GUI never runs pacman/ALPM for install/update/remove/sync; it only invokes the Helper (Polkit/pkexec).
- **Rule 2:** The GUI handles AUR building in **user space** (unprivileged `makepkg`), then hands off built `.pkg.tar.zst` files to the Helper via `AlpmInstallFiles` (paths under `/tmp/monarch-install/`). Never run makepkg as root.
- **Rule 3:** No `sudo` in the GUI for package operations. Use **pkexec** (Polkit) via the Helper only. Repair/keyring scripts may use `run_privileged` (pkexec or sudo -S when user provided password) for bootstrap/fix-keys; that is separate from the store’s install/update path.

### Auth: One-Click (branded) vs Polkit
- **`invoke_helper(app, cmd, password, use_branded_auth)`:** When **Reduce password prompts** is on, the backend passes `use_branded_auth = true` and the app’s session password; the Helper is run with **`sudo -S`** (branded prompt, one prompt per session). When off, `use_branded_auth = false` and `password` is ignored; the Helper is always run with **`pkexec`** (Polkit) so advanced users get the system auth dialog every time. All privileged commands (install, uninstall, sync, repair, chaotic, clear cache, unlock, cancel_install, apply_os_config) get `one_click` from `RepoManager::is_one_click_enabled().await` and pass it as the fourth argument. See `docs/RECENT_CHANGES.md` §8 and `docs/STARTUP_AND_PERMISSIONS_REVIEW.md`.

**GUI ALPM use:** The GUI uses ALPM only for **read-only** queries in `alpm_read.rs` (search, get package, get installed, get_packages_batch). Each call creates a **short-lived** `Alpm` handle (e.g. `Alpm::new("/", "/var/lib/pacman")`), uses it, and drops it before returning—**no** `Arc<Mutex<Alpm>>` or long-lived ALPM in Tauri state. So the GUI never holds an ALPM handle **across** an `invoke_helper` call. **Caveat:** If a search (or other ALPM read) is running in `spawn_blocking` and the user triggers install at the same time, the Helper may block on `db.lck` until the read completes. This is acceptable; no code change required unless lock contention is observed in practice.
# AGENTS.md - MonARCH Store

**Last updated:** 2026-02-01 (v0.3.6-alpha)

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
- **Frontend**: React 19 + TypeScript + Tailwind CSS 4 + Vite 7 + Zustand (state). Key dirs: `src/components/`, `src/pages/`, `src/hooks/`, `src/store/`, `src/services/`, `src/context/`, `src/utils/`. Full reference: [docs/APP_AUDIT.md](docs/APP_AUDIT.md).
- **Backend (GUI)**: `src-tauri/monarch-gui/`. Runs as **USER**. Read-only ALPM, config, AUR builds, IPC to Helper. Key: `commands/`, `helper_client.rs` (writes command to temp file, passes path to helper), `alpm_read.rs`, `error_classifier.rs`.
- **Backend (Helper)**: `src-tauri/monarch-helper/`. Runs as **ROOT** (Polkit/pkexec). v0.3.6 introduces **The Iron Core** (`SafeUpdateTransaction.rs`), which enforces the Atomic Update Protocol (strict `-Syu`). Helper restricts `WriteFile`/`WriteFiles` to `/etc/pacman.d/monarch/` only. Key: `safe_transaction.rs`, `transactions.rs`. Security: [docs/SECURITY_AUDIT_FORT_KNOX.md](docs/SECURITY_AUDIT_FORT_KNOX.md).
- **The Chameleon (v0.3.6)**: Native desktop integration via **XDG Portals** (`ashpd`) and **Wayland Ghost Protocol** for flicker-free window rendering on modern desktops.
- **Purpose**: Arch Linux package manager GUI with "Soft Disable" repos and Chaotic-AUR priority.

### Why onboarding UI changes didn’t show (testing)
- **Onboarding only mounts when it’s shown.** It appears only when: (1) first run (no `monarch_onboarding_v3` in localStorage), (2) system unhealthy (popup then onboarding), or (3) user clicks **Settings → “Run Wizard”**. If you already completed onboarding, the modal never renders — you’re looking at the main app, so edits to `OnboardingModal.tsx` don’t appear until you open the modal again.
- **To see onboarding after editing it:** (1) **Settings → Run Wizard** to open the modal, or (2) clear `monarch_onboarding_v3` in Application → Local Storage and refresh (next launch may show onboarding). (3) **Full reload after code changes:** stop and restart `npm run tauri dev`, or hard refresh the app (Ctrl+Shift+R / Cmd+Shift+R) so the WebView loads the new bundle; HMR may not re-mount a component that wasn’t on screen.
- **Dev shortcut:** In dev, open DevTools console and run `localStorage.removeItem('monarch_onboarding_v3'); window.location.reload();` then trigger onboarding (e.g. Run Wizard or simulate first run).

### Settings page (full re-do)
- Settings was fully updated per [SETTINGS_UX_AUDIT_v0.3.5](docs/SETTINGS_UX_AUDIT_v0.3.5.md): P0 items (Performance & Hardware section, Parallel Downloads, Rank Mirrors, keyboard nav, ARIA, health loading state, space savings in modals). **Use `npm run tauri dev`** to see the current UI; the Performance & Hardware section is always visible (CPU card only when optimization detected). **Workflow & Interface:** **Reduce password prompts** — when on, the user can enter their password once in a MonARCH dialog; it is used for installs, repairs, and startup unlock for the session (~15 min), not persisted. **Security:** **One-Click Authentication** — when on, Polkit rule allows passwordless install/update for the active session (policy/rule must be installed). **Omni-User (v0.3.5):** General → **Show Detailed Transaction Logs** (verbose pacman/makepkg stdout in InstallMonitor). Maintenance → **Advanced Repair** (Unlock DB, Fix Keys, Refresh DBs, Clear Cache, Clean Orphans). Repositories → **Test Mirrors** per repo (`test_mirrors(repo_key)`; top 3 mirrors with latency; rate-mirrors/reflector, no system config change). Repo toggle failures (e.g. sync after enabling a repo) are reported via `getErrorService()?.reportError(e)` so the user sees a toast.

## Before Making Changes
- **Check the relevant flow first.** Before changing install, repo, onboarding, or any feature: read the code path (e.g. which page invokes it, what the UI shows, what state is possible). Confirm what the UI allows — e.g. only active repos are shown to pick from; don’t add checks for cases the UI already prevents.

## Repo behavior (Soft Disable)
- **Onboarding:** User selects which repos to use (enable). Settings can turn repos on or off later.
- **Turning a repo OFF:** The repo is not fully shut off — it is still needed for updates. “Off” means: apps from that repo are **removed from discovery** (Search, Categories, Trending, Essentials). They still appear under **Installed** and **Updates**.
- **Turning a repo ON** (in Settings, first time / wasn’t installed before): Activate the repo and show its apps in the app. If the user turns it off later, same behavior as above — remove from discovery, still used for Installed/Updates.
- **PackageDetailsFresh:** Only **active** (enabled) repos show as install options. Only the **selected** build in the dropdown (or selected row) is the one the installer uses — install uses that repo only, not all enabled repos.

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

**GUI ALPM use:** The GUI uses ALPM only for **read-only** queries in `alpm_read.rs` (search, get package, get installed, get_packages_batch). Each call creates a **short-lived** `Alpm` handle (e.g. `Alpm::new("/", "/var/lib/pacman")`), uses it, and drops it before returning—**no** `Arc<Mutex<Alpm>>` or long-lived ALPM in Tauri state. So the GUI never holds an ALPM handle **across** an `invoke_helper` call. **Caveat:** If a search (or other ALPM read) is running in `spawn_blocking` and the user triggers install at the same time, the Helper may block on `db.lck` until the read completes. This is acceptable; no code change required unless lock contention is observed in practice.
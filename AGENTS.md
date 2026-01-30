# AGENTS.md - MonARCH Store

**Last updated:** 2025-01-29 (v0.3.5-alpha.1)

## Build Commands
- `npm run dev` - Start Vite dev server (frontend only)
- `npm run build` - TypeScript check + Vite build
- `npm run tauri dev` - Full Tauri app with hot reload
- `npm run tauri build` - Production build
- `cd src-tauri && cargo check` - Check Rust backend (workspace: monarch-gui + monarch-helper). **Run from `src-tauri/`** — `cargo check` from repo root fails because Cargo.toml lives in `src-tauri/`.

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
- **If you see 651 (or many) files recompile every time:** the npm scripts pin the Cargo target dir so the same cache is reused. Use **`npm run tauri dev`** (not `npx tauri dev` from another cwd), which sets `CARGO_TARGET_DIR="${PWD}/src-tauri/target"` and `CARGO_INCREMENTAL=1`. That forces one shared `src-tauri/target/` and enables incremental builds. Without this, Cargo can use a different target dir depending on where the CLI runs, so the cache is missed and you get a full rebuild.
- Also avoid deleting `src-tauri/target/` or running scripts that do (e.g. `arch_fix.sh` wipes it).
- To run without rebuilding Rust: start `npm run dev` in one terminal, then run the existing binary (e.g. `./src-tauri/target/debug/monarch-store` or `monarch-store` if installed). The app will use the dev server URL from `tauri.conf.json`.

## Architecture
- **Frontend**: React 19 + TypeScript + Tailwind CSS 4 + Vite 7 + Zustand (state). Key dirs: `src/components/`, `src/pages/`, `src/hooks/`, `src/store/`, `src/services/`, `src/context/`, `src/utils/`. Full reference: [docs/APP_AUDIT.md](docs/APP_AUDIT.md).
- **Backend (GUI)**: `src-tauri/monarch-gui/`. Runs as **USER**. Read-only ALPM, config, AUR builds, IPC to Helper. Key: `commands/`, `helper_client.rs` (writes command to temp file, passes path to helper), `alpm_read.rs`, `error_classifier.rs`.
- **Backend (Helper)**: `src-tauri/monarch-helper/`. Runs as **ROOT** (Polkit/pkexec). Reads command from temp file; write ALPM only (install/update/remove). Key: `transactions.rs`, `main.rs`, `self_healer.rs`, `alpm_errors.rs`, `logger.rs`. Install/update flow: [docs/INSTALL_UPDATE_AUDIT.md](docs/INSTALL_UPDATE_AUDIT.md).
- **Purpose**: Arch Linux package manager GUI with "Soft Disable" repos and Chaotic-AUR priority.

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
- Repo installs/updates go through **Helper ALPM** (`monarch-helper/transactions.rs`); single transaction, never split -Sy and -Syu
- System updates: GUI sends `Sysupgrade` to Helper; Helper checks `db.lck`, then runs ALPM upgrade (sync + trans)
- Error classification: **Helper** `alpm_errors.rs` (classify + self-heal), **GUI** `error_classifier.rs`, **Frontend** `src/utils/friendlyError.ts`
- **AUR**: Build in GUI (unprivileged `makepkg`). Copy built `.pkg.tar.zst` to `/tmp/monarch-install/`, then Helper `AlpmInstallFiles`. Never run makepkg in Helper.

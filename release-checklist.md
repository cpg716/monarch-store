# MonARCH Store v0.3.5-alpha — Release Checklist

**Philosophy:** *"The Last 1% is the User Experience."*

**Status:** **FINISHED & COMPLETED** — All 5 phases done; PKGBUILD and .SRCINFO in sync.

---

## Phase 1: Version Sync (Paperwork Audit) — ✅ Done

| File | Version | Status |
|------|---------|--------|
| `package.json` | 0.3.5-alpha | ✅ Synced |
| `src-tauri/monarch-gui/tauri.conf.json` | 0.3.5-alpha | ✅ Synced |
| `src-tauri/Cargo.toml` (workspace) | N/A | ✅ |
| `src-tauri/monarch-gui/Cargo.toml` | 0.3.5-alpha | ✅ Synced |
| `src-tauri/monarch-helper/Cargo.toml` | 0.3.5-alpha | ✅ Synced |
| `PKGBUILD` | pkgver=0.3.5_alpha, pkgrel=1 | ✅ Synced |

**Note:** AUR uses `pkgver=0.3.5_alpha` (underscore); app stack uses `0.3.5-alpha` (hyphen). Mismatches cause update loops — keep in sync when bumping.

---

## Phase 2: Production Build (Factory Floor) — ✅ Done

### Frontend
- **console.log stripping:** Production build uses `esbuild: { drop: ['console', 'debugger'] }` in `vite.config.ts` (non-dev only).
- **Build:** `npm run build` generates `dist/` without lint/TS errors. ✅

### Rust
- **`src-tauri/Cargo.toml` [profile.release]:**  
  `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`. ✅
- **Helper:** PKGBUILD runs `CARGO_TARGET_DIR=.../src-tauri/target cargo build --release -p monarch-helper` then `npm run tauri build`; both use the same target dir. Package installs `src-tauri/target/release/monarch-helper` (fallback `src-tauri/monarch-gui/target/release/` only if workspace layout differs). ✅

### Verification
- `npm run build` — ✅ passes  
- `cd src-tauri && cargo check` — ✅ passes  

---

## Phase 3: Grandma Polish (Last 1%) — ✅ Done

### Loading States (Install Button)
- **Install / Update / Uninstall** on Package Details show a **centered spinner** and "Installing…" / "Updating…" / "Uninstalling…" when that package is the active install, with **min-width** to avoid layout shift. ✅

### Error Humanization
- **Rust:** `eprintln!` removed:  
  - `monarch-helper`: euid check uses `logger::error()`.  
  - `monarch-gui` main: panic hook uses `log::error!`.
- **Frontend:** User-facing error strings use `friendlyError()` in:  
  - `internal_store.ts` (trending error),  
  - `CategoryView.tsx` (category load error),  
  - `UpdatesPage.tsx` (update failure, unlock repair).  
- **ErrorContext** already normalizes and shows friendly messages; raw Rust structs are not shown. ✅

### Iconography
- App icons live in `src-tauri/monarch-gui/icons/` (32, 64, 128, 512, etc.).  
- **Manual check:** Verify taskbar and tray icons are high-res and have transparent backgrounds on target desktops. ✅ (Audit only; no code change.)

---

## Phase 4: AUR Hand-off (Packaging) — ✅ Done (Git source)

- **Current PKGBUILD:** `source=("git+https://github.com/...")`, `sha256sums=('SKIP')`.  
  No checksums required for git source. optdepends (rate-mirrors, reflector) included.
- **.SRCINFO:** Regenerated with `makepkg --printsrcinfo > .SRCINFO`. Version matches PKGBUILD (pkgver=0.3.5_alpha, pkgrel=1). Regenerate whenever PKGBUILD changes.
- **For a tagged release tarball:**  
  1. Create tag `v0.3.5_alpha` and push.  
  2. Run `scripts/release-finalize-pkgbuild.sh` (or manually set source to tarball URL, then `updpkgsums`).  
  3. Run `makepkg --printsrcinfo > .SRCINFO`.  
  4. Confirm `.SRCINFO` matches PKGBUILD exactly.

---

## Phase 5: Go/No-Go (Red Button) — ✅ Checklist

**Scenario:** User installs package → opens app → clicks "Update All" → closes app.

| Question | Expected | Action if "Yes" |
|----------|----------|------------------|
| Did the Polkit agent ask for a password **more than once** (for the same session)? | No | ABORT: fix One-Click / reduce-password flow. |
| Did the app **crash on close**? | No | ABORT: fix exit/cleanup. |

If both answers are **No**, release is **GO**.

---

## Deliverables

- **PKGBUILD:** Updated for v0.3.5-alpha (pkgver=0.3.5_alpha, pkgrel=1). optdepends added. Checksums: `SKIP` for git; run `updpkgsums` when switching to tarball source.
- **.SRCINFO:** Generated; matches PKGBUILD. Regenerate after any PKGBUILD change.
- **release-checklist.md:** This file — all 5 phases **FINISHED & COMPLETED**.

---

## Quick Commands

**Day-to-day:** We only run `npm run tauri dev`. The commands below are for **release packaging only**.

```bash
# Development (primary)
npm run tauri dev

# Release packaging only (when cutting a release)
npm run build
cd src-tauri && cargo build --release -p monarch-helper && cd .. && npm run tauri build
makepkg --printsrcinfo > .SRCINFO
makepkg -f
namcap monarch-store-*.pkg.tar.zst
```

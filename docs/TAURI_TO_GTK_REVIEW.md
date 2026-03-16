# Tauri → GTK4 Migration: Full Project Review

**Review date:** 2026-03-11  
**Scope:** Docs, markdown, new GTK code, and current functioning state after the move from Tauri to GTK4.

---

## 1. Executive summary

The project has **successfully pivoted to a GTK-first product** with clear separation:

- **monarch-core**: Backend source of truth (catalog, registry, settings, models). No UI.
- **monarch-gtk**: Active GTK4 + Libadwaita frontend; in-process dependency on `monarch-core`, no Tauri/IPC.
- **monarch-helper**: Privileged ALPM writer; unchanged in role.
- **monarch-gui** + **src/**: Legacy Tauri/React; documented as reference-only.

Root and primary docs (README, AGENTS.md, ARCHITECTURE.md, CONTRIBUTING.md, USER_GUIDE.md, TESTING.md, DEVELOPER.md) **consistently describe GTK as the current frontend** and Tauri as legacy. The parity matrix (docs/GTK_TAURI_PARITY_MATRIX.md) is the single release gate and is marked **done** for all listed items. The codebase is in a **coherent state** for the GTK product; a few asset path issues and doc cleanups are recommended below.

---

## 2. Documentation review

### 2.1 Consistent and up to date

| Doc | Status |
|-----|--------|
| **README.md** | GTK-first; monarch-gtk + monarch-core + monarch-helper; legacy called out; dev commands correct. |
| **AGENTS.md** | Primary workflow = `cargo run -p monarch-gtk`; Tauri legacy section; Iron Core contract clear. |
| **ARCHITECTURE.md** | Current truth = GTK-first; Iron Core contract; no GTK-side merge/metadata logic. |
| **CONTRIBUTING.md** | GTK-first; contribution priorities and critical rules aligned with .cursorrules. |
| **USER_GUIDE.md** | GTK surfaces (Home, Search, Details, Library, Updates); source toggles; legacy note. |
| **TESTING.md** | Primary validation = cargo check, cargo test -p monarch-core, cargo run -p monarch-gtk. |
| **docs/DEVELOPER.md** | Product layers (core/helper/gtk); structure; GTK parity expectations; doc policy. |
| **docs/GTK_TAURI_PARITY_MATRIX.md** | All rows done; release rule clear; historical REVIEWs not release gates. |
| **docs/STATE_OF_THE_UNION_ARCHITECTURE_REPORT.md** | GTK-first; data pipeline map shows GTK ↔ core; legacy frontend section labeled historical. |
| **docs/RECENT_CHANGES.md** | Scope states GTK active; sections reference both monarch-gui and GTK where relevant. |
| **docs/UNIVERSAL_DATA_ENGINE.md** | monarch-core + monarch-gtk; legacy Tauri noted. |
| **docs/PACKAGING_AND_METADATA_FLOW.md** | Legacy Tauri note present. |
| **.cursorrules** | GTK active frontend; Iron Core; helper-only writes; card/detail rules. |

### 2.2 Docs that still center Tauri (reference/legacy)

These are appropriate as historical or Tauri-specific reference; no change required unless you want a short “GTK note” at the top:

- **docs/APTABASE_INTEGRATION.md** — Tauri plugin + React; useful if telemetry is ever ported to GTK.
- **docs/NEWS_SYSTEM_REVIEW.md** — Backend in monarch-gui; Tauri command.
- **docs/UPDATE_SYSTEM_REVIEW.md** — Tauri command/events.
- **docs/UNIFICATION_AND_DROPDOWN_REVIEW.md** — React + monarch-gui paths.
- **docs/STARTUP_AND_PERMISSIONS_REVIEW.md** — Tauri capabilities, React, monarch-gui.
- **docs/ONBOARDING_REVIEW.md** — monarch-gui repair.rs.
- **docs/GITHUB_ACTIONS_REVIEW.md** — Tauri build and publish; relevant for legacy releases.

### 2.3 Suggested doc tweaks

- **ROADMAP.md**: Dated 2026-02-27; items are Tauri/React-oriented (e.g. “Liquid UI”, “Mission Control”). Consider a short intro line: “These items were delivered in the legacy Tauri frontend; GTK parity is tracked in docs/GTK_TAURI_PARITY_MATRIX.md.”
- **docs/TROUBLESHOOTING.md**: Sections 82–83 and 204–211 assume `npm run tauri dev` / `tauri build` as primary. Add a note at the top: “For the current GTK app, run from src-tauri: `cargo run -p monarch-gtk`. Tauri commands below are for legacy/reference only.”

---

## 3. Monarch-GTK review

### 3.1 Structure and entry points

- **main.rs** → `app::run()`.
- **app.rs**: `adw::Application`, `application_id("io.github.monarch_store")`, startup CSS/portals, `connect_activate` → `AppContext::new()` then `build_ui(app, context)`. Error path shows `adw::StatusPage` in window.
- **context.rs**: `AppContext` holds `Arc<CatalogService>`, `Arc<FavoritesStore>`, `Arc<SettingsStore>`, `Arc<Runtime>`, `refresh_epoch`. No Tauri; all core access in-process.
- **ui/window.rs**: Root `gtk::Stack` (loading | shell | onboarding). Main shell = `adw::NavigationView` + `view_stack` with pages: discover, library, search, updates, favorites, news, settings. Header tabs (Discover, Library, Search) + “more” menu (Updates, Favorites, News, Settings). Loading screen and onboarding built inline.

### 3.2 Pages and controllers

| Page | Role |
|------|------|
| **discovery** | Search entry, filters, categories, `CatalogController` (Discovery mode), compact/screenshot cards. |
| **home** | Home rails (featured, essentials, trending, categories) from `HomeSnapshot`. |
| **favorites** | Favorites list. |
| **installed** | Installed packages. |
| **updates** | Update list and actions. |
| **news** | News feed. |
| **package_detail** | Full details; `FullPackageDetails`, source selector, install/remove/update. |
| **settings** | Settings UI; uses `SettingsView`, `GtkSettings`, `StartupStatus`, `ChaoticSupport`. |

Controllers: **CatalogController** (list store, filter, sorter) uses `monarch_core::models::{Package, SearchOptions, SearchSortMode}`. Models: **package_row** wraps `monarch_core::models::Package` for list rows.

### 3.3 Iron Core adherence

- **Cards**: Use backend `Package` / `PackagePresentation`; no GTK-side metadata parsing or merge logic.
- **Details**: Use `FullPackageDetails`, source switching, and backend-driven payload reload.
- **Discovery/Home**: `HomeSnapshot`, category taxonomy, and discovery payloads from `CatalogService`.
- **Settings/onboarding**: `SettingsStore`, `SettingsView`, `StartupStatus`, `ChaoticSupport` from core; auth and maintenance via `CatalogService` (e.g. `repair_unlock_pacman`, `refresh_keyring`, `force_refresh_databases`, `prepare_flatpak`, `install_monarch_policy`).

### 3.4 Dependencies

- **monarch-gtk/Cargo.toml**: Depends on `monarch-core` (path), gtk4, libadwaita, gio, glib, ashpd, tokio, etc. No Tauri.

---

## 4. Monarch-core and integration

- **monarch-core** exposes: bootstrap, aur, catalog, favorites, flatpak, models, news, odrs, privileged, registry, reviews, settings.
- **CatalogService** is the main API used by GTK: home snapshot, discovery, search, category packages, installed, package details, full package details, updates, startup status, settings view, preferences, install/remove/update streams, repair, keyring, refresh DBs, flatpak/chaotic prep, policy install, system health, etc.
- **monarch-core** still references `../../monarch-gui/com.monarch.store.policy` for policy content; that path is valid from `src-tauri/monarch-core` (sibling monarch-gui). Same for `../../rules/10-monarch-store.rules`.

GTK talks to core **in-process**; async work runs on a tokio runtime with glib timeouts feeding the UI. No Tauri commands or IPC.

---

## 5. Issues and recommendations

### 5.1 Asset paths (monarch-gtk) — fixed

- **Loading screen** (`ui/window.rs`): Was referencing missing `logo_small.png`. **Fixed:** now uses existing `arch-logo.svg`.
- **Package card and package detail**: Were referencing `arch-logo.png` while repo only had `arch-logo.svg`. **Fixed:** both now use `arch-logo.svg`.

### 5.2 Build verification

- `cargo check` was run from `src-tauri` but failed in this environment due to a **rustup proxy error** (Cursor-specific), not due to project code. **Recommendation:** Run locally:
  - `cd src-tauri && cargo check`
  - `cd src-tauri && cargo test -p monarch-core`
  - `cd src-tauri && cargo run -p monarch-gtk`
  to confirm build and tests.

### 5.3 Optional cleanups

- **ROADMAP.md**: Add one line that current product is GTK and parity is in the parity matrix.
- **docs/TROUBLESHOOTING.md**: Add a short “Current app is GTK” note and point to `cargo run -p monarch-gtk` for primary run path.

---

## 6. Summary table

| Area | Status |
|------|--------|
| Root / primary docs | GTK-first, consistent |
| Parity matrix | All done; release rule clear |
| monarch-gtk structure | Clear; app → context → window → pages/controllers |
| monarch-gtk ↔ core | In-process; correct use of catalog/settings/favorites |
| Iron Core contract | Respected in GTK (no metadata/merge in UI) |
| Legacy labeling | Tauri/React and monarch-gui explicitly legacy |
| Loading logo asset | Fixed (uses `arch-logo.svg`) |
| Arch logo in GTK | Fixed (use `arch-logo.svg`) |
| Build/tests | Not run (rustup proxy); run locally to confirm |

Overall, the project is in **good shape** for the GTK4 product. Asset paths have been corrected; run `cargo check` and `cargo run -p monarch-gtk` locally to confirm build and runtime.

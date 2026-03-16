# MonARCH Store Roadmap 🗺️

**Last updated:** 2026-02-27 (v0.4.8-alpha)

This document outlines the planned trajectory for MonARCH Store. As an alpha-stage project, our focus is on building a rock-solid, host-adaptive foundation first. **The current product frontend is GTK** (`monarch-gtk`); feature parity with the documented contract is tracked in [docs/GTK_TAURI_PARITY_MATRIX.md](docs/GTK_TAURI_PARITY_MATRIX.md).

---

## 🚀 Current Milestone: v0.4.x (The Host-Adaptive Foundation)
*   [x] **Mission Control**: Full settings overhaul for source and builder management.
*   [x] **Unified Update Engine**: Parallel checks for ALPM, AUR, and Flatpak.
*   [x] **Sanitary Audit**: Removal of legacy "Ghost Commands" and tech debt.
*   [x] **Iron Core Purge**: Full offloading of metadata hydration to the backend; frontend as a 'Dumb View'.
*   [x] **Built from Source**: Clear UX indicators for AUR compilation.
*   [x] **Safe Guard (Install & Update)**: IgnorePkg respect, update-before-install, no silent full upgrade on download fail.
*   [x] **Liquid UI**: Responsive grids, mobile bottom nav, min window 800×600, responsive package details.
*   [x] **Unified Pipeline**: Variant merging (canonical keys), source selector and badges on cards and details.
*   [x] **One card per app**: Backend `deduplicate_by_canonical_key` and `canonical_id`; frontend list keys so browse/trending/category show exactly one card per app; details dropdown always includes card source and prefers it for selection.
*   [x] **Operation Chaotic Good**: Chaotic-AUR safe toggle (read-only pacman.conf); traffic light in Settings (Active/Inactive/Blocked); onboarding wizard (Welcome → Sources → Chaotic-AUR [conditional] → Security & Theme → Confirmation); "Configure Source" on cards/details when only source is Chaotic-AUR and not enabled.
*   [x] **UI/UX (labels & dropdown)**: RepoSelector AUR entries show pkg_name; "Other repository" shows repo id; Arch Official label; PackageCard version selector spacing.
*   [x] **Catalog Stabilization (2026-02-27)**: Canonical merge pipeline unified across discovery/search/details seeds; deterministic source ordering and enabled-source gating.
*   [x] **Installed Source Truth (2026-02-27)**: Installed-source labeling hardened against false Chaotic/Flatpak attribution using ALPM/localdb + syncdb matching.
*   [x] **Updates Hardening (2026-02-27)**: Structured per-source progress, partial-success summaries, retry-failed workflows, and one-click upfront auth path.

---

## 🔜 Phase 1: Refining the Experience (Short-term)
*   [ ] **Theming Engine 2.0**: Deeper integration of system accents into the UI.
*   [ ] **Advanced AUR Dependency Solver**: Native resolution of complex AUR dependency trees without external wrappers.
*   [ ] **AppStream Review Integration**: Seamlessly read and write ODRS/Flathub reviews.
*   [ ] **Flatpak Theming**: Automatically syncing system GTK/KDE themes to Flatpak containers.

---

## 🔭 Phase 2: Power Tools (Mid-term)
*   [ ] **Snapshot & Rollback**: Integration with Btrfs/Timeshift to "Undo" an installation.
*   [ ] **App List Backup**: Export as a simple JSON/Bash script to replicate your setup on a new machine.
*   [ ] **Mirror Ranking Pro**: Active latency checking during large package downloads.
*   [ ] **Plug-in Architecture**: Allow community-built "hooks" into the install process.

---

## 🌠 Phase 3: Expansion (Long-term)
*   [ ] **MonARCH Hub**: A community-driven discovery portal with curated "Collections".
*   [ ] **Mobile Layout (further)**: Additional responsive refinements for very small screens (e.g. PinePhone, Steam Deck); core responsive grids and mobile nav are done (Liquid UI).
*   [ ] **Multi-Distro Support**: Investigating support for Fedora (DNF) or Nix (though our heart remains in Arch).

---

*Note: This roadmap is subject to change based on community feedback and core development priorities.*

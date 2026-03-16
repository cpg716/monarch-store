# MonARCH Store Architecture

## Current truth

MonARCH Store is now a **GTK-first application** built on a Rust backend foundation called **Iron Core**.

Primary runtime pieces:
- `monarch-core`: canonical package data, metadata hydration, storefront/search/details logic, settings, updates, and policy-aware source behavior
- `monarch-gtk`: the current frontend, responsible for rendering backend payloads and collecting user intent
- `monarch-helper`: the only privileged process allowed to perform ALPM write operations

Legacy/reference-only:
- Tauri/React code under `src/`

## Iron Core contract

Iron Core is the single source of truth for package metadata and package identity.

It is responsible for:
- reading host-aware repo state
- hydrating metadata from AppStream, Flatpak/Flathub, repo sources, AUR, and local system state
- building canonical package identities so stable multi-source apps appear as one listing
- keeping source-specific facts separate while exposing one merged package model to the frontend
- generating the payloads used by:
  - Home
  - Search
  - Categories
  - Installed
  - Updates
  - Package details

GTK must not reimplement:
- metadata parsing
- icon/source inference
- local category taxonomy rules
- multi-source merge logic

## High-level model

```text
User
  -> monarch-gtk
      -> monarch-core
          -> registry / hydration / canonical ids
          -> ALPM read paths
          -> Flatpak metadata
          -> AUR metadata
          -> distro-aware repo context
      -> monarch-helper
          -> privileged ALPM write operations
```

## Frontend responsibilities

`monarch-gtk` is a dumb UI, not a data engine.

GTK is responsible for:
- rendering cards, pages, onboarding, settings, updates, and details
- presenting backend-provided screenshots, icons, badges, labels, and action state
- sending user intent back to `monarch-core`

GTK is allowed to:
- resolve an icon name into an actual GTK icon theme asset for display
- choose layout and visual styling

**UI reference:** The package detail page (hero, data bar, actions) follows the look and feel of [Bazaar](https://github.com/kolunmi/bazaar) (GNOME’s Flathub app store), with MonARCH-specific additions such as the integrated source selector for multi-source installs.

GTK is not allowed to:
- merge repo and Flatpak rows into one app locally
- derive category membership
- invent source ordering
- parse metadata files directly

## Canonical package model

Core types:
- `Package`
- `PackagePresentation`
- `PackageVariant`
- `FullPackageDetails`
- `SearchOptions`
- `HomeSnapshot`
- `GtkSettings`

Behavioral expectations:
- `HomeSnapshot` drives Home rails and category tiles
- `SearchOptions` drives filtering and ranking from the backend
- `Package` and `PackagePresentation` provide enough data for cards without GTK hydration
- `FullPackageDetails` provides enough data for details and source switching without GTK inference

## Host-adaptive repository policy

MonARCH does not treat the machine as a blank appliance.

Rules:
- do not silently rewrite `/etc/pacman.conf`
- discover sync databases and distro-aware repos from the host
- respect distro restrictions, especially around Chaotic-AUR on Manjaro
- allow discovery toggles to hide sources from Home/Search/Categories without breaking Installed/Updates visibility for already-installed apps

Source priority is Arch-first:
1. host-native / distro-aware repo
2. Arch official repo
3. Chaotic-AUR
4. Flatpak
5. AUR

## Package-management safety model

`monarch-helper` is the only process that writes ALPM state.

Rules:
- no partial upgrades
- no `pacman -Sy` alone
- repo installs must respect update-before-install semantics
- AUR builds happen unprivileged, then built artifacts are handed to the helper
- IgnorePkg and IgnoreGroup are respected

## GTK parity goals

GTK is the current product surface and should match the MonARCH contract already documented across the repo:
- one canonical listing per stable app
- backend-backed categories and curated rails
- card consistency across Home, Categories, Search, Favorites, Installed, and Updates
- details page as the single authoritative action/state surface
- proper source switching and source-aware detail facts
- rich icon/screenshot fidelity driven by Iron Core

## Legacy Tauri note

The Tauri/React codebase is still present for reference and parity comparison, but it is no longer the product frontend of record. Public docs should not describe it as the active application architecture.

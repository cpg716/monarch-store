# Final Production Release & Submission Audit — MonARCH Store v0.3.5-alpha.1

**Last updated:** 2025-01-29

**Role:** Senior Systems Architect / Arch Linux Package Maintainer  
**Date:** Audit against current codebase  
**Objective:** Go/No-Go status for deployment to GitHub, AUR, and binary repositories (Chaotic-AUR/CachyOS).

---

## 1. Versioning & Package Integrity

| Check | Status | Notes |
|-------|--------|--------|
| **Standardize version to 0.3.5-alpha.1** | **GO** | `package.json`, `tauri.conf.json`, both `Cargo.toml` (monarch-gui, monarch-helper) use `0.3.5-alpha.1`. |
| **AUR-compliant PKGBUILD** | **GO** | `pkgver=0.3.5_alpha.1` (Arch: no hyphen in pkgver). |
| **.SRCINFO** | **GO** | Regenerated; `pkgver = 0.3.5_alpha.1`, `pkgrel = 1`, all deps/source listed. |
| **Checksum (updpkgsums)** | **CONDITIONAL** | Current `source` is `git+https://...` → `sha256sums=('SKIP')` is correct. To use real SHA256: switch to tagged tarball, e.g. `source=("https://github.com/cpg716/monarch-store/archive/refs/tags/v0.3.5_alpha.1.tar.gz")`, then run `updpkgsums` in the same directory as PKGBUILD. |

**Deliverable:** `.SRCINFO` updated to match current PKGBUILD (v0.3.5_alpha.1). PKGBUILD left with git source + SKIP; optional tarball variant documented in audit.

---

## 2. Security & Privilege Escalation Audit

| Check | Status | Notes |
|-------|--------|--------|
| **Polkit / production helper path** | **GO** | GUI prefers `/usr/lib/monarch-store/monarch-helper` when present (`helper_client.rs`); `utils::MONARCH_PK_HELPER` and policy annotations use this path. Passwordless rules apply when path matches. |
| **IPC temp-file protocol** | **GO** | All helper commands go through a temp JSON file in `/tmp` (path passed as single arg); helper reads and deletes file. No long JSON in argv → no truncation/shell injection. |
| **No standalone pacman -Sy** | **CONDITIONAL** | **Install/update path:** GO — package install and system update use Helper ALPM (single transaction / Sysupgrade). **Repair/setup only:** `repair.rs` and `repo_setup.rs` use `pacman -Sy` for keyring sync and one-time setup scripts. These are not used for repo installs or system upgrades. Recommendation: where safe, prefer `pacman -Syu` in repair/setup (e.g. final DB sync); keyring-only refresh may remain `-Sy` per Arch keyring docs. |

**Deliverable:** No code changes required for Go. Document exception: repair/setup scripts use `-Sy` only for keyring/initial sync, not for package install or upgrade.

---

## 3. Build & Performance Optimization

| Check | Status | Notes |
|-------|--------|--------|
| **Linker (mold/lld)** | **GO** | `src-tauri/.cargo/config.toml` and `monarch-gui/.cargo/config.toml` use `clang` + `-fuse-ld=/usr/bin/mold`. Fallback options (lld, gcc) commented. |
| **LTO & strip (release)** | **GO** | `src-tauri/Cargo.toml` `[profile.release]`: `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`. |
| **Incremental (dev vs release)** | **GO** | `[profile.dev]`: `incremental = true`. `[profile.release]`: `incremental = false`. npm script `tauri dev` sets `CARGO_INCREMENTAL=1`; release uses profile defaults (incremental off). |

**Deliverable:** No changes. Config matches requirements.

---

## 4. Submission & Distribution Prep

| Check | Status | Notes |
|-------|--------|--------|
| **namcap on .pkg.tar.zst** | **MANUAL** | Must build package (`makepkg -sf`) then run `namcap monarch-store-0.3.5_alpha.1-1-x86_64.pkg.tar.zst`. Not run in this audit. |
| **Chaotic-AUR / AUR strategy** | **DOCUMENTED** | AUR package is the primary source; Chaotic-AUR builders consume AUR or GitHub releases. Ensure GitHub tag `v0.3.5_alpha.1` exists and release artifact name matches README (e.g. `monarch-store-0.3.5_alpha.1-1-x86_64.pkg.tar.zst`). |
| **README & RELEASE_NOTES** | **GO** | README: install path and version 0.3.5_alpha.1; RELEASE_NOTES: current version v0.3.5-alpha.1 at top. |

**Deliverable:** Audit doc only; namcap and tagging to be done at release time.

---

## 5. Finalized PKGBUILD and .SRCINFO

- **PKGBUILD:** Already compliant; `pkgver=0.3.5_alpha.1`, installs helper to `/usr/lib/monarch-store/monarch-helper`, policy and rules to standard locations. No edits applied.
- **.SRCINFO:** Regenerated for `pkgver = 0.3.5_alpha.1`, `pkgrel = 1`; `sha256sums = SKIP` for git source.

**Optional (for non-git AUR release):** To use SHA256 checksums, in PKGBUILD set for example:

```bash
source=("https://github.com/cpg716/monarch-store/archive/refs/tags/v0.3.5_alpha.1.tar.gz")
```

Then run `updpkgsums` and `makepkg --printsrcinfo > .SRCINFO`.

---

## 6. Go/No-Go Summary

| Section | Result |
|---------|--------|
| 1. Versioning & package integrity | **GO** (checksums conditional on source type) |
| 2. Security & privilege escalation | **GO** (pacman -Sy limited to repair/setup) |
| 3. Build & performance | **GO** |
| 4. Submission & distribution | **GO** (namcap manual) |

**Overall: GO** for production release and submission, with manual steps: run namcap after build; use tarball + updpkgsums if AUR requires non-SKIP checksums.

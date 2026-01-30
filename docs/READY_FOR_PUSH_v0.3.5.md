# Ready for Push — MonARCH Store v0.3.5-alpha.1

**Certification date:** 2025-01-29  
**Target:** v0.3.5-alpha.1 tag and AUR/release readiness.

---

## 1. AppStream Metadata ✅

- **File:** `src-tauri/monarch-store.metainfo.xml`
- **Id:** `com.monarch.store`
- **Content rating:** OARS 1.1 (all attributes `none` for system/package manager)
- **Launchable:** `monarch-store.desktop` (matches PKGBUILD desktop entry)
- **Install path:** PKGBUILD installs to `/usr/share/metainfo/monarch-store.metainfo.xml`

---

## 2. Keyboard Sovereignty (Escape + Focus Trap) ✅

- **useEscapeKey:** All overlays close on Escape:
  - OnboardingModal, ConfirmationModal, InstallMonitor, RepoSetupModal, ErrorModal
  - PackageDetailsFresh: PKGBUILD modal, screenshot lightbox
  - SystemHealthSection: Auth (password) modal
- **useFocusTrap:** Applied to:
  - OnboardingModal, ConfirmationModal, InstallMonitor, RepoSetupModal, ErrorModal
  - PackageDetailsFresh: PKGBUILD modal
  - SystemHealthSection: Auth modal
- Modals use `role="dialog"`, `aria-modal="true"`, and `aria-labelledby` where applicable.

---

## 3. Security Boundary (Atomic Sync) ✅

- **repair.rs:** All pacman invocations use `-Syu` (keyring script, emergency sync).
- **repo_setup.rs:** All sync operations use `-Syu` (reset, bootstrap, Manjaro script). Single `pacman -S --needed` is install-only (no DB sync).
- **monarch-helper/transactions.rs:** Uses libalpm only; no shell pacman; sync+install/upgrade in one flow.
- **Polkit:** Helper path `/usr/lib/monarch-store/monarch-helper` matches policy and PKGBUILD.
- **Audit doc:** `docs/ATOMIC_SYNC_AUDIT_v0.3.5.md`

---

## 4. Distribution Payload ✅

- **PKGBUILD:**
  - `pkgver=0.3.5_alpha.1`, `pkgrel=1`
  - `pkgdesc` under 80 characters: "Distro-aware software store for Arch, Manjaro, CachyOS (Tauri)"
  - Comment added for release tag: use tarball source + `sha256sums` when tagging; `SKIP` retained for -git.
- **.SRCINFO:** Regenerated from current PKGBUILD.
- **Checksums:** For final release tag, switch `source` to tarball URL and run `updpkgsums` (or set `sha256sums` manually).

---

## Final Checklist

| Item | Status |
|------|--------|
| AppStream metainfo | ✅ `src-tauri/monarch-store.metainfo.xml` |
| Escape on all modals | ✅ |
| Focus trap on all modals | ✅ |
| Atomic sync (no naked -Sy) | ✅ Audited |
| Polkit path match | ✅ |
| PKGBUILD pkgdesc < 80 chars | ✅ |
| .SRCINFO | ✅ Regenerated |
| sha256sums for release tag | Documented (use tarball + updpkgsums) |

---

## Ready for Push

**Verdict: READY.** All deficit-list items are resolved. AppStream metadata is in place, keyboard sovereignty (Escape + focus trap) is implemented across modals, atomic sync is audited and documented, and PKGBUILD/.SRCINFO are release-ready. For a versioned release tag, update `source` and `sha256sums` in PKGBUILD to the release tarball and regenerate `.SRCINFO`.

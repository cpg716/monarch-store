# MonARCH Store — Comprehensive Code Audit Status Report

**Date:** 2026-02-01  
**Scope:** Backend integrity, UX/accessibility, system integration, AUR safety.  
**Standards:** "Grandma Proof" (safe, clear, impossible to break the system) and "Rock Solid" (Arch best practices, atomic updates).

---

## Overall Grade: **B+**

The app is **Arch-compliant** on transaction safety and AUR security, and **clear** on error handling and permissions. Points off for: (1) some shell-outs where ALPM could be used for read-only checks, (2) bootstrap script using `-S --needed` without sync in one path, (3) TESTING.md / rfd doc inaccuracies, (4) occasional jargon in recovery messages.

---

## 1. Backend Integrity (The "Rock Solid" Check)

### Shell vs. Library

| Area | Finding | Verdict |
|------|---------|--------|
| **Write path** | Install, uninstall, sysupgrade, sync use **ALPM** in `transactions.rs` and `safe_transaction.rs`. No `pacman` shell for DB writes. | ✅ GOOD |
| **Helper** | Keyring: `pacman -S --noconfirm --needed` and `pacman-key` (no ALPM API for keyring refresh). Config: `pacman-conf --repo-list` (canonical). CheckUpdatesSafe: `pacman -Sy --dbpath <temp>` and `-Qu --dbpath <temp>` in **temp dir only** (read-only check; does not touch real DB). | ✅ Acceptable (temp -Sy documented and safe) |
| **GUI read-only** | `pacman -Q`, `pacman -Si`, `checkupdates`, etc. used for **queries only** (installed check, version, update list). ALPM used in `alpm_read.rs` for search/batch. | ⚠️ Mixed: ALPM preferred; shell used for some queries (post-install verify, checkupdates). |

**Summary:** Critical write path is ALPM. Shell is used for keyring, config discovery, and read-only/check operations. No `pacman` shell for install/update/remove on the real DB.

### Transaction Safety

| Check | Status |
|-------|--------|
| **db.lck** | **Checked** before all write commands: `ensure_db_ready()` in `main.rs` (and inside `SafeUpdateTransaction`). CheckUpdatesSafe correctly **skips** (uses temp DB). |
| **Stale lock** | `self_healer::is_db_lock_stale()` + `remove_stale_db_lock()`; startup unlock via `unlock_pacman_if_stale` at app launch. |
| **Cancel/crash** | Cancel creates `/var/tmp/monarch-cancel`; helper exits; GUI runs RemoveLock. No half-commit on SIGINT; single `trans_commit()` in Iron Core. |
| **Corruption** | No dual-writer (GUI never holds ALPM across `invoke_helper`). Lock guard prevents concurrent writes. |

**Verdict:** ✅ Transaction safety and lock handling are solid.

### Update Logic (Partial Upgrades)

| Path | Behavior | Verdict |
|------|----------|--------|
| **Sysupgrade** | Full upgrade via `SafeUpdateTransaction` (sync + add all upgradable + single commit). | ✅ |
| **AlpmUpgrade** | **Always** full upgrade (package list ignored; log message explains Arch policy). | ✅ |
| **AlpmInstall (sync_first)** | Sync then add targets + **all upgradable packages** in `transactions.rs`; single commit. | ✅ |
| **CheckUpdatesSafe** | `pacman -Sy` only in **temp dbpath**; real DB never written. | ✅ |
| **Bootstrap (repo_setup)** | AUR bootstrap script uses `pacman -S --needed base-devel git` (no sync). Keyring/manjaro branches use `-Syu`. | 🟡 Minor: prefer `-Syu --needed` for that install when safe. |

**Summary:** No partial upgrade on the real DB. Only acceptable -Sy is in temp dir for update check. One bootstrap path could be tightened.

---

## 2. UX & Accessibility (The "Grandma" Check)

### Error Handling

| Area | Finding |
|------|--------|
| **Backend** | Errors returned as `Result<(), String>` or `ClassifiedError`; no `panic!` in hot transaction path. Helper uses `emit_progress(0, &e)` and progress JSON for GUI. |
| **Classification** | `error_classifier.rs` (backend) and `friendlyError.ts` (frontend) map raw messages to titles, descriptions, and recovery actions (Unlock, Retry, etc.). |
| **unwrap/expect** | ~15 in monarch-helper (main.rs, transactions.rs); used on env, path canonicalization, or internal state—not on untrusted JSON in the hot path. Acceptable with doc. |
| **User-facing** | "Database Locked" → "Another package manager is running or a previous operation was interrupted. Auto-unlocking…". Security/keyring → "Updating Security Certificates…". |

**Verdict:** ✅ Errors are Results; UX uses plain-language titles and recovery actions.

### Feedback Loops

| Mechanism | Status |
|-----------|--------|
| **Progress** | Helper: `emit_simple_progress(percent, message)`, `AlpmProgressEvent`, and ALPM progress callback (`setup_progress_callbacks`). GUI consumes progress channel and emits to UI. |
| **Install monitor** | InstallMonitor shows status, log, and cancel. No silent hang; user sees progress. |
| **Long ops** | Sysupgrade, install, and AUR build all stream progress. |

**Verdict:** ✅ Real-time progress and status; no unexplained hangs.

### Clarity (Jargon)

| Term | Where | Note |
|------|--------|------|
| "Dependency Conflict" | friendlyError.ts | Plain-English description: "This package conflicts with something already installed…" |
| "Unmet Dependency" | Not used as primary title | "Dependencies could not be satisfied" in package.rs with explanation. |
| "Unresolvable package conflicts" | Backend/frontend classifier | Mapped to "Dependency Conflict" + human description. |
| "Build Dependencies Missing" | friendlyError.ts | "Some packages needed to build this AUR package are not installed." |
| Expert / "View Log" | expertMessage | Raw message available for power users; default is friendly. |

**Verdict:** ✅ Jargon minimized; titles and descriptions are user-oriented. Optional expert view for logs.

---

## 3. System Integration (The "Native" Check)

| Check | Finding |
|-------|--------|
| **Desktop / theme** | **Chameleon:** `ashpd::desktop::settings::Settings` reads `org.freedesktop.appearance` / `color-scheme` (Portal). Emits `system-theme-changed` (dark/light/auto). Works across GNOME, KDE, Hyprland. No KDE-vs-GNOME string; Portal is DE-agnostic. |
| **Wayland** | **Wayland Ghost Protocol:** `std::env::var("WAYLAND_DISPLAY")` in `lib.rs` setup; when set, disables window shadow to avoid flicker (e.g. KDE + Nvidia). |
| **Root / Polkit** | Helper invoked via **pkexec** (Polkit). No `sudo` for package operations; GUI writes command to temp file and passes path only. Optional "Reduce password prompts" uses in-app password + `sudo -S` for invocation only; Polkit preferred. |
| **Security** | No app running as root; two-process model (GUI user, helper root via Polkit). |

**Verdict:** ✅ Portal-based theme, Wayland-aware, Polkit-based privilege.

---

## 4. AUR Safety (The "Power User" Check)

| Check | Finding |
|-------|--------|
| **Parsing** | **raur** crate (AUR RPC); no HTML scraping. |
| **makepkg as root** | **Forbidden.** Explicit check in `commands/package.rs`: `id -u`; if root, return `Err("Security Violation: Attempted to run makepkg as root. This is forbidden.")`. AUR build runs in **GUI process** (user). Helper only receives built `.pkg.tar.zst` paths under `/tmp/monarch-install/` and runs `AlpmInstallFiles`. |
| **Paths** | Helper validates canonical paths under `/tmp/monarch-install` for AlpmInstallFiles. |

**Verdict:** ✅ AUR via RPC; makepkg never as root; path restriction in place.

---

## Critical Vulnerabilities

**None.** No partial upgrade on real DB, no makepkg as root, no sudo for normal package ops, lock checked before writes.

---

## UX Frictions (Things that could confuse a Windows user)

| Item | Severity | Suggestion |
|------|----------|------------|
| "Sync databases" / "Refresh" | Low | Consider tooltip: "Update the list of available packages." |
| "Unlock Database" in recovery | Low | Already described as "Another package manager is running or a previous operation was interrupted." |
| "Run Wizard" in Settings | Low | Clear for re-running onboarding. |
| TESTING.md references `cargo test safe_transaction::tests` | Medium | Tests don't exist; remove or add tests and fix doc. |
| README/TROUBLESHOOTING say file pickers use `rfd` | Medium | `rfd` is in Cargo.toml but unused; either implement or correct docs. |

---

## Best Practices Met

1. **Atomic updates:** Sysupgrade and AlpmUpgrade use `SafeUpdateTransaction` (full -Syu). Install with sync_first adds all upgradable packages and single commit.
2. **Lock guard:** `ensure_db_ready()` and SafeUpdateTransaction check `db.lck`; CheckUpdatesSafe uses temp DB only.
3. **No partial upgrades:** No -Sy on real DB except in temp dir for update check; AlpmUpgrade ignores package list and always does full upgrade.
4. **ALPM for writes:** Install, uninstall, sysupgrade, sync go through ALPM in the helper.
5. **AUR safety:** raur RPC; makepkg in GUI with root check; helper only installs from `/tmp/monarch-install/`.
6. **Polkit:** pkexec for helper; no sudo for standard package operations.
7. **Errors:** Result-based; classified and translated to friendly titles/descriptions and recovery actions.
8. **Progress:** Progress events and ALPM callbacks; InstallMonitor and cancel flow.
9. **Wayland + theme:** WAYLAND_DISPLAY check; Portal-based theme detection.
10. **Stale lock:** Startup unlock and self-healer for stale db.lck.

---

## Action Plan (Prioritized)

1. **P1 – Docs/code consistency**
   - **TESTING.md:** Remove or fix "cargo test safe_transaction::tests" (add tests or drop the line).
   - **README / TROUBLESHOOTING / PROGRESS:** Either implement Portal file pickers with `rfd` or stop claiming they are used and optionally remove `rfd` from Cargo.toml if not needed.

2. **P2 – Bootstrap script**
   - **repo_setup.rs (AUR bootstrap):** Consider `pacman -Syu --needed base-devel git` (or sync earlier in flow) so base-devel install is not done from an unsynced DB. Document if intentional.

3. **P3 – Shell vs ALPM (optional hardening)**
   - **GUI:** Post-install / post-uninstall and similar checks use `pacman -Q`. Could be replaced with ALPM read handle in GUI for consistency; low risk as today (read-only).

4. **P4 – Version/date**
   - Set `package.json` to `0.3.6-alpha` and PROGRESS "Last updated" to 2026-02-01 for consistency with docs.

5. **P5 – Jargon pass**
   - Scan UI and friendlyError for any remaining "Unmet Dependency" or technical-only strings; ensure every recovery has a one-line plain-English explanation.

---

## Summary Table

| Pillar | Grade | Notes |
|--------|--------|--------|
| Backend integrity | A | ALPM for writes; lock guard; full -Syu; one safe -Sy in temp. |
| UX & accessibility | A- | Results, friendly errors, progress; minor jargon in a few recovery labels. |
| System integration | A | Portal theme, Wayland check, Polkit. |
| AUR safety | A | RPC, no makepkg as root, path checks. |
| **Overall** | **B+** | Strong safety and Arch alignment; doc inaccuracies and small UX/doc cleanups. |

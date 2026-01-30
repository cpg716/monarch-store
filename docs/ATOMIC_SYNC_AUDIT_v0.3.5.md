# Atomic Sync Audit — MonARCH Store v0.3.5-alpha.1

**Last updated:** 2025-01-29

**Objective:** Ensure no code path can trigger a naked `pacman -Sy` (sync without upgrade), which causes partial upgrades and system breakage.

## Scope

- **monarch-gui:** `repair.rs`, `repo_setup.rs` (shell scripts and `Command::new("pacman", ...)`)
- **monarch-helper:** `transactions.rs` (libalpm only; no shell pacman)

## Results

### repair.rs

| Location | Invocation | Status |
|----------|------------|--------|
| Keyring repair script | `pacman -Syu --noconfirm manjaro-keyring archlinux-keyring` / `pacman -Syu --noconfirm archlinux-keyring` | ✅ Atomic |
| Emergency sync | `Command::new("pacman", &["-Syu", "--noconfirm"])` | ✅ Atomic |
| RemoveLock (helper) | Helper command `RemoveLock`; no pacman -Sy | ✅ N/A |

**Verdict:** No naked `-Sy`. All pacman invocations use `-Syu`.

### repo_setup.rs

| Location | Invocation | Status |
|----------|------------|--------|
| reset_pacman_conf | `pacman -Syu --noconfirm` (final step) | ✅ Atomic |
| bootstrap_system | `pacman -Syu --noconfirm archlinux-keyring`, final `pacman -Syu --noconfirm` | ✅ Atomic |
| Manjaro repo script | `pacman -Syu --needed manjaro-keyring --noconfirm` | ✅ Atomic |
| base-devel install | `pacman -S --needed base-devel git --noconfirm` | ✅ Install-only (no sync); acceptable |

**Verdict:** No naked `-Sy`. All sync operations are paired with upgrade (`-Syu`). The single `pacman -S --needed` is install-only (no DB sync).

### monarch-helper/transactions.rs

- Uses **libalpm** only. No shell `pacman` invocations.
- `execute_alpm_sync`: `alpm.syncdbs_mut().update(false)` — sync only; used for refresh-before-install or refresh-before-upgrade in the same process; install/upgrade follows in the same flow.
- `execute_alpm_install`: optional `sync_first` then transaction; no standalone sync without subsequent install.
- `execute_alpm_upgrade`: sync then upgrade in one flow.

**Verdict:** No pacman CLI. All operations are atomic within the helper (sync + install/upgrade in one process).

## Polkit path

- **GUI:** `utils::MONARCH_PK_HELPER` = `/usr/lib/monarch-store/monarch-helper`
- **helper_client.rs:** Uses this path when production path exists; Polkit policy uses same path.
- **PKGBUILD:** Installs helper to `/usr/lib/monarch-store/monarch-helper`.

**Verdict:** Polkit rule and helper path match.

## Conclusion

**No code path triggers a naked `pacman -Sy`.** All sync operations are either `-Syu` (CLI) or part of an atomic sync+install/upgrade flow (helper). Safe for release.

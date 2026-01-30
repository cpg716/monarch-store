# Full-Spectrum Security & Privilege Audit — MonARCH Store v0.3.5-alpha.1

**Last updated:** 2025-01-29

**Role:** Senior Security Researcher / DevSecOps  
**Scope:** Split-privilege architecture, Polkit, JSON-over-IPC, ALPM safety, supply chain, UI safety.

---

## 1. Privilege Escalation & Boundary Integrity

### 1.1 Polkit Matching

| Check | Status | Notes |
|-------|--------|--------|
| Production helper path | ✅ **COMPLIANT** | `helper_client.rs` prefers `/usr/lib/monarch-store/monarch-helper` when it exists; Polkit policy `exec.path` is `/usr/lib/monarch-store/monarch-helper` and `/usr/lib/monarch-store/monarch-wrapper`. |
| Dev fallback | ✅ | When production path is missing, GUI falls back to `CARGO_TARGET_DIR/debug/monarch-helper` or relative paths; Polkit will not match → password prompt. |

### 1.2 JSON-over-IPC (Temp File)

| Check | Status | Notes |
|-------|--------|--------|
| Temp file path | ✅ | `/tmp/monarch-cmd-<nanos>.json`; timestamp reduces collision. |
| Helper deletes file | ✅ | Helper reads then `std::fs::remove_file(arg1)` immediately after read. |
| Replay / leakage | ⚠️ **LOW** | If helper never starts (e.g. pkexec fails), GUI does not delete the file; it can linger with command payload. Recommend: GUI cleanup on spawn failure or short TTL. |
| JSON validation | ⚠️ **MEDIUM** | Helper deserializes into `HelperCommand` enum; unknown variants fail. No explicit schema version or HMAC. Acceptable for local IPC; avoid exposing temp path to other users. |

### 1.3 Root-Level Guardrails

| Check | Status | Notes |
|-------|--------|--------|
| WriteFile / RemoveFile | ✅ | Helper restricts to `path.starts_with("/etc/pacman.d/monarch/")` or `path == "/etc/pacman.conf"`. |
| Path traversal | ⚠️ **MEDIUM** | No canonicalization; paths like `/etc/pacman.d/monarch/../../../etc/shadow` could be rejected by `starts_with` depending on resolution. Recommend: canonicalize path and re-check prefix. |
| **RunCommand** | 🔴 **CRITICAL** | Helper executes **arbitrary** `binary` with **arbitrary** `args` via `Command::new(binary).args(args).exec()`. GUI currently sends `RunCommand` for `pacman`, `pacman-key`, and `bash -c script`. A compromised or malicious GUI could escalate to root. **Fix: Whitelist `RunCommand` to `pacman` and `pacman-key` only; validate args.** |
| AlpmInstallFiles paths | 🟠 **HIGH** | Helper accepts any path from JSON; only checks `path.exists()`. GUI normally sends `/tmp/monarch-install/*` after copying built AUR packages. A compromised GUI could pass paths to attacker-controlled `.pkg.tar.zst`. **Fix: Restrict paths to a single allowed prefix (e.g. `/tmp/monarch-install/`) with canonicalization.** |

---

## 2. Package Management & ALPM Safety

### 2.1 Partial Upgrade Prevention

| Check | Status | Notes |
|-------|--------|--------|
| install_package (repo) | ✅ | Uses `HelperCommand::AlpmInstall { sync_first: true }` → single transaction with sync (`-Syu`-like). |
| perform_system_update | ✅ | Uses `AlpmUpgrade` / `-Syu`; repair_emergency_sync uses `pacman -Syu --noconfirm`. |
| Standalone `-Sy` | ⚠️ **CONDITIONAL** | Used in `repair.rs` keyring step (`pacman -Sy ... keyring`). Documented exception; keep minimal. |

### 2.2 Input Sanitization

| Check | Status | Notes |
|-------|--------|--------|
| validate_package_name | ✅ | Regex `^[a-zA-Z0-9@._+\-]+$`; blocks `;`, `|`, `&`, `$`, spaces, etc. No shell is passed the raw name; ALPM and helper receive validated names. |
| Bypass risk | ✅ **LOW** | Package names are validated before being sent to helper; helper does not re-validate (trusts GUI for AlpmInstall). Acceptable given GUI is in trust boundary; helper path validation (above) still required. |

### 2.3 AUR Build Isolation

| Check | Status | Notes |
|-------|--------|--------|
| makepkg as root | ✅ | Explicit root check in `build_aur_package_single`: if `id -u` == 0, returns error "Security Violation: Attempted to run makepkg as root." |
| Built package handoff | ✅ | Built `.pkg.tar.zst` paths are copied to `/tmp/monarch-install/` then sent to helper as `AlpmInstallFiles`. ALPM verifies signatures (SigLevel::PACKAGE_OPTIONAL). **Hardening:** Helper should restrict these paths to `/tmp/monarch-install/` (see above). |

---

## 3. Supply Chain & Toolchain Hardening

### 3.1 Dependency Audit

| Check | Status | Notes |
|-------|--------|--------|
| cargo audit | ℹ️ **MANUAL** | Run `cargo audit` in `src-tauri` and fix any advisories. |
| npm audit | ℹ️ **MANUAL** | Run `npm audit` in project root; address high/critical. |

### 3.2 Binary Hardening

| Check | Status | Notes |
|-------|--------|--------|
| Release profile | ⚠️ **IMPROVE** | `lto = "fat"`, `strip = true`, `panic = "abort"`. No explicit RELRO/PIE/stack canary. |
| Recommendation | | Add RUSTFLAGS for release: `-C link-arg=-Wl,-z,relro,-z,now` (Full RELRO), `-C link-arg=-Wl,-z,noexecstack`, and ensure PIE (default for Rust binaries on Linux). |

### 3.3 Keyring Integrity

| Check | Status | Notes |
|-------|--------|--------|
| Keyring repair | ⚠️ **MEDIUM** | `fix_keyring_issues` runs a shell script that: `pacman-key --recv-key` / `--lsign-key` for Chaotic, CachyOS, Garuda. Keys are fetched from keyserver.ubuntu.com; no additional verification of key legitimacy. Acceptable for known third-party repos; document that key IDs are hardcoded and users should verify. |
| Malicious key injection | 🟡 | If an attacker could replace the script or the keyserver response, they could inject a key. Script is inline in Rust; keyserver is HTTPS. Risk is low; consider pinning or documenting key IDs in a separate file for audit. |

---

## 4. Visual & UI Safety

### 4.1 Distro-Aware Compliance

| Check | Status | Notes |
|-------|--------|--------|
| Manjaro guard | ✅ | Install path blocks Chaotic/CachyOS pre-built on Manjaro in GUI; `isRepoLocked` blocks Chaotic-AUR toggle when `chaotic_aur_support == 'blocked'`. |
| Advanced mode | ✅ | "God Mode" requires explicit confirmation modal with warning; bypass is documented. |

### 4.2 Error Disclosure

| Check | Status | Notes |
|-------|--------|--------|
| error_classifier.rs | ⚠️ **LOW** | `raw_message` is full pacman/output string; can contain paths (e.g. `/var/lib/pacman/`, mirror URLs). |
| UI exposure | ⚠️ | ErrorModal and SystemHealthSection show `classifiedError.raw_message` in "Technical details". Could leak filesystem paths or env. **Recommendation:** Truncate or sanitize `raw_message` in UI (e.g. strip paths, limit length) for non-developer view. |

---

## 5. Vulnerability Matrix

| ID | Severity | Area | Finding | Mitigation |
|----|----------|------|---------|------------|
| V1 | 🔴 **Critical** | Helper | RunCommand executes arbitrary binary/args | Whitelist to `pacman` / `pacman-key` only; validate args |
| V2 | 🟠 **High** | Helper | AlpmInstallFiles accepts any path | Restrict to paths under `/tmp/monarch-install/` (canonicalized) |
| V3 | 🟡 **Medium** | Helper | WriteFile/RemoveFile no canonicalization | Canonicalize path and re-check prefix |
| V4 | 🟡 **Medium** | IPC | Temp command file not deleted if helper fails to start | GUI: delete file on spawn failure or after timeout |
| V5 | 🟡 **Medium** | Keyring | Key IDs and script in code; no pinning | Document key IDs; consider config file for audit |
| V6 | 🟢 **Low** | UI | raw_message may contain paths/env | Sanitize/truncate in ErrorModal and repair UIs |
| V7 | ℹ️ | Build | No explicit RELRO/PIE in Cargo | Add RUSTFLAGS for release builds |

---

## 6. Hardening Roadmap (v0.4.x)

1. **Immediate (pre-release)**  
   - [ ] **V1:** Implement RunCommand whitelist in monarch-helper (pacman, pacman-key only; validate pacman args).  
   - [ ] **V2:** Validate AlpmInstallFiles paths: require path within `/tmp/monarch-install/` after canonicalization.

2. **Short-term**  
   - [ ] **V3:** Canonicalize paths in WriteFile/RemoveFile/WriteFiles/RemoveFiles and re-check whitelist.  
   - [ ] **V4:** GUI: delete temp command file if helper spawn fails; optional TTL cleanup.  
   - [ ] **V7:** Add RUSTFLAGS for Full RELRO and noexecstack for release profile.

3. **Medium-term**  
   - [ ] **V5:** Document keyring key IDs; consider moving to a config or doc for audit.  
   - [ ] **V6:** Sanitize or truncate `raw_message` in UI (e.g. strip paths, limit to N chars).  
   - [ ] Run `cargo audit` and `npm audit` in CI; fix advisories.

4. **Optional**  
   - AppArmor profile for `monarch-store` and `monarch-helper` to restrict capabilities and file access.

---

## 7. Immediate Rust Fixes (Critical / High)

- **V1 (RunCommand):** In `monarch-helper` `execute_command`, reject `RunCommand` unless `binary` is `pacman` or `pacman-key`; for `pacman`, allow only args that match a safe set (e.g. `-S`, `-Syu`, `-R`, `-U`, `-V`, `--noconfirm`, etc.).  
- **V2 (AlpmInstallFiles):** In `execute_alpm_install_files`, canonicalize each path and require `path.starts_with("/tmp/monarch-install/")` (or a single configured prefix); otherwise return error.

Implementing V1 and V2 in the helper follows.

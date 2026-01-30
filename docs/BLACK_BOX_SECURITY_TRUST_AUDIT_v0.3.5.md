# Black Box Security & Trust Audit — MonARCH Store v0.3.5-alpha.1

**Last updated:** 2025-01-29

**Role:** Red Team Security Auditor + Senior Arch Linux Trusted User (TU)  
**Objective:** Identify deal-breaker issues that would cause loss of trust, malware flags, or AUR/official repo rejection.

---

## 1. Malware-Like Behavior Audit (Trust Gate)

### 1.1 Telemetry & Tracking

| Finding | Status | Notes |
|--------|--------|--------|
| Undocumented outgoing connections | **PASS** | All telemetry goes through `track_event_safe()` which checks `is_telemetry_enabled()` before sending. |
| Opt-in vs opt-out | **PASS** | Default is **opt-in**: `StoredConfig::telemetry_enabled` uses `#[serde(default)]` → `false`; `initial_telemetry = false` in `RepoManager::new()`. |
| Events sent when disabled | **PASS** | Backend gatekeeper: `track_event_safe()` only calls Aptabase when `state.is_telemetry_enabled().await` is true. Frontend `invoke('track_event', …)` hits Rust `track_event` → `track_event_safe()`. |
| Visibility | **PASS** | Settings has "Anonymous Telemetry" toggle; onboarding explains "Transparent, anonymous telemetry" and "We never track personal identity." |

**Verdict:** Telemetry is opt-in, gated in backend, and disclosed. No trust violation.

### 1.2 Keyring Poisoning (Smart Repair GPG)

| Finding | Status | Notes |
|--------|--------|--------|
| Import without provenance | **MEDIUM** | `repair.rs` and `repo_setup.rs` import third-party keys (Chaotic, CachyOS, Garuda) from `keyserver.ubuntu.com` with **hardcoded key IDs**. No additional verification. |
| User consent | **PASS** | Keyring repair and bootstrap are explicit user actions (e.g. "Initialize Keyring", "Keyring Repair"); no silent background import. |
| Recommendation | — | Document key IDs in `docs/` or `SECURITY.md` and state that users should verify them; consider a config file for auditability. |

**Verdict:** Key import is user-triggered and for known third-party repos; document key IDs for transparency.

### 1.3 Silent Updates

| Finding | Status | Notes |
|--------|--------|--------|
| Background sync without indicator | **PASS** | Repo sync is triggered by user ("Sync Now"); Settings listens to `sync-progress` and shows step-wise progress. No silent background ALPM write. |
| System update | **PASS** | "Perform System Update" is explicit; Helper runs single `pacman -Syu` (or equivalent ALPM upgrade) with progress. |

**Verdict:** No silent updates; state changes are user-initiated and visible.

---

## 2. System-Breaker Audit (Stability Gate)

### 2.1 Partial Upgrade Risks (`pacman -Sy`)

| Location | Usage | Risk |
|----------|--------|------|
| **repair.rs** | `pacman -Sy --noconfirm manjaro-keyring archlinux-keyring` / `archlinux-keyring` in keyring fix script | **CONDITIONAL** — Keyring-only sync; documented exception. Safer: `pacman -Syu … keyring` in one transaction. |
| **repo_setup.rs** | `reset_pacman_conf`: final step `pacman -Sy --noconfirm` | **MEDIUM** — Full sync without upgrade can leave system in partial upgrade. Prefer `pacman -Syu --noconfirm` or remove and document. |
| **repo_setup.rs** | `bootstrap_system`: `pacman -Sy --noconfirm archlinux-keyring` and final `pacman -Sy --noconfirm` | **CONDITIONAL** — Same as repair: keyring + final sync. Prefer `-Syu` for final sync. |
| **utils.rs** | `run_pacman_command_transparent`: rejects args containing `-Sy` or `-Syy` | **PASS** — GUI blocks standalone `-Sy` in transparent mode. |
| **Install/update path** | Helper `AlpmInstall` / `AlpmUpgrade` / `Sysupgrade` | **PASS** — Single transaction; no standalone `-Sy`. |

**Verdict:** Install/update path is safe. Repair and bootstrap use `-Sy` in keyring/init flows; recommend replacing with `-Syu` where safe to avoid partial upgrade state.

### 2.2 Polkit Over-Privilege

| Finding | Status | Notes |
|--------|--------|--------|
| Policy actions | **PASS** | `com.monarch.store.policy`: two actions — `com.monarch.store.script` (wrapper) and `com.monarch.store.package-manage` (helper). |
| Exec path binding | **PASS** | `org.freedesktop.policykit.exec.path` set to `/usr/lib/monarch-store/monarch-wrapper` and `/usr/lib/monarch-store/monarch-helper` respectively. Polkit restricts to these binaries. |
| Rules scope | **PASS** | `10-monarch-store.rules`: grants `YES` only for `com.monarch.store.package-manage` for wheel; script gets `AUTH_ADMIN_KEEP`. |
| Helper command set | **PASS** | Helper accepts only fixed commands (AlpmInstall, RemoveLock, RunCommand with **pacman/pacman-key only**, etc.). RunCommand whitelist: basename must be `pacman` or `pacman-key`; path forced to `/usr/bin/pacman` or `/usr/bin/pacman-key`. |

**Verdict:** Polkit is scoped to the helper/wrapper binaries and package-manage action; no arbitrary command execution.

### 2.3 Database Locking (`db.lck`)

| Location | Behavior | Risk |
|----------|----------|------|
| **repair.rs `repair_unlock_pacman`** | Checks `pgrep -x pacman`; if not running, runs `rm -f /var/lib/pacman/db.lck` via run_privileged. | **PASS** — Safe: no removal while pacman is running. |
| **repair_unlock_pacman with one-click** | Uses `run_privileged("rm", ["-f", "…"])` → HelperCommand::RunCommand(rm). Helper **rejects** RunCommand for anything other than pacman/pacman-key. | **HIGH** — One-click "Database Unlock" **fails** when using Polkit (no password). User must use sudo path. **Fix:** Use `invoke_helper(HelperCommand::RemoveLock, None)` when password is None. |
| **Helper `remove_lock()`** | Checks `pgrep -x pacman`; removes db.lck only if not running. | **PASS** — Correct. |
| **Helper `ensure_db_ready()`** | Before each modifying transaction: if db.lck exists, checks `is_db_lock_stale()` (PID in file dead); only then removes. | **PASS** — Safe. |
| **repo_setup bootstrap script** | `rm -f /var/lib/pacman/db.lck 2>/dev/null || true` with **no check** for running pacman. | **CRITICAL** — Can remove lock while pacman is running → DB corruption. **Fix:** Add `pgrep -x pacman` check before rm, or remove this line and rely on explicit Unlock. |

**Verdict:** Normal unlock and Helper lock handling are safe. Two issues: (1) **CRITICAL**: Bootstrap removes db.lck without checking pacman; (2) **HIGH**: One-click unlock fails because GUI uses RunCommand(rm) instead of HelperCommand::RemoveLock.

---

## 3. Repository Compliance Audit (Maintainer Gate)

### 3.1 Filesystem Hierarchy (FHS)

| Finding | Status | Notes |
|--------|--------|--------|
| PKGBUILD install paths | **PASS** | `/usr/bin`, `/usr/share/metainfo`, `/usr/share/applications`, `/usr/share/icons`, `/usr/share/polkit-1`, `/usr/lib/monarch-store`, `/usr/share/licenses`. No `/usr/local/` or `/opt/` without justification. |

**Verdict:** FHS compliant.

### 3.2 Bundled Dependencies

| Finding | Status | Notes |
|--------|--------|--------|
| System libs | **PASS** | PKGBUILD `depends` includes `webkit2gtk-4.1`, `gtk3`, `openssl`, `polkit`, `pacman-contrib`, `git`. Rust build links against system; no vendored glibc/openssl in package. |

**Verdict:** No inappropriate bundling.

### 3.3 Input Injection (package name → privileged helper)

| Finding | Status | Notes |
|--------|--------|--------|
| `validate_package_name` | **PASS** | Regex: `^[a-zA-Z0-9@._+\-]+$`. Rejects spaces, `$`, `` ` ``, `;`, `|`, `\n`, etc. Used before install, repo order, batch repo enable. |
| Helper package args | **PASS** | GUI validates names before sending AlpmInstall/AlpmUninstall; Helper does not re-validate but ALPM rejects invalid names. |
| Edge-case names | **PASS** | Names like `pkg$(id)`, `foo; rm -rf /`, `$(…)` fail the regex. |

**Verdict:** Package name validation is strict; no shell injection via package names.

---

## 4. Visibility & Transparency (Error Disclosure)

| Finding | Status | Notes |
|--------|--------|--------|
| error_classifier.rs | **PASS** | ClassifiedError includes `raw_message` (full pacman output). |
| UI exposure | **PASS** | ErrorModal and SystemHealthSection show `raw_message` in expandable "Error Log" &lt;details&gt;. Users can see root-level output for manual fixing. |

**Verdict:** Errors are not masked; raw output is available.

---

## 5. Blacklist Risk Assessment Summary

| Category | Level | Notes |
|----------|--------|------|
| Telemetry / tracking | **Low** | Opt-in, backend-gated, disclosed. |
| Keyring provenance | **Medium** | Hardcoded key IDs; document and optionally make auditable. |
| Silent updates | **Low** | None. |
| Partial upgrades | **Medium** | Repair/bootstrap use `-Sy` in keyring/init; recommend `-Syu` where safe. |
| Polkit scope | **Low** | Scoped to helper/wrapper and package-manage. |
| **db.lck (bootstrap)** | **Critical** | Bootstrap script removes db.lck without checking if pacman is running → fix required. |
| **db.lck (one-click unlock)** | **High** | One-click Unlock fails (RunCommand(rm) rejected); use HelperCommand::RemoveLock. |
| FHS / bundling / input | **Low** | Compliant; no injection. |
| Error disclosure | **Low** | Raw messages shown. |

---

## 6. Immediate Corrections (Critical / High)

### 6.1 CRITICAL: Bootstrap — Do not remove db.lck without checking pacman

**File:** `src-tauri/monarch-gui/src/repo_setup.rs` (bootstrap script)

**Problem:** Line ~185: `rm -f /var/lib/pacman/db.lck 2>/dev/null || true` runs with no check for a running pacman process. If pacman is running, removing the lock can cause database corruption.

**Fix:** Either (a) remove this line and rely on the user using "Database Unlock" (which checks), or (b) add a guard: only remove db.lck if `! pgrep -x pacman` (e.g. `pgrep -x pacman || rm -f /var/lib/pacman/db.lck` with clear logging). Prefer (a) or a safe (b) with explicit check and abort if pacman is running.

### 6.2 HIGH: One-click Database Unlock — Use Helper RemoveLock

**File:** `src-tauri/monarch-gui/src/repair.rs` — `repair_unlock_pacman`

**Problem:** When `password` is `None`, the code uses `run_privileged("rm", ["-f", "/var/lib/pacman/db.lck"], …)`, which sends `HelperCommand::RunCommand { binary: "rm", … }`. The helper only allows `pacman` and `pacman-key`, so it rejects `rm` and one-click Unlock fails.

**Fix:** When `password.is_none()`, call `invoke_helper(app, HelperCommand::RemoveLock, None)` and stream progress; the helper already implements safe lock removal (pgrep check + remove). When `password.is_some()`, keep existing sudo path (run_privileged with rm) or also use RemoveLock via helper with sudo for consistency.

---

## 7. Recommendations (Non–Deal-Breaker) — Implemented

1. **Keyring:** ✅ Documented third-party key IDs (Chaotic, CachyOS, Garuda, Manjaro) in `SECURITY.md` with a table and note that they are used for Smart Repair; users may verify independently.
2. **Partial upgrade:** ✅ In repair and bootstrap, all `pacman -Sy` usages replaced with `pacman -Syu` (repair keyring script, bootstrap keyring + final sync, reset_pacman_conf, manjaro repo script).
3. **reset_pacman_conf:** ✅ Trailing `pacman -Sy --noconfirm` replaced with `pacman -Syu --noconfirm`.

The two corrections in §6 (bootstrap db.lck check, one-click Unlock via RemoveLock) plus the above recommendations are applied. **Critical** and **High** risks are addressed and partial-upgrade risk is reduced.

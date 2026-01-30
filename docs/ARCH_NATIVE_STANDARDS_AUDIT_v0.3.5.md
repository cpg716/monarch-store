# Arch Linux Native Standards & KISS Audit — MonARCH Store v0.3.5-alpha.1

**Last updated:** 2025-01-29

**Role:** Senior Arch Linux Developer and Repository Maintainer  
**Objective:** Ensure native integration, simplicity, and strict adherence to upstream/Arch standards.

---

## 1. PKGBUILD & Build Ethics

### 1.1 Dependency Minimization

| Item | Status | Notes |
|------|--------|--------|
| **depends** | ✅ | `webkit2gtk-4.1` `gtk3` `openssl` `polkit` `pacman-contrib` `git` — all runtime; no redundant entries. |
| **makedepends** | ✅ | `cargo` `nodejs` `npm` — correct for Tauri + Vite. |
| **Hidden npm deps** | ✅ | `package.json` has only JS/TS deps; no native node-gyp modules requiring extra system libs. No `python` or `patch` needed unless a transitive dep adds one; none found. |
| **Rust deps** | — | Cargo fetches crates at build; not listed in PKGBUILD (standard for Rust). |

**Verdict:** depends/makedepends are minimal and appropriate. No "dependency hell" from unlisted system packages.

### 1.2 Build Transparency

| Item | Status | Notes |
|------|--------|--------|
| **source=()** | ⚠️ | Only `git+https://...` — no checksum (sha256sums=('SKIP')). Acceptable for VCS packages; consider tagged release tarball for reproducible builds. |
| **Network during build** | ⚠️ | `npm install` and `npm run tauri build` (Cargo) fetch from registry/crates.io. These are **not** in source=(); integrity is by npm/cargo lockfiles. Arch allows this for npm/cargo; document that build is non-offline. |
| **Binary blobs** | ✅ | No unverified binary downloads; Tauri builds from source. |

**Verdict:** Build uses network for npm/cargo; no bypass of integrity for shipped code. Consider documenting "Build requires network (npm, cargo)."

### 1.3 Cleaning the Environment

| Item | Status | Notes |
|------|--------|--------|
| **prepare()** | ⚠️ | `npm install` uses default npm cache; can write to **$HOME/.npm** unless constrained. |
| **build()** | ⚠️ | `npm run tauri build` runs Cargo; **CARGO_HOME** defaults to **$HOME/.cargo** if unset. So build can leave artifacts in **$HOME**. |
| **Containment** | ❌ | No `npm_config_cache`, `CARGO_HOME`, or `XDG_CACHE_HOME` set to **$srcdir**; build is **not** fully contained in **$srcdir** / **$pkgdir**. |

**Non-Arch behavior:** Build may pollute user's home. For clean packaging, contain cache/home:

- In **prepare()**: `export npm_config_cache="$srcdir/.npm"` (and run `npm install`).
- In **build()**: `export CARGO_HOME="$srcdir/.cargo"` (and run `npm run tauri build`).

**Verdict:** Build is not fully contained; recommend cache dirs in **$srcdir** before v0.4.0.

---

## 2. File Hierarchy & System Integration

### 2.1 Path Sanctity

| Path | Expected | Actual | Status |
|------|----------|--------|--------|
| **Config** | ~/.config/monarch-store or $XDG_CONFIG_HOME | `dirs::config_dir().join("monarch-store")` → ~/.config/monarch-store | ✅ |
| **Cache** | ~/.cache/monarch-store or $XDG_CACHE_HOME | `dirs::cache_dir().join("monarch-store")` → ~/.cache/monarch-store | ✅ |
| **Helper binary** | /usr/lib/monarch-store/ | `MONARCH_PK_HELPER` = `/usr/lib/monarch-store/monarch-helper`; PKGBUILD installs there | ✅ |
| **No /opt** | — | No writes to /opt | ✅ |
| **No /usr/local** | — | No writes; only **read** of `/usr/local/share/applications` for gtk-launch (correct) | ✅ |
| **App data** | $XDG_DATA_HOME or ~/.local/share | reviews use `app.path().app_data_dir()` (Tauri); metadata cache in cache_dir | ✅ |

**Verdict:** All writes follow XDG and FHS; helper in /usr/lib/monarch-store; no /opt or /usr/local writes.

### 2.2 Systemd & Hooks

| Item | Status | Notes |
|------|--------|--------|
| **Background worker** | N/A | No daemon; GUI only. No .service file required. |
| **Pacman hook** | ❌ | **None provided.** When user runs `pacman -Syu` in terminal, MonARCH's local repo cache (~/.cache/monarch-store/dbs) is **not** refreshed automatically; user must open app (which triggers sync) or click "Sync Now." |
| **Recommendation** | — | Provide an optional **pacman hook** (e.g. `monarch-store-refresh.hook`) that runs a small script or command to refresh MonARCH's index after transaction. See delivered hook below. |

**Verdict:** No systemd needed. Pacman hook is missing; add optional hook for "Arch-native" behavior (refresh on terminal pacman update).

### 2.3 AppStream & Desktop Standards

| Item | Status | Notes |
|------|--------|--------|
| **.desktop** | ✅ | PKGBUILD writes to **$pkgdir/usr/share/applications/monarch-store.desktop**; Type=Application, Exec=monarch-store, Icon=monarch-store, Categories=System;PackageManager, Terminal=false. |
| **metainfo.xml** | ✅ | **$pkgdir/usr/share/metainfo/monarch-store.metainfo.xml**; component type=desktop-application, id, name, summary, description, categories, project_license, releases. |
| **Icons** | ✅ | Installed under **/usr/share/icons/hicolor/** (128, 32, 512, 64); XDG icon theme. |
| **StartupNotify** | ⚠️ | .desktop does not set `StartupNotify=true`; optional for better launcher feedback. |

**Verdict:** AppStream and desktop layout are correct; StartupNotify is an optional improvement.

---

## 3. "The Arch Way" Logic & Safety

### 3.1 No Partial Upgrades (Enforcement)

| Code path | Behavior | Status |
|-----------|----------|--------|
| **Sysupgrade** | Helper `execute_alpm_upgrade(None, enabled_repos, alpm)`: first **syncs** (`alpm.syncdbs_mut().update(false)`), then collects upgrade targets from syncdbs, then download+install in one flow. | ✅ Sync and upgrade in same logical transaction. |
| **AlpmInstall (repo install)** | GUI sends `sync_first: true`; helper `execute_alpm_install` runs `alpm.syncdbs_mut().update(false)` before resolving/installing. | ✅ Sync + install in one transaction (-Syu style). |
| **AlpmUpgrade (single pkg)** | Same as Sysupgrade; sync first, then upgrade. | ✅ |
| **Standalone -Sy** | Already audited (Black Box); all repair/bootstrap use -Syu. | ✅ |

**Verdict:** No partial-upgrade code paths; sync and install/upgrade are coupled. Failsafe is structural (sync always before install/upgrade in helper).

### 3.2 User Agency

| Item | Status | Notes |
|------|--------|--------|
| **System updates** | ✅ | "Perform System Update" is explicit user action; no automatic `pacman -Syu`. |
| **Repo metadata sync** | ⚠️ | **On app startup**, frontend calls `invoke('trigger_repo_sync', { syncIntervalHours: 3 })` (App.tsx). So **repo sync runs automatically when the user opens the app** (no "Sync Now" click). It's metadata-only (~/.cache/monarch-store/dbs), not package install. |
| **Recommendation** | — | Consider making "sync on startup" optional (Settings: "Sync repositories on startup" off by default), or keep but ensure "Syncing repositories..." is clearly visible so the user sees the action. |

**Verdict:** No automatic **package** updates. Auto **metadata** sync on startup is acceptable if visible; optional setting would align with maximum user agency.

### 3.3 Error Transparency

| Item | Status | Notes |
|------|--------|--------|
| **Backend** | ✅ | `error_classifier.rs` attaches **raw_message** (full pacman output) to ClassifiedError. |
| **UI** | ✅ | ErrorModal and SystemHealthSection show **raw_message** in expandable "Error Log" &lt;details&gt;; power users get full output. |
| **GPG / 404** | ✅ | Classified errors (KeyringError, MirrorFailure) still include raw_message; UI does not hide it. |

**Verdict:** Raw pacman output is available; no masking behind a generic "Error."

---

## 4. Hardware & Distro-Aware Intelligence

### 4.1 CachyOS/AVX Detection (Identity Matrix)

| Item | Status | Notes |
|------|--------|--------|
| **Method** | ✅ | Uses **raw_cpuid** (Rust `raw-cpuid` crate): **CPUID** instruction directly, not /proc/cpuinfo or lscpu. |
| **Kernel independence** | ✅ | CPUID is kernel-agnostic; works on standard and non-standard kernels. No parsing of /proc or lscpu. |
| **v3/v4/znver4** | ✅ | `is_cpu_v3_compatible()` (AVX2, FMA, BMI2, LZCNT, etc.), `is_cpu_v4_compatible()` (AVX-512F/BW/CD/DQ/VL), `is_cpu_znver4_compatible()` (v4 + AuthenticAMD + AVX512-VNNI/BITALG). Matches Arch/CachyOS x86-64-v3/v4 and znver4. |
| **Fallback** | — | If raw_cpuid fails (e.g. very old CPU), functions return false; app still works with non-optimized repos. |

**Verdict:** CPU detection is standard (CPUID), kernel-independent, and correct for v3/v4/znver4. No change required.

---

## 5. Native Compliance Scorecard

| Category | Score | Notes |
|----------|-------|--------|
| **PKGBUILD / Build** | 7/10 | Deps good; build uses network (acceptable); **build not contained** in $srcdir ($HOME cache). |
| **File Hierarchy** | 10/10 | XDG config/cache; helper in /usr/lib; no /opt, /usr/local writes. |
| **Systemd / Hooks** | 6/10 | No daemon (OK); **no pacman hook** for post-transaction refresh. |
| **AppStream / Desktop** | 9/10 | Correct paths and content; optional StartupNotify. |
| **No Partial Upgrades** | 10/10 | Sync and install/upgrade always in same flow. |
| **User Agency** | 8/10 | No auto package updates; **auto metadata sync on startup** (visible; optional setting recommended). |
| **Error Transparency** | 10/10 | Raw pacman output in UI. |
| **CPU Detection** | 10/10 | CPUID-based; kernel-independent; v3/v4/znver4 correct. |

**Overall:** **8.5/10** — Strong Arch alignment; main gaps: build containment, optional pacman hook, optional "sync on startup" setting.

---

## 6. Non-Arch Behaviors to Address Before v0.4.0

1. **Build leaves artifacts in $HOME (prepare/build)**  
   - **Fix:** Set `npm_config_cache="$srcdir/.npm"` in prepare() and `CARGO_HOME="$srcdir/.cargo"` (and optionally `npm_config_cache` in build) so all cache lives under **$srcdir**.

2. **No pacman hook for index refresh**  
   - **Fix:** Ship an optional hook (e.g. `usr/share/libalpm/hooks/monarch-store-refresh.hook`) and a script that triggers MonARCH cache refresh (or document that users can run a command after pacman). Delivered below.

3. **Automatic repo metadata sync on app startup**  
   - **Fix (optional):** Add Settings option "Sync repositories when app starts" (default true); when false, do not call `trigger_repo_sync` on startup; user uses "Sync Now" manually.

4. **.desktop missing StartupNotify**  
   - **Fix (optional):** Add `StartupNotify=true` to the .desktop entry for better launcher integration.

---

## 7. Optional Deliverable: Pacman Hook for MonARCH

A hook that runs after a pacman transaction and refreshes MonARCH's local index (so that after `pacman -Syu` in terminal, the app’s search/cache is up to date when the user opens MonARCH).

**Option A — Hook + script (recommended):**  
- **Hook:** `usr/share/libalpm/hooks/monarch-store-refresh.hook`  
- **Script:** `usr/bin/monarch-store-refresh-cache` (or similar) that touches a flag file or runs a minimal refresh that the GUI can react to on next start (or via a small daemon/socket; for KISS, "on next start" is enough).

**Option B — Hook only, no script:**  
- Hook runs a command that the package provides. MonARCH GUI has no daemon, so the only option is to write a flag file under ~/.cache/monarch-store that the app reads on startup and then refreshes if the flag is set. That requires a script that runs as the **user**, which pacman hooks do **not** (they run as root). So a hook can only run as root and could at best update a root-owned trigger file that the app checks — overly complex.

**Simpler approach:**  
- **Hook:** After `TransactionCommit`, run a **user** command via `sudo -u $SUDO_USER monarch-store --refresh-cache` **only if** the app supports a `--refresh-cache` CLI that refreshes and exits. If the app has no such mode, the hook can be documented as "optional future addition" and for now just document: "After updating via terminal, open MonARCH and use Sync Now, or restart the app to refresh."

**Delivered:**  
- **src-tauri/pacman-hooks/monarch-store-refresh.hook** — runs after any package upgrade (PostTransaction); Exec = `/usr/bin/monarch-store-refresh-cache`.  
- **src-tauri/scripts/monarch-store-refresh-cache** — creates `/var/lib/monarch-store/refresh-requested`. The app can check this file on startup and, if present, delete it and run a repo sync so the index is fresh after the user ran `pacman -Syu` in the terminal.  
- **PKGBUILD** installs the hook under **/usr/share/libalpm/hooks/** and the script under **/usr/bin/**; creates **/var/lib/monarch-store**.  

To complete the flow, add startup logic in the app: if `/var/lib/monarch-store/refresh-requested` exists (and is readable), remove it and call `trigger_repo_sync` so the next open reflects terminal updates.

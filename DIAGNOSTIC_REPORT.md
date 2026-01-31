# MonARCH Store: Installation & Repair Failure Diagnostic Report

**Date:** 2025-01-31 · **Release:** v0.3.5-alpha  
**Issue:** Installs/updates fail, onboarding doesn't fix issues, Settings repairs don't work

---

## Executive Summary

The root cause is a **cascading failure chain** involving:
1. **Helper version mismatch** causing fallback to legacy install path
2. **Legacy path lacks proper corruption handling** 
3. **Bootstrap script timing issues** with sync DB refresh
4. **Force refresh operations don't work** when sync DBs are completely missing/corrupt
5. **Password not passed** to critical repair operations

---

## 1. Installation Flow Failure Chain

### 1.1 The Install Attempt

**Flow:**
```
User clicks Install → install_package() → install_package_core()
  → Try AlpmInstall command → Helper rejects (unknown variant)
  → Fallback to legacy: Refresh + InstallTargets
  → Refresh fails (corrupt DBs)
  → InstallTargets fails (corrupt DBs)
  → Error: "Package not found"
```

**Code Location:** `src-tauri/monarch-gui/src/commands/package.rs:206-288`

**Problem:** The GUI tries the new `AlpmInstall` command, but the installed helper at `/usr/lib/monarch-store/monarch-helper` doesn't recognize it:

```rust
// Line 227-232: Detects helper version mismatch
if (msg.message.contains("unknown variant") && msg.message.contains("AlpmInstall"))
    || (msg.message.contains("expected one of") && msg.message.contains("InstallTargets"))
    || msg.message.contains("outdated and does not support ALPM")
{
    saw_unknown_variant = true;
}
```

**Why Helper is Outdated:**
- In dev mode (`npm run tauri dev`), the helper is built to `src-tauri/target/debug/monarch-helper`
- Bootstrap script tries to copy it to `/usr/lib/monarch-store/monarch-helper` (line 203-207 in `repo_setup.rs`)
- But the helper path detection in bootstrap might fail, or the copy might not happen if bootstrap itself fails
- The installed system helper remains old

### 1.2 Legacy Path Corruption Handling Gap

**When fallback occurs, the legacy path does:**

1. **Refresh command** (`HelperCommand::Refresh`):
   - Uses old `Refresh` command which calls `pacman -Sy` 
   - **Does NOT have self-healing logic** like the new `AlpmInstall` path
   - If DBs are corrupt, it just fails

2. **Force Refresh attempt** (lines 258-271):
   - GUI detects corruption during Refresh
   - Tries `ForceRefreshDb` command
   - But `ForceRefreshDb` in helper has a critical flaw (see section 2.3)

3. **InstallTargets** (lines 273-288):
   - Uses old `InstallTargets` command
   - **Also lacks self-healing** - if DBs are corrupt, it fails immediately

**Code Location:** `src-tauri/monarch-helper/src/main.rs:433-437` (ForceRefreshDb handler)

---

## 2. Why Onboarding (Bootstrap) Doesn't Fix Things

### 2.1 Bootstrap Script Flow

**Location:** `src-tauri/monarch-gui/src/repo_setup.rs:180-296`

**What it does:**
1. Removes sync DBs: `rm -rf /var/lib/pacman/sync/*` (line 190)
2. Resets keyring: `pacman-key --init` (line 233)
3. **Tries to force refresh:** `pacman -Syy --noconfirm` (line 238-242) ← **NEWLY ADDED**
4. Updates keyring package: `pacman -Syu --noconfirm archlinux-keyring` (line 246)
5. Imports keys for Chaotic/CachyOS
6. Writes repo configs
7. Final sync: `pacman -Sy --noconfirm` (line 293)

### 2.2 Critical Issues with Bootstrap

**Issue A: Timing Problem**
- Line 190: Deletes ALL sync DBs
- Line 238: Tries `pacman -Syy` 
- **BUT:** At this point, `/etc/pacman.conf` might not have all repos configured yet
- The `Include = /etc/pacman.d/monarch/*.conf` line is added at line 268, AFTER the -Syy attempt
- So `pacman -Syy` might only sync core/extra/multilib, missing Chaotic/CachyOS repos

**Issue B: Helper Deployment Failure**
- Lines 201-215: Tries to copy helper from source to `/usr/lib/monarch-store/monarch-helper`
- Helper path detection (lines 166-178) looks for helper next to executable
- In dev mode, this might not find the helper correctly
- If helper copy fails, the old helper remains, causing version mismatch

**Issue C: No Verification**
- Bootstrap completes with "Bootstrap complete" message
- But there's no verification that:
  - Sync DBs are actually valid (not corrupt)
  - Helper was successfully deployed
  - All repos are properly configured

### 2.3 Why Force Refresh Doesn't Work

**Location:** `src-tauri/monarch-helper/src/transactions.rs:40-59`

**The `force_refresh_sync_dbs()` function:**

```rust
pub fn force_refresh_sync_dbs(alpm: &mut Alpm) -> Result<(), String> {
    // 1. Delete all sync DB files
    // 2. Get enabled repos from ALPM
    let enabled_repos: Vec<String> = alpm
        .syncdbs()
        .iter()
        .map(|db| db.name().to_string())
        .collect();
    // 3. Call execute_alpm_sync()
    execute_alpm_sync(enabled_repos, alpm)?;
}
```

**Critical Flaw:**
- It gets `enabled_repos` from `alpm.syncdbs()` - the repos that are **already registered** in ALPM
- But if sync DBs are corrupt/missing, ALPM might not have registered them yet
- Or if repos were just added to pacman.conf, they might not be registered
- So `force_refresh_sync_dbs()` only refreshes repos that ALPM already knows about
- Missing repos (like Chaotic-AUR) won't be refreshed

**Why This Fails:**
1. User has corrupt sync DBs
2. Calls `force_refresh_databases` from Settings
3. Helper's `ForceRefreshDb` deletes sync DBs
4. Tries to get enabled repos from ALPM - but ALPM can't read corrupt DBs, so it has no repos registered
5. `enabled_repos` is empty or incomplete
6. `execute_alpm_sync()` only syncs what's in the list (maybe just core/extra)
7. Chaotic-AUR and other repos remain missing/corrupt

---

## 3. Why Settings Repairs Don't Work

### 3.1 Force Refresh Databases

**Location:** `src-tauri/monarch-gui/src/commands/system.rs:631-643`

**What it does:**
```rust
pub async fn force_refresh_databases(app: AppHandle) -> Result<(), String> {
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ForceRefreshDb,
        None,  // ← NO PASSWORD!
    )
    .await?;
}
```

**Problems:**
1. **No password parameter** - always uses Polkit (prompts user)
2. Uses helper's `ForceRefreshDb` which has the flaw described in 2.3
3. If helper is outdated, `ForceRefreshDb` might not exist (falls back to error)

### 3.2 Keyring Repair

**Location:** `src-tauri/monarch-gui/src/repair.rs:241-362`

**What it does:**
- Resets GPG keyring
- Re-populates keys
- Updates keyring packages
- Imports third-party keys

**Why it doesn't fix installs:**
- **Does NOT refresh sync databases**
- Only fixes keyring, not the corrupt sync DBs
- After keyring repair, sync DBs are still corrupt
- Installs still fail with "Unrecognized archive format"

### 3.3 Database Unlock

**Location:** `src-tauri/monarch-gui/src/repair.rs:184-237`

**What it does:**
- Removes `/var/lib/pacman/db.lck` if stale

**Why it doesn't fix installs:**
- Only removes lock file
- **Does NOT fix corrupt sync DBs**
- After unlock, DBs are still corrupt
- Installs still fail

---

## 4. Root Cause Analysis

### 4.1 The Core Problem

**Sync databases are corrupt, and nothing properly fixes them because:**

1. **Bootstrap script** tries `pacman -Syy` but repos might not be configured yet
2. **Helper's `force_refresh_sync_dbs()`** only refreshes repos already registered in ALPM
3. **If ALPM can't read corrupt DBs, it has no repos registered**
4. **Circular dependency:** Need valid DBs to register repos, but need repos registered to refresh DBs

### 4.2 The Helper Version Mismatch Amplifies Everything

- New code path (`AlpmInstall`) has better corruption handling
- But helper doesn't support it → falls back to legacy
- Legacy path has minimal corruption handling
- Legacy path fails → user stuck

### 4.3 Why Self-Healing Doesn't Work

**New `AlpmInstall` path has self-healing** (lines 94-162 in `transactions.rs`):
- Detects corruption during package resolution
- Calls `force_refresh_sync_dbs()`
- Retries

**But:**
- Only works if helper supports `AlpmInstall`
- If helper is old, we never use this path
- Legacy path doesn't have this logic

---

## 5. Specific Code Issues

### 5.1 Bootstrap Script: `pacman -Syy` Too Early

**File:** `src-tauri/monarch-gui/src/repo_setup.rs:236-242`

```bash
# 2. Force refresh sync databases (CRITICAL: must use -Syy to rebuild after deletion)
echo "Force refreshing package databases..."
pacman -Syy --noconfirm || {
    echo "WARNING: pacman -Syy failed, retrying..."
    sleep 2
    pacman -Syy --noconfirm || echo "ERROR: Could not refresh databases"
}
```

**Problem:** This runs BEFORE:
- Repo configs are written (line 501-520 in `apply_os_config`)
- `Include = /etc/pacman.d/monarch/*.conf` is added (line 268)
- So `-Syy` only syncs core/extra/multilib, missing custom repos

**Fix needed:** Move `-Syy` to AFTER repo configs are written, or do it twice (once for base, once after configs)

### 5.2 Helper's Force Refresh: Wrong Repo List

**File:** `src-tauri/monarch-helper/src/transactions.rs:40-59`

```rust
pub fn force_refresh_sync_dbs(alpm: &mut Alpm) -> Result<(), String> {
    // Delete sync DBs...
    let enabled_repos: Vec<String> = alpm
        .syncdbs()  // ← Gets repos from ALPM's current state
        .iter()
        .map(|db| db.name().to_string())
        .collect();
    execute_alpm_sync(enabled_repos, alpm)?;
}
```

**Problem:** If DBs are corrupt, `alpm.syncdbs()` might be empty or incomplete

**Fix needed:** Read repos from `/etc/pacman.conf` directly, or pass repos as parameter

### 5.3 Force Refresh Databases: No Password

**File:** `src-tauri/monarch-gui/src/commands/system.rs:631-643`

```rust
pub async fn force_refresh_databases(app: AppHandle) -> Result<(), String> {
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ForceRefreshDb,
        None,  // ← Should accept password parameter
    )
    .await?;
}
```

**Problem:** Always prompts for password, even if user provided one in onboarding

**Fix needed:** Add `password: Option<String>` parameter

### 5.4 Legacy Path: No Corruption Detection

**File:** `src-tauri/monarch-gui/src/commands/package.rs:243-288`

The legacy `Refresh` + `InstallTargets` path:
- Detects corruption in Refresh output (line 253)
- Tries ForceRefreshDb (line 261)
- But then proceeds with InstallTargets even if ForceRefreshDb might have failed
- InstallTargets has no corruption detection/retry logic

**Fix needed:** Add corruption detection to legacy InstallTargets, or ensure ForceRefreshDb actually works

---

## 6. Why Nothing Works: The Perfect Storm

1. **User has corrupt sync DBs**
2. **Helper is outdated** (doesn't support AlpmInstall)
3. **Install tries new path** → helper rejects → falls back to legacy
4. **Legacy Refresh** → detects corruption → tries ForceRefreshDb
5. **ForceRefreshDb** → deletes DBs → tries to get repos from ALPM → ALPM has no repos (corrupt DBs) → only syncs core/extra → Chaotic still missing
6. **InstallTargets** → tries to find package → DBs still corrupt/missing → fails
7. **User tries onboarding** → bootstrap runs → `-Syy` too early (repos not configured) → only syncs base repos → Chaotic still missing
8. **User tries Settings → Refresh Databases** → same ForceRefreshDb flaw → doesn't work
9. **User tries Settings → Fix Keys** → fixes keyring but not DBs → still fails
10. **User tries Settings → Unlock** → removes lock but not DBs → still fails

**Result:** Nothing works because the fundamental issue (corrupt/missing sync DBs for custom repos) is never properly addressed.

---

## 7. Recommended Fixes (Priority Order)

### 7.1 CRITICAL: Fix Helper's Force Refresh

**File:** `src-tauri/monarch-helper/src/transactions.rs:40-59`

**Change:** Read repos from pacman.conf instead of ALPM state:

```rust
pub fn force_refresh_sync_dbs(alpm: &mut Alpm) -> Result<(), String> {
    // Delete sync DBs...
    
    // Read repos from pacman.conf instead of ALPM
    let pacman_conf = std::fs::read_to_string("/etc/pacman.conf")
        .map_err(|e| format!("Failed to read pacman.conf: {}", e))?;
    
    let mut enabled_repos = Vec::new();
    // Parse pacman.conf to find [repo] sections
    // ... parsing logic ...
    
    execute_alpm_sync(enabled_repos, alpm)?;
}
```

### 7.2 CRITICAL: Fix Bootstrap Script Timing

**File:** `src-tauri/monarch-gui/src/repo_setup.rs`

**Change:** Do `pacman -Syy` AFTER repo configs are written, or do it in two phases:
1. Early: `pacman -Syy` for core/extra/multilib (base repos)
2. Late: After repo configs written, sync again for custom repos

### 7.3 HIGH: Add Password to Force Refresh

**File:** `src-tauri/monarch-gui/src/commands/system.rs:631`

**Change:**
```rust
pub async fn force_refresh_databases(
    app: AppHandle,
    password: Option<String>,  // ← Add this
) -> Result<(), String> {
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ForceRefreshDb,
        password,  // ← Use it
    )
    .await?;
}
```

### 7.4 HIGH: Verify Helper Deployment in Bootstrap

**File:** `src-tauri/monarch-gui/src/repo_setup.rs:201-215`

**Change:** After copying helper, verify it exists and is executable:
```bash
if [ -f /usr/lib/monarch-store/monarch-helper ]; then
    /usr/lib/monarch-store/monarch-helper --version || echo "Helper verification failed"
fi
```

### 7.5 MEDIUM: Add Corruption Detection to Legacy InstallTargets

**File:** `src-tauri/monarch-helper/src/main.rs` (InstallTargets handler)

**Change:** Add same corruption detection/retry logic as AlpmInstall

### 7.6 MEDIUM: Bootstrap Verification Step

**File:** `src-tauri/monarch-gui/src/repo_setup.rs:292-293`

**Change:** After bootstrap, verify sync DBs are valid:
```bash
echo "Verifying database health..."
for db in core extra multilib chaotic-aur; do
    if [ -f /var/lib/pacman/sync/${db}.db ]; then
        pacman -Si pacman >/dev/null 2>&1 || echo "WARNING: ${db} DB may be corrupt"
    fi
done
```

---

## 8. Immediate Workaround for Users

Until fixes are implemented, users should:

1. **Manually fix sync DBs:**
   ```bash
   sudo rm -rf /var/lib/pacman/sync/*
   sudo pacman -Syy
   ```

2. **Verify helper is up to date:**
   ```bash
   /usr/lib/monarch-store/monarch-helper --version
   # If missing or old, rebuild and reinstall monarch-store
   ```

3. **After manual fix, retry install in MonARCH**

---

## 9. Testing Checklist

After implementing fixes, verify:

- [ ] Bootstrap script successfully refreshes ALL repos (core, extra, multilib, chaotic-aur, cachyos)
- [ ] Helper is correctly deployed to `/usr/lib/monarch-store/monarch-helper`
- [ ] Helper version matches GUI (supports AlpmInstall)
- [ ] Force Refresh Databases in Settings works without password prompt (if password provided)
- [ ] Force Refresh Databases refreshes ALL repos, not just core/extra
- [ ] Install works after corrupt DBs are detected (self-healing)
- [ ] Legacy path (old helper) still works with corruption detection
- [ ] Onboarding completes and installs work immediately after

---

## Conclusion

The fundamental issue is that **corrupt sync databases cannot be properly refreshed** because:
1. The refresh mechanism relies on ALPM's state (which is broken when DBs are corrupt)
2. Bootstrap tries to refresh before repos are configured
3. No repair function reads repos directly from pacman.conf

The fix requires changing `force_refresh_sync_dbs()` to read repos from pacman.conf instead of ALPM state, and ensuring bootstrap does the refresh at the right time.

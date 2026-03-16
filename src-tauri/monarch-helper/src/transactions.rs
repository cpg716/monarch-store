use crate::alpm_errors::classify_alpm_error;
use crate::logger;
use crate::progress;
use alpm::{Alpm, SigLevel, TransFlag};
use serde::{Deserialize, Serialize};

/// Minimum free space (200 MB) below which we warn the user before prepare.
const LOW_DISK_SPACE_THRESHOLD_B: u64 = 200 * 1024 * 1024;

#[cfg(unix)]
fn free_space_bytes(path: &std::path::Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    Some(stat.f_bavail as u64 * stat.f_frsize as u64)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlpmProgressEvent {
    pub event_type: String,
    pub package: Option<String>,
    pub percent: Option<u8>,
    pub downloaded: Option<u64>,
    pub total: Option<u64>,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct TransactionManifest {
    pub update_system: bool,          // Should we run -Syu?
    pub refresh_db: bool,             // Should we run -Sy?
    pub clear_cache: bool,            // Should we run -Sc?
    pub remove_lock: bool,            // Should we remove pacman lock?
    pub remove_orphans: bool,         // Should we remove unused dependencies?
    pub install_targets: Vec<String>, // List of repo packages
    pub remove_targets: Vec<String>,  // List of packages to remove
    pub local_paths: Vec<String>,     // List of pre-built AUR packages (.pkg.tar.zst) to install
    pub parallel_downloads: Option<u32>,
    pub cpu_optimization: Option<String>,
    pub target_repo: Option<String>,
}

pub fn emit_progress_event(event: AlpmProgressEvent) {
    if let Ok(json) = serde_json::to_string(&event) {
        progress::send_progress_line(json);
    }
}

fn emit_simple_progress(percent: u8, message: &str) {
    emit_progress_event(AlpmProgressEvent {
        event_type: "progress".to_string(),
        package: None,
        percent: Some(percent),
        downloaded: None,
        total: None,
        message: message.to_string(),
    });
}

const CACHE_PKG_DIR: &str = "/var/cache/pacman/pkg";

fn cleanup_partial_downloads() {
    let dir = std::path::Path::new(CACHE_PKG_DIR);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "part" {
                        let _ = std::fs::remove_file(&path);
                        logger::trace(&format!("Cleaned partial download: {}", path.display()));
                    }
                }
            }
        }
    }
}

fn is_corrupt_db_error(err: &str) -> bool {
    err.contains("Unrecognized archive format") || err.contains("could not open database")
}

fn check_db_freshness(alpm: &Alpm) -> bool {
    let sync_dir = std::path::Path::new("/var/lib/pacman/sync");
    let one_hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    let mut any_db_exists = false;

    for db in alpm.syncdbs().iter() {
        let repo = db.name();
        let db_file = sync_dir.join(format!("{}.db", repo));
        let Ok(metadata) = std::fs::metadata(&db_file) else {
            logger::trace(&format!(
                "DB {} not on disk, skipping freshness check",
                repo
            ));
            continue;
        };
        any_db_exists = true;
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if modified < one_hour_ago {
            logger::trace(&format!("DB {} is stale", repo));
            return true;
        }
    }

    if !any_db_exists {
        logger::trace("No sync DBs found, need sync");
        return true;
    }
    false
}

pub fn force_refresh_sync_dbs(alpm: &mut Alpm) -> Result<(), String> {
    emit_simple_progress(5, "Refreshing sync databases...");
    match alpm.syncdbs_mut().update(true) {
        Ok(_) => {
            emit_simple_progress(100, "Sync databases refreshed");
            Ok(())
        }
        Err(e) => {
            let err_str = e.to_string();
            if is_corrupt_db_error(&err_str) {
                emit_simple_progress(
                    25,
                    "Corrupt database detected. Clearing sync cache safely...",
                );
                let sync_dir = std::path::Path::new("/var/lib/pacman/sync");
                if let Ok(entries) = std::fs::read_dir(sync_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
                match alpm.syncdbs_mut().update(true) {
                    Ok(_) => {
                        emit_simple_progress(100, "Sync databases refreshed after recovery");
                        Ok(())
                    }
                    Err(e2) => Err(e2.to_string()),
                }
            } else {
                Err(err_str)
            }
        }
    }
}

const KEYRING_CHECK_FILE: &str = "/var/tmp/monarch-keyring-check";
const KEYRING_CHECK_INTERVAL_SECS: u64 = 43200; // 12 hours

fn ensure_keyrings_updated(alpm: &Alpm) -> Result<(), String> {
    // ✅ THROTTLING: Skip if we checked recently (within 12 hours)
    if let Ok(meta) = std::fs::metadata(KEYRING_CHECK_FILE) {
        if let Ok(modified) = meta.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                if elapsed.as_secs() < KEYRING_CHECK_INTERVAL_SECS {
                    logger::trace("Keyring pre-flight: recently checked, skipping.");
                    return Ok(());
                }
            }
        }
    }

    emit_simple_progress(
        1,
        "Pre-Flight: Verifying security keys (this may take a moment on first run)...",
    );
    let mut targets = vec!["archlinux-keyring"];

    let has_chaotic = alpm
        .syncdbs()
        .iter()
        .any(|db| db.name().contains("chaotic"));
    let has_cachy = alpm
        .syncdbs()
        .iter()
        .any(|db| db.name().contains("cachyos"));

    if has_chaotic {
        targets.push("chaotic-keyring");
    }
    if has_cachy {
        targets.push("cachyos-keyring");
    }

    let output = std::process::Command::new("pacman")
        .args(["-S", "--noconfirm", "--needed"])
        .args(&targets)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| format!("Failed to launch pacman for keyring: {}", e))?;

    if output.status.success() {
        // ✅ Update timestamp on success
        let _ = std::fs::File::create(KEYRING_CHECK_FILE);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        logger::warn(&format!("Keyring update warning: {}", stderr));
        // Don't update timestamp on failure so we retry next time
        Ok(())
    }
}

pub fn execute_alpm_install(
    packages: Vec<String>,
    sync_first: bool,
    _cpu_optimization: Option<String>,
    target_repo: Option<String>,
    alpm: &mut Alpm,
) -> Result<(), String> {
    emit_simple_progress(5, "Initializing transaction...");

    if sync_first && check_db_freshness(alpm) {
        emit_simple_progress(10, "Synchronizing databases...");
        if let Err(e) = alpm.syncdbs_mut().update(false) {
            let err = e.to_string();
            if is_corrupt_db_error(&err) {
                force_refresh_sync_dbs(alpm)?;
            } else {
                return Err(format!("Database sync failed: {}", err));
            }
        }
    }

    emit_simple_progress(20, "Resolving packages...");

    // ATTEMPT 1: Lookup in current DBs (optionally restricted to target_repo)
    let mut found_packages = lookup_packages(alpm, &packages, &target_repo);

    // If failed, FORCE SYNC and RETRY
    if found_packages.len() != packages.len() {
        emit_simple_progress(15, "Package not found locally. Syncing databases...");
        if let Err(e) = alpm.syncdbs_mut().update(true) {
            logger::warn(&format!("Database sync warning: {}", e));
            // Don't error out yet, maybe the package is there but sync failed partially
        }
        // ATTEMPT 2: Retry Lookup (same target_repo)
        found_packages = lookup_packages(alpm, &packages, &target_repo);
    }

    // If still not found and we were restricting to a specific repo (e.g. "community"), the repo
    // may not exist on this distro (e.g. CachyOS uses cachyos-extra-v4). Fall back to searching
    // all syncdbs so the package can be found in whatever repo actually provides it.
    if found_packages.len() != packages.len() && target_repo.is_some() {
        found_packages = lookup_packages(alpm, &packages, &None);
    }

    // Final Check
    if found_packages.len() != packages.len() {
        return Err(format!(
            "Package(s) not found in enabled repositories even after sync: {:?}",
            packages
        ));
    }

    alpm.trans_init(TransFlag::ALL_DEPS)
        .map_err(|e| e.to_string())?;

    for pkg in &found_packages {
        alpm.trans_add_pkg(*pkg).map_err(|e| e.to_string())?;
    }

    // Safety: If we synced databases (sync_first), we MUST perform a full system upgrade
    // to avoid "partial upgrade" scenarios which break Arch systems (ABI mismatches).
    // See: https://wiki.archlinux.org/title/System_maintenance#Partial_upgrades_are_unsupported
    if sync_first {
        emit_simple_progress(25, "Ensuring system integrity (Full Upgrade)...");
        let local_pkgs = alpm.localdb().pkgs().iter().collect::<Vec<_>>();
        for local in local_pkgs {
            for db in alpm.syncdbs() {
                if let Ok(sync_pkg) = db.pkg(local.name()) {
                    if sync_pkg.version() > local.version() {
                        // Try to add update. available package is usually a reference
                        let _ = alpm.trans_add_pkg(sync_pkg);
                        break;
                    }
                }
            }
        }
    }

    setup_progress_callbacks(alpm)?;

    // Pre-flight: warn if package cache or root is low on space (premium app-store UX)
    #[cfg(unix)]
    {
        let cache_path = std::path::Path::new(CACHE_PKG_DIR);
        if let Some(free) =
            free_space_bytes(cache_path).or_else(|| free_space_bytes(std::path::Path::new("/")))
        {
            if free < LOW_DISK_SPACE_THRESHOLD_B {
                let mb = free / (1024 * 1024);
                emit_simple_progress(
                    38,
                    &format!(
                        "Low disk space (~{} MB free). Installation may fail if cache is full.",
                        mb
                    ),
                );
            }
        }
    }

    emit_simple_progress(40, "Preparing transaction...");
    alpm.trans_prepare().map_err(|e| {
        let msg = format!("Transaction preparation failed: {}", e);
        cleanup_partial_downloads();
        msg
    })?;

    emit_simple_progress(
        50,
        "Installing packages and running install scripts (large apps may take 1–2 minutes)…",
    );
    match alpm.trans_commit() {
        Ok(_) => {
            emit_simple_progress(100, "Installation complete!");
            Ok(())
        }
        Err(e) => {
            let msg = e.to_string();
            logger::warn(&format!("ALPM Transaction Commit Failed: {}", msg));
            let classified = classify_alpm_error(&msg);
            emit_progress_event(AlpmProgressEvent {
                event_type: "error".to_string(),
                package: None,
                percent: None,
                downloaded: None,
                total: None,
                message: serde_json::to_string(&classified).unwrap_or(msg.clone()),
            });
            Err(msg)
        }
    }
}

pub fn execute_alpm_check_updates_safe(_alpm: &mut Alpm) {
    emit_simple_progress(
        5,
        "Safe Update Check: Initializing temporary environment...",
    );

    let temp_dir = match tempfile::Builder::new().prefix("monarch-check").tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            emit_simple_progress(0, &format!("Error creating temp dir: {}", e));
            return;
        }
    };
    let temp_path = temp_dir.path();
    logger::info(&format!(
        "CheckUpdatesSafe: using temp dir {}",
        temp_path.display()
    ));

    let local_dest = temp_path.join("local");
    #[cfg(unix)]
    if let Err(e) = std::os::unix::fs::symlink("/var/lib/pacman/local", &local_dest) {
        emit_simple_progress(0, &format!("Error linking local db: {}", e));
        return;
    }

    emit_simple_progress(20, "Syncing Safe DBs...");

    let dbpath = temp_path.to_string_lossy();
    let sync_status = std::process::Command::new("pacman")
        .args(["-Sy", "--dbpath", dbpath.as_ref()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match sync_status {
        Ok(s) if s.success() => {
            emit_simple_progress(50, "Checking for updates...");
            let qu_out = std::process::Command::new("pacman")
                .args(["-Qu", "--dbpath", dbpath.as_ref()])
                .output();

            if let Ok(qu) = qu_out {
                let stdout = String::from_utf8_lossy(&qu.stdout);
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let name = parts[0];
                        let new_ver = parts[3];
                        emit_progress_event(AlpmProgressEvent {
                            event_type: "package_found".to_string(),
                            package: Some(name.to_string()),
                            percent: None,
                            downloaded: None,
                            total: None,
                            message: format!(
                                "Update available: {} {} -> {}",
                                name, parts[1], new_ver
                            ),
                        });
                    }
                }
                emit_simple_progress(100, "Safe check complete");
            } else {
                emit_simple_progress(0, "Error running check (pacman -Qu failed)");
            }
        }
        _ => {
            emit_simple_progress(0, "Error syncing safe environment");
        }
    }
}

/// Uninstall packages via ALPM. Package scriptlets (e.g. post_remove) run synchronously
/// during trans_commit; if a scriptlet blocks, the whole uninstall blocks until it finishes.
pub fn execute_alpm_uninstall(
    packages: Vec<String>,
    remove_deps: bool,
    alpm: &mut Alpm,
) -> Result<(), String> {
    let flags = if remove_deps {
        TransFlag::CASCADE
    } else {
        TransFlag::NONE
    };
    alpm.trans_init(flags).map_err(|e| e.to_string())?;

    for pkg_name in packages {
        if let Ok(pkg) = alpm.localdb().pkg(pkg_name.as_str()) {
            alpm.trans_remove_pkg(pkg).map_err(|e| e.to_string())?;
        } else {
            return Err(format!("Package {} not installed", pkg_name));
        }
    }

    setup_progress_callbacks(alpm)?;
    alpm.trans_prepare().map_err(|e| e.to_string())?;

    emit_simple_progress(50, "Removing packages and running uninstall scripts (large apps like OBS may take 1–2 minutes)…");
    match alpm.trans_commit() {
        Ok(_) => {
            emit_simple_progress(100, "Uninstallation complete!");
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn execute_alpm_install_files(paths: Vec<String>, alpm: &mut Alpm) -> Result<(), String> {
    ensure_keyrings_updated(alpm)?;
    emit_simple_progress(5, "Initializing local install...");

    for path in paths {
        let pkg = alpm
            .pkg_load(path.as_str(), true, SigLevel::USE_DEFAULT)
            .map_err(|e| e.to_string())?;
        alpm.trans_add_pkg(pkg).map_err(|e| e.to_string())?;
    }

    setup_progress_callbacks(alpm)?;
    alpm.trans_prepare().map_err(|e| e.to_string())?;
    emit_simple_progress(80, "Installing packages and running install scripts…");
    match alpm.trans_commit() {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn execute_alpm_sync(repos: Vec<String>, alpm: &mut Alpm) -> Result<(), String> {
    for repo_name in repos {
        if alpm.syncdbs().iter().any(|db| db.name() == repo_name) {
            continue;
        }
        // CRITICAL: Do NOT register a DB if it's not already there.
        // It was likely unregistered because it had NO SERVERS.
        // Re-registering it here without adding servers causes "no servers configured" errors.
        logger::warn(&format!(
            "Skipping sync for repo '{}' as it has no server configuration.",
            repo_name
        ));
    }

    match alpm.syncdbs_mut().update(true) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

extern "C" fn safe_progress_cb(
    _ctx: *mut std::ffi::c_void,
    _progress: alpm_sys::alpm_progress_t,
    pkgname: *const std::ffi::c_char,
    percent: std::ffi::c_int,
    _howmany: usize,
    _current: usize,
) {
    let pkg_str = if pkgname.is_null() {
        "system components"
    } else {
        unsafe { std::ffi::CStr::from_ptr(pkgname) }
            .to_str()
            .unwrap_or("unknown")
    };
    let msg = format!("Processing {}... {}%", pkg_str, percent);
    emit_simple_progress(percent as u8, &msg);
}

extern "C" fn safe_dl_cb(
    _ctx: *mut std::ffi::c_void,
    filename: *const std::ffi::c_char,
    _event: alpm_sys::alpm_download_event_type_t,
    _data: *mut std::ffi::c_void,
) {
    if filename.is_null() {
    }
}

fn setup_progress_callbacks(alpm: &mut Alpm) -> Result<(), String> {
    // Callback signatures fixed for alpm 5.x
    // BUGFIX: bypassing alpm-rs to handle NULL pkgname/filenames from libalpm directly.
    unsafe {
        alpm_sys::alpm_option_set_dlcb(
            alpm.as_alpm_handle_t(),
            Some(safe_dl_cb),
            std::ptr::null_mut(),
        );
        alpm_sys::alpm_option_set_progresscb(
            alpm.as_alpm_handle_t(),
            Some(safe_progress_cb),
            std::ptr::null_mut(),
        );
    }

    // BUGFIX: bypassing alpm-rs to handle NULL hook/event fields.
    // The alpm-rs crate panics if fields like name or desc are NULL during HookRunStart.
    unsafe {
        alpm_sys::alpm_option_set_eventcb(
            alpm.as_alpm_handle_t(),
            Some(safe_event_cb),
            std::ptr::null_mut(),
        );
    }

    Ok(())
}

extern "C" fn safe_event_cb(_ctx: *mut std::ffi::c_void, event: *mut alpm_sys::alpm_event_t) {
    if event.is_null() {
        return;
    }

    // We only care about HookRunStart right now for UI progress
    unsafe {
        if (*event).type_ == alpm_sys::_alpm_event_type_t::ALPM_EVENT_HOOK_RUN_START {
            let hook_run = (*event).hook_run;

            let name_str = if hook_run.name.is_null() {
                "Unknown Hook"
            } else {
                std::ffi::CStr::from_ptr(hook_run.name)
                    .to_str()
                    .unwrap_or("Unknown Hook")
            };

            let desc_str = if hook_run.desc.is_null() {
                "Running system maintenance hook..."
            } else {
                std::ffi::CStr::from_ptr(hook_run.desc)
                    .to_str()
                    .unwrap_or("Running system maintenance hook...")
            };

            emit_progress_event(AlpmProgressEvent {
                event_type: "hook_start".to_string(),
                package: None,
                percent: Some(95),
                downloaded: None,
                total: None,
                message: format!(
                    "Running hook: {} ({}) [{}/{}]",
                    name_str, desc_str, hook_run.position, hook_run.total
                ),
            });
        }
    }
}

fn lookup_packages<'a>(
    alpm: &'a Alpm,
    packages: &[String],
    target_repo: &Option<String>,
) -> Vec<&'a alpm::Package> {
    let mut found_packages: Vec<&'a alpm::Package> = Vec::new();
    for pkg_name in packages {
        if let Some(tr) = target_repo {
            for db in alpm.syncdbs() {
                if db.name() == tr {
                    if let Ok(pkg) = db.pkg(pkg_name.as_str()) {
                        found_packages.push(pkg);
                        break;
                    }
                }
            }
        } else {
            for db in alpm.syncdbs() {
                if let Ok(pkg) = db.pkg(pkg_name.as_str()) {
                    found_packages.push(pkg);
                    break;
                }
            }
        }
    }
    found_packages
}

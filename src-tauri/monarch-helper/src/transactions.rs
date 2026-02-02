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
    pub install_targets: Vec<String>, // List of repo packages
    pub remove_targets: Vec<String>,  // List of packages to remove
    pub local_paths: Vec<String>,     // List of pre-built AUR packages (.pkg.tar.zst) to install
}

fn emit_progress_event(event: AlpmProgressEvent) {
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

fn check_db_freshness(repos_to_check: &[String]) -> bool {
    let sync_dir = std::path::Path::new("/var/lib/pacman/sync");
    let one_hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    let mut any_db_exists = false;

    for repo in repos_to_check {
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

pub fn get_enabled_repos_from_config() -> Vec<String> {
    extract_repos_from_config()
}

fn extract_repos_from_config() -> Vec<String> {
    let mut repos = Vec::new();
    if let Ok(content) = std::fs::read_to_string("/etc/pacman.conf") {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                let section = &line[1..line.len() - 1];
                if section != "options" {
                    repos.push(section.to_string());
                }
            }
        }
    }
    repos
}

pub fn force_refresh_sync_dbs(alpm: &mut Alpm) -> Result<(), String> {
    emit_simple_progress(5, "Force refreshing sync databases...");
    let sync_dir = std::path::Path::new("/var/lib/pacman/sync");
    if let Ok(entries) = std::fs::read_dir(sync_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    emit_simple_progress(25, "Cleared local sync database cache");

    let enabled_repos = extract_repos_from_config();
    if enabled_repos.is_empty() {
        return Err("No repositories found in pacman.conf".to_string());
    }

    if let Err(e) = execute_alpm_sync(enabled_repos, alpm) {
        return Err(e);
    }
    emit_simple_progress(100, "Sync databases refreshed");
    Ok(())
}

fn ensure_keyrings_updated(enabled_repos: &[String]) -> Result<(), String> {
    emit_simple_progress(1, "Pre-Flight: Verifying security keys...");
    let mut targets = vec!["archlinux-keyring"];
    if enabled_repos.iter().any(|r| r.contains("chaotic")) {
        targets.push("chaotic-keyring");
    }
    if enabled_repos.iter().any(|r| r.contains("cachyos")) {
        targets.push("cachyos-keyring");
    }

    let output = std::process::Command::new("pacman")
        .args(&["-S", "--noconfirm", "--needed"])
        .args(&targets)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| format!("Failed to launch pacman for keyring: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        logger::warn(&format!("Keyring update warning: {}", stderr));
        Ok(())
    }
}

pub fn execute_alpm_install(
    packages: Vec<String>,
    sync_first: bool,
    enabled_repos: Vec<String>,
    cpu_optimization: Option<String>,
    target_repo: Option<String>,
    alpm: &mut Alpm,
) -> Result<(), String> {
    if let Err(e) = ensure_keyrings_updated(&enabled_repos) {
        logger::warn(&format!("Keyring pre-flight failed: {}", e));
    }

    emit_simple_progress(5, "Initializing transaction...");
    let config_repos = get_enabled_repos_from_config();

    if sync_first {
        if check_db_freshness(&config_repos) {
            emit_simple_progress(10, "Synchronizing databases...");
            if let Err(e) = alpm.syncdbs_mut().update(false) {
                let err = e.to_string();
                if is_corrupt_db_error(&err) {
                    force_refresh_sync_dbs(alpm)?;
                    alpm.syncdbs_mut().update(true).map_err(|e| e.to_string())?;
                } else {
                    return Err(format!("Database sync failed: {}", err));
                }
            }
        }
    }

    let priority_order = build_priority_order(&enabled_repos, &cpu_optimization);
    emit_simple_progress(20, "Resolving packages...");

    let mut found_packages = Vec::new();

    for pkg_name in &packages {
        let mut found = false;
        if let Some(tr) = &target_repo {
            if let Some(db) = alpm.syncdbs().iter().find(|d| d.name() == tr) {
                if let Ok(pkg) = db.pkg(pkg_name.as_str()) {
                    found_packages.push(pkg);
                    found = true;
                }
            }
        } else {
            for db_name in &priority_order {
                if let Some(db) = alpm.syncdbs().iter().find(|d| d.name() == db_name.as_str()) {
                    if let Ok(pkg) = db.pkg(pkg_name.as_str()) {
                        found_packages.push(pkg);
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found {
            return Err(format!(
                "Package {} not found in enabled repositories",
                pkg_name
            ));
        }
    }

    alpm.trans_init(TransFlag::ALL_DEPS)
        .map_err(|e| e.to_string())?;

    for pkg in found_packages {
        alpm.trans_add_pkg(pkg).map_err(|e| e.to_string())?;
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
                        // Try to add update. If already added (e.g. it's the target pkg), this might error or be no-op.
                        // We ignore error to be safe.
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

    emit_simple_progress(50, "Downloading packages...");
    match alpm.trans_commit() {
        Ok(_) => {
            emit_simple_progress(100, "Installation complete!");
            Ok(())
        }
        Err(e) => {
            let msg = e.to_string();
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

pub fn execute_alpm_check_updates_safe(_enabled_repos: Vec<String>, _system_alpm: &mut Alpm) {
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

    let sync_status = std::process::Command::new("pacman")
        .args(&["-Sy", "--dbpath", temp_path.to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match sync_status {
        Ok(s) if s.success() => {
            emit_simple_progress(50, "Checking for updates...");
            let qu_out = std::process::Command::new("pacman")
                .args(&["-Qu", "--dbpath", temp_path.to_str().unwrap()])
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

    emit_simple_progress(50, "Removing packages...");
    match alpm.trans_commit() {
        Ok(_) => {
            emit_simple_progress(100, "Uninstallation complete!");
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn execute_alpm_upgrade(
    packages: Option<Vec<String>>,
    enabled_repos: Vec<String>,
    alpm: &mut Alpm,
) -> Result<(), String> {
    if packages.is_some() {
        logger::info(
            "AlpmUpgrade with package list: doing full system upgrade (Arch does not support partial upgrades).",
        );
    }

    ensure_keyrings_updated(&enabled_repos)?;

    // RETRY LOOP: Scoped manually to avoid borrow checker issues
    let mut retry_needed = false;

    // Attempt 1
    {
        emit_simple_progress(5, "Synchronizing databases...");
        if let Err(e) = alpm.syncdbs_mut().update(false) {
            logger::warn(&format!("Database sync warning (continuing): {}", e));
        }

        setup_progress_callbacks(alpm)?;

        emit_simple_progress(10, "Calculating upgrades...");
        if let Err(e) = alpm.trans_init(TransFlag::ALL_DEPS) {
            return Err(e.to_string());
        }

        if let Err(e) = alpm.sync_sysupgrade(false) {
            let _ = alpm.trans_release();
            return Err(e.to_string());
        }

        emit_simple_progress(20, "Preparing transaction...");

        let prepare_err = match alpm.trans_prepare() {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };

        if let Some(msg) = prepare_err {
            let _ = alpm.trans_release();

            if is_corrupt_db_error(&msg) {
                logger::warn(&format!(
                    "Upgrade failed due to corrupt DB: {}. triggered retry.",
                    msg
                ));
                retry_needed = true;
            } else {
                cleanup_partial_downloads();
                return Err(format!("Transaction preparation failed: {}", msg));
            }
        } else if !retry_needed {
            // Success path (only if no error)
            emit_simple_progress(50, "Upgrading system...");
            match alpm.trans_commit() {
                Ok(_) => {
                    emit_simple_progress(100, "System upgrade complete!");
                    return Ok(());
                }
                Err(e) => {
                    let msg = e.to_string();
                    let classified = classify_alpm_error(&msg);
                    emit_progress_event(AlpmProgressEvent {
                        event_type: "error".to_string(),
                        package: None,
                        percent: None,
                        downloaded: None,
                        total: None,
                        message: serde_json::to_string(&classified).unwrap_or(msg.clone()),
                    });
                    return Err(msg);
                }
            }
        }
    }

    // Attempt 2 (Recovery)
    if retry_needed {
        emit_simple_progress(0, "Attempting self-repair of corrupted databases...");

        if let Err(e) = force_refresh_sync_dbs(alpm) {
            logger::error(&format!("Failed to refresh DBs during recovery: {}", e));
            return Err(format!("Database repair failed: {}", e));
        }

        emit_simple_progress(5, "Synchronizing databases...");
        if let Err(e) = alpm.syncdbs_mut().update(false) {
            logger::warn(&format!("Database sync warning (continuing): {}", e));
        }

        setup_progress_callbacks(alpm)?;
        emit_simple_progress(10, "Calculating upgrades...");

        if let Err(e) = alpm.trans_init(TransFlag::ALL_DEPS) {
            return Err(e.to_string());
        }
        if let Err(e) = alpm.sync_sysupgrade(false) {
            let _ = alpm.trans_release();
            return Err(e.to_string());
        }

        emit_simple_progress(20, "Preparing transaction...");

        let prepare_err = match alpm.trans_prepare() {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };

        if let Some(msg) = prepare_err {
            let _ = alpm.trans_release();
            cleanup_partial_downloads();
            return Err(format!("Transaction preparation failed (Retry): {}", msg));
        }

        emit_simple_progress(50, "Upgrading system...");
        match alpm.trans_commit() {
            Ok(_) => {
                emit_simple_progress(100, "System upgrade complete!");
                return Ok(());
            }
            Err(e) => {
                let msg = e.to_string();
                emit_progress_event(AlpmProgressEvent {
                    event_type: "error".to_string(),
                    package: None,
                    percent: None,
                    downloaded: None,
                    total: None,
                    message: msg.clone(),
                });
                return Err(msg);
            }
        }
    }

    Ok(())
}

pub fn execute_alpm_install_files(paths: Vec<String>, alpm: &mut Alpm) -> Result<(), String> {
    ensure_keyrings_updated(&vec![])?;
    emit_simple_progress(5, "Initializing local install...");

    alpm.trans_init(TransFlag::ALL_DEPS)
        .map_err(|e| e.to_string())?;
    for path in paths {
        let pkg = alpm
            .pkg_load(path.as_str(), true, SigLevel::USE_DEFAULT)
            .map_err(|e| e.to_string())?;
        alpm.trans_add_pkg(pkg).map_err(|e| e.to_string())?;
    }

    setup_progress_callbacks(alpm)?;
    alpm.trans_prepare().map_err(|e| e.to_string())?;
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
        let _ = alpm.register_syncdb(repo_name, SigLevel::USE_DEFAULT);
    }

    match alpm.syncdbs_mut().update(true) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn build_priority_order(repos: &[String], _cpu: &Option<String>) -> Vec<String> {
    repos.to_vec()
}

fn setup_progress_callbacks(alpm: &mut Alpm) -> Result<(), String> {
    // Callback signatures fixed for alpm 5.x
    // Ignoring download events for now to simplify type checking
    alpm.set_dl_cb((), move |_, _, _| {
        // no-op for now to satisfy type checker
    });

    // Progress Callback: FnMut(&mut Ctx, &str, i32, usize, usize, ?)
    // We cannot reliably access Progress enum (AddStart etc) due to API changes/version mismatch.
    // Falling back to generic "Processing" message using package name and percent.
    alpm.set_progress_cb((), move |_, pkg_name, percent, _, _, _| {
        // percent arg is i32, pkg_name is &str
        let msg = format!("Processing {}... {}%", pkg_name, percent);
        emit_simple_progress(percent as u8, &msg);
    });

    Ok(())
}

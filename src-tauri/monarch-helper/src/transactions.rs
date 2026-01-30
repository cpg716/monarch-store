use crate::alpm_errors::classify_alpm_error;
use crate::logger;
use crate::self_healer;
use alpm::{Alpm, AnyDownloadEvent, DownloadEvent, Progress, SigLevel, TransFlag};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlpmProgressEvent {
    pub event_type: String,
    pub package: Option<String>,
    pub percent: Option<u8>,
    pub downloaded: Option<u64>,
    pub total: Option<u64>,
    pub message: String,
}

fn emit_progress_event(event: AlpmProgressEvent) {
    use std::io::Write;
    if let Ok(json) = serde_json::to_string(&event) {
        let _ = writeln!(std::io::stdout(), "{}", json);
        let _ = std::io::stdout().flush();
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

pub fn execute_alpm_install(
    packages: Vec<String>,
    sync_first: bool,
    enabled_repos: Vec<String>,
    cpu_optimization: Option<String>,
    alpm: &mut Alpm,
) -> Result<(), String> {
    emit_simple_progress(5, "Initializing transaction...");

    // 1. Sync databases if needed (register enabled repos, then update all)
    if sync_first {
        emit_simple_progress(10, "Synchronizing package databases...");
        for db_name in &enabled_repos {
            let _ = alpm.register_syncdb(db_name.as_bytes().to_vec(), SigLevel::PACKAGE_OPTIONAL);
        }
        if let Err(e) = alpm.syncdbs_mut().update(false) {
            emit_simple_progress(0, &format!("Warning: Failed to sync databases: {}", e));
        }
        emit_simple_progress(15, "Databases synchronized");
    }

    // 2. Build priority order based on enabled repos and CPU optimization
    let priority_order = build_priority_order(&enabled_repos, &cpu_optimization);

    // 3. Find packages in priority order
    emit_simple_progress(20, "Resolving packages...");
    
    let mut found_packages = Vec::new();
    for pkg_name in packages {
        let mut found = false;

        // Search in priority order (respecting Soft Disable)
        for db_name in &priority_order {
            // Ensure DB is registered
            let _ = alpm.register_syncdb(db_name.as_bytes().to_vec(), SigLevel::PACKAGE_OPTIONAL);
            
            // Find package in registered DBs (Soft Disable: only look in enabled repos)
            for db in alpm.syncdbs().iter() {
                if db.name() == db_name.as_str() {
                    if let Ok(_pkg) = db.pkg(pkg_name.as_str()) {
                        found_packages.push((pkg_name.clone(), db_name.clone()));
                        emit_progress_event(AlpmProgressEvent {
                            event_type: "package_found".to_string(),
                            package: Some(pkg_name.clone()),
                            percent: None,
                            downloaded: None,
                            total: None,
                            message: format!("Found {} in {}", pkg_name, db_name),
                        });
                        found = true;
                        break;
                    }
                }
            }
            if found {
                break;
            }
        }

        if !found {
            return Err(format!("Package {} not found in any enabled repository", pkg_name));
        }
    }

    emit_simple_progress(30, &format!("Found {} package(s), preparing transaction...", found_packages.len()));

    // 4. Create transaction and add packages (transaction lives in handle)
    logger::trace("trans_init(ALL_DEPS)");
    alpm
        .trans_init(TransFlag::ALL_DEPS)
        .map_err(|e| format!("Failed to initialize transaction: {}", e))?;

    for (pkg_name, db_name) in &found_packages {
        for db in alpm.syncdbs().iter() {
            if db.name() == db_name.as_str() {
                if let Ok(pkg) = db.pkg(pkg_name.as_str()) {
                    logger::trace(&format!("trans_add_pkg {}", pkg_name));
                    if let Err(e) = alpm.trans_add_pkg(pkg) {
                        return Err(format!("Failed to add {} to transaction: {}", pkg_name, e));
                    }
                    break;
                }
            }
        }
    }

    // 5. Set up progress callbacks
    setup_progress_callbacks(alpm)?;

    // 6. Prepare transaction (resolves dependencies)
    logger::trace("trans_prepare");
    emit_simple_progress(40, "Preparing transaction (resolving dependencies)...");
    if let Err(e) = alpm.trans_prepare() {
        let error_msg = format!("Transaction preparation failed: {}", e);
        emit_simple_progress(0, &error_msg);
        return Err(error_msg);
    }

    emit_simple_progress(50, "Transaction prepared, downloading packages...");

    // 7. Commit transaction (with optional keyring self-heal retry)
    logger::trace("trans_commit");
    if let Err(e) = commit_with_self_heal(alpm, "Installation") {
        let error_msg = e;
        let classified = classify_alpm_error(&error_msg);
        emit_progress_event(AlpmProgressEvent {
            event_type: "error".to_string(),
            package: None,
            percent: None,
            downloaded: None,
            total: None,
            message: serde_json::to_string(&classified).unwrap_or(error_msg.clone()),
        });
        emit_simple_progress(0, &error_msg);
        return Err(error_msg);
    }

    emit_simple_progress(100, "Installation complete!");
    Ok(())
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
    emit_simple_progress(5, "Initializing uninstall transaction...");
    logger::trace(if remove_deps {
        "trans_init(CASCADE) uninstall (with dependencies)"
    } else {
        "trans_init(NONE) uninstall"
    });

    alpm
        .trans_init(flags)
        .map_err(|e| format!("Failed to initialize transaction: {}", e))?;

    emit_simple_progress(10, "Resolving packages to remove...");

    for pkg_name in packages {
        if let Ok(pkg) = alpm.localdb().pkg(pkg_name.as_str()) {
            if let Err(e) = alpm.trans_remove_pkg(pkg) {
                return Err(format!("Failed to add {} to removal: {}", pkg_name, e));
            }
            emit_progress_event(AlpmProgressEvent {
                event_type: "package_marked".to_string(),
                package: Some(pkg_name.clone()),
                percent: None,
                downloaded: None,
                total: None,
                message: format!("Marked {} for removal", pkg_name),
            });
        } else {
            return Err(format!("Package {} is not installed", pkg_name));
        }
    }

    emit_simple_progress(30, "Preparing removal transaction...");
    logger::trace("trans_prepare uninstall");
    setup_progress_callbacks(alpm)?;

    if let Err(e) = alpm.trans_prepare() {
        let error_msg = format!("Transaction preparation failed: {}", e);
        emit_simple_progress(0, &error_msg);
        return Err(error_msg);
    }

    emit_simple_progress(50, "Removing packages...");
    logger::trace("trans_commit uninstall");
    if let Err(e) = commit_with_self_heal(alpm, "Uninstall") {
        emit_simple_progress(0, &e);
        return Err(e);
    }

    emit_simple_progress(100, "Uninstallation complete!");
    Ok(())
}

pub fn execute_alpm_upgrade(
    packages: Option<Vec<String>>,
    enabled_repos: Vec<String>,
    alpm: &mut Alpm,
) -> Result<(), String> {
    emit_simple_progress(5, "Synchronizing databases...");

    for db_name in &enabled_repos {
        let _ = alpm.register_syncdb(db_name.as_bytes().to_vec(), SigLevel::PACKAGE_OPTIONAL);
    }
    if let Err(e) = alpm.syncdbs_mut().update(false) {
        emit_simple_progress(0, &format!("Warning: Failed to sync databases: {}", e));
    }

    emit_simple_progress(15, "Checking for updates...");

    // Collect upgrade targets (name, db_name) for two-phase: download then install
    let mut upgrade_targets: Vec<(String, String)> = Vec::new();
    if let Some(specific_packages) = &packages {
        for pkg_name in specific_packages {
            if let Ok(local_pkg) = alpm.localdb().pkg(pkg_name.as_str()) {
                for db in alpm.syncdbs().iter() {
                    if !enabled_repos.iter().any(|r| r.as_str() == db.name()) {
                        continue;
                    }
                    if let Ok(sync_pkg) = db.pkg(pkg_name.as_str()) {
                        if sync_pkg.version() > local_pkg.version() {
                            upgrade_targets.push((pkg_name.clone(), db.name().to_string()));
                            break;
                        }
                    }
                }
            }
        }
    } else {
        for local_pkg in alpm.localdb().pkgs().iter() {
            for db in alpm.syncdbs().iter() {
                if !enabled_repos.iter().any(|r| r.as_str() == db.name()) {
                    continue;
                }
                if let Ok(sync_pkg) = db.pkg(local_pkg.name()) {
                    if sync_pkg.version() > local_pkg.version() {
                        upgrade_targets.push((local_pkg.name().to_string(), db.name().to_string()));
                        break;
                    }
                }
            }
        }
    }

    if upgrade_targets.is_empty() {
        emit_simple_progress(100, "Nothing to upgrade.");
        return Ok(());
    }

    // Phase 1: DOWNLOAD_ONLY (UI shows "Downloading...")
    logger::trace("trans_init(ALL_DEPS | DOWNLOAD_ONLY) upgrade phase 1");
    alpm
        .trans_init(TransFlag::ALL_DEPS | TransFlag::DOWNLOAD_ONLY)
        .map_err(|e| format!("Failed to initialize transaction: {}", e))?;

    for (pkg_name, db_name) in &upgrade_targets {
        for db in alpm.syncdbs().iter() {
            if db.name() == db_name.as_str() {
                if let Ok(sync_pkg) = db.pkg(pkg_name.as_str()) {
                    let _ = alpm.trans_add_pkg(sync_pkg);
                }
                break;
            }
        }
    }

    setup_progress_callbacks(alpm)?;
    if let Err(e) = alpm.trans_prepare() {
        let error_msg = format!("Transaction preparation failed: {}", e);
        emit_simple_progress(0, &error_msg);
        return Err(error_msg);
    }

    emit_simple_progress(55, "Downloading updates...");
    logger::trace("trans_commit upgrade phase 1 (download only)");
    if let Err(e) = commit_with_self_heal(alpm, "Upgrade") {
        emit_simple_progress(0, &e);
        return Err(e);
    }

    alpm.trans_release().ok();

    // Phase 2: Install from cache (UI shows "Installing...")
    emit_simple_progress(65, "Installing updates...");
    logger::trace("trans_init(ALL_DEPS) upgrade phase 2");
    alpm
        .trans_init(TransFlag::ALL_DEPS)
        .map_err(|e| format!("Failed to initialize install transaction: {}", e))?;

    for (pkg_name, db_name) in &upgrade_targets {
        for db in alpm.syncdbs().iter() {
            if db.name() == db_name.as_str() {
                if let Ok(sync_pkg) = db.pkg(pkg_name.as_str()) {
                    let _ = alpm.trans_add_pkg(sync_pkg);
                }
                break;
            }
        }
    }

    setup_progress_callbacks(alpm)?;
    if let Err(e) = alpm.trans_prepare() {
        let error_msg = format!("Transaction preparation failed: {}", e);
        emit_simple_progress(0, &error_msg);
        return Err(error_msg);
    }

    emit_simple_progress(80, "Installing packages...");
    logger::trace("trans_commit upgrade phase 2");
    if let Err(e) = commit_with_self_heal(alpm, "Upgrade") {
        emit_simple_progress(0, &e);
        return Err(e);
    }

    emit_simple_progress(100, "System upgrade complete!");
    Ok(())
}

pub fn execute_alpm_install_files(
    paths: Vec<String>,
    alpm: &mut Alpm,
) -> Result<(), String> {
    emit_simple_progress(5, "Initializing local package installation...");

    alpm
        .trans_init(TransFlag::ALL_DEPS)
        .map_err(|e| format!("Failed to initialize transaction: {}", e))?;

    emit_simple_progress(10, "Adding package files...");

    for path in paths {
        if !std::path::Path::new(&path).exists() {
            return Err(format!("Package file not found: {}", path));
        }

        let loaded = alpm
            .pkg_load(path.as_bytes(), true, SigLevel::PACKAGE_OPTIONAL)
            .map_err(|e| format!("Failed to load package file {}: {}", path, e))?;
        if let Err(e) = alpm.trans_add_pkg(loaded) {
            return Err(format!("Failed to add package file {}: {}", path, e));
        }

        emit_progress_event(AlpmProgressEvent {
            event_type: "file_added".to_string(),
            package: Some(path.clone()),
            percent: None,
            downloaded: None,
            total: None,
            message: format!("Added {}", path),
        });
    }

    emit_simple_progress(30, "Preparing installation...");

    setup_progress_callbacks(alpm)?;

    if let Err(e) = alpm.trans_prepare() {
        let error_msg = format!("Transaction preparation failed: {}", e);
        emit_simple_progress(0, &error_msg);
        return Err(error_msg);
    }

    emit_simple_progress(50, "Installing packages...");
    logger::trace("trans_commit install_files");
    if let Err(e) = commit_with_self_heal(alpm, "InstallFiles") {
        emit_simple_progress(0, &e);
        return Err(e);
    }

    emit_simple_progress(100, "Local packages installed successfully!");
    Ok(())
}

pub fn execute_alpm_sync(
    enabled_repos: Vec<String>,
    alpm: &mut Alpm,
) -> Result<(), String> {
    emit_simple_progress(5, "Synchronizing package databases...");

    for db_name in &enabled_repos {
        let _ = alpm.register_syncdb(db_name.as_bytes().to_vec(), SigLevel::PACKAGE_OPTIONAL);
    }
    let count = enabled_repos.len();
    emit_simple_progress(20, "Updating sync databases...");
    if let Err(e) = alpm.syncdbs_mut().update(false) {
        emit_simple_progress(0, &format!("Warning: Failed to sync: {}", e));
    }
    emit_simple_progress(100, &format!("Synchronized {} database(s)", count));
    Ok(())
}

fn build_priority_order(enabled_repos: &[String], cpu_optimization: &Option<String>) -> Vec<String> {
    let mut priority = Vec::new();

    // 1. Hardware optimized (if enabled and CPU supports)
    if let Some(opt) = cpu_optimization {
        match opt.as_str() {
            "znver4" => {
                if enabled_repos.iter().any(|r| r.contains("znver4")) {
                    priority.push("cachyos-extra-znver4".to_string());
                    priority.push("cachyos-core-znver4".to_string());
                }
            }
            "v4" => {
                if enabled_repos.iter().any(|r| r.contains("v4")) {
                    priority.push("cachyos-v4".to_string());
                    priority.push("cachyos-core-v4".to_string());
                    priority.push("cachyos-extra-v4".to_string());
                }
            }
            "v3" => {
                if enabled_repos.iter().any(|r| r.contains("v3")) {
                    priority.push("cachyos-v3".to_string());
                    priority.push("cachyos-core-v3".to_string());
                    priority.push("cachyos-extra-v3".to_string());
                }
            }
            _ => {}
        }
    }

    // 2. Chaotic-AUR (if enabled)
    if enabled_repos.iter().any(|r| r == "chaotic-aur") {
        priority.push("chaotic-aur".to_string());
    }

    // 3. Official repos (always enabled)
    priority.push("core".to_string());
    priority.push("extra".to_string());
    priority.push("multilib".to_string());

    // 4. Other enabled repos
    for repo in enabled_repos {
        if !priority.contains(repo) {
            priority.push(repo.clone());
        }
    }

    priority
}

/// Commit transaction with self-heal: on KeyringError run pacman-key --refresh-keys and retry once.
fn commit_with_self_heal(alpm: &mut Alpm, _op: &str) -> Result<(), String> {
    let err_msg = match alpm.trans_commit() {
        Ok(()) => return Ok(()),
        Err(e) => e.to_string(),
    };
    let classified = classify_alpm_error(&err_msg);
    if classified.recovery_action.as_deref() == Some("RepairKeyring") {
        emit_simple_progress(50, self_healer::keyring_refresh_message());
        if self_healer::refresh_keyring().is_ok() {
            logger::info("Keyring refreshed, retrying transaction");
            return alpm.trans_commit().map_err(|e| e.to_string());
        }
    }
    if classified.kind == "DatabaseLocked" {
        return Err(self_healer::db_lock_busy_message().to_string());
    }
    Err(err_msg)
}

fn setup_progress_callbacks(alpm: &mut Alpm) -> Result<(), String> {
    // Download callback: set_dl_cb(data, FnMut(&str, AnyDownloadEvent, &mut T))
    alpm.set_dl_cb((), |filename: &str, event: AnyDownloadEvent, _: &mut ()| {
        let (xfered, total) = match event.event() {
            DownloadEvent::Progress(p) => (p.downloaded as u64, p.total as u64),
            _ => (0, 0),
        };
        let percent = if total > 0 {
            ((xfered * 100) / total) as u8
        } else {
            0
        };
        emit_progress_event(AlpmProgressEvent {
            event_type: "download_progress".to_string(),
            package: Some(filename.to_string()),
            percent: Some(percent),
            downloaded: Some(xfered),
            total: Some(total),
            message: format!("Downloading {}: {}%", filename, percent),
        });
    });

    // Progress callback: set_progress_cb(data, FnMut(Progress, &str, i32, usize, usize, &mut T))
    alpm.set_progress_cb((), |progress: Progress, pkgname: &str, percent: i32, _howmany: usize, _current: usize, _: &mut ()| {
        let event_str = format!("{:?}", progress).to_uppercase();
        if event_str.contains("EXTRACT") || event_str.contains("LOAD") {
            emit_progress_event(AlpmProgressEvent {
                event_type: "extract_progress".to_string(),
                package: Some(pkgname.to_string()),
                percent: Some(percent.clamp(0, 100) as u8),
                downloaded: None,
                total: None,
                message: format!("Processing {}: {}%", pkgname, percent),
            });
        } else if event_str.contains("INSTALL") || event_str.contains("UPGRADE") || event_str.contains("REINSTALL") {
            emit_progress_event(AlpmProgressEvent {
                event_type: "install_progress".to_string(),
                package: Some(pkgname.to_string()),
                percent: Some(percent.clamp(0, 100) as u8),
                downloaded: None,
                total: None,
                message: format!("Installing {}: {}%", pkgname, percent),
            });
        } else if event_str.contains("REMOVE") {
            emit_progress_event(AlpmProgressEvent {
                event_type: "remove_progress".to_string(),
                package: Some(pkgname.to_string()),
                percent: Some(percent.clamp(0, 100) as u8),
                downloaded: None,
                total: None,
                message: format!("Removing {}: {}%", pkgname, percent),
            });
        }
    });

    Ok(())
}

use crate::aur_api;
use crate::commands::package::PendingUpdate;
use crate::repo_manager::RepoManager;
use std::process::Stdio;
use tauri::{AppHandle, Emitter, State};

/// Command and label for "Update in terminal" (Apdatifier-style transparency).
#[derive(Clone, serde::Serialize)]
pub struct SystemUpdateCommandPayload {
    pub command: String,
    pub description: String,
}

/// Returns the exact command we conceptually run for a full system upgrade.
/// Use for "Update in terminal": copy to clipboard or open user's terminal.
/// Always full -Syu (sync + upgrade) — never -Sy alone.
#[tauri::command]
pub fn get_system_update_command() -> SystemUpdateCommandPayload {
    SystemUpdateCommandPayload {
        command: "sudo pacman -Syu".to_string(),
        description: "Full system upgrade (sync databases + upgrade all packages)".to_string(),
    }
}

/// Payload for update-complete event so the UI can stop spinning and show result without blocking.
#[derive(Clone, serde::Serialize)]
pub struct UpdateCompletePayload {
    pub success: bool,
    pub message: String,
}

/// Payload for update-progress so the Updates page progress bar and step can move (not just status text).
#[derive(Clone, serde::Serialize)]
pub struct UpdateProgressPayload {
    pub phase: String,
    pub progress: u8,
    pub message: String,
}

#[tauri::command]
pub async fn perform_system_update(
    app: AppHandle,
    _state: State<'_, RepoManager>,
    password: Option<String>,
) -> Result<String, String> {
    log::info!("Update: starting process (background)");

    // Run the full update in a background task so the app does not freeze.
    let app_bg = app.clone();
    let password_bg = password.clone();
    tauri::async_runtime::spawn(async move {
        // Yield so the IPC response "started" is sent before we do any work.
        tokio::task::yield_now().await;
        let result = run_system_update_impl(app_bg.clone(), password_bg).await;
        let (success, message) = match &result {
            Ok(msg) => (true, msg.clone()),
            Err(e) => (false, e.clone()),
        };
        let payload = UpdateCompletePayload { success, message };
        let _ = app_bg.emit("update-complete", payload);
    });

    // Return immediately so the UI stays responsive.
    Ok("started".to_string())
}

/// Runs the full system update; used inside the background task.
async fn run_system_update_impl(
    app: AppHandle,
    password: Option<String>,
) -> Result<String, String> {
    // Acquire global lock to prevent concurrent pacman operations
    let _guard = crate::utils::PRIVILEGED_LOCK.lock().await;

    // Phase 1: Sanity Check (Ping)
    let _ = app.emit("update-status", "Checking connectivity...");

    let is_online = tokio::process::Command::new("ping")
        .args(["-c", "1", "-W", "2", "archlinux.org"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    log::info!("[Update] Online status: {}", is_online);

    if !is_online {
        return Err("OFFLINE: Cannot perform update without internet connectivity.".to_string());
    }

    // Phase 2: Full System Upgrade (SINGLE TRANSACTION via ALPM)
    let _ = app.emit(
        "update-status",
        "Synchronizing databases and upgrading system...",
    );
    let _ = app.emit(
        "update-progress",
        UpdateProgressPayload {
            phase: "refresh".to_string(),
            progress: 0,
            message: "Synchronizing databases...".to_string(),
        },
    );

    log::info!("Update: running ALPM system upgrade transaction");

    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ExecuteBatch {
            manifest: crate::models::TransactionManifest {
                update_system: true,
                refresh_db: true,
                ..Default::default()
            },
        },
        password.clone(),
    )
    .await?;

    // Tell the user to look for the Polkit/auth dialog so the app doesn't appear frozen.
    let _ = app.emit("update-status", "Waiting for authentication...");

    // Use timeout so we can remind the user every 45s if still waiting (e.g. password dialog behind other windows).
    let mut sysupgrade_failed = false;
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(45), rx.recv()).await {
            Ok(Some(msg)) => {
                let _ = app.emit("update-status", &msg.message);

                // Detect critical failure messages (helper sends "Error: ..." via progress message)
                if msg.message.starts_with("Error:")
                    || msg.message.contains("Transaction preparation failed")
                {
                    sysupgrade_failed = true;
                    let _ = app.emit(
                        "install-output",
                        &format!("CRITICAL: System update failed: {}", msg.message),
                    );
                } else {
                    let _ = app.emit("install-output", &msg.message);
                }

                let phase = if msg.message.to_lowercase().contains("sync")
                    || msg.message.to_lowercase().contains("database")
                {
                    "refresh"
                } else if msg.message.to_lowercase().contains("download")
                    || msg.message.to_lowercase().contains("install")
                    || msg.message.to_lowercase().contains("upgrade")
                {
                    "upgrade"
                } else {
                    "upgrade"
                };
                let _ = app.emit(
                    "update-progress",
                    UpdateProgressPayload {
                        phase: phase.to_string(),
                        progress: msg.progress,
                        message: msg.message.clone(),
                    },
                );
            }
            Ok(None) => break, // channel closed, helper finished
            Err(_) => {
                // No message in 45s — likely waiting for password or slow mirror
                let _ = app.emit(
                    "update-status",
                    "Still waiting... If a password dialog is open, bring it to the front and enter your password.",
                );
            }
        }
    }

    if sysupgrade_failed {
        let msg = "System update failed. Aborting AUR updates to prevent partial upgrade state.";
        let _ = app.emit("update-status", msg);
        let _ = app.emit("install-output", msg);
        let _ = app.emit(
            "update-progress",
            UpdateProgressPayload {
                phase: "error".to_string(),
                progress: 0,
                message: msg.to_string(),
            },
        );
        return Err(msg.to_string());
    }

    // Phase 3: AUR Batch
    let _ = app.emit("update-status", "Checking for AUR updates...");
    let _ = app.emit(
        "update-progress",
        UpdateProgressPayload {
            phase: "upgrade".to_string(),
            progress: 100,
            message: "System upgrade complete.".to_string(),
        },
    );

    let aur_updates = check_aur_updates().await.unwrap_or_default();
    if aur_updates.is_empty() {
        let _ = app.emit("update-status", "No AUR updates found.");
    } else {
        let _ = app.emit(
            "update-status",
            format!("Building {} AUR packages...", aur_updates.len()),
        );
        let _ = app.emit(
            "update-progress",
            UpdateProgressPayload {
                phase: "aur".to_string(),
                progress: 0,
                message: format!("Building {} AUR packages...", aur_updates.len()),
            },
        );

        let mut built_packages = Vec::new();
        for pkg in aur_updates {
            let _ = app.emit("update-status", format!("Building {}...", pkg.name));

            match build_aur_package(&pkg.name, &app, &password).await {
                Ok(paths) => {
                    built_packages.extend(paths);
                }
                Err(e) => {
                    let _ = app.emit(
                        "install-output",
                        format!("Warning: Failed to build {}: {}. Skipping...", pkg.name, e),
                    );
                }
            }
        }

        if !built_packages.is_empty() {
            let _ = app.emit("update-status", "Installing built AUR packages...");

            install_built_packages(built_packages, &password, &app).await?;
        }
    }

    let _ = app.emit("update-status", "All updates completed successfully.");
    let _ = app.emit(
        "update-progress",
        UpdateProgressPayload {
            phase: "complete".to_string(),
            progress: 100,
            message: "All updates completed successfully.".to_string(),
        },
    );
    Ok("System fully updated".to_string())
}

async fn check_aur_updates() -> Result<Vec<PendingUpdate>, String> {
    let foreign = tokio::task::spawn_blocking(crate::alpm_read::get_foreign_installed_packages)
        .await
        .map_err(|e| format!("Task join error: {}", e))?;
    let mut installed_aur = std::collections::HashMap::new();
    let mut names = Vec::new();
    for (name, version) in foreign {
        installed_aur.insert(name.clone(), version);
        names.push(name);
    }
    if names.is_empty() {
        return Ok(vec![]);
    }

    let names_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let aur_info = aur_api::get_multi_info(&names_refs).await?;

    let mut pending = Vec::new();
    for pkg in aur_info {
        if let Some(installed_ver) = installed_aur.get(&pkg.name) {
            if pkg.version != *installed_ver {
                pending.push(PendingUpdate {
                    name: pkg.name.clone(),
                    old_version: installed_ver.clone(),
                    new_version: pkg.version,
                    repo: "aur".to_string(),
                });
            }
        }
    }

    // Only build from AUR packages that are NOT in any sync repo (Chaotic, CachyOS, etc.).
    // If the package is in a repo, it was already updated by Phase 2 (Sysupgrade) or should be
    // updated via repo; building from AUR would be wrong and often fails (e.g. makepkg unknown error).
    let mut truly_aur_only = Vec::new();
    for p in pending {
        if !crate::commands::package::is_in_sync_repos(&p.name).await {
            truly_aur_only.push(p);
        }
    }

    Ok(truly_aur_only)
}

/// Unified AUR build function - delegates to the improved package.rs implementation
/// which includes automatic PGP key handling and proper error recovery.
async fn build_aur_package(
    pkg: &str,
    app: &AppHandle,
    password: &Option<String>,
) -> Result<Vec<String>, String> {
    // Pass the actual password to the improved AUR build pipeline
    crate::commands::package::build_aur_package(app, pkg, password).await
}

async fn install_built_packages(
    paths: Vec<String>,
    password: &Option<String>,
    app: &AppHandle,
) -> Result<(), String> {
    // Zone 4: Copy to /tmp/monarch-install so root helper can read and verify
    let install_paths = crate::commands::package::copy_paths_to_monarch_install(paths).await?;
    // ✅ NEW: Use ALPM transaction to install built AUR packages
    let mut rx = crate::helper_client::invoke_helper(
        app,
        crate::helper_client::HelperCommand::AlpmInstallFiles {
            paths: install_paths,
        },
        password.clone(),
    )
    .await?;

    // Stream progress events
    while let Some(msg) = rx.recv().await {
        let _ = app.emit("install-output", &msg.message);
    }

    Ok(())
}

/// Unified Update Aggregator (Phase 2)
/// Fetches updates from Repo, AUR, and Flatpak in parallel.
#[tauri::command]
pub async fn check_updates() -> Result<Vec<crate::models::UpdateItem>, String> {
    log::info!("Checking for updates (Unified)...");

    // Task A: Repo (Official) - Fast, local DB read
    // Note: We assume DBs are refreshed. If not, user should hit "Refresh" or we call refresh separately.
    let repo_task = tokio::task::spawn_blocking(crate::alpm_read::get_host_updates);

    // Task B: AUR - Web query + Raur
    let aur_task = crate::aur_api::get_candidate_updates();

    // Task C: Flatpak - CLI process
    let flatpak_task = crate::flathub_api::get_updates();

    // Parallel Join
    let (repo_res, aur_res, flatpak_res) = tokio::join!(repo_task, aur_task, flatpak_task);

    let mut all_updates = Vec::new();

    // 1. Repo
    match repo_res {
        Ok(items) => all_updates.extend(items),
        Err(e) => log::error!("Failed to check repo updates: {}", e),
    }

    // 2. AUR
    match aur_res {
        Ok(items) => all_updates.extend(items),
        Err(e) => log::error!("Failed to check AUR updates: {}", e),
    }

    // 3. Flatpak
    match flatpak_res {
        Ok(items) => all_updates.extend(items),
        Err(e) => log::error!("Failed to check Flatpak updates: {}", e),
    }

    log::info!("Found {} total updates", all_updates.len());
    Ok(all_updates)
}

/// Unified Execution Engine (Phase 3 & 4)
/// Safely executes the update queue respecting the "Safety Lock".
#[tauri::command]
pub async fn apply_updates(
    app: AppHandle,
    targets: Vec<crate::models::UpdateItem>,
    password: Option<String>,
) -> Result<String, String> {
    if targets.is_empty() {
        return Ok("No updates selected".to_string());
    }

    log::info!("Applying {} updates...", targets.len());

    // Phase 4: Safety Lock
    // If ANY official package is selected, we MUST do a full system upgrade.
    // We cannot selectively upgrade "core/pacman" without "-Syu".
    let has_official = targets.iter().any(|t| t.source.source_type == "repo");

    // Group targets
    let aur_targets: Vec<&crate::models::UpdateItem> = targets
        .iter()
        .filter(|t| t.source.source_type == "aur")
        .collect();

    let flatpak_targets: Vec<&crate::models::UpdateItem> = targets
        .iter()
        .filter(|t| t.source.source_type == "flatpak")
        .collect();

    // 1. Execute Repo Loop (The Iron Core)
    if has_official {
        log::info!("Safety Lock: Official updates detected. Enforcing System Upgrade.");
        // We reuse the existing logic which does -Syu
        // This updates ALL system packages, not just the selected ones.
        // The UI should ideally warn user "Updating System..."
        // Calls `run_system_update_impl` but we might want to skip AUR/Flatpak phase of that old function
        // if we are handling them here specially.
        // However, `run_system_update_impl` handles the heavy lifting of Sysupgrade transaction.
        // Let's call a simplified version or reuse.
        // reuse `run_system_update_impl` covers Sysupgrade + AUR.
        // But here we have specific targets.
        // If we call `run_system_update_impl`, it checks *all* AUR updates.
        // We want to update only `aur_targets`.

        // Let's trigger the Sysupgrade part manually.
        let _ = app.emit(
            "update-status",
            "Starting System Upgrade (Official Repos)...",
        );
        let mut rx = crate::helper_client::invoke_helper(
            &app,
            crate::helper_client::HelperCommand::ExecuteBatch {
                manifest: crate::models::TransactionManifest {
                    update_system: true,
                    refresh_db: true,
                    ..Default::default()
                },
            },
            password.clone(),
        )
        .await?;

        // Monitor Sysupgrade
        while let Some(msg) = rx.recv().await {
            let _ = app.emit("install-output", &msg.message);
            if msg.message.starts_with("Error:") {
                return Err(format!("System update failed: {}", msg.message));
            }
        }
    }

    // 2. Execute AUR Loop (Native Builder)
    if !aur_targets.is_empty() {
        let _ = app.emit(
            "update-status",
            format!("Processing {} AUR updates...", aur_targets.len()),
        );
        let mut built_paths = Vec::new();

        for item in aur_targets {
            let _ = app.emit("update-status", format!("Building {}...", item.name));
            match build_aur_package(&item.name, &app, &password).await {
                Ok(paths) => built_paths.extend(paths),
                Err(e) => {
                    let _ = app.emit(
                        "install-output",
                        format!("Failed to build {}: {}", item.name, e),
                    );
                    // Check if we should abort or continue? Usually continue best effort.
                }
            }
        }

        if !built_paths.is_empty() {
            let _ = app.emit("update-status", "Installing AUR packages...");
            install_built_packages(built_paths, &password, &app).await?;
        }
    }

    // 3. Execute Flatpak Loop (Safety Net)
    if !flatpak_targets.is_empty() {
        let _ = app.emit(
            "update-status",
            format!("Updating {} Flatpaks...", flatpak_targets.len()),
        );
        for item in flatpak_targets {
            // item.name should be App ID based on our flathub_api.rs change.
            let _ = app.emit("install-output", format!("Updating Flatpak: {}", item.name));

            // Call flatpak update <id> -y
            // We can implement a helper or call Command direct.
            if let Err(e) = crate::flathub_api::update_flatpak(app.clone(), item.name.clone()).await
            {
                let _ = app.emit("install-output", format!("Flatpak update error: {}", e));
            }
        }
    }

    let _ = app.emit("update-status", "All selected updates applied.");
    let _ = app.emit(
        "update-complete",
        UpdateCompletePayload {
            success: true,
            message: "Done".into(),
        },
    );

    Ok("Updates applied".to_string())
}

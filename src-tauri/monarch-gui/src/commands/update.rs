use crate::aur_api;
use crate::commands::package::PendingUpdate;
use crate::repo_manager::RepoManager;
use std::process::Stdio;
use tauri::{AppHandle, Emitter, State};

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
    let _ = app.emit("update-status", "Synchronizing databases and upgrading system...");
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
        crate::helper_client::HelperCommand::Sysupgrade,
        password.clone(),
    )
    .await?;

    // Tell the user to look for the Polkit/auth dialog so the app doesn't appear frozen.
    let _ = app.emit("update-status", "Waiting for authentication...");

    // Use timeout so we can remind the user every 45s if still waiting (e.g. password dialog behind other windows).
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(45), rx.recv()).await {
            Ok(Some(msg)) => {
                let _ = app.emit("update-status", &msg.message);
                let _ = app.emit("install-output", &msg.message);
                let phase = if msg.message.to_lowercase().contains("sync") || msg.message.to_lowercase().contains("database") {
                    "refresh"
                } else if msg.message.to_lowercase().contains("download") || msg.message.to_lowercase().contains("install") || msg.message.to_lowercase().contains("upgrade") {
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
    let output = tokio::process::Command::new("pacman")
        .args(["-Qm"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut installed_aur = std::collections::HashMap::new();
    let mut names = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 {
            let name = parts[0];
            installed_aur.insert(name.to_string(), parts[1].to_string());
            names.push(name.to_string());
        }
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

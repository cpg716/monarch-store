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

#[tauri::command]
pub async fn perform_system_update(
    app: AppHandle,
    _state: State<'_, RepoManager>,
    password: Option<String>,
) -> Result<String, String> {
    println!("[Update] Starting update process (background)...");

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
    app.emit("update-status", "Checking connectivity...")
        .map_err(|e| e.to_string())?;

    let is_online = tokio::process::Command::new("ping")
        .args(["-c", "1", "-W", "2", "archlinux.org"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    println!("[Update] Online status: {}", is_online);

    if !is_online {
        return Err("OFFLINE: Cannot perform update without internet connectivity.".to_string());
    }

    // Phase 2: Full System Upgrade (SINGLE TRANSACTION via ALPM)
    app.emit(
        "update-status",
        "Synchronizing databases and upgrading system...",
    )
    .map_err(|e| e.to_string())?;

    println!("[Update] Running ALPM system upgrade transaction...");

    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::Sysupgrade,
        password.clone(),
    )
    .await?;

    // Tell the user to look for the Polkit/auth dialog so the app doesn't appear frozen.
    app.emit("update-status", "Waiting for authentication...")
        .map_err(|e| e.to_string())?;

    while let Some(msg) = rx.recv().await {
        let _ = app.emit("update-status", &msg.message);
        let _ = app.emit("install-output", &msg.message);
    }

    // Phase 3: AUR Batch
    app.emit("update-status", "Checking for AUR updates...")
        .map_err(|e| e.to_string())?;

    let aur_updates = check_aur_updates().await.unwrap_or_default();
    if aur_updates.is_empty() {
        app.emit("update-status", "No AUR updates found.")
            .map_err(|e| e.to_string())?;
    } else {
        app.emit(
            "update-status",
            format!("Building {} AUR packages...", aur_updates.len()),
        )
        .map_err(|e| e.to_string())?;

        let mut built_packages = Vec::new();
        for pkg in aur_updates {
            app.emit("update-status", format!("Building {}...", pkg.name))
                .map_err(|e| e.to_string())?;

            match build_aur_package(&pkg.name, &app, &password).await {
                Ok(paths) => {
                    built_packages.extend(paths);
                }
                Err(e) => {
                    app.emit(
                        "install-output",
                        format!("Warning: Failed to build {}: {}. Skipping...", pkg.name, e),
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }

        if !built_packages.is_empty() {
            app.emit("update-status", "Installing built AUR packages...")
                .map_err(|e| e.to_string())?;

            install_built_packages(built_packages, &password, &app).await?;
        }
    }

    app.emit("update-status", "All updates completed successfully.")
        .map_err(|e| e.to_string())?;
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
                    name: pkg.name,
                    old_version: installed_ver.clone(),
                    new_version: pkg.version,
                    repo: "aur".to_string(),
                });
            }
        }
    }

    Ok(pending)
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

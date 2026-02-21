use crate::aur_api;
use crate::commands::package::PendingUpdate;
use crate::repo_manager::RepoManager;
use specta::Type;
use std::process::Stdio;
use tauri::{AppHandle, Emitter, State};

/// Command and label for "Update in terminal" (Apdatifier-style transparency).
#[derive(Clone, serde::Serialize, Type)]
pub struct SystemUpdateCommandPayload {
    pub command: String,
    pub description: String,
}

/// Returns the exact command we conceptually run for a full system upgrade.
/// Use for "Update in terminal": copy to clipboard or open user's terminal.
/// Always full -Syu (sync + upgrade) — never -Sy alone.
#[tauri::command]
#[specta::specta]
pub fn get_system_update_command() -> SystemUpdateCommandPayload {
    SystemUpdateCommandPayload {
        command: "sudo pacman -Syu".to_string(),
        description: "Full system upgrade (sync databases + upgrade all packages)".to_string(),
    }
}

/// Payload for update-complete event so the UI can stop spinning and show result without blocking.
#[derive(Clone, serde::Serialize, Type)]
pub struct UpdateCompletePayload {
    pub success: bool,
    pub message: String,
}

/// Payload for update-progress so the Updates page progress bar and step can move (not just status text).
#[derive(Clone, serde::Serialize, Type)]
pub struct UpdateProgressPayload {
    pub phase: String,
    pub progress: u8,
    pub message: String,
}

#[tauri::command]
#[specta::specta]
pub async fn perform_system_update(
    app: AppHandle,
    _state: State<'_, RepoManager>,
    password: Option<String>,
    include_aur: Option<bool>,
    include_flatpak: Option<bool>,
) -> Result<String, String> {
    let do_aur = include_aur.unwrap_or(true);
    let do_flatpak = include_flatpak.unwrap_or(true);
    log::info!(
        "Update: starting process (background), AUR={}, Flatpak={}",
        do_aur,
        do_flatpak
    );

    let one_click = _state.inner().is_one_click_enabled().await;
    let parallel_downloads = _state.inner().get_parallel_downloads().await;
    let app_bg = app.clone();
    let password_bg = password.clone();
    tauri::async_runtime::spawn(async move {
        tokio::task::yield_now().await;
        let result = run_system_update_impl(
            app_bg.clone(),
            password_bg,
            one_click,
            do_aur,
            do_flatpak,
            Some(parallel_downloads),
        )
        .await;
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
/// include_aur / include_flatpak: when false, skip that phase (so "Update All" can match user's Sources settings).
async fn run_system_update_impl(
    app: AppHandle,
    password: Option<String>,
    one_click: bool,
    include_aur: bool,
    include_flatpak: bool,
    parallel_downloads: Option<u32>,
) -> Result<String, String> {
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
                parallel_downloads,
                ..Default::default()
            },
        },
        password.clone(),
        one_click,
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
                    if !msg.is_structured {
                        let _ = app.emit("install-output", &msg.message);
                    }
                }

                let phase = if msg.message.to_lowercase().contains("sync")
                    || msg.message.to_lowercase().contains("database")
                {
                    "refresh"
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

    // Phase 3: AUR Batch (only when user has AUR included in updates)
    let aur_updates = if include_aur {
        let _ = app.emit("update-status", "Checking for AUR updates...");
        let _ = app.emit(
            "update-progress",
            UpdateProgressPayload {
                phase: "upgrade".to_string(),
                progress: 100,
                message: "System upgrade complete.".to_string(),
            },
        );
        check_aur_updates().await.unwrap_or_default()
    } else {
        let _ = app.emit("update-status", "Skipping AUR (disabled in update scope).");
        Vec::new()
    };

    if aur_updates.is_empty() && include_aur {
        let _ = app.emit("update-status", "No AUR updates found.");
    }
    if !aur_updates.is_empty() {
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

            install_built_packages(built_packages, &password, &app, one_click).await?;
        }
    }

    // Phase 4: Flatpak Updates (only when user has Flatpak included in updates)
    if include_flatpak {
        let _ = app.emit("update-status", "Checking for Flatpak updates...");
        let _ = app.emit(
            "update-progress",
            UpdateProgressPayload {
                phase: "flatpak".to_string(),
                progress: 80,
                message: "Checking Flatpak updates...".to_string(),
            },
        );

        match crate::flathub_api::get_updates().await {
            Ok(flatpak_updates) => {
                if flatpak_updates.is_empty() {
                    let _ = app.emit("update-status", "No Flatpak updates found.");
                    let _ = app.emit("install-output", "No Flatpak updates available.");
                } else {
                    let _ = app.emit(
                        "update-status",
                        format!("Updating {} Flatpaks...", flatpak_updates.len()),
                    );
                    let _ = app.emit(
                        "install-output",
                        format!("Found {} Flatpak updates", flatpak_updates.len()),
                    );

                    for item in flatpak_updates {
                        let _ = app.emit(
                            "update-status",
                            format!("Updating Flatpak: {}...", item.name),
                        );
                        let _ =
                            app.emit("install-output", format!("Updating Flatpak: {}", item.name));

                        if let Err(e) =
                            crate::flathub_api::update_flatpak(app.clone(), item.name.clone()).await
                        {
                            let _ = app.emit(
                                "install-output",
                                format!("Flatpak update warning: {} - {}", item.name, e),
                            );
                            // Continue with other Flatpaks - don't abort the whole update
                        }
                    }

                    let _ = app.emit("update-status", "Flatpak updates completed.");
                }
            }
            Err(e) => {
                // Flatpak errors are non-critical - log but don't fail the update
                let _ = app.emit("install-output", format!("Flatpak check skipped: {}", e));
            }
        }
    } else {
        let _ = app.emit(
            "update-status",
            "Skipping Flatpak (disabled in update scope).",
        );
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
    one_click: bool,
) -> Result<(), String> {
    let install_paths = crate::commands::package::copy_paths_to_monarch_install(paths).await?;
    let mut rx = crate::helper_client::invoke_helper(
        app,
        crate::helper_client::HelperCommand::AlpmInstallFiles {
            paths: install_paths,
        },
        password.clone(),
        one_click,
    )
    .await?;

    // Stream progress events
    while let Some(msg) = rx.recv().await {
        let _ = app.emit("install-output", &msg.message);
    }

    Ok(())
}

/// Unified Update Aggregator (Phase 2)
/// Fetches updates from Repo (including Chaotic-AUR, CachyOS, etc.), AUR, and Flatpak.
/// For updates, a source is never "turned off": we always check installed packages from every
/// source; discovery toggles (Settings → Sources) only affect search/browse, not the Updates list.
/// Repo updates come from full system pacman.conf (distro-agnostic: Arch, Manjaro, Garuda, CachyOS,
/// EOS, etc.); no filtering by app "enabled" state. Params default to true.
#[tauri::command]
#[specta::specta]
pub async fn check_updates(
    state_registry: State<'_, crate::registry::RegistryState>,
    include_aur: Option<bool>,
    include_flatpak: Option<bool>,
) -> Result<Vec<crate::models::UpdateItem>, String> {
    let do_aur = include_aur.unwrap_or(true);
    let do_flatpak = include_flatpak.unwrap_or(true);
    log::info!(
        "Checking for updates (Unified), AUR={}, Flatpak={}",
        do_aur,
        do_flatpak
    );

    // Task A: Repo (Official) - Fast, local DB read
    let repo_handle = tokio::task::spawn_blocking(crate::alpm_read::get_host_updates);

    // Tasks B & C: AUR and Flatpak (empty result when disabled)
    let aur_fut = async move {
        if do_aur {
            crate::aur_api::get_candidate_updates().await
        } else {
            Ok(vec![])
        }
    };
    let flatpak_fut = async move {
        if do_flatpak {
            crate::flathub_api::get_updates().await
        } else {
            Ok(vec![])
        }
    };
    let (aur_res, flatpak_res) = tokio::join!(aur_fut, flatpak_fut);

    let repo_res = repo_handle.await;

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

    // IRON CORE (SSOT): Enrich Updates with Registry Metadata
    // "Every Inch" enforcement: Updates tab must show "Discord" not "com.discordapp.Discord".
    let mut candidate_keys = std::collections::HashSet::new();
    for u in &all_updates {
        // Flatpak text is usually ID in u.name.
        // Repo text is package name in u.name.
        candidate_keys.insert(u.name.clone());
        candidate_keys.insert(u.name.to_lowercase());

        // Also try canonical merge key logic if applicable
        if u.source.source_type == "flatpak" {
            // For flatpak, u.name IS the App ID.
            // We can try to derive a name key if it has one?
            // Usually just searching by ID (lowercase) is enough for Registry lookup.
        } else {
            // For Repo/AUR, u.name is "firefox".
        }
    }

    let candidate_vec: Vec<String> = candidate_keys.into_iter().collect();
    if !candidate_vec.is_empty() {
        if let Ok(pkgs) = state_registry.get_packages_by_canonical_ids(&candidate_vec) {
            let registry_map: std::collections::HashMap<String, crate::models::Package> = pkgs
                .into_iter()
                .map(|p| (p.canonical_id.clone(), p))
                .collect();

            if !registry_map.is_empty() {
                for u in &mut all_updates {
                    // Try to find registry match
                    // 1. By exact name/ID
                    let key1 = u.name.to_lowercase();
                    // 2. By canonical key (if name has dots/suffixes)
                    let key2 = crate::utils::canonical_merge_key(&u.name, None); // We don't have separate App ID easily here, but for Flatpak u.name IS App ID.

                    let reg_entry = registry_map.get(&key1).or_else(|| registry_map.get(&key2));

                    if let Some(reg) = reg_entry {
                        if let Some(dn) = &reg.display_name {
                            if !dn.is_empty() {
                                u.display_name = Some(dn.clone());
                            }
                        }

                        let reg_is_rich = reg
                            .icon
                            .as_deref()
                            .map(|i| i.starts_with("http") || i.starts_with("data:"))
                            .unwrap_or(false);
                        if reg_is_rich || u.icon.is_none() {
                            u.icon = reg.icon.clone();
                        }
                    }
                }
                log::info!("[UPDATES] Iron Core enriched {} updates", all_updates.len());
            }
        }
    }

    log::info!("Found {} total updates", all_updates.len());
    Ok(all_updates)
}

/// Unified Execution Engine (Phase 3 & 4)
/// Safely executes the update queue respecting the "Safety Lock".
#[tauri::command]
#[specta::specta]
pub async fn apply_updates(
    app: AppHandle,
    state_repo: State<'_, RepoManager>,
    targets: Vec<crate::models::UpdateItem>,
    password: Option<String>,
) -> Result<String, String> {
    if targets.is_empty() {
        return Ok("No updates selected".to_string());
    }
    let one_click = state_repo.inner().is_one_click_enabled().await;
    log::info!("Applying {} updates...", targets.len());

    // --- Transaction Manifest: emit summary for frontend hard-gate confirmation ---
    {
        let repo_items: Vec<&str> = targets
            .iter()
            .filter(|t| t.source.source_type == "repo")
            .map(|t| t.name.as_str())
            .collect();
        let aur_items: Vec<&str> = targets
            .iter()
            .filter(|t| t.source.source_type == "aur")
            .map(|t| t.name.as_str())
            .collect();
        let flatpak_items: Vec<&str> = targets
            .iter()
            .filter(|t| t.source.source_type == "flatpak")
            .map(|t| t.name.as_str())
            .collect();

        #[derive(serde::Serialize, Clone)]
        struct UpdateManifest {
            total: usize,
            repo_count: usize,
            aur_count: usize,
            flatpak_count: usize,
            repo_packages: Vec<String>,
            aur_packages: Vec<String>,
            flatpak_packages: Vec<String>,
        }

        let manifest = UpdateManifest {
            total: targets.len(),
            repo_count: repo_items.len(),
            aur_count: aur_items.len(),
            flatpak_count: flatpak_items.len(),
            repo_packages: repo_items.iter().map(|s| s.to_string()).collect(),
            aur_packages: aur_items.iter().map(|s| s.to_string()).collect(),
            flatpak_packages: flatpak_items.iter().map(|s| s.to_string()).collect(),
        };

        let _ = app.emit("update-manifest", &manifest);
        log::info!(
            "Manifest: {} repo, {} AUR, {} Flatpak",
            manifest.repo_count,
            manifest.aur_count,
            manifest.flatpak_count
        );
    }

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
    let mut sysupgrade_failed = false;
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
            one_click,
        )
        .await?;

        // Monitor Sysupgrade with Safety Gate
        while let Some(msg) = rx.recv().await {
            let _ = app.emit("install-output", &msg.message);
            if msg.message.starts_with("Error:")
                || msg.message.contains("Transaction preparation failed")
                || msg.message.contains("failed retrieving")
                || msg.message.to_lowercase().contains("404")
            {
                sysupgrade_failed = true;
            }
        }

        // Safety Gate: If sysupgrade failed, abort AUR/Flatpak updates to prevent partial upgrade state
        if sysupgrade_failed {
            let msg = "System update failed. Aborting AUR/Flatpak updates to prevent partial upgrade state.";
            let _ = app.emit("update-status", msg);
            let _ = app.emit("install-output", msg);
            let _ = app.emit(
                "update-complete",
                UpdateCompletePayload {
                    success: false,
                    message: msg.into(),
                },
            );
            return Err(msg.to_string());
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
            install_built_packages(built_paths, &password, &app, one_click).await?;
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

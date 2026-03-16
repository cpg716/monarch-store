use crate::aur_api;
use crate::commands::package::PendingUpdate;
use crate::models::{Package, UpdateSnapshot, UpdateSnapshotItem, UpdateSourceStatus};
use crate::repo_manager::RepoManager;
use once_cell::sync::Lazy;
use specta::Type;
use std::sync::Mutex as StdMutex;
use std::process::Stdio;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

static UPDATE_SNAPSHOT_CACHE: Lazy<StdMutex<Option<(Instant, UpdateSnapshot)>>> =
    Lazy::new(|| StdMutex::new(None));
static UPDATE_SNAPSHOT_GATE: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
const UPDATE_SNAPSHOT_TTL_MS: u128 = 10_000;

fn invalidate_update_snapshot_cache() {
    if let Ok(mut cache) = UPDATE_SNAPSHOT_CACHE.lock() {
        *cache = None;
    }
}

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
    pub overall: String,
    pub summary: UpdateRunSummary,
    pub message: String,
}

/// Payload for update-progress so the Updates page progress bar and step can move (not just status text).
#[derive(Clone, serde::Serialize, Type)]
pub struct UpdateProgressPayload {
    pub phase: String,
    pub progress: u8,
    pub message: String,
}

#[derive(Clone, serde::Serialize, Type)]
pub struct UpdateSourceProgressPayload {
    pub source: String,
    pub stage: String,
    pub current: u32,
    pub total: u32,
    pub package: Option<String>,
}

#[derive(Clone, serde::Serialize, Type)]
pub struct UpdateFailedPackage {
    pub name: String,
    pub source: String,
    pub reason: String,
}

#[derive(Clone, serde::Serialize, Type)]
pub struct UpdateRunSummary {
    pub repo: String,
    pub aur: String,
    pub flatpak: String,
    pub succeeded_packages: Vec<String>,
    pub failed_packages: Vec<UpdateFailedPackage>,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
}

#[derive(Clone)]
struct UpdateExecutionResult {
    overall: String,
    summary: UpdateRunSummary,
    message: String,
}

fn emit_source_progress(
    app: &AppHandle,
    source: &str,
    stage: &str,
    current: usize,
    total: usize,
    package: Option<String>,
) {
    let _ = app.emit(
        "update-source-progress",
        UpdateSourceProgressPayload {
            source: source.to_string(),
            stage: stage.to_string(),
            current: current as u32,
            total: total as u32,
            package,
        },
    );
}

async fn collect_update_snapshot(
    state_meta: &crate::metadata::MetadataState,
    state_registry: &crate::registry::RegistryState,
    include_aur: Option<bool>,
    include_flatpak: Option<bool>,
) -> UpdateSnapshot {
    let legacy_updates = check_updates_inner(state_registry, include_aur, include_flatpak).await;

    let do_aur = include_aur.unwrap_or(true);
    let do_flatpak = include_flatpak.unwrap_or(true);
    let has_aur = legacy_updates.iter().any(|item| item.source.source_type == "aur");
    let has_flatpak = legacy_updates
        .iter()
        .any(|item| item.source.source_type == "flatpak");
    let has_repo = legacy_updates.iter().any(|item| item.source.source_type == "repo");

    let all_updates = legacy_updates;
    let mut sources = vec![
        UpdateSourceStatus {
            source: "repo".to_string(),
            status: if has_repo { "ok".to_string() } else { "empty".to_string() },
            duration_ms: 0,
            error: None,
        },
        UpdateSourceStatus {
            source: "aur".to_string(),
            status: if !do_aur {
                "disabled".to_string()
            } else if has_aur {
                "ok".to_string()
            } else {
                "empty".to_string()
            },
            duration_ms: 0,
            error: None,
        },
        UpdateSourceStatus {
            source: "flatpak".to_string(),
            status: if !do_flatpak {
                "disabled".to_string()
            } else if has_flatpak {
                "ok".to_string()
            } else {
                "empty".to_string()
            },
            duration_ms: 0,
            error: None,
        },
    ];

    let mut candidate_keys = std::collections::HashSet::new();
    for u in &all_updates {
        candidate_keys.insert(u.name.clone());
        candidate_keys.insert(u.name.to_lowercase());
        candidate_keys.insert(crate::utils::canonical_merge_key(&u.name, None));
    }
    let registry_map = state_registry
        .get_packages_by_canonical_ids(&candidate_keys.into_iter().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.canonical_id.clone(), p))
        .collect::<std::collections::HashMap<_, _>>();

    let loader_guard = state_meta.loader.lock().ok();
    let mut items = Vec::with_capacity(all_updates.len());

    for mut update in all_updates {
        let mut package = Package {
            name: update.name.clone(),
            display_name: update.display_name.clone(),
            description: format!("Update available from {}", update.source.label),
            version: update.new_version.clone(),
            source: update.source.clone(),
            installed: true,
            icon: update.icon.clone(),
            canonical_id: crate::utils::canonical_merge_key(&update.name, None),
            available_sources: Some(vec![update.source.clone()]),
            launch_target: Some(update.name.clone()),
            installed_sources: Some(vec![update.name.clone()]),
            ..Default::default()
        };

        if update.source.source_type == "flatpak" {
            package.app_id = Some(update.name.clone());
            package.launch_target = Some(update.name.clone());
        }

        if let Some(reg) = registry_map
            .get(&package.canonical_id)
            .or_else(|| registry_map.get(&update.name.to_lowercase()))
        {
            crate::middleware::aggregation::apply_registry_backfill(&mut package, reg);
        }

        if let Some(loader) = loader_guard.as_ref() {
            crate::middleware::aggregation::enrich_with_local_metadata(
                std::slice::from_mut(&mut package),
                loader,
            );
        }

        if let Some(display_name) = package.display_name.clone() {
            update.display_name = Some(display_name);
        }
        if package.icon.is_some() {
            update.icon = package.icon.clone();
        }
        crate::utils::finalize_package_contract(&mut package);

        items.push(UpdateSnapshotItem {
            package,
            current_version: update.current_version,
            new_version: update.new_version,
        });
    }

    UpdateSnapshot { items, sources: std::mem::take(&mut sources) }
}

async fn check_updates_inner(
    state_registry: &crate::registry::RegistryState,
    include_aur: Option<bool>,
    include_flatpak: Option<bool>,
) -> Vec<crate::models::UpdateItem> {
    let do_aur = include_aur.unwrap_or(true);
    let do_flatpak = include_flatpak.unwrap_or(true);
    let repo_handle = tokio::task::spawn_blocking(crate::alpm_read::get_host_updates);

    let aur_fut = async move {
        if do_aur {
            match tokio::time::timeout(
                std::time::Duration::from_secs(4),
                crate::aur_api::get_candidate_updates(),
            )
            .await
            {
                Ok(result) => result.unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        }
    };
    let flatpak_fut = async move {
        if do_flatpak {
            match tokio::time::timeout(
                std::time::Duration::from_secs(4),
                crate::flathub_api::get_updates(),
            )
            .await
            {
                Ok(result) => result.unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        }
    };

    let (aur_items, flatpak_items) = tokio::join!(aur_fut, flatpak_fut);
    let mut all_updates = repo_handle.await.unwrap_or_default();
    all_updates.extend(aur_items);
    all_updates.extend(flatpak_items);

    let mut candidate_keys = std::collections::HashSet::new();
    for u in &all_updates {
        candidate_keys.insert(u.name.clone());
        candidate_keys.insert(u.name.to_lowercase());
        candidate_keys.insert(crate::utils::canonical_merge_key(&u.name, None));
    }

    let candidate_vec: Vec<String> = candidate_keys.into_iter().collect();
    if !candidate_vec.is_empty() {
        if let Ok(pkgs) = state_registry.get_packages_by_canonical_ids(&candidate_vec) {
            let registry_map: std::collections::HashMap<String, crate::models::Package> = pkgs
                .into_iter()
                .map(|p| (p.canonical_id.clone(), p))
                .collect();

            for u in &mut all_updates {
                let key1 = u.name.to_lowercase();
                let key2 = crate::utils::canonical_merge_key(&u.name, None);
                if let Some(reg) = registry_map.get(&key1).or_else(|| registry_map.get(&key2)) {
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
        }
    }

    all_updates
}

#[tauri::command]
#[specta::specta]
pub async fn get_update_snapshot(
    state_meta: State<'_, crate::metadata::MetadataState>,
    state_registry: State<'_, crate::registry::RegistryState>,
    include_aur: Option<bool>,
    include_flatpak: Option<bool>,
) -> Result<UpdateSnapshot, String> {
    if let Ok(cache) = UPDATE_SNAPSHOT_CACHE.lock() {
        if let Some((created_at, snapshot)) = cache.as_ref() {
            if created_at.elapsed().as_millis() < UPDATE_SNAPSHOT_TTL_MS {
                log::debug!(
                    "[UPDATES] snapshot cache hit: {} items",
                    snapshot.items.len()
                );
                return Ok(snapshot.clone());
            }
        }
    }

    let _guard = UPDATE_SNAPSHOT_GATE.lock().await;
    if let Ok(cache) = UPDATE_SNAPSHOT_CACHE.lock() {
        if let Some((created_at, snapshot)) = cache.as_ref() {
            if created_at.elapsed().as_millis() < UPDATE_SNAPSHOT_TTL_MS {
                log::debug!(
                    "[UPDATES] snapshot cache hit after gate: {} items",
                    snapshot.items.len()
                );
                return Ok(snapshot.clone());
            }
        }
    }

    let started = Instant::now();
    let snapshot = collect_update_snapshot(
        state_meta.inner(),
        state_registry.inner(),
        include_aur,
        include_flatpak,
    )
    .await;
    log::info!(
        "[UPDATES] snapshot loaded: {} items in {} ms",
        snapshot.items.len(),
        started.elapsed().as_millis()
    );
    if let Ok(mut cache) = UPDATE_SNAPSHOT_CACHE.lock() {
        *cache = Some((Instant::now(), snapshot.clone()));
    }
    Ok(snapshot)
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
        let payload = match result {
            Ok(execution) => UpdateCompletePayload {
                overall: execution.overall,
                summary: execution.summary,
                message: execution.message,
            },
            Err(e) => UpdateCompletePayload {
                overall: "failed".to_string(),
                summary: UpdateRunSummary {
                    repo: "failed".to_string(),
                    aur: "skipped".to_string(),
                    flatpak: "skipped".to_string(),
                    succeeded_packages: Vec::new(),
                    failed_packages: vec![UpdateFailedPackage {
                        name: "system".to_string(),
                        source: "repo".to_string(),
                        reason: e.clone(),
                    }],
                    warnings: Vec::new(),
                    duration_ms: 0,
                },
                message: e,
            },
        };
        invalidate_update_snapshot_cache();
        if payload.overall != "failed" {
            crate::commands::package::invalidate_installed_catalog_cache();
        }
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
) -> Result<UpdateExecutionResult, String> {
    let started_at = Instant::now();
    let mut summary = UpdateRunSummary {
        repo: "skipped".to_string(),
        aur: "skipped".to_string(),
        flatpak: "skipped".to_string(),
        succeeded_packages: Vec::new(),
        failed_packages: Vec::new(),
        warnings: Vec::new(),
        duration_ms: 0,
    };

    // Phase 1: Sanity Check (Ping)
    emit_source_progress(&app, "repo", "preparing", 0, 100, None);
    let _ = app.emit("update-status", "Preparing update...");

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
        let message = "OFFLINE: Cannot perform update without internet connectivity.".to_string();
        summary.repo = "failed".to_string();
        summary.failed_packages.push(UpdateFailedPackage {
            name: "system".to_string(),
            source: "repo".to_string(),
            reason: message.clone(),
        });
        summary.duration_ms = started_at.elapsed().as_millis() as u64;
        let _ = app.emit(
            "update-progress",
            UpdateProgressPayload {
                phase: "error".to_string(),
                progress: 0,
                message: message.clone(),
            },
        );
        emit_source_progress(&app, "repo", "failed", 0, 100, None);
        return Ok(UpdateExecutionResult {
            overall: "failed".to_string(),
            summary,
            message,
        });
    }

    // Phase 2: Full System Upgrade (SINGLE TRANSACTION via ALPM)
    let _ = app.emit("update-status", "Updating system packages...");
    let _ = app.emit(
        "update-progress",
        UpdateProgressPayload {
            phase: "refresh".to_string(),
            progress: 0,
            message: "Synchronizing databases...".to_string(),
        },
    );
    emit_source_progress(&app, "repo", "synchronizing", 0, 100, None);

    log::info!("Update: running ALPM system upgrade transaction");

    let mut rx = match crate::helper_client::invoke_helper(
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
    .await
    {
        Ok(rx) => rx,
        Err(e) => {
            summary.repo = "failed".to_string();
            summary.failed_packages.push(UpdateFailedPackage {
                name: "system".to_string(),
                source: "repo".to_string(),
                reason: e.clone(),
            });
            summary.duration_ms = started_at.elapsed().as_millis() as u64;
            emit_source_progress(&app, "repo", "failed", 0, 100, None);
            return Ok(UpdateExecutionResult {
                overall: "failed".to_string(),
                summary,
                message: e,
            });
        }
    };

    // Tell the user to look for the Polkit/auth dialog so the app doesn't appear frozen.
    let _ = app.emit("update-status", "Waiting for authentication...");
    emit_source_progress(&app, "repo", "authenticating", 0, 100, None);

    // Use timeout so we can remind the user every 45s if still waiting (e.g. password dialog behind other windows).
    let mut sysupgrade_failed = false;
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(45), rx.recv()).await {
            Ok(Some(msg)) => {
                let _ = app.emit("update-status", &msg.message);
                emit_source_progress(
                    &app,
                    "repo",
                    "upgrading",
                    msg.progress as usize,
                    100,
                    None,
                );

                // Detect critical failure messages (helper sends "Error: ..." via progress message)
                if msg.message.starts_with("Error:")
                    || msg.message.contains("Transaction preparation failed")
                {
                    sysupgrade_failed = true;
                    let _ = app.emit(
                        "install-output",
                        &format!("CRITICAL: System update failed: {}", msg.message),
                    );
                } else if !msg.is_structured {
                    let _ = app.emit("install-output", &msg.message);
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
        summary.repo = "failed".to_string();
        let msg = "System update failed. Aborting AUR/Flatpak updates to prevent partial upgrade state.";
        summary.failed_packages.push(UpdateFailedPackage {
            name: "system".to_string(),
            source: "repo".to_string(),
            reason: msg.to_string(),
        });
        summary.duration_ms = started_at.elapsed().as_millis() as u64;
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
        emit_source_progress(&app, "repo", "failed", 0, 100, None);
        return Ok(UpdateExecutionResult {
            overall: "failed".to_string(),
            summary,
            message: msg.to_string(),
        });
    }
    summary.repo = "success".to_string();
    emit_source_progress(&app, "repo", "complete", 100, 100, None);

    // Phase 3: AUR Batch (only when user has AUR included in updates)
    let aur_updates = if include_aur {
        let _ = app.emit("update-status", "Checking for AUR updates...");
        emit_source_progress(&app, "aur", "checking", 0, 1, None);
        let _ = app.emit(
            "update-progress",
            UpdateProgressPayload {
                phase: "upgrade".to_string(),
                progress: 100,
                message: "System upgrade complete.".to_string(),
            },
        );
        match check_aur_updates().await {
            Ok(items) => items,
            Err(e) => {
                let warn = format!("AUR update check failed: {}", e);
                let _ = app.emit("install-output", &warn);
                summary.warnings.push(warn.clone());
                summary.failed_packages.push(UpdateFailedPackage {
                    name: "aur-check".to_string(),
                    source: "aur".to_string(),
                    reason: e,
                });
                summary.aur = "failed".to_string();
                emit_source_progress(&app, "aur", "failed", 0, 1, None);
                Vec::new()
            }
        }
    } else {
        let _ = app.emit("update-status", "Skipping AUR (disabled in update scope).");
        summary.aur = "skipped".to_string();
        emit_source_progress(&app, "aur", "skipped", 0, 0, None);
        Vec::new()
    };

    if aur_updates.is_empty() && include_aur
        && summary.aur != "failed" {
            summary.aur = "skipped".to_string();
            let _ = app.emit("update-status", "No AUR updates found.");
            emit_source_progress(&app, "aur", "complete", 0, 0, None);
        }
    if !aur_updates.is_empty() {
        let aur_total = aur_updates.len();
        let mut aur_succeeded_names: Vec<String> = Vec::new();
        let mut aur_failed = 0usize;
        let mut built_packages = Vec::new();
        let mut built_package_names = Vec::new();
        let _ = app.emit(
            "update-status",
            format!("Building community packages (AUR): {}...", aur_total),
        );
        let _ = app.emit(
            "update-progress",
            UpdateProgressPayload {
                phase: "aur".to_string(),
                progress: 0,
                message: format!("Building community packages (AUR): {}...", aur_total),
            },
        );
        emit_source_progress(&app, "aur", "building", 0, aur_total, None);

        for (idx, pkg) in aur_updates.into_iter().enumerate() {
            let _ = app.emit("update-status", format!("Building {}...", pkg.name));
            emit_source_progress(
                &app,
                "aur",
                "building",
                idx + 1,
                aur_total,
                Some(pkg.name.clone()),
            );

            match build_aur_package(&pkg.name, &app, &password).await {
                Ok(paths) => {
                    built_package_names.push(pkg.name.clone());
                    built_packages.extend(paths);
                }
                Err(e) => {
                    aur_failed += 1;
                    summary.failed_packages.push(UpdateFailedPackage {
                        name: pkg.name.clone(),
                        source: "aur".to_string(),
                        reason: e.clone(),
                    });
                    let _ = app.emit(
                        "install-output",
                        format!("Warning: Failed to build {}: {}. Skipping...", pkg.name, e),
                    );
                    summary.warnings.push(format!("AUR build failed: {} ({})", pkg.name, e));
                }
            }
        }

        if !built_packages.is_empty() {
            let _ = app.emit("update-status", "Installing built AUR packages...");
            emit_source_progress(
                &app,
                "aur",
                "installing",
                built_package_names.len(),
                aur_total,
                None,
            );

            match install_built_packages(built_packages, &password, &app, one_click).await {
                Ok(()) => {
                    aur_succeeded_names.extend(built_package_names.iter().cloned());
                }
                Err(e) => {
                    for name in built_package_names {
                        summary.failed_packages.push(UpdateFailedPackage {
                            name,
                            source: "aur".to_string(),
                            reason: format!("Install failed: {}", e),
                        });
                        aur_failed += 1;
                    }
                    summary.warnings.push(format!("AUR install phase failed: {}", e));
                    let _ = app.emit(
                        "install-output",
                        format!("Warning: AUR install phase failed: {}", e),
                    );
                }
            }
        }

        summary.succeeded_packages.extend(aur_succeeded_names);
        let aur_success_count = aur_total.saturating_sub(aur_failed);
        summary.aur = if aur_failed == 0 {
            "success".to_string()
        } else if aur_success_count > 0 {
            "partial".to_string()
        } else {
            "failed".to_string()
        };
        emit_source_progress(
            &app,
            "aur",
            if summary.aur == "failed" {
                "failed"
            } else {
                "complete"
            },
            aur_success_count,
            aur_total,
            None,
        );
    }

    // Phase 4: Flatpak Updates (only when user has Flatpak included in updates)
    if include_flatpak {
        let _ = app.emit("update-status", "Updating Flatpak apps...");
        let _ = app.emit(
            "update-progress",
            UpdateProgressPayload {
                phase: "flatpak".to_string(),
                progress: 80,
                message: "Checking Flatpak updates...".to_string(),
            },
        );
        emit_source_progress(&app, "flatpak", "checking", 0, 1, None);

        match crate::flathub_api::get_updates().await {
            Ok(flatpak_updates) => {
                if flatpak_updates.is_empty() {
                    summary.flatpak = "skipped".to_string();
                    let _ = app.emit("update-status", "No Flatpak updates found.");
                    let _ = app.emit("install-output", "No Flatpak updates available.");
                    emit_source_progress(&app, "flatpak", "complete", 0, 0, None);
                } else {
                    let total = flatpak_updates.len();
                    let mut success_count = 0usize;
                    let mut failed_count = 0usize;
                    let _ = app.emit(
                        "update-status",
                        format!("Updating Flatpak apps: {}...", total),
                    );
                    let _ = app.emit(
                        "install-output",
                        format!("Found {} Flatpak updates", total),
                    );
                    emit_source_progress(&app, "flatpak", "updating", 0, total, None);

                    for (idx, item) in flatpak_updates.into_iter().enumerate() {
                        let _ = app.emit(
                            "update-status",
                            format!("Updating Flatpak: {}...", item.name),
                        );
                        let _ =
                            app.emit("install-output", format!("Updating Flatpak: {}", item.name));
                        emit_source_progress(
                            &app,
                            "flatpak",
                            "updating",
                            idx + 1,
                            total,
                            Some(item.name.clone()),
                        );

                        if let Err(e) =
                            crate::flathub_api::update_flatpak(app.clone(), item.name.clone()).await
                        {
                            failed_count += 1;
                            summary.failed_packages.push(UpdateFailedPackage {
                                name: item.name.clone(),
                                source: "flatpak".to_string(),
                                reason: e.clone(),
                            });
                            summary
                                .warnings
                                .push(format!("Flatpak update failed: {} ({})", item.name, e));
                            let _ = app.emit(
                                "install-output",
                                format!("Flatpak update warning: {} - {}", item.name, e),
                            );
                            // Continue with other Flatpaks - don't abort the whole update
                        } else {
                            success_count += 1;
                            summary.succeeded_packages.push(item.name.clone());
                        }
                    }

                    summary.flatpak = if failed_count == 0 {
                        "success".to_string()
                    } else if success_count > 0 {
                        "partial".to_string()
                    } else {
                        "failed".to_string()
                    };
                    let _ = app.emit("update-status", "Flatpak updates completed.");
                    emit_source_progress(
                        &app,
                        "flatpak",
                        if summary.flatpak == "failed" {
                            "failed"
                        } else {
                            "complete"
                        },
                        success_count,
                        total,
                        None,
                    );
                }
            }
            Err(e) => {
                // Flatpak errors are non-critical - log but don't fail the update
                summary.flatpak = "failed".to_string();
                summary.failed_packages.push(UpdateFailedPackage {
                    name: "flatpak-check".to_string(),
                    source: "flatpak".to_string(),
                    reason: e.clone(),
                });
                summary.warnings.push(format!("Flatpak check failed: {}", e));
                let _ = app.emit("install-output", format!("Flatpak check skipped: {}", e));
                emit_source_progress(&app, "flatpak", "failed", 0, 1, None);
            }
        }
    } else {
        summary.flatpak = "skipped".to_string();
        let _ = app.emit(
            "update-status",
            "Skipping Flatpak (disabled in update scope).",
        );
        emit_source_progress(&app, "flatpak", "skipped", 0, 0, None);
    }

    let overall = if summary.repo == "failed" {
        "failed".to_string()
    } else if summary.aur == "failed"
        || summary.aur == "partial"
        || summary.flatpak == "failed"
        || summary.flatpak == "partial"
        || !summary.failed_packages.is_empty()
    {
        "partial".to_string()
    } else {
        "success".to_string()
    };

    let final_message = match overall.as_str() {
        "success" => "All updates completed successfully.".to_string(),
        "partial" => "Updates completed with some issues. Review failed items.".to_string(),
        _ => "Update failed.".to_string(),
    };

    let _ = app.emit("update-status", &final_message);
    let _ = app.emit(
        "update-progress",
        UpdateProgressPayload {
            phase: if overall == "failed" {
                "error".to_string()
            } else {
                "complete".to_string()
            },
            progress: if overall == "failed" { 0 } else { 100 },
            message: final_message.clone(),
        },
    );
    summary.duration_ms = started_at.elapsed().as_millis() as u64;
    Ok(UpdateExecutionResult {
        overall,
        summary,
        message: final_message,
    })
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
    Ok(check_updates_inner(state_registry.inner(), include_aur, include_flatpak).await)
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
    let started_at = Instant::now();
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

    let mut summary = UpdateRunSummary {
        repo: "skipped".into(),
        aur: "skipped".into(),
        flatpak: "skipped".into(),
        succeeded_packages: Vec::new(),
        failed_packages: Vec::new(),
        warnings: Vec::new(),
        duration_ms: 0,
    };

    // 1. Execute Repo Loop (The Iron Core)
    let mut sysupgrade_failed = false;
    if has_official {
        emit_source_progress(&app, "repo", "synchronizing", 0, 100, None);
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
            emit_source_progress(&app, "repo", "upgrading", msg.progress as usize, 100, None);
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
                    overall: "failed".into(),
                    summary: UpdateRunSummary {
                        repo: "failed".into(),
                        aur: "skipped".into(),
                        flatpak: "skipped".into(),
                        succeeded_packages: Vec::new(),
                        failed_packages: vec![UpdateFailedPackage {
                            name: "system".into(),
                            source: "repo".into(),
                            reason: msg.into(),
                        }],
                        warnings: Vec::new(),
                        duration_ms: 0,
                    },
                    message: msg.into(),
                },
            );
            emit_source_progress(&app, "repo", "failed", 0, 100, None);
            return Err(msg.to_string());
        }
        summary.repo = "success".into();
        emit_source_progress(&app, "repo", "complete", 100, 100, None);
    }

    // 2. Execute AUR Loop (Native Builder)
    if !aur_targets.is_empty() {
        let aur_total = aur_targets.len();
        let mut aur_failed = 0usize;
        let mut built_names: Vec<String> = Vec::new();
        let _ = app.emit(
            "update-status",
            format!("Processing {} AUR updates...", aur_total),
        );
        emit_source_progress(&app, "aur", "building", 0, aur_total, None);
        let mut built_paths = Vec::new();

        for (idx, item) in aur_targets.iter().enumerate() {
            let _ = app.emit("update-status", format!("Building {}...", item.name));
            emit_source_progress(
                &app,
                "aur",
                "building",
                idx + 1,
                aur_total,
                Some(item.name.clone()),
            );
            match build_aur_package(&item.name, &app, &password).await {
                Ok(paths) => {
                    built_names.push(item.name.clone());
                    built_paths.extend(paths);
                }
                Err(e) => {
                    aur_failed += 1;
                    summary.failed_packages.push(UpdateFailedPackage {
                        name: item.name.clone(),
                        source: "aur".into(),
                        reason: e.clone(),
                    });
                    let _ = app.emit(
                        "install-output",
                        format!("Failed to build {}: {}", item.name, e),
                    );
                    summary
                        .warnings
                        .push(format!("AUR build failed for {}: {}", item.name, e));
                }
            }
        }

        if !built_paths.is_empty() {
            let _ = app.emit("update-status", "Installing AUR packages...");
            emit_source_progress(&app, "aur", "installing", built_names.len(), aur_total, None);
            if let Err(e) = install_built_packages(built_paths, &password, &app, one_click).await {
                for name in built_names {
                    aur_failed += 1;
                    summary.failed_packages.push(UpdateFailedPackage {
                        name,
                        source: "aur".into(),
                        reason: format!("Install failed: {}", e),
                    });
                }
                summary
                    .warnings
                    .push(format!("AUR install phase failed: {}", e));
            } else {
                summary.succeeded_packages.extend(built_names);
            }
        }
        let aur_success = aur_total.saturating_sub(aur_failed);
        summary.aur = if aur_failed == 0 {
            "success".into()
        } else if aur_success > 0 {
            "partial".into()
        } else {
            "failed".into()
        };
        emit_source_progress(
            &app,
            "aur",
            if summary.aur == "failed" { "failed" } else { "complete" },
            aur_success,
            aur_total,
            None,
        );
    }

    // 3. Execute Flatpak Loop (Safety Net)
    if !flatpak_targets.is_empty() {
        let total = flatpak_targets.len();
        let mut failed = 0usize;
        let _ = app.emit(
            "update-status",
            format!("Updating {} Flatpaks...", total),
        );
        emit_source_progress(&app, "flatpak", "updating", 0, total, None);
        for (idx, item) in flatpak_targets.iter().enumerate() {
            // item.name should be App ID based on our flathub_api.rs change.
            let _ = app.emit("install-output", format!("Updating Flatpak: {}", item.name));
            emit_source_progress(
                &app,
                "flatpak",
                "updating",
                idx + 1,
                total,
                Some(item.name.clone()),
            );

            // Call flatpak update <id> -y
            // We can implement a helper or call Command direct.
            if let Err(e) = crate::flathub_api::update_flatpak(app.clone(), item.name.clone()).await
            {
                failed += 1;
                summary.failed_packages.push(UpdateFailedPackage {
                    name: item.name.clone(),
                    source: "flatpak".into(),
                    reason: e.clone(),
                });
                summary
                    .warnings
                    .push(format!("Flatpak update failed for {}: {}", item.name, e));
                let _ = app.emit("install-output", format!("Flatpak update error: {}", e));
            } else {
                summary.succeeded_packages.push(item.name.clone());
            }
        }
        let success = total.saturating_sub(failed);
        summary.flatpak = if failed == 0 {
            "success".into()
        } else if success > 0 {
            "partial".into()
        } else {
            "failed".into()
        };
        emit_source_progress(
            &app,
            "flatpak",
            if summary.flatpak == "failed" { "failed" } else { "complete" },
            success,
            total,
            None,
        );
    }

    let overall = if summary.repo == "failed" {
        "failed".to_string()
    } else if summary.aur == "failed"
        || summary.aur == "partial"
        || summary.flatpak == "failed"
        || summary.flatpak == "partial"
        || !summary.failed_packages.is_empty()
    {
        "partial".to_string()
    } else {
        "success".to_string()
    };
    let final_message = match overall.as_str() {
        "success" => "All selected updates applied.".to_string(),
        "partial" => "Selected updates completed with some issues.".to_string(),
        _ => "Selected update run failed.".to_string(),
    };
    summary.duration_ms = started_at.elapsed().as_millis() as u64;

    let _ = app.emit("update-status", &final_message);
    let _ = app.emit(
        "update-complete",
        UpdateCompletePayload {
            overall: overall.clone(),
            summary,
            message: final_message,
        },
    );

    Ok(if overall == "success" {
        "Updates applied".to_string()
    } else {
        "Updates finished with warnings".to_string()
    })
}

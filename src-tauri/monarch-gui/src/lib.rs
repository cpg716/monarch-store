pub(crate) mod alpm_progress;
pub(crate) mod alpm_read;
pub(crate) mod labels;
pub(crate) mod aur_api;
pub(crate) mod chaotic_api;
pub(crate) mod commands;
pub(crate) mod distro_context;
pub(crate) mod error_classifier;
pub(crate) mod flathub_api;
pub(crate) mod helper_client;
pub(crate) mod metadata;
pub(crate) mod models;
pub(crate) mod odrs_api;
pub(crate) mod pkgstats_api;
pub(crate) mod repair;
pub(crate) mod repo_db;
pub(crate) mod repo_manager;
pub(crate) mod scm_api;
pub(crate) mod utils;

#[cfg(test)]
mod tests;

use chaotic_api::ChaoticApiClient;
use repo_manager::RepoManager;
use tauri::{Emitter, Manager};

pub struct ScmState(pub scm_api::ScmClient);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_aptabase::Builder::new("A-US-1496058535")
                .with_panic_hook(Box::new(|client, info, msg| {
                    let location = info
                        .location()
                        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                        .unwrap_or_else(|| "unknown".to_string());
                    let _ = client.track_event(
                        "panic",
                        Some(serde_json::json!({
                            "event_category": "error",
                            "event_label": "App panic",
                            "message": msg,
                            "location": location,
                        })),
                    );
                }))
                .build(),
        )
        .manage(RepoManager::new())
        .manage(ChaoticApiClient::new())
        .manage(flathub_api::FlathubApiClient::new()) // ENRICHMENT: Metadata Fallback Active
        .manage(metadata::MetadataState(std::sync::Mutex::new(
            metadata::AppStreamLoader::new(),
        )))
        .manage(ScmState(scm_api::ScmClient::new()))
        .manage(distro_context::get_distro_context()) // Operation True Identity: Shared Context
        .setup(|app| {
            let handle = app.handle().clone();

            // v0.2.40: RUNTIME REQUIREMENT CHECK
            // Prevent silent crashes if the PKGBUILD failed us.
            let required_bins = vec!["git", "checkupdates", "pkexec"];
            for bin in required_bins {
                if which::which(bin).is_err() {
                    log::error!("CRITICAL: Runtime dependency '{}' is missing!", bin);
                    // We can't use toast yet as frontend isn't ready. Polling later handles this.
                }
            }

            tauri::async_runtime::spawn(async move {
                {
                    // Use the safe tracker to respect user consent
                    crate::utils::track_event_safe(&handle, "app_started", None).await;
                }

                let state_repo = handle.state::<RepoManager>();
                let _state_chaotic = handle.state::<ChaoticApiClient>();

                // Fast load from disk first (Non-blocking)
                state_repo.load_initial_cache().await;

                // metadata init is fine as it's separate
                let state_meta = handle.state::<metadata::MetadataState>();
                state_meta.init(24).await;
            });

            // Phase 2: The Chameleon (Cross-DE GUI)
            // 2. Ghost Protocol: Wayland Detection
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                log::info!(
                    "Wayland Detected (Ghost Protocol): Disabling transparency specific artifacts."
                );
                if let Some(window) = app.get_webview_window("main") {
                    // On Wayland (+Nvidia/KDE), transparency can cause black flickering.
                    // We forcibly disable it to ensure solidity.
                    // Note: set_shadow(false) often helps too.
                    let _ = window.set_shadow(false);
                    // Verify if set_transparent is exposed/needed.
                    // Usually handled by config, but explicit disable is safe.
                    // window.set_transparent(false) // API check needed.
                }
            }

            // 1. Native Dark Mode (Portals)
            let handle_theme = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                #[cfg(target_os = "linux")]
                {
                    use ashpd::desktop::settings::Settings;
                    log::info!("Initializing Portal Theme Detection...");
                    match Settings::new().await {
                        Ok(proxy) => {
                            // namespace: org.freedesktop.appearance, key: color-scheme
                            // 0: No pref, 1: Dark, 2: Light
                            match proxy
                                .read::<u8>("org.freedesktop.appearance", "color-scheme")
                                .await
                            {
                                Ok(scheme) => {
                                    let mode = match scheme {
                                        1 => "dark",
                                        2 => "light",
                                        _ => "auto",
                                    };
                                    log::info!("Portal Theme Detected: {}", mode);
                                    let _ = handle_theme.emit("system-theme-changed", mode);
                                }
                                Err(e) => log::warn!("Failed to read Portal theme: {}", e),
                            }
                        }
                        Err(e) => log::warn!("Failed to connect to Settings Portal: {}", e),
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Search Commands
            commands::search::search_aur,
            commands::search::search_packages,
            commands::search::get_packages_by_names,
            commands::search::get_chaotic_package_info,
            commands::search::get_chaotic_packages_batch,
            commands::search::get_trending,
            commands::search::get_package_variants,
            commands::search::get_category_packages_paginated,
            // Package Commands
            commands::package::install_package,
            commands::package::uninstall_package,
            commands::package::get_essentials_list,
            commands::package::abort_installation,
            commands::package::check_installed_status,
            commands::update::perform_system_update,
            commands::update::get_system_update_command,
            commands::update::check_updates,
            commands::update::apply_updates,
            commands::package::fetch_pkgbuild,
            commands::package::get_installed_packages,
            commands::package::check_for_updates,
            commands::package::check_reboot_required,
            commands::package::get_pacnew_warnings,
            commands::package::get_orphans,
            commands::package::remove_orphans,
            commands::system::get_cache_size,
            commands::system::get_orphans_with_size,
            commands::system::set_parallel_downloads,
            commands::system::get_mirror_rank_tool,
            commands::system::rank_mirrors,
            commands::system::test_mirrors,
            commands::system::force_refresh_databases,
            repo_manager::check_repo_sync_status,
            // Package Commands
            // System Commands
            commands::system::get_system_info,
            commands::system::get_infra_stats,
            commands::system::get_repo_counts,
            commands::system::get_repo_states,
            commands::system::is_aur_enabled,
            commands::system::toggle_repo,
            commands::system::toggle_repo_family,
            commands::system::set_aur_enabled,
            commands::system::is_one_click_enabled,
            commands::system::set_one_click_enabled,
            commands::system::is_advanced_mode,
            commands::system::set_advanced_mode,
            commands::system::check_security_policy,
            commands::system::install_monarch_policy,
            commands::system::optimize_system,
            commands::system::get_all_installed_names, // Smart Curation
            repair::fix_keyring_issues,
            repair::repair_reset_keyring,
            commands::system::trigger_repo_sync,
            commands::system::sync_system_databases,
            commands::system::update_and_install_package,
            commands::system::check_app_update,
            commands::system::get_install_mode_command,
            commands::system::is_telemetry_enabled,
            commands::system::is_notifications_enabled,
            commands::system::set_notifications_enabled,
            commands::system::set_telemetry_enabled,
            commands::system::is_sync_on_startup_enabled,
            commands::system::set_sync_on_startup_enabled,
            commands::system::check_and_clear_refresh_requested,
            // Utils Commands
            commands::utils::get_package_icon,
            commands::utils::clear_cache,
            commands::utils::launch_app,
            commands::utils::track_event,
            // External Module Commands (Pre-refactor)
            metadata::get_metadata,
            metadata::get_metadata_batch,
            repair::check_system_health,
            repair::check_initialization_status,
            repair::clear_sync_db_health_cache,
            repair::get_last_sync_age_seconds,
            commands::reviews::submit_review,
            commands::reviews::get_local_reviews,
            odrs_api::get_app_rating,
            odrs_api::get_app_ratings_batch,
            odrs_api::get_app_reviews,
            repair::cancel_install,
            repair::repair_unlock_pacman,
            repair::check_keyring_health,
            repair::repair_emergency_sync,
            repair::check_pacman_lock,
            repair::needs_startup_unlock,
            repair::unlock_pacman_if_stale,
            repair::clear_pacman_package_cache,
            repair::fix_keyring_issues_alias,
            repair::clear_build_cache,
            repo_manager::apply_os_config,
            commands::system::emit_sync_progress,
            // Identity Matrix Command
            distro_context::get_distro_context,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run({
            use std::sync::Mutex;
            use tauri::RunEvent;
            use tauri::WindowEvent;
            let windows_icon_set: Mutex<std::collections::HashSet<String>> =
                Mutex::new(std::collections::HashSet::new());
            move |app_handle, event| match &event {
                RunEvent::Ready => {
                    if let Some(icon) = app_handle.default_window_icon() {
                        for (label, win) in app_handle.webview_windows() {
                            let _ = win.set_icon(icon.clone());
                            let _ = windows_icon_set
                                .lock()
                                .map(|mut s| s.insert(label.to_string()));
                        }
                    }
                }
                RunEvent::WindowEvent { label, event, .. } => {
                    if matches!(event, WindowEvent::Resized(_) | WindowEvent::Focused(_)) {
                        if let Ok(set) = windows_icon_set.lock() {
                            if !set.contains(label) {
                                drop(set);
                                if let Some(icon) = app_handle.default_window_icon() {
                                    if let Some(win) = app_handle.get_webview_window(label) {
                                        let _ = win.set_icon(icon.clone());
                                        let _ = windows_icon_set
                                            .lock()
                                            .map(|mut s| s.insert(label.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
                RunEvent::Exit => {
                    log::info!("App exiting");
                }
                _ => {}
            }
        });
}

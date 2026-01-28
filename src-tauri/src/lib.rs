mod aur_api;
mod chaotic_api;
mod commands;
mod distro_context;
mod flathub_api;
mod metadata;
mod models;
mod odrs_api;
mod pkgstats_api;
mod repair;
mod repo_db;
mod repo_manager;
mod repo_setup;
mod scm_api;
mod utils;

#[cfg(test)]
mod tests;

use chaotic_api::ChaoticApiClient;
use repo_manager::RepoManager;
use tauri::Manager;

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
            tauri_plugin_aptabase::Builder::new("A-EU-3907248034")
                .with_panic_hook(Box::new(|client, info, msg| {
                    let location = info
                        .location()
                        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                        .unwrap_or_else(|| "unknown".to_string());
                    let _ = client.track_event(
                        "panic",
                        Some(serde_json::json!({
                            "message": msg,
                            "location": location
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
        .setup(|app| {
            let handle = app.handle().clone();

            // v0.2.40: RUNTIME REQUIREMENT CHECK
            // Prevent silent crashes if the PKGBUILD failed us.
            let required_bins = vec!["git", "checkupdates", "pkexec"];
            for bin in required_bins {
                if which::which(bin).is_err() {
                    eprintln!("CRITICAL ERROR: Runtime dependency '{}' is missing!", bin);
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Search Commands
            commands::search::search_aur,
            commands::search::search_packages,
            commands::search::get_packages_by_names,
            commands::search::get_trending,
            commands::search::get_package_variants,
            commands::search::get_category_packages_paginated,
            // Package Commands
            commands::package::install_package,
            commands::package::uninstall_package,
            commands::package::get_essentials_list,
            commands::package::abort_installation,
            commands::package::check_installed_status,
            commands::package::perform_system_update,
            commands::package::fetch_pkgbuild,
            commands::package::get_installed_packages,
            commands::package::check_for_updates,
            commands::package::get_orphans,
            commands::package::remove_orphans,
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
            commands::system::trigger_repo_sync,
            commands::system::update_and_install_package,
            commands::system::check_app_update,
            commands::system::get_install_mode_command,
            commands::system::is_telemetry_enabled,
            commands::system::set_telemetry_enabled,
            // Utils Commands
            commands::utils::get_package_icon,
            commands::utils::clear_cache,
            commands::utils::launch_app,
            // External Module Commands (Pre-refactor)
            metadata::get_metadata,
            metadata::get_metadata_batch,
            metadata::check_system_health,
            metadata::check_initialization_status,
            commands::reviews::submit_review,
            commands::reviews::get_local_reviews,
            odrs_api::get_app_rating,
            odrs_api::get_app_ratings_batch,
            odrs_api::get_app_reviews,
            repair::repair_unlock_pacman,
            repair::check_keyring_health,
            repair::repair_emergency_sync,
            repair::check_pacman_lock,
            repair::initialize_system,
            repo_setup::bootstrap_system,
            repo_setup::enable_repos_batch,
            repo_setup::enable_repo,
            repo_setup::reset_pacman_conf,
            repo_setup::set_repo_priority,
            repo_setup::check_repo_status,
            repo_setup::set_one_click_control,
            // Identity Matrix Command
            distro_context::get_distro_context,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_handler, event| match event {
            tauri::RunEvent::Exit { .. } => {
                println!("App Exiting...");
            }
            _ => {}
        });
}

pub(crate) mod alpm_progress;
pub(crate) mod alpm_read;
pub(crate) mod aur_api;
pub(crate) mod chaotic_api;
pub(crate) mod commands;
pub(crate) mod constants;
pub(crate) mod discovery_manager;
pub(crate) mod distro_context;
pub(crate) mod error_classifier;
pub(crate) mod flathub_api;
pub(crate) mod helper_client;
pub(crate) mod labels;
pub(crate) mod metadata;
pub(crate) mod middleware;
pub(crate) mod models;
pub(crate) mod odrs_api;
pub(crate) mod pkgstats_api;
pub(crate) mod registry;
pub(crate) mod repair;
pub(crate) mod repo_db;
pub(crate) mod repo_manager;
pub(crate) mod scm_api;
#[cfg(debug_assertions)]
pub(crate) mod specta_gen;
pub(crate) mod utils;

#[cfg(test)]
mod tests;

use chaotic_api::ChaoticApiClient;
use repo_manager::RepoManager;
use tauri::{Emitter, Manager};

pub struct ScmState(pub scm_api::ScmClient);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logger so RUST_LOG=debug (or monarch_store=debug) shows [CARD/DETAILS] and other log output.
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();

    // Log panic message and location so terminal shows real cause (Tokio task panics often only show "scheduler line 88" otherwise).
    std::panic::set_hook(Box::new(move |info| {
        let msg = {
            let payload = info.payload();
            if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                format!("{:?}", payload)
            }
        };
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());
        eprintln!("[monarch-store] PANIC: {} at {}", msg, loc);
        log::error!("PANIC: {} at {}", msg, loc);
        if std::env::var("RUST_BACKTRACE").is_ok() {
            eprintln!("{}", std::backtrace::Backtrace::capture());
        }
    }));

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
        .manage(discovery_manager::DiscoveryManager::new())
        .manage(metadata::MetadataState::new())
        .manage(ScmState(scm_api::ScmClient::new()))
        .manage(distro_context::get_distro_context()) // Operation True Identity: Shared Context
        .manage(registry::RegistryState::new())
        .setup(|app| {
            let handle = app.handle().clone();

            #[cfg(debug_assertions)]
            {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap() // src-tauri
                    .parent()
                    .unwrap() // monarch-store
                    .join("src/services/bindings.ts");

                match crate::specta_gen::builder().export(
                    specta_typescript::Typescript::default()
                        .bigint(specta_typescript::BigIntExportBehavior::String),
                    path,
                ) {
                    Ok(_) => log::info!("Specta bindings exported successfully"),
                    Err(e) => log::error!("Failed to export specta bindings: {}", e),
                }
            }

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
                let state_discovery = handle.state::<discovery_manager::DiscoveryManager>();
                let state_meta = handle.state::<metadata::MetadataState>();

                // Run discovery and metadata in parallel so both are ready sooner for Trending + Categories
                state_discovery.load_from_disk();
                state_discovery.refresh_if_stale();

                // 1. Critical Phase: Repo Cache + AppStream Init
                let ((), ()) = tokio::join!(
                    async {
                        state_repo.load_initial_cache().await;
                    },
                    async {
                        state_meta.init(24).await;
                    }
                );

                // 2. Background Phase: Registry Actor + Sync (Detached)
                let state_registry = handle.state::<registry::RegistryState>();
                state_registry.spawn_actor(handle.clone());

                let handle_clone = handle.clone();
                tauri::async_runtime::spawn(async move {
                    {
                        let state_meta = handle_clone.state::<metadata::MetadataState>();
                        let state_repo = handle_clone.state::<RepoManager>();
                        let state_chaotic = handle_clone.state::<chaotic_api::ChaoticApiClient>();
                        let state_flathub =
                            handle_clone.state::<flathub_api::FlathubApiClient>();
                        let state_discovery =
                            handle_clone.state::<discovery_manager::DiscoveryManager>();
                        let state_registry = handle_clone.state::<registry::RegistryState>();

                        if let Err(e) = crate::commands::search::build_discovery_home_snapshot_impl(
                            state_meta.inner(),
                            state_chaotic.inner(),
                            state_repo.inner(),
                            state_flathub.inner(),
                            state_discovery.inner(),
                            state_registry.inner(),
                        )
                        .await
                        {
                            log::warn!("[DISCOVERY] Startup home snapshot prewarm failed: {}", e);
                        } else {
                            log::info!("[DISCOVERY] Startup home snapshot prewarm complete");
                        }

                        crate::commands::search::prewarm_core_category_snapshots(
                            state_meta.inner(),
                            state_registry.inner(),
                        )
                        .await;

                        if let Err(e) = crate::commands::search::prewarm_search_snapshot(
                            state_registry.inner(),
                        )
                        .await
                        {
                            log::warn!("[SEARCH] Startup search snapshot prewarm failed: {}", e);
                        } else {
                            log::info!("[SEARCH] Startup search snapshot prewarm complete");
                        }
                    }

                    // Give the frontend 5 seconds to finish initial Essentials/Trending calls
                    // This prevents locking the Registry DB during the first frames of the app.
                    // tokio::time::sleep(std::time::Duration::from_secs(5)).await; // REMOVED: Iron Core Atomic Hydration makes this safe.

                    let state_meta = handle_clone.state::<metadata::MetadataState>();
                    let entries = {
                        let loader = match state_meta.loader.lock() {
                            Ok(l) => l,
                            Err(_) => return,
                        };
                        loader.get_all_entries_with_categories()
                    };

                    if !entries.is_empty() {
                        let handle_for_blocking = handle_clone.clone();
                        // Use spawn_blocking for the heavy SQL work so we don't stall the async executor
                        let _ = tokio::task::spawn_blocking(move || {
                            log::info!(
                                "[REGISTRY] Background AppStream sync starting ({} entries)",
                                entries.len()
                            );
                            let state_registry =
                                handle_for_blocking.state::<registry::RegistryState>();
                            if let Err(e) = state_registry.manager.sync_appstream_entries(entries) {
                                log::error!("[REGISTRY] AppStream sync failed: {}", e);
                            } else {
                                log::info!("[REGISTRY] AppStream sync complete.");
                                crate::commands::search::invalidate_runtime_search_caches();
                                state_registry.manager.trigger_bulk_sync();
                            }
                        })
                        .await;

                        let state_meta = handle_clone.state::<metadata::MetadataState>();
                        let state_repo = handle_clone.state::<RepoManager>();
                        let state_chaotic = handle_clone.state::<chaotic_api::ChaoticApiClient>();
                        let state_flathub =
                            handle_clone.state::<flathub_api::FlathubApiClient>();
                        let state_discovery =
                            handle_clone.state::<discovery_manager::DiscoveryManager>();
                        let state_registry = handle_clone.state::<registry::RegistryState>();

                        let _ = crate::commands::search::build_discovery_home_snapshot_impl(
                            state_meta.inner(),
                            state_chaotic.inner(),
                            state_repo.inner(),
                            state_flathub.inner(),
                            state_discovery.inner(),
                            state_registry.inner(),
                        )
                        .await;
                        crate::commands::search::prewarm_core_category_snapshots(
                            state_meta.inner(),
                            state_registry.inner(),
                        )
                        .await;
                        let _ =
                            crate::commands::search::prewarm_search_snapshot(state_registry.inner())
                                .await;
                    }

                    // v0.2.41+: lighter startup warmup
                    // Seed a smaller slice of popular items so cold boot stays responsive.
                    let state_discovery =
                        handle_clone.state::<discovery_manager::DiscoveryManager>();
                    let mut discovery_names = state_discovery.inner().get_all_popular_names().await;
                    discovery_names.truncate(120);

                    if !discovery_names.is_empty() {
                        log::info!(
                            "[WARMUP] Enriching Registry with {} discovery items...",
                            discovery_names.len()
                        );
                        let items: Vec<(String, Option<String>)> =
                            discovery_names.into_iter().map(|n| (n, None)).collect();

                        let state_meta = handle_clone.state::<metadata::MetadataState>();
                        let state_repo = handle_clone.state::<RepoManager>();
                        let state_chaotic = handle_clone.state::<chaotic_api::ChaoticApiClient>();
                        let state_flathub = handle_clone.state::<flathub_api::FlathubApiClient>();
                        let state_registry = handle_clone.state::<registry::RegistryState>();

                        if let Ok(pkgs) =
                            crate::middleware::aggregation::fetch_and_merge_packages_by_names_impl(
                                &state_meta,
                                &state_chaotic,
                                &state_repo,
                                &state_flathub,
                                &state_registry.manager,
                                items,
                                false, // defer Flatpak-heavy enrichment until on-demand
                                false, // defer AUR-heavy enrichment until on-demand
                                true,  // include_chaotic
                                false, // installed_lookup
                            )
                            .await
                        {
                            log::info!(
                                "[REGISTRY] Warmup complete, upserting {} packages",
                                pkgs.len()
                            );
                            let warm_entries = {
                                let loader = match state_meta.loader.lock() {
                                    Ok(loader) => loader,
                                    Err(_) => {
                                        let _ = state_registry.manager.bulk_upsert_packages(&pkgs);
                                        state_registry.manager.trigger_bulk_sync();
                                        return;
                                    }
                                };

                                pkgs.into_iter()
                                    .map(|pkg| {
                                        let categories = loader.resolve_categories_for_package(
                                            &pkg.name,
                                            pkg.app_id.as_deref(),
                                        );
                                        (pkg, categories)
                                    })
                                    .collect::<Vec<_>>()
                            };
                            let _ = state_registry
                                .manager
                                .bulk_upsert_packages_with_categories(&warm_entries);
                            crate::commands::search::invalidate_runtime_search_caches();
                            let _ = crate::commands::search::build_discovery_home_snapshot_impl(
                                state_meta.inner(),
                                state_chaotic.inner(),
                                state_repo.inner(),
                                state_flathub.inner(),
                                state_discovery.inner(),
                                state_registry.inner(),
                            )
                            .await;
                            crate::commands::search::prewarm_core_category_snapshots(
                                state_meta.inner(),
                                state_registry.inner(),
                            )
                            .await;
                            let _ =
                                crate::commands::search::prewarm_search_snapshot(state_registry.inner())
                                    .await;
                            state_registry.manager.trigger_bulk_sync();
                        }
                    }
                });
            });

            // App Identity (Linux): Taskbar/dock icon handshake.
            // tauri.conf.json has app.enableGTKAppId: true and identifier: "com.monarch.store".
            // The Tauri runtime sets the window's GTK application ID to that identifier, so the DE
            // associates the window with monarch-store.desktop (StartupWMClass=com.monarch.store).
            // No X11-only hacks; works on Wayland and X11.

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
                            let mut scheme_value: Option<u8> = None;
                            if let Ok(scheme) = proxy
                                .read::<u32>("org.freedesktop.appearance", "color-scheme")
                                .await
                            {
                                scheme_value = Some(scheme as u8);
                            } else if let Ok(scheme) = proxy
                                .read::<u8>("org.freedesktop.appearance", "color-scheme")
                                .await
                            {
                                scheme_value = Some(scheme);
                            }

                            if let Some(scheme) = scheme_value {
                                let mode = match scheme {
                                    1 => "dark",
                                    2 => "light",
                                    _ => "auto",
                                };
                                log::info!("Portal Theme Detected: {}", mode);
                                let _ = handle_theme.emit("system-theme-changed", mode);
                            }

                            let rgb_opt = if let Ok(rgb) = proxy
                                .read::<(f64, f64, f64)>(
                                    "org.freedesktop.appearance",
                                    "accent-color",
                                )
                                .await
                            {
                                Some(rgb)
                            } else if let Ok(rgb) = proxy
                                .read::<Vec<f64>>(
                                    "org.freedesktop.appearance",
                                    "accent-color",
                                )
                                .await
                            {
                                if rgb.len() >= 3 {
                                    Some((rgb[0], rgb[1], rgb[2]))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            if let Some((r, g, b)) = rgb_opt {
                                let to_u8 = |v: f64| -> u8 {
                                    let clamped = v.clamp(0.0, 1.0);
                                    (clamped * 255.0).round() as u8
                                };
                                let hex = format!(
                                    "#{:02x}{:02x}{:02x}",
                                    to_u8(r),
                                    to_u8(g),
                                    to_u8(b)
                                );
                                log::info!("Portal Accent Detected: {}", hex);
                                let _ = handle_theme.emit("system-accent-changed", hex);
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
            commands::search::search_packages_rich,
            commands::search::get_packages_by_names,
            commands::search::get_packages_by_canonical_ids,
            commands::search::get_chaotic_package_info,
            commands::search::get_chaotic_packages_batch,
            commands::search::get_trending,
            commands::search::get_trending_snapshot,
            commands::search::get_essentials_snapshot,
            commands::search::get_discovery_home_snapshot,
            commands::search::get_package_variants,
            commands::search::get_category_packages_paginated,
            // Package Commands
            commands::package::install_package,
            commands::package::uninstall_package,
            commands::package::get_essentials_list,
            commands::package::abort_installation,
            commands::package::check_installed_status,
            commands::news::fetch_news,
            commands::update::perform_system_update,
            commands::update::get_system_update_command,
            commands::update::check_updates,
            commands::update::get_update_snapshot,
            commands::update::apply_updates,
            commands::package::fetch_pkgbuild,
            commands::package::get_installed_catalog,
            commands::package::get_installed_packages,
            commands::package::check_for_updates,
            commands::package::check_reboot_required,
            commands::package::get_pacnew_warnings,
            commands::package::get_orphans,
            commands::package::remove_orphans,
            commands::package::get_cache_stats,
            commands::package::clean_package_cache,
            commands::package::check_services_restart,
            commands::package::restart_service,
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
            commands::system::get_host_appearance,
            commands::system::get_infra_stats,
            commands::system::get_repo_counts,
            commands::system::get_repo_states,
            commands::system::check_chaotic_status,
            commands::system::get_snapshot_status,
            commands::system::create_system_snapshot,
            commands::system::prepare_chaotic_components,
            commands::system::open_chaotic_terminal,
            commands::system::prepare_flatpak,
            commands::system::ensure_flathub_remote,
            commands::system::is_aur_enabled,
            commands::system::toggle_repo,
            commands::system::toggle_repo_family,
            commands::system::set_aur_enabled,
            commands::system::is_one_click_enabled,
            commands::system::set_one_click_enabled,
            commands::system::is_advanced_mode,
            commands::system::set_advanced_mode,
            commands::system::get_missing_required_bins,
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
            commands::system::show_desktop_notification,
            commands::system::set_telemetry_enabled,
            commands::telemetry::track_telemetry_event,
            commands::system::is_sync_on_startup_enabled,
            commands::system::set_sync_on_startup_enabled,
            commands::system::check_and_clear_refresh_requested,
            commands::system::is_automatic_housekeeping_enabled,
            commands::system::set_automatic_housekeeping_enabled,
            commands::system::perform_housekeeping,
            commands::system::is_flatpak_enabled,
            commands::system::set_flatpak_enabled,
            commands::system::get_sync_interval_hours,
            commands::system::set_sync_interval_hours,
            commands::system::get_repo_priority_order,
            commands::system::set_repo_priority_order,
            commands::system::is_verbose_logs_enabled,
            commands::system::set_verbose_logs_enabled,
            commands::system::is_clean_build_enabled,
            commands::system::set_clean_build_enabled,
            commands::system::get_parallel_downloads,
            commands::system::is_onboarding_completed,
            commands::system::set_onboarding_completed,
            commands::system::get_theme_mode,
            commands::system::set_theme_mode,
            commands::system::get_accent_color,
            commands::system::set_accent_color,
            commands::system::is_declined_system_setup,
            commands::system::set_declined_system_setup,
            commands::system::is_sidebar_expanded,
            commands::system::set_sidebar_expanded,
            commands::system::is_alpha_notice_dismissed,
            commands::system::set_alpha_notice_dismissed,
            commands::system::get_search_history,
            commands::system::set_search_history,
            commands::system::get_read_news_ids,
            commands::system::set_read_news_ids,
            commands::system::get_active_tab,
            commands::system::set_active_tab,
            // Utils Commands
            commands::cmd_helpers::get_package_icon,
            commands::cmd_helpers::clear_metadata_caches,
            commands::cmd_helpers::rebuild_metadata_index,
            commands::cmd_helpers::clear_cache,
            commands::cmd_helpers::launch_package,
            commands::cmd_helpers::launch_app,
            commands::cmd_helpers::track_event,
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
            commands::package::get_flatpak_permissions,
            commands::package::get_full_package_details,
            commands::package::get_full_package_details_by_canonical_id,
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

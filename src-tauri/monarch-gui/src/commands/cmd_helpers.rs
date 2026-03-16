use crate::commands::search;
use crate::{chaotic_api, metadata};
use base64::prelude::*;
use serde::Deserialize;
use specta::Type;
use std::process::Command;
use tauri::State;

#[derive(Debug, Deserialize, Type)]
pub(crate) struct LaunchAppArgs {
    #[serde(alias = "pkgName")]
    pkg_name: String,
}

#[derive(Debug, Deserialize, Type)]
pub(crate) struct LaunchRequest {
    #[serde(alias = "packageName")]
    package_name: String,
    #[serde(alias = "appId")]
    app_id: Option<String>,
    #[serde(alias = "desktopEntry")]
    desktop_entry: Option<String>,
    #[serde(alias = "launchTarget")]
    launch_target: Option<String>,
    source: Option<crate::models::PackageSource>,
}

#[tauri::command]
#[specta::specta]
pub async fn get_package_icon(pkg_name: String) -> Result<Option<String>, String> {
    let icons_dir = metadata::get_icons_dir();
    if let Ok(entries) = std::fs::read_dir(&icons_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name_os) = path.file_name() {
                let name = name_os.to_string_lossy();
                if (name.starts_with(&pkg_name) && name.ends_with(".png"))
                    && (name == format!("{}.png", pkg_name)
                        || name.starts_with(&format!("{}_", pkg_name)))
                {
                    if let Ok(bytes) = std::fs::read(&path) {
                        let encoded = BASE64_STANDARD.encode(&bytes);
                        return Ok(Some(format!("data:image/png;base64,{}", encoded)));
                    }
                }
            }
        }
    }
    Ok(None)
}

#[tauri::command]
#[specta::specta]
pub async fn clear_cache(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, crate::repo_manager::RepoManager>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_scm: State<'_, crate::ScmState>,
) -> Result<(), String> {
    state_chaotic.inner().clear_cache().await;
    state_flathub.inner().clear_cache();
    state_scm.inner().0.clear_cache();
    search::clear_search_and_list_caches();
    state_repo.inner().sync_all(true, 0, None, None).await?;
    state_meta.inner().init(0).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn clear_metadata_caches(
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_scm: State<'_, crate::ScmState>,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    state_chaotic.inner().clear_cache().await;
    state_flathub.inner().clear_cache();
    state_scm.inner().0.clear_cache();
    search::clear_search_and_list_caches();
    crate::commands::package::invalidate_installed_catalog_cache();
    log::info!(
        "[CACHE] cleared metadata caches in {} ms",
        started.elapsed().as_millis()
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn rebuild_metadata_index(
    state_meta: State<'_, metadata::MetadataState>,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    state_meta.inner().init(0).await;
    crate::commands::package::invalidate_installed_catalog_cache();
    log::info!(
        "[CACHE] rebuilt metadata index in {} ms",
        started.elapsed().as_millis()
    );
    Ok(())
}

fn try_spawn_desktop(target: &str) -> Result<(), String> {
    Command::new("gtk-launch")
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch desktop entry '{}': {}", target, e))
}

fn resolve_desktop_entry(pkg_name: &str) -> Option<String> {
    let search_paths = [
        "/usr/share/applications".to_string(),
        "/usr/local/share/applications".to_string(),
        format!(
            "{}/.local/share/applications",
            std::env::var("HOME").unwrap_or_default()
        ),
    ];

    for path in search_paths {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".desktop")
                    && (name == format!("{}.desktop", pkg_name) || name.contains(pkg_name))
                {
                    return Some(name.trim_end_matches(".desktop").to_string());
                }
            }
        }
    }

    None
}

fn launch_package_impl(req: LaunchRequest) -> Result<(), String> {
    let pkg_name = req.package_name.trim();
    if pkg_name.is_empty() {
        return Err("Package name is empty".to_string());
    }

    if pkg_name == "reboot" {
        return Command::new("systemctl")
            .arg("reboot")
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to request reboot: {}", e));
    }

    let is_flatpak = req
        .source
        .as_ref()
        .map(|source| source.source_type == "flatpak")
        .unwrap_or(false)
        || req
            .app_id
            .as_ref()
            .map(|app_id| app_id.contains('.'))
            .unwrap_or(false)
        || req
            .launch_target
            .as_ref()
            .map(|target| target.contains('.'))
            .unwrap_or(false)
        || pkg_name.contains('.');

    if is_flatpak {
        let app_id = req
            .launch_target
            .clone()
            .or(req.app_id.clone())
            .unwrap_or_else(|| pkg_name.to_string());
        let flatpak = crate::flathub_api::flatpak_binary()?;
        return Command::new(&flatpak)
            .args(["run", &app_id])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch Flatpak app '{}': {}", app_id, e));
    }

    if let Some(desktop_entry) = req.desktop_entry.clone().or(req.launch_target.clone()) {
        if let Ok(()) = try_spawn_desktop(&desktop_entry) {
            return Ok(());
        }
    }

    if let Some(desktop_entry) = resolve_desktop_entry(pkg_name) {
        if let Ok(()) = try_spawn_desktop(&desktop_entry) {
            return Ok(());
        }
    }

    Command::new(pkg_name)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch '{}': {}", pkg_name, e))
}

#[tauri::command]
#[specta::specta]
pub async fn launch_package(launch_request: LaunchRequest) -> Result<(), String> {
    let started = std::time::Instant::now();
    let result = tokio::task::spawn_blocking(move || launch_package_impl(launch_request))
        .await
        .map_err(|e| e.to_string())?;
    log::info!(
        "[LAUNCH] completed in {} ms",
        started.elapsed().as_millis()
    );
    result
}

#[tauri::command]
#[specta::specta]
pub async fn launch_app(LaunchAppArgs { pkg_name }: LaunchAppArgs) -> Result<(), String> {
    launch_package(LaunchRequest {
        package_name: pkg_name,
        app_id: None,
        desktop_entry: None,
        launch_target: None,
        source: None,
    })
    .await
}

pub(crate) fn build_pacman_cmd(
    action_args: &[&str],
    password: &Option<String>,
) -> (String, Vec<String>) {
    let pacman = "/usr/bin/pacman";
    let wrapper_path = "/usr/lib/monarch-store/monarch-wrapper";
    let _helper_path = crate::utils::MONARCH_PK_HELPER;

    if password.is_none() && std::path::Path::new(wrapper_path).exists() {
        // Phase 3: Branded Identity Refactor; --disable-internal-agent = DE agent = once-per-session
        (
            "/usr/bin/pkexec".to_string(),
            std::iter::once("--disable-internal-agent".to_string())
                .chain(std::iter::once(wrapper_path.to_string()))
                .chain(std::iter::once(pacman.to_string()))
                .chain(action_args.iter().map(|s| s.to_string()))
                .collect(),
        )
    } else if password.is_none() {
        (
            "/usr/bin/pkexec".to_string(),
            std::iter::once("--disable-internal-agent".to_string())
                .chain(std::iter::once(pacman.to_string()))
                .chain(action_args.iter().map(|s| s.to_string()))
                .collect(),
        )
    } else {
        // Sudo pathway (usually with password)
        (
            "/usr/bin/sudo".to_string(),
            std::iter::once("-S".to_string())
                .chain(std::iter::once(pacman.to_string()))
                .chain(action_args.iter().map(|s| s.to_string()))
                .collect(),
        )
    }
}

/// Frontend-facing telemetry: gates by user consent (RepoManager) then forwards to Aptabase plugin.
#[tauri::command]
pub async fn track_event(app: tauri::AppHandle, event: String, payload: Option<serde_json::Value>) {
    crate::utils::track_event_safe(&app, &event, payload).await;
}

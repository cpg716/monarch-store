use crate::{chaotic_api, metadata};
use base64::prelude::*;
use tauri::State;

#[tauri::command]
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
    state_repo.inner().sync_all(true, 0, None).await?;
    state_meta.inner().init(0).await;
    Ok(())
}

#[tauri::command]
pub async fn launch_app(pkg_name: String) -> Result<(), String> {
    let status = std::process::Command::new("gtk-launch")
        .arg(&pkg_name)
        .status();

    if status.is_ok() && status.unwrap().success() {
        return Ok(());
    }

    let search_paths = [
        "/usr/share/applications",
        "/usr/local/share/applications",
        &format!(
            "{}/.local/share/applications",
            std::env::var("HOME").unwrap_or_default()
        ),
    ];

    for path in search_paths {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains(&pkg_name) && name.ends_with(".desktop") {
                    let _ = std::process::Command::new("gtk-launch").arg(name).spawn();
                    return Ok(());
                }
            }
        }
    }

    std::process::Command::new(&pkg_name)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch {}: {}", pkg_name, e))
}

pub(crate) fn build_pacman_cmd(
    action_args: &[&str],
    password: &Option<String>,
) -> (String, Vec<String>) {
    let pacman = "/usr/bin/pacman";
    let wrapper_path = "/usr/lib/monarch-store/monarch-wrapper";
    let helper_path = crate::utils::MONARCH_PK_HELPER;

    if password.is_none() && std::path::Path::new(wrapper_path).exists() {
        // Phase 3: Branded Identity Refactor
        // Use the shell wrapper proxy to trigger the branded Polkit identity (com.monarch.store.script)
        (
            "/usr/bin/pkexec".to_string(),
            std::iter::once(wrapper_path.to_string())
                .chain(std::iter::once(pacman.to_string()))
                .chain(action_args.iter().map(|s| s.to_string()))
                .collect(),
        )
    } else if password.is_none() && std::path::Path::new(helper_path).exists() {
        // Fallback to helper as Proxy for Polkit authorization (com.monarch.store.package-manage)
        let cmd = crate::helper_client::HelperCommand::RunCommand {
            binary: pacman.to_string(),
            args: action_args.iter().map(|s| s.to_string()).collect(),
        };
        let json = serde_json::to_string(&cmd).unwrap_or_default();

        (
            "/usr/bin/pkexec".to_string(),
            vec![helper_path.to_string(), json],
        )
    } else if password.is_none() {
        // Fallback to direct pkexec if no proxy installed (will prompt generically)
        (
            "/usr/bin/pkexec".to_string(),
            std::iter::once(pacman.to_string())
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

#[tauri::command]
pub async fn track_event(app: tauri::AppHandle, event: String, payload: Option<serde_json::Value>) {
    crate::utils::track_event_safe(&app, &event, payload).await;
}

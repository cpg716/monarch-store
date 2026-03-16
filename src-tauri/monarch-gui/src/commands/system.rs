use crate::{chaotic_api, distro_context, helper_client, repo_manager, utils};
use serde::Serialize;
use specta::Type;
use std::process::Command;
use tauri::{AppHandle, Emitter, State};
#[cfg(not(target_os = "linux"))]
use tauri_plugin_notification::NotificationExt;

/// Embedded Polkit rules for passwordless package-manage (wheel → YES) and script (AUTH_ADMIN_KEEP).
const MONARCH_POLKIT_RULES: &str = include_str!("../../../rules/10-monarch-store.rules");
const MONARCH_POLKIT_POLICY: &str = include_str!("../../com.monarch.store.policy");

#[derive(Serialize, serde::Deserialize, Type, Clone, Debug)]
pub enum SnapshotTool {
    Snapper,
    Timeshift,
    None,
}

#[derive(Serialize, Type)]
pub struct SnapshotStatus {
    pub tool: SnapshotTool,
    pub is_configured: bool,
    pub message: String,
}

#[tauri::command]
#[specta::specta]
pub async fn get_snapshot_status() -> Result<SnapshotStatus, String> {
    // 1. Check Timeshift
    let ts_check = std::process::Command::new("timeshift")
        .arg("--list-devices")
        .output();

    if let Ok(output) = ts_check {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains("No devices found") {
                return Ok(SnapshotStatus {
                    tool: SnapshotTool::Timeshift,
                    is_configured: true,
                    message: "Timeshift is ready".to_string(),
                });
            }
        }
    }

    // 2. Check Snapper
    let sn_check = std::process::Command::new("snapper")
        .arg("list-configs")
        .output();

    if let Ok(output) = sn_check {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Snapper output usually has a header; check if there's at least one config line
            let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
            if lines.len() > 1 {
                // More than just header
                return Ok(SnapshotStatus {
                    tool: SnapshotTool::Snapper,
                    is_configured: true,
                    message: "Snapper is ready".to_string(),
                });
            }
        }
    }

    Ok(SnapshotStatus {
        tool: SnapshotTool::None,
        is_configured: false,
        message: "No snapshot tool detected or configured".to_string(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn create_system_snapshot(
    app: AppHandle,
    tool: SnapshotTool,
    comment: String,
) -> Result<String, String> {
    log::info!("Creating system snapshot using {:?}: {}", tool, comment);
    let _ = app.emit("update-status", "Creating system snapshot...");

    match tool {
        SnapshotTool::Timeshift => {
            let output = tokio::process::Command::new("pkexec")
                .args([
                    "timeshift",
                    "--create",
                    "--comments",
                    &comment,
                    "--scripted",
                ])
                .output()
                .await
                .map_err(|e| format!("Failed to run timeshift: {}", e))?;

            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).to_string());
            }
            Ok("Timeshift snapshot created".to_string())
        }
        SnapshotTool::Snapper => {
            let output = tokio::process::Command::new("pkexec")
                .args(["snapper", "create", "--description", &comment])
                .output()
                .await
                .map_err(|e| format!("Failed to run snapper: {}", e))?;

            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).to_string());
            }
            Ok("Snapper snapshot created".to_string())
        }
        SnapshotTool::None => Err("No snapshot tool selected".to_string()),
    }
}

fn set_policy_allow_active(policy: &str, action_id: &str, allow_active: &str) -> String {
    let action_marker = format!("<action id=\"{}\">", action_id);
    let Some(action_start) = policy.find(&action_marker) else {
        return policy.to_string();
    };

    let rest = &policy[action_start..];
    let Some(action_end_rel) = rest.find("</action>") else {
        return policy.to_string();
    };
    let action_end = action_start + action_end_rel;
    let action_block = &policy[action_start..action_end];

    let allow_start_tag = "<allow_active>";
    let allow_end_tag = "</allow_active>";
    let Some(allow_start_rel) = action_block.find(allow_start_tag) else {
        return policy.to_string();
    };
    let allow_value_start = action_start + allow_start_rel + allow_start_tag.len();
    let Some(allow_end_rel) = action_block[allow_start_rel..].find(allow_end_tag) else {
        return policy.to_string();
    };
    let allow_value_end = action_start + allow_start_rel + allow_end_rel;

    let mut updated = String::with_capacity(policy.len() + allow_active.len());
    updated.push_str(&policy[..allow_value_start]);
    updated.push_str(allow_active);
    updated.push_str(&policy[allow_value_end..]);
    updated
}

#[derive(Serialize, Type)]
pub struct SystemInfo {
    pub kernel: String,
    pub distro: String,
    pub pacman_version: String,
    pub chaotic_enabled: bool,
    pub cpu_optimization: String,
}

#[derive(Serialize, Type)]
pub struct HostAppearance {
    pub color_scheme: String, // "dark" | "light" | "system"
    pub accent_color: Option<String>, // #RRGGBB
    pub desktop: String,      // gnome/kde/plasma/xfce/unknown
}

fn detect_desktop_environment() -> String {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("XDG_SESSION_DESKTOP"))
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_else(|_| "unknown".to_string())
        .to_lowercase();

    if desktop.contains("gnome") {
        "gnome".to_string()
    } else if desktop.contains("kde") || desktop.contains("plasma") {
        "kde".to_string()
    } else if desktop.contains("xfce") {
        "xfce".to_string()
    } else if desktop.contains("cinnamon") {
        "cinnamon".to_string()
    } else {
        desktop
    }
}

#[cfg(target_os = "linux")]
fn rgb_tuple_to_hex((r, g, b): (f64, f64, f64)) -> String {
    let to_u8 = |v: f64| -> u8 {
        let clamped = v.clamp(0.0, 1.0);
        (clamped * 255.0).round() as u8
    };
    format!("#{:02x}{:02x}{:02x}", to_u8(r), to_u8(g), to_u8(b))
}

#[tauri::command]
#[specta::specta]
pub async fn get_host_appearance() -> Result<HostAppearance, String> {
    let mut color_scheme = "system".to_string();
    let mut accent_color = None;

    #[cfg(target_os = "linux")]
    {
        use ashpd::desktop::settings::Settings;
        if let Ok(proxy) = Settings::new().await {
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
                color_scheme = match scheme {
                    1 => "dark".to_string(),
                    2 => "light".to_string(),
                    _ => "system".to_string(),
                };
            }

            if let Ok(rgb) = proxy
                .read::<(f64, f64, f64)>("org.freedesktop.appearance", "accent-color")
                .await
            {
                accent_color = Some(rgb_tuple_to_hex(rgb));
            } else if let Ok(rgb) = proxy
                .read::<Vec<f64>>("org.freedesktop.appearance", "accent-color")
                .await
            {
                if rgb.len() >= 3 {
                    accent_color = Some(rgb_tuple_to_hex((rgb[0], rgb[1], rgb[2])));
                }
            }
        }
    }

    Ok(HostAppearance {
        color_scheme,
        accent_color,
        desktop: detect_desktop_environment(),
    })
}

/// Typed response for get_cache_size (replaces raw serde_json::json!).
#[derive(Serialize, Type)]
pub struct CacheSizeResult {
    pub size_bytes: u64,
    pub human_readable: String,
}

/// Typed response for get_orphans_with_size (replaces raw serde_json::json!).
#[derive(Serialize, Type)]
pub struct OrphansWithSizeResult {
    pub orphans: Vec<String>,
    pub total_size_bytes: u64,
    pub human_readable: String,
}

#[tauri::command]
#[specta::specta]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    let kernel = std::process::Command::new("uname")
        .arg("-r")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let distro = std::fs::read_to_string("/etc/os-release")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("PRETTY_NAME="))
        .map(|l| l.split('=').nth(1).unwrap_or("Unknown").replace('"', ""))
        .unwrap_or_else(|| "Arch Linux".to_string());

    let pacman_version = std::process::Command::new("pacman")
        .arg("--version")
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("Unknown")
                .to_string()
        })
        .unwrap_or_else(|_| "Unknown".to_string());

    let chaotic_enabled = std::fs::read_to_string("/etc/pacman.conf")
        .map(|c| c.contains("[chaotic-aur]"))
        .unwrap_or(false);

    let cpu_optimization = if utils::is_cpu_znver4_compatible() {
        "x86-64-v4 (Zen 4/5)".to_string()
    } else if utils::is_cpu_v4_compatible() {
        "x86-64-v4 (AVX-512)".to_string()
    } else if utils::is_cpu_v3_compatible() {
        "x86-64-v3 (AVX2)".to_string()
    } else {
        "Standard (x86-64-v1)".to_string()
    };

    Ok(SystemInfo {
        kernel,
        distro,
        pacman_version,
        chaotic_enabled,
        cpu_optimization,
    })
}

/// Chaotic-AUR status: compatible = host may enable (not Manjaro); chaotic_in_alpm = [chaotic-aur] in syncdbs.
#[derive(Serialize, Type)]
pub struct ChaoticStatus {
    pub compatible: bool,
    pub chaotic_in_alpm: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn check_chaotic_status() -> Result<ChaoticStatus, String> {
    let distro = distro_context::DistroContext::new();
    let compatible = distro.is_chaotic_compatible();
    let chaotic_in_alpm = tokio::task::spawn_blocking(|| {
        crate::alpm_read::chaotic_aur_in_syncdbs("/etc/pacman.conf")
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;
    Ok(ChaoticStatus {
        compatible,
        chaotic_in_alpm,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn prepare_chaotic_components(
    app: AppHandle,
    state_repo: State<'_, repo_manager::RepoManager>,
    password: Option<String>,
) -> Result<(), String> {
    let one_click = state_repo.inner().is_one_click_enabled().await;
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::PrepareChaoticComponents,
        password,
        one_click,
    )
    .await?;
    while let Some(msg) = rx.recv().await {
        if msg.message.starts_with("Error:") {
            return Err(msg.message);
        }
    }
    Ok(())
}

/// Prepares Flatpak for use: if the flatpak binary is missing, installs the flatpak package via pacman, then ensures the Flathub remote exists.
/// Call this when the user enables Flatpak in onboarding or Settings so they can install Flatpak apps without a separate "install flatpak" step.
#[tauri::command]
#[specta::specta]
pub async fn prepare_flatpak(
    app: AppHandle,
    state_repo: State<'_, repo_manager::RepoManager>,
    password: Option<String>,
) -> Result<(), String> {
    if crate::flathub_api::flatpak_binary().is_ok() {
        return crate::flathub_api::ensure_flathub_remote(app).await;
    }
    let repo_manager = state_repo.inner();
    let one_click = repo_manager.is_one_click_enabled().await;
    let all_repos = repo_manager.get_all_repos().await;
    let mut enabled_repos: Vec<String> = all_repos
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.name.clone())
        .collect();
    for sys in ["core", "extra", "community", "multilib"] {
        if !enabled_repos.contains(&sys.to_string()) {
            enabled_repos.push(sys.to_string());
        }
    }
    let _guard = utils::PRIVILEGED_LOCK.lock().await;
    let _ = app.emit(
        "install-output",
        "Installing Flatpak (required for Flathub apps)...",
    );
    let mut rx = helper_client::invoke_helper(
        &app,
        helper_client::HelperCommand::AlpmInstall {
            packages: vec!["flatpak".to_string()],
            sync_first: true,
            enabled_repos,
            cpu_optimization: None,
            target_repo: None,
        },
        password,
        one_click,
    )
    .await
    .map_err(|e| format!("Failed to install Flatpak: {}", e))?;
    while let Some(msg) = rx.recv().await {
        let _ = app.emit("install-output", &msg.message);
    }
    crate::flathub_api::ensure_flathub_remote(app).await
}

/// Ensures the Flathub remote exists so Flatpak install/uninstall work when Flatpak is turned on (onboarding or Settings).
/// Call this when the user enables Flatpak; also run before first install (done inside install_flatpak).
#[tauri::command]
#[specta::specta]
pub async fn ensure_flathub_remote(app: AppHandle) -> Result<(), String> {
    crate::flathub_api::ensure_flathub_remote(app).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_all_installed_names() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("pacman")
        .arg("-Qq")
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn get_infra_stats(
    state: State<'_, chaotic_api::ChaoticApiClient>,
) -> Result<crate::chaotic_api::InfraStats, String> {
    state.inner().fetch_infra_stats().await
}

#[tauri::command]
#[specta::specta]
pub async fn get_repo_counts(
    state_repo: State<'_, repo_manager::RepoManager>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
) -> Result<std::collections::HashMap<String, usize>, String> {
    let mut counts: std::collections::HashMap<String, usize> =
        state_repo.inner().get_package_counts().await;
    if let Ok(chaotic) = state_chaotic.inner().fetch_packages().await {
        counts.insert("chaotic-aur".to_string(), chaotic.len());
    }
    Ok(counts)
}

#[tauri::command]
#[specta::specta]
pub async fn get_repo_states(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<Vec<repo_manager::RepoConfig>, String> {
    Ok(state.inner().get_all_repos().await)
}

#[tauri::command]
#[specta::specta]
pub async fn is_aur_enabled(state: State<'_, repo_manager::RepoManager>) -> Result<bool, String> {
    Ok(state.inner().is_aur_enabled().await)
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_repo(
    app: tauri::AppHandle,
    state: State<'_, repo_manager::RepoManager>,
    name: String,
    enabled: bool,
    _password: Option<String>,
) -> Result<(), String> {
    state.inner().set_repo_state(&app, &name, enabled).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_repo_family(
    app: tauri::AppHandle,
    state: State<'_, repo_manager::RepoManager>,
    family: String,
    enabled: bool,
    skip_os_sync: Option<bool>,
    password: Option<String>,
) -> Result<(), String> {
    let skip = skip_os_sync.unwrap_or(false);
    state
        .inner()
        .set_repo_family_state(&app, &family, enabled, skip, password)
        .await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_aur_enabled(
    _app: tauri::AppHandle,
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_aur_enabled(enabled).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn is_one_click_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    let json_enabled = state.inner().is_one_click_enabled().await;

    // Check disk reality
    let policy_path = std::path::Path::new("/usr/share/polkit-1/actions/com.monarch.store.policy");
    let disk_enabled = if policy_path.exists() {
        std::fs::read_to_string(policy_path)
            .map(|c| c.contains("<allow_active>yes</allow_active>"))
            .unwrap_or(false)
    } else {
        false
    };

    // Auto-sync JSON if disk says yes but JSON says no
    if disk_enabled && !json_enabled {
        state.inner().set_one_click_enabled(true).await;
        return Ok(true);
    }

    Ok(json_enabled)
}

#[tauri::command]
#[specta::specta]
pub async fn set_one_click_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_one_click_enabled(enabled).await;
    Ok(())
}

/// Returns names of required runtime binaries that are missing (git, checkupdates, pkexec).
/// Surfaces in Settings so users know why AUR or system updates may fail.
#[tauri::command]
#[specta::specta]
pub fn get_missing_required_bins() -> Result<Vec<String>, String> {
    let required = ["git", "checkupdates", "pkexec"];
    let missing: Vec<String> = required
        .iter()
        .filter(|bin| which::which(*bin).is_err())
        .map(|s| (*s).to_string())
        .collect();
    Ok(missing)
}

#[tauri::command]
#[specta::specta]
pub async fn check_security_policy() -> Result<bool, String> {
    let helper_path = std::path::Path::new("/usr/lib/monarch-store/monarch-helper");
    let policy_path = std::path::Path::new("/usr/share/polkit-1/actions/com.monarch.store.policy");

    if !helper_path.exists() || !policy_path.exists() {
        return Ok(false);
    }

    Ok(true)
}

#[tauri::command]
#[specta::specta]
pub async fn install_monarch_policy(
    state: State<'_, repo_manager::RepoManager>,
    password: Option<String>,
) -> Result<String, String> {
    let one_click = state.inner().is_one_click_enabled().await;
    let allow_active = if one_click { "yes" } else { "auth_admin_keep" };
    let policy_content = set_policy_allow_active(
        MONARCH_POLKIT_POLICY,
        "com.monarch.store.package-manage",
        allow_active,
    );

    let rules_escaped = MONARCH_POLKIT_RULES.replace('{', "{{").replace('}', "}}");
    let script = format!(
        r#"
        echo 'Setting up MonARCH Polkit Policy and Rules...'
        mkdir -p /usr/lib/monarch-store
        cat <<'POLICYEOF' > /usr/share/polkit-1/actions/com.monarch.store.policy
{}
POLICYEOF
        cat <<'RULESEOF' > /usr/share/polkit-1/rules.d/10-monarch-store.rules
{}
RULESEOF
        chmod 644 /usr/share/polkit-1/actions/com.monarch.store.policy /usr/share/polkit-1/rules.d/10-monarch-store.rules
        echo '✓ MonARCH Polkit Policy ({}) and Rules (passwordless for wheel) installed.'
    "#,
        policy_content, rules_escaped, allow_active
    );

    let result = utils::run_privileged_script(script.as_str(), password, true).await;
    result
}

#[tauri::command]
#[specta::specta]
pub async fn optimize_system(password: Option<String>) -> Result<String, String> {
    let script = r#"
        echo '--- Starting MonARCH System Optimization ---'
        if grep -q "options=.*COMPRESSZST" /etc/makepkg.conf; then
            echo '✓ Parallel ZSTD is already enabled.'
        else
            echo 'Enabling Parallel ZSTD compression...'
            sed -i 's/COMPRESSZST=(zstd -c -z -q -)/COMPRESSZST=(zstd -c -z -q --threads=0 -)/' /etc/makepkg.conf
        fi
        if grep -q "MAKEFLAGS=.*-j" /etc/makepkg.conf; then
            echo '✓ Parallel compilation is already configured.'
        else
            echo 'Optimizing MAKEFLAGS for CPU cores...'
            echo 'MAKEFLAGS="-j$(nproc)"' >> /etc/makepkg.conf
        fi
        echo '✓ System optimization complete!'
    "#;
    utils::run_privileged_script(script, password, false).await
}
#[tauri::command]
#[specta::specta]
pub async fn trigger_repo_sync(
    app: tauri::AppHandle,
    state_repo: State<'_, repo_manager::RepoManager>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    sync_interval_hours: Option<u32>,
    password: Option<String>,
) -> Result<String, String> {
    use tauri::Emitter;
    let interval = sync_interval_hours.unwrap_or(3) as u64;

    let _ = app.emit("sync-progress", "Refreshing package sources in background");
    let repo_res = state_repo
        .inner()
        .sync_all(false, interval, Some(app.clone()), password)
        .await?;

    let _ = app.emit("sync-progress", "Fetching Chaotic-AUR metadata...");
    let _ = state_chaotic.inner().fetch_packages().await;

    let _ = app.emit("sync-progress", "Initialization complete.");
    Ok(repo_res)
}

#[tauri::command]
#[specta::specta]
pub async fn update_and_install_package(
    app: tauri::AppHandle,
    state_repo: State<'_, repo_manager::RepoManager>,
    name: String,
    repo_name: Option<String>,
    password: Option<String>,
) -> Result<String, String> {
    use tauri::Emitter;
    let _ = app.emit(
        "install-output",
        format!("--- System Update & Install: {} ---", name),
    );

    // No conflicting-process check: same as install_package (rely on db.lck / helper).

    let cpu_optimization = if crate::utils::is_cpu_znver4_compatible() {
        Some("znver4".to_string())
    } else if crate::utils::is_cpu_v4_compatible() {
        Some("v4".to_string())
    } else if crate::utils::is_cpu_v3_compatible() {
        Some("v3".to_string())
    } else {
        None
    };

    let _ = app.emit(
        "install-output",
        "Synchronizing databases and updating system...",
    );

    let one_click = state_repo.inner().is_one_click_enabled().await;
    let housekeeping = state_repo.inner().is_automatic_housekeeping_enabled().await;

    let parallel_downloads = Some(state_repo.inner().get_parallel_downloads().await);

    // ✅ OPERATION SILENT GUARD: Bundle Update + Install into ONE helper invocation
    // This provides a single password prompt and ensures atomicity.
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ExecuteBatch {
            manifest: crate::models::TransactionManifest {
                update_system: true,
                refresh_db: true,
                remove_orphans: housekeeping,
                clear_cache: housekeeping,
                install_targets: vec![name.clone()],
                cpu_optimization,
                target_repo: repo_name,
                parallel_downloads,
                ..Default::default()
            },
        },
        password.clone(),
        one_click,
    )
    .await?;

    while let Some(msg) = rx.recv().await {
        let _ = app.emit("install-output", &msg.message);
        if msg.message.starts_with("Error:") {
            let _ = app.emit("install-complete", "failed");
            return Err(format!(
                "Update & Install failed for {}: {}",
                name, msg.message
            ));
        }
    }

    // Verification
    let verification = tokio::task::spawn_blocking({
        let pkg_name = name.clone();
        move || crate::alpm_read::is_package_installed(&pkg_name)
    })
    .await
    .map_err(|e| format!("Verification task failed: {}", e))?;

    if verification {
        let _ = app.emit("install-complete", "success");
        Ok("System updated and package installed successfully.".to_string())
    } else {
        let _ = app.emit("install-complete", "failed");
        Err(format!(
            "Installation reported success but {} is still missing after system upgrade.",
            name
        ))
    }
}

#[tauri::command]
#[specta::specta]
pub async fn is_onboarding_completed(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_onboarding_completed().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_onboarding_completed(
    state: State<'_, repo_manager::RepoManager>,
    completed: bool,
) -> Result<bool, String> {
    Ok(state.inner().set_onboarding_completed(completed).await)
}

#[tauri::command]
#[specta::specta]
pub async fn get_theme_mode(state: State<'_, repo_manager::RepoManager>) -> Result<String, String> {
    Ok(state.inner().get_theme_mode().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_theme_mode(
    state: State<'_, repo_manager::RepoManager>,
    mode: String,
) -> Result<(), String> {
    state.inner().set_theme_mode(mode).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_accent_color(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<String, String> {
    Ok(state.inner().get_accent_color().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_accent_color(
    state: State<'_, repo_manager::RepoManager>,
    color: String,
) -> Result<(), String> {
    state.inner().set_accent_color(color).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn is_declined_system_setup(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_declined_system_setup().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_declined_system_setup(
    state: State<'_, repo_manager::RepoManager>,
    declined: bool,
) -> Result<(), String> {
    state.inner().set_declined_system_setup(declined).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn is_sidebar_expanded(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_sidebar_expanded().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_sidebar_expanded(
    state: State<'_, repo_manager::RepoManager>,
    expanded: bool,
) -> Result<(), String> {
    state.inner().set_sidebar_expanded(expanded).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn is_alpha_notice_dismissed(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_alpha_notice_dismissed().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_alpha_notice_dismissed(
    state: State<'_, repo_manager::RepoManager>,
    dismissed: bool,
) -> Result<(), String> {
    state.inner().set_alpha_notice_dismissed(dismissed).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_search_history(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<Vec<String>, String> {
    Ok(state.inner().get_search_history().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_search_history(
    state: State<'_, repo_manager::RepoManager>,
    history: Vec<String>,
) -> Result<(), String> {
    state.inner().set_search_history(history).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_read_news_ids(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<Vec<String>, String> {
    Ok(state.inner().get_read_news_ids().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_read_news_ids(
    state: State<'_, repo_manager::RepoManager>,
    ids: Vec<String>,
) -> Result<(), String> {
    state.inner().set_read_news_ids(ids).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_active_tab(state: State<'_, repo_manager::RepoManager>) -> Result<String, String> {
    Ok(state.inner().get_active_tab().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_tab(
    state: State<'_, repo_manager::RepoManager>,
    tab: String,
) -> Result<(), String> {
    state.inner().set_active_tab(tab).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn is_automatic_housekeeping_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_automatic_housekeeping_enabled().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_automatic_housekeeping_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state
        .inner()
        .set_automatic_housekeeping_enabled(enabled)
        .await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn perform_housekeeping(
    app: AppHandle,
    state_repo: State<'_, repo_manager::RepoManager>,
    password: Option<String>,
) -> Result<(), String> {
    let one_click = state_repo.inner().is_one_click_enabled().await;
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ExecuteBatch {
            manifest: crate::models::TransactionManifest {
                remove_orphans: true,
                clear_cache: true,
                ..Default::default()
            },
        },
        password,
        one_click,
    )
    .await?;

    while let Some(msg) = rx.recv().await {
        let _ = app.emit("housekeeping-progress", &msg.message);
        if msg.message.starts_with("Error:") {
            return Err(msg.message);
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn is_advanced_mode(state: State<'_, repo_manager::RepoManager>) -> Result<bool, String> {
    Ok(state.inner().is_advanced_mode().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_advanced_mode(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_advanced_mode(enabled).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn check_app_update() -> Result<Option<String>, String> {
    // Uses checkupdates from pacman-contrib to check for updates safely without root
    let output = std::process::Command::new("checkupdates")
        .output()
        .map_err(|e| format!("Failed to run checkupdates: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.starts_with("monarch-store ") {
            // line looks like: monarch-store 0.2.29-1 -> 0.2.30-1
            let parts: Vec<&str> = line.split(" -> ").collect();
            if parts.len() == 2 {
                return Ok(Some(parts[1].trim().to_string()));
            }
        }
    }

    Ok(None)
}

#[tauri::command]
#[specta::specta]
pub async fn is_telemetry_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_telemetry_enabled().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_telemetry_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    log::info!("Setting telemetry enabled to: {}", enabled);
    state.inner().set_telemetry_enabled(enabled).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn is_notifications_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_notifications_enabled().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_notifications_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_notifications_enabled(enabled).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn is_flatpak_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_flatpak_enabled().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_flatpak_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_flatpak_enabled(enabled).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_sync_interval_hours(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<u32, String> {
    Ok(state.inner().get_sync_interval_hours().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_sync_interval_hours(
    state: State<'_, repo_manager::RepoManager>,
    hours: u32,
) -> Result<(), String> {
    state.inner().set_sync_interval_hours(hours).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_repo_priority_order(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<Vec<String>, String> {
    Ok(state.inner().get_repo_priority_order().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_repo_priority_order(
    state: State<'_, repo_manager::RepoManager>,
    order: Vec<String>,
) -> Result<(), String> {
    state.inner().set_repo_priority_order(order).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn is_verbose_logs_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_verbose_logs_enabled().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_verbose_logs_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_verbose_logs_enabled(enabled).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn is_clean_build_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_clean_build_enabled().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_clean_build_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_clean_build_enabled(enabled).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_parallel_downloads(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<u32, String> {
    Ok(state.inner().get_parallel_downloads().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_parallel_downloads(
    state: State<'_, repo_manager::RepoManager>,
    count: u32,
) -> Result<(), String> {
    state.inner().set_parallel_downloads(count).await;
    Ok(())
}

/// Show a desktop notification. On Linux we use notify-send in a blocking thread to avoid
/// "Cannot start a runtime from within a runtime" (tauri-plugin-notification uses notify-rust/zbus
/// which calls block_on). On other platforms we use the plugin from the same blocking thread.
pub async fn show_desktop_notification_safe(app: &AppHandle, title: String, body: String) {
    #[cfg(target_os = "linux")]
    let _ = app;
    #[cfg(target_os = "linux")]
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let _ = Command::new("notify-send")
            .arg(&title)
            .arg(&body)
            .arg("--app-name=MonARCH Store")
            .output();
    })
    .await;

    #[cfg(not(target_os = "linux"))]
    {
        let app = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let _ = app.notification().builder().title(title).body(body).show();
        })
        .await;
    }
}

#[tauri::command]
#[specta::specta]
pub async fn show_desktop_notification(
    app: AppHandle,
    title: String,
    body: String,
) -> Result<(), String> {
    show_desktop_notification_safe(&app, title, body).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_install_mode_command() -> String {
    match utils::get_install_mode() {
        utils::InstallMode::System => "system".to_string(),
        utils::InstallMode::Portable => "portable".to_string(),
        utils::InstallMode::Dev => "portable".to_string(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn is_sync_on_startup_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_sync_on_startup_enabled().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_sync_on_startup_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_sync_on_startup_enabled(enabled).await;
    Ok(())
}

/// Returns true if the pacman hook set a refresh flag (user ran pacman in terminal);
/// we clear the flag and the caller should trigger a repo sync.
#[tauri::command]
#[specta::specta]
pub fn check_and_clear_refresh_requested() -> Result<bool, String> {
    let path = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("monarch-store")
        .join("refresh-requested");
    if path.exists() {
        let _ = std::fs::remove_file(&path);
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
#[specta::specta]
pub async fn get_cache_size() -> Result<CacheSizeResult, String> {
    tokio::task::spawn_blocking(|| {
        let cache_dir = std::path::Path::new("/var/cache/pacman/pkg");
        let mut total_bytes: u64 = 0;

        fn calculate_dir_size(path: &std::path::Path, total: &mut u64) -> std::io::Result<()> {
            if path.is_file() {
                if let Ok(metadata) = path.metadata() {
                    *total += metadata.len();
                }
            } else if path.is_dir() {
                let entries = std::fs::read_dir(path)?;
                for entry in entries {
                    let entry = entry?;
                    let path = entry.path();
                    let _ = calculate_dir_size(&path, total);
                }
            }
            Ok(())
        }

        if cache_dir.exists() {
            let _ = calculate_dir_size(cache_dir, &mut total_bytes);
        }

        let human_readable = if total_bytes < 1024 {
            format!("{} B", total_bytes)
        } else if total_bytes < 1024 * 1024 {
            format!("{:.1} KB", total_bytes as f64 / 1024.0)
        } else if total_bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", total_bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", total_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        };
        Ok(CacheSizeResult {
            size_bytes: total_bytes,
            human_readable,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
#[specta::specta]
pub async fn get_orphans_with_size() -> Result<OrphansWithSizeResult, String> {
    tokio::task::spawn_blocking(|| {
        let output = std::process::Command::new("pacman")
            .args(["-Qtdq"])
            .output()
            .map_err(|e| e.to_string())?;
        let orphans: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        if orphans.is_empty() {
            return Ok(OrphansWithSizeResult {
                orphans: vec![],
                total_size_bytes: 0,
                human_readable: "0 B".to_string(),
            });
        }

        let mut total_bytes: u64 = 0;
        for pkg in &orphans {
            let output = std::process::Command::new("pacman")
                .args(["-Qi", pkg])
                .output()
                .ok();
            if let Some(ok_output) = output {
                let info = String::from_utf8_lossy(&ok_output.stdout);
                for line in info.lines() {
                    if line.starts_with("Installed Size") {
                        if let Some(size_str) = line.split(':').nth(1) {
                            let parts: Vec<&str> = size_str.split_whitespace().collect();
                            if parts.len() >= 2 {
                                if let Ok(num) = parts[0].parse::<f64>() {
                                    let multiplier = match parts[1] {
                                        "KiB" => 1024,
                                        "MiB" => 1024 * 1024,
                                        "GiB" => 1024 * 1024 * 1024,
                                        _ => 1,
                                    };
                                    total_bytes += (num * multiplier as f64) as u64;
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }

        let human_readable = if total_bytes < 1024 {
            format!("{} B", total_bytes)
        } else if total_bytes < 1024 * 1024 {
            format!("{:.1} KB", total_bytes as f64 / 1024.0)
        } else if total_bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", total_bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", total_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        };

        Ok(OrphansWithSizeResult {
            orphans,
            total_size_bytes: total_bytes,
            human_readable,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Result of testing one mirror: URL and optional latency in ms.
#[derive(serde::Serialize, specta::Type)]
pub struct MirrorTestResult {
    pub url: String,
    pub latency_ms: Option<u32>,
}

/// Test mirrors for a repo without changing system config. Returns top 3 with latency (ms).
/// repo_key: "arch" | "Arch" | "cachyos" | "chaotic-aur" (others fall back to arch or N/A).
#[tauri::command]
#[specta::specta]
pub async fn test_mirrors(repo_key: String) -> Result<Vec<MirrorTestResult>, String> {
    let key = repo_key.to_lowercase();
    let (distro, mirrorlist_path): (&str, Option<std::path::PathBuf>) =
        if key == "arch" || key == "official arch linux" || key.is_empty() {
            ("arch", None)
        } else if key.contains("cachyos") {
            (
                "cachyos",
                Some(std::path::PathBuf::from("/etc/pacman.d/cachyos-mirrorlist")),
            )
        } else if key.contains("chaotic") {
            (
                "chaotic",
                Some(std::path::PathBuf::from("/etc/pacman.d/chaotic-mirrorlist")),
            )
        } else {
            ("arch", None)
        };

    let out = tokio::task::spawn_blocking(move || {
        if distro == "arch" {
            // rate-mirrors prints mirrorlist to stdout (no root needed to test)
            let output = std::process::Command::new("rate-mirrors")
                .args(["arch"])
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    parse_mirrorlist_latency(&stdout, 3)
                }
                Ok(_) => {
                    // Fallback: reflector --list (no latency, just URLs)
                    let o = std::process::Command::new("reflector")
                        .args(["--list"])
                        .output();
                    match o {
                        Ok(reflector_out) if reflector_out.status.success() => {
                            let s = String::from_utf8_lossy(&reflector_out.stdout);
                            let list = parse_mirrorlist_latency(&s, 3)?;
                            Ok(list
                                .into_iter()
                                .map(|m| MirrorTestResult {
                                    url: m.url,
                                    latency_ms: None,
                                })
                                .collect())
                        }
                        _ => Err("Install rate-mirrors or reflector to test mirrors (e.g. pacman -S rate-mirrors)".to_string()),
                    }
                }
                Err(_) => Err("rate-mirrors not found. Install it: pacman -S rate-mirrors (or reflector)".to_string()),
            }
        } else if let Some(path) = mirrorlist_path {
            // Read existing mirrorlist; optionally run rate-mirrors for cachyos if available
            match std::fs::read_to_string(path) {
                Ok(contents) => {
                    let mut results = parse_mirrorlist_latency(&contents, 5).unwrap_or_else(|_| vec![]);
                    if results.iter().all(|r| r.latency_ms.is_none()) && distro == "cachyos" {
                        if let Ok(o) = std::process::Command::new("rate-mirrors")
                            .args(["cachyos"])
                            .output()
                        {
                            if o.status.success() {
                                let stdout = String::from_utf8_lossy(&o.stdout);
                                if let Ok(rated) = parse_mirrorlist_latency(&stdout, 3) {
                                    results = rated;
                                }
                            }
                        }
                    }
                    results.truncate(3);
                    Ok(results)
                }
                Err(_) => Ok(vec![
                    MirrorTestResult {
                        url: "Mirrorlist file not found".to_string(),
                        latency_ms: None,
                    },
                ]),
            }
        } else {
            Ok(vec![])
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    out
}

/// Parse mirrorlist lines: "Server = https://... # 45ms" or "Server = https://..."
fn parse_mirrorlist_latency(text: &str, take: usize) -> Result<Vec<MirrorTestResult>, String> {
    let re = regex::Regex::new(r"(?m)^\s*Server\s*=\s*(\S+)(?:\s*#\s*(\d+)\s*ms)?")
        .map_err(|e| e.to_string())?;
    let mut list: Vec<(String, Option<u32>)> = re
        .captures_iter(text)
        .filter_map(|c| {
            let url = c.get(1).map(|m| m.as_str().to_string())?;
            let ms = c.get(2).and_then(|m| m.as_str().parse::<u32>().ok());
            Some((url, ms))
        })
        .collect();
    // If no latency, still include URLs (e.g. from reflector or static mirrorlist)
    if list.is_empty() {
        let re_url = regex::Regex::new(r"(?m)^\s*Server\s*=\s*(\S+)").map_err(|e| e.to_string())?;
        list = re_url
            .captures_iter(text)
            .filter_map(|c| c.get(1).map(|m| (m.as_str().to_string(), None)))
            .collect();
    }
    list.truncate(take);
    Ok(list
        .into_iter()
        .map(|(url, latency_ms)| MirrorTestResult { url, latency_ms })
        .collect())
}

/// Returns which mirror-ranking tool will be used (distro-aware). Used by Settings UI to show correct label.
/// Never runs reflector on Manjaro — rank_mirrors script uses pacman-mirrors there.
#[tauri::command]
#[specta::specta]
pub fn get_mirror_rank_tool() -> Option<String> {
    if std::path::Path::new("/usr/bin/pacman-mirrors").exists()
        && std::path::Path::new("/etc/manjaro-release").exists()
    {
        return Some("pacman-mirrors".to_string());
    }
    if which::which("reflector").is_ok() {
        return Some("reflector".to_string());
    }
    if which::which("rate-mirrors").is_ok() {
        return Some("rate-mirrors".to_string());
    }
    None
}

#[tauri::command]
#[specta::specta]
pub async fn rank_mirrors(password: Option<String>) -> Result<String, String> {
    let script = r#"
        echo 'Ranking mirrors by download speed (this may take ~30 seconds)...'
        if [ -f /etc/manjaro-release ] && command -v pacman-mirrors >/dev/null 2>&1; then
            pacman-mirrors -f 5
            echo '✓ Manjaro mirrors ranked successfully.'
        elif command -v reflector >/dev/null 2>&1; then
            reflector --latest 5 --sort rate --save /etc/pacman.d/mirrorlist
            echo '✓ Mirrors ranked successfully. Fastest mirrors are now prioritized.'
        elif command -v rate-mirrors >/dev/null 2>&1; then
            rate-mirrors arch | sudo tee /etc/pacman.d/mirrorlist >/dev/null
            echo '✓ Mirrors ranked successfully using rate-mirrors.'
        else
            echo 'ERROR: Neither reflector nor rate-mirrors is installed (or pacman-mirrors on Manjaro).'
            echo 'Install one: sudo pacman -S reflector'
            exit 1
        fi
    "#;
    utils::run_privileged_script(script, password, false).await
}

#[tauri::command]
#[specta::specta]
pub async fn emit_sync_progress(app: AppHandle, status: String) -> Result<(), String> {
    let _ = app.emit("sync-progress", status);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn force_refresh_databases(
    app: AppHandle,
    state_repo: State<'_, repo_manager::RepoManager>,
    password: Option<String>,
) -> Result<(), String> {
    let _ = app.emit("install-output", "Force refreshing sync databases...");
    let one_click = state_repo.inner().is_one_click_enabled().await;
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ExecuteBatch {
            manifest: crate::models::TransactionManifest {
                refresh_db: true,
                ..Default::default()
            },
        },
        password,
        one_click,
    )
    .await?;
    while let Some(msg) = rx.recv().await {
        let _ = app.emit("install-output", &msg.message);
    }
    crate::repair::write_last_sync_timestamp();
    Ok(())
}

/// Updates system pacman sync DBs (/var/lib/pacman/sync/). At launch we only run when DBs are stale (>6h) so we don't sync every open.
/// Emits to "sync-progress" so the UI can show status.
#[tauri::command]
#[specta::specta]
pub async fn sync_system_databases(
    app: AppHandle,
    state_repo: State<'_, repo_manager::RepoManager>,
    password: Option<String>,
) -> Result<(), String> {
    let _ = app.emit("sync-progress", "Updating package databases...");
    let one_click = state_repo.inner().is_one_click_enabled().await;
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ExecuteBatch {
            manifest: crate::models::TransactionManifest {
                refresh_db: true,
                ..Default::default()
            },
        },
        password,
        one_click,
    )
    .await?;
    while let Some(msg) = rx.recv().await {
        let _ = app.emit("sync-progress", &msg.message);
    }
    let _ = app.emit("sync-progress", "Package databases up to date.");
    crate::repair::write_last_sync_timestamp();
    Ok(())
}
#[tauri::command]
#[specta::specta]
pub async fn open_chaotic_terminal() -> Result<(), String> {
    let script_content = r#"#!/bin/bash
clear
echo -e "\033[1;35mChaotic-AUR Setup\033[0m"
echo "================="
echo "This script will enable the Chaotic-AUR repository on your system."
echo "This provides pre-built binaries for popular AUR packages."
echo ""
echo -e "\033[1;33mWARNING: Trust & Security\033[0m"
echo "Chaotic-AUR is a third-party repository. While widely trusted,"
echo "you are trusting their build servers with your system security."
echo ""
echo "Steps to be performed:"
echo "1. Receive & Sign Chaotic keys"
echo "2. Install keyring & mirrorlist"
echo "3. Configure /etc/pacman.conf"
echo ""
read -p "Press [Enter] to proceed or Ctrl+C to cancel..."

echo ""
echo "1. Installing Keys..."
sudo pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com
sudo pacman-key --lsign-key 3056513887B78AEB

echo ""
echo "2. Installing Repository Packages..."
sudo pacman -U --noconfirm 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst' 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst'

echo ""
echo "3. Updating Configuration..."
if ! grep -q "\[chaotic-aur\]" /etc/pacman.conf; then
    echo "Appending to /etc/pacman.conf..."
    echo -e "\n[chaotic-aur]\nInclude = /etc/pacman.d/chaotic-mirrorlist" | sudo tee -a /etc/pacman.conf
else
    echo "Chaotic-AUR already present in pacman.conf"
fi

echo ""
echo "4. Finalization"
echo "Skipping standalone database refresh to avoid partial-upgrade risk."
echo "Back in MonARCH, use Check Connection / Refresh Databases."

echo ""
echo -e "\033[1;32mSuccess! Chaotic-AUR is now enabled.\033[0m"
echo "You can now close this window."
read -p "Press [Enter] to exit..."
"#;

    let script_path = "/tmp/monarch_chaotic_setup.sh";
    std::fs::write(script_path, script_content).map_err(|e| e.to_string())?;

    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(script_path)
        .map_err(|e| e.to_string())?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(script_path, perms).map_err(|e| e.to_string())?;

    let terminals = [
        ("xdg-terminal-exec", vec![]),
        ("konsole", vec!["-e"]),
        ("gnome-terminal", vec!["--"]),
        ("xfce4-terminal", vec!["-x"]),
        ("kitty", vec![]), // kitty script.sh works
        ("alacritty", vec!["-e"]),
        ("wezterm", vec!["start"]),
        ("terminator", vec!["-x"]),
        ("tilix", vec!["-e"]),
        ("xterm", vec!["-e"]),
    ];

    for (term, args) in terminals {
        if which::which(term).is_ok() {
            let mut cmd = std::process::Command::new(term);
            cmd.args(args);
            // gnome-terminal requires special handling if passing arguments, but for basic script it might be tricky with '--'.
            // Actually, gnome-terminal -- /bin/bash -c "script" is standard.
            // For simplicity, let's try to run the script directly.
            // Most terminals accept [TERMINAL] [ARGS] [COMMAND]

            // Refined args logic:
            if term == "gnome-terminal" {
                cmd.args(["--", "/bin/bash", "-c", script_path]);
            } else if term == "xfce4-terminal" || term == "terminator" {
                cmd.args(["-x", "/bin/bash", "-c", script_path]);
            } else if term == "kitty" || term == "xdg-terminal-exec" {
                cmd.arg(script_path);
            } else {
                // konsole -e, alacritty -e, xterm -e, tilix -e
                cmd.args(["-e", "/bin/bash", "-c", script_path]);
            }

            if cmd.spawn().is_ok() {
                return Ok(());
            }
        }
    }

    Err("No supported terminal emulator found".to_string())
}

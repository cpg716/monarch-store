use crate::{chaotic_api, repo_manager, utils};
use serde::Serialize;
use tauri::State;

/// Embedded Polkit rules for passwordless package-manage (wheel → YES) and script (AUTH_ADMIN_KEEP).
const MONARCH_POLKIT_RULES: &str = include_str!("../../../../rules/10-monarch-store.rules");

#[derive(Serialize)]
pub struct SystemInfo {
    pub kernel: String,
    pub distro: String,
    pub pacman_version: String,
    pub chaotic_enabled: bool,
    pub cpu_optimization: String,
}

#[tauri::command]
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

#[tauri::command]
pub async fn get_all_installed_names() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("pacman")
        .arg("-Qq")
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

#[tauri::command]
pub async fn get_infra_stats(
    state: State<'_, chaotic_api::ChaoticApiClient>,
) -> Result<crate::chaotic_api::InfraStats, String> {
    state.inner().fetch_infra_stats().await
}

#[tauri::command]
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
pub async fn get_repo_states(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<Vec<repo_manager::RepoConfig>, String> {
    Ok(state.inner().get_all_repos().await)
}

#[tauri::command]
pub async fn is_aur_enabled(state: State<'_, repo_manager::RepoManager>) -> Result<bool, String> {
    Ok(state.inner().is_aur_enabled().await)
}

#[tauri::command]
pub async fn toggle_repo(
    app: tauri::AppHandle,
    state: State<'_, repo_manager::RepoManager>,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_repo_state(&app, &name, enabled).await?;
    Ok(())
}

#[tauri::command]
pub async fn toggle_repo_family(
    app: tauri::AppHandle,
    state: State<'_, repo_manager::RepoManager>,
    family: String,
    enabled: bool,
    skip_os_sync: Option<bool>,
) -> Result<(), String> {
    let skip = skip_os_sync.unwrap_or(false);
    state
        .inner()
        .set_repo_family_state(&app, &family, enabled, skip)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn set_aur_enabled(
    app: tauri::AppHandle,
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_aur_enabled(&app, enabled).await;
    Ok(())
}

#[tauri::command]
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
pub async fn set_one_click_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_one_click_enabled(enabled).await;
    Ok(())
}

#[tauri::command]
pub async fn check_security_policy() -> Result<bool, String> {
    let helper_path = std::path::Path::new("/usr/lib/monarch-store/monarch-helper");
    let policy_path = std::path::Path::new("/usr/share/polkit-1/actions/com.monarch.store.policy");

    if !helper_path.exists() || !policy_path.exists() {
        return Ok(false);
    }

    Ok(true)
}

#[tauri::command]
pub async fn install_monarch_policy(
    state: State<'_, repo_manager::RepoManager>,
    password: Option<String>,
) -> Result<String, String> {
    let one_click = state.inner().is_one_click_enabled().await;
    let allow_active = if one_click { "yes" } else { "auth_admin_keep" };

    let policy_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
<policyconfig>
  <vendor>MonARCH Store</vendor>
  <vendor_url>https://github.com/monarch-store/monarch-store</vendor_url>
  <action id="com.monarch.store.package-manage">
    <description>Manage system packages and repositories</description>
    <message>Authentication is required to install, update, or remove applications.</message>
    <icon_name>package-x-generic</icon_name>
    <defaults>
      <allow_any>auth_admin</allow_any>
      <allow_inactive>auth_admin</allow_inactive>
      <allow_active>{}</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.exec.path">/usr/lib/monarch-store/monarch-helper</annotate>
    <annotate key="org.freedesktop.policykit.exec.allow_gui">false</annotate>
  </action>
</policyconfig>"#,
        allow_active
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

    let result = utils::run_privileged_script(&script, password, true).await;
    result
}

#[tauri::command]
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
    utils::run_privileged_script(&script, password, false).await
}
#[tauri::command]
pub async fn trigger_repo_sync(
    app: tauri::AppHandle,
    state_repo: State<'_, repo_manager::RepoManager>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    sync_interval_hours: Option<u64>,
) -> Result<String, String> {
    use tauri::Emitter;
    let interval = sync_interval_hours.unwrap_or(3);

    let _ = app.emit("sync-progress", "Syncing repositories...");
    let repo_res = state_repo
        .inner()
        .sync_all(false, interval, Some(app.clone()))
        .await?;

    let _ = app.emit("sync-progress", "Fetching Chaotic-AUR metadata...");
    let _ = state_chaotic.inner().fetch_packages().await;

    let _ = app.emit("sync-progress", "Initialization complete.");
    Ok(repo_res)
}

#[tauri::command]
pub async fn update_and_install_package(
    app: tauri::AppHandle,
    state_repo: State<'_, repo_manager::RepoManager>,
    name: String,
    _repo_name: Option<String>,
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

    // Step 1: Update system (Sysupgrade)
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::Sysupgrade,
        password.clone(),
    )
    .await?;

    while let Some(msg) = rx.recv().await {
        let _ = app.emit("install-output", &msg.message);
    }

    // Step 2: Install target package (only if Step 1 succeeded)
    let enabled_repos: Vec<String> = state_repo
        .inner()
        .get_all_repos()
        .await
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.name.clone())
        .collect();

    let _ = app.emit(
        "install-output",
        format!("Installing/upgrading {}...", name),
    );

    let mut rx2 = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::AlpmInstall {
            packages: vec![name.clone()],
            sync_first: false,
            enabled_repos,
            cpu_optimization,
        },
        password.clone(),
    )
    .await?;

    while let Some(msg) = rx2.recv().await {
        let _ = app.emit("install-output", &msg.message);
    }

    Ok("System updated and package installed successfully.".to_string())
}
#[tauri::command]
pub async fn is_advanced_mode(state: State<'_, repo_manager::RepoManager>) -> Result<bool, String> {
    Ok(state.inner().is_advanced_mode().await)
}

#[tauri::command]
pub async fn set_advanced_mode(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_advanced_mode(enabled).await;
    Ok(())
}

#[tauri::command]
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
pub async fn is_telemetry_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_telemetry_enabled().await)
}

#[tauri::command]
pub async fn set_telemetry_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    println!("[Backend] Setting telemetry enabled to: {}", enabled);
    state.inner().set_telemetry_enabled(enabled).await;
    Ok(())
}

#[tauri::command]
pub async fn is_notifications_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_notifications_enabled().await)
}

#[tauri::command]
pub async fn set_notifications_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_notifications_enabled(enabled).await;
    Ok(())
}

#[tauri::command]
pub fn get_install_mode_command() -> String {
    match utils::get_install_mode() {
        utils::InstallMode::System => "system".to_string(),
        utils::InstallMode::Portable => "portable".to_string(),
        utils::InstallMode::Dev => "portable".to_string(),
    }
}

#[tauri::command]
pub async fn is_sync_on_startup_enabled(
    state: State<'_, repo_manager::RepoManager>,
) -> Result<bool, String> {
    Ok(state.inner().is_sync_on_startup_enabled().await)
}

#[tauri::command]
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

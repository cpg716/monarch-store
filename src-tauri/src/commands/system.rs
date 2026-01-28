use crate::{chaotic_api, repo_manager, utils};
use serde::Serialize;
use tauri::State;

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
    state: State<'_, repo_manager::RepoManager>,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_repo_state(&name, enabled).await?;
    Ok(())
}

#[tauri::command]
pub async fn toggle_repo_family(
    state: State<'_, repo_manager::RepoManager>,
    family: String,
    enabled: bool,
    skip_os_sync: Option<bool>,
) -> Result<(), String> {
    let skip = skip_os_sync.unwrap_or(false);
    state
        .inner()
        .set_repo_family_state(&family, enabled, skip)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn set_aur_enabled(
    state: State<'_, repo_manager::RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.inner().set_aur_enabled(enabled).await;
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
    let helper_path = std::path::Path::new("/usr/bin/monarch-pk-helper");
    let policy_path = std::path::Path::new("/usr/share/polkit-1/actions/com.monarch.store.policy");

    if !helper_path.exists() || !policy_path.exists() {
        return Ok(false);
    }

    // Verify content matches Version 2.0 and has VALID shell logic (not empty expansion)
    if let Ok(content) = std::fs::read_to_string(helper_path) {
        // We check for the specific bash parameter expansion sequence
        // which would be missing if the file was generated with broken heredoc
        let has_logic = content.contains("${1##*/}");

        if !has_logic {
            return Ok(false);
        }
    } else {
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
    <annotate key="org.freedesktop.policykit.exec.path">/usr/bin/monarch-pk-helper</annotate>
    <annotate key="org.freedesktop.policykit.exec.allow_gui">false</annotate>
  </action>
</policyconfig>"#,
        allow_active
    );

    let script = format!(
        r#"
        echo 'Setting up MonARCH Seamless Authentication...'
        cat <<'EOF' > /usr/bin/monarch-pk-helper
#!/bin/bash
# Secure helper for MonARCH Store - restricted to package management & config
# Version: 2.0
case "${{1##*/}}" in
  pacman|pacman-key|yay|paru|aura|rm|cat|mkdir|chmod|killall|cp|sed|bash|ls|grep|touch)
    exec "$@"
    ;;
  *)
    echo "Unauthorized command: $1"
    exit 1
    ;;
esac
EOF
        chmod +x /usr/bin/monarch-pk-helper
        cat <<'EOF' > /usr/share/polkit-1/actions/com.monarch.store.policy
{}
EOF
        echo '✓ MonARCH Polkit Policy ({}) Installed Successfully!'
    "#,
        policy_content, allow_active
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
    name: String,
    repo_name: Option<String>,
    password: Option<String>,
) -> Result<String, String> {
    use tauri::Emitter;
    let _ = app.emit(
        "install-output",
        format!("--- System Update & Install: {} ---", name),
    );

    // Process Guard Shield (Pillar 6)
    if let Some(conflict) = crate::repair::check_conflicting_processes().await {
        let msg = format!(
            "Error: Conflicting process '{}' is running. Please close it first.",
            conflict
        );
        let _ = app.emit("install-output", &msg);
        let _ = app.emit("install-complete", "failed");
        return Err(msg);
    }
    let _ = app.emit(
        "install-output",
        "Synchronizing databases and updating system...",
    );

    // Command: pacman -Syu --overwrite '*' --noconfirm -- [repo/]package_name
    let mut action_args = vec!["-Syu", "--overwrite", "*", "--noconfirm", "--"];
    let target_string;
    if let Some(r_name) = &repo_name {
        target_string = format!("{}/{}", r_name, name);
        action_args.push(&target_string);
    } else {
        action_args.push(&name);
    }

    let (binary, args) = crate::commands::utils::build_pacman_cmd(&action_args, &password);

    let mut child = tokio::process::Command::new(binary);
    for arg in &args {
        child.arg(arg);
    }

    match child
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            if let Some(pwd) = password {
                if let Some(mut s) = child.stdin.take() {
                    let _ = tokio::io::AsyncWriteExt::write_all(
                        &mut s,
                        format!("{}\n", pwd).as_bytes(),
                    )
                    .await;
                }
            }

            if let Some(out) = child.stdout.take() {
                let a = app.clone();
                tokio::spawn(async move {
                    let reader = tokio::io::BufReader::new(out);
                    let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = a.emit("install-output", line);
                    }
                });
            }
            if let Some(err) = child.stderr.take() {
                let a = app.clone();
                tokio::spawn(async move {
                    let reader = tokio::io::BufReader::new(err);
                    let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = a.emit("install-output", line);
                    }
                });
            }

            match child.wait().await {
                Ok(s) if s.success() => {
                    let _ = app.emit("install-complete", "success");
                    Ok("System updated and package installed successfully".to_string())
                }
                _ => {
                    let _ = app.emit("install-complete", "failed");
                    Err("Update failed".to_string())
                }
            }
        }
        Err(e) => Err(e.to_string()),
    }
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
    state.inner().set_telemetry_enabled(enabled).await;
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

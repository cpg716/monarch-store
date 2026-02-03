use crate::{chaotic_api, repo_manager, utils};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

/// Embedded Polkit rules for passwordless package-manage (wheel → YES) and script (AUTH_ADMIN_KEEP).
const MONARCH_POLKIT_RULES: &str = include_str!("../../../rules/10-monarch-store.rules");
const MONARCH_POLKIT_POLICY: &str = include_str!("../../com.monarch.store.policy");

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

#[derive(Serialize)]
pub struct SystemInfo {
    pub kernel: String,
    pub distro: String,
    pub pacman_version: String,
    pub chaotic_enabled: bool,
    pub cpu_optimization: String,
}

/// Typed response for get_cache_size (replaces raw serde_json::json!).
#[derive(Serialize)]
pub struct CacheSizeResult {
    pub size_bytes: u64,
    pub human_readable: String,
}

/// Typed response for get_orphans_with_size (replaces raw serde_json::json!).
#[derive(Serialize)]
pub struct OrphansWithSizeResult {
    pub orphans: Vec<String>,
    pub total_size_bytes: u64,
    pub human_readable: String,
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
    _password: Option<String>,
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
    password: Option<String>,
) -> Result<(), String> {
    let _password = password;
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
    let policy_content =
        set_policy_allow_active(MONARCH_POLKIT_POLICY, "com.monarch.store.package-manage", allow_active);

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

    // Step 1: Perform full system upgrade (refresh + upgrade) before install.
    let mut sys_rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ExecuteBatch {
            manifest: crate::models::TransactionManifest {
                update_system: true,
                refresh_db: true,
                ..Default::default()
            },
        },
        password.clone(),
    )
    .await?;

    while let Some(msg) = sys_rx.recv().await {
        let _ = app.emit("install-output", &msg.message);
        if msg.message.starts_with("Error:") {
            let _ = app.emit("install-complete", "failed");
            return Err(format!(
                "System update failed while preparing to install {}: {}",
                name, msg.message
            ));
        }
    }

    // Step 2: Install target package (only if Step 1 succeeded)
    let mut enabled_repos: Vec<String> = state_repo
        .inner()
        .get_all_repos()
        .await
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.name.clone())
        .collect();
    for sys in ["core", "extra", "community", "multilib"] {
        if !enabled_repos.contains(&sys.to_string()) {
            enabled_repos.push(sys.to_string());
        }
    }

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
            target_repo: repo_name,
        },
        password.clone(),
    )
    .await?;

    while let Some(msg) = rx2.recv().await {
        let _ = app.emit("install-output", &msg.message);
        if msg.message.starts_with("Error:") {
            let _ = app.emit("install-complete", "failed");
            return Err(format!(
                "Installation failed after system update: {}",
                msg.message
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
    log::info!("Setting telemetry enabled to: {}", enabled);
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

#[tauri::command]
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
                            let parts: Vec<&str> = size_str.trim().split_whitespace().collect();
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

#[tauri::command]
pub async fn set_parallel_downloads(
    count: u32,
    password: Option<String>,
) -> Result<String, String> {
    let script = format!(
        r#"
        echo 'Updating ParallelDownloads in /etc/pacman.conf...'
        cp /etc/pacman.conf /etc/pacman.conf.bak.parallel.$(date +%s) || true
        if grep -q "^ParallelDownloads" /etc/pacman.conf; then
            sed -i "s/^ParallelDownloads.*/ParallelDownloads = {}/" /etc/pacman.conf
        else
            sed -i '/^\[options\]/a ParallelDownloads = {}' /etc/pacman.conf
        fi
        echo '✓ ParallelDownloads set to {}. Restart MonARCH for full effect.'
    "#,
        count, count, count
    );
    utils::run_privileged_script(&script, password, false).await
}

/// Result of testing one mirror: URL and optional latency in ms.
#[derive(serde::Serialize)]
pub struct MirrorTestResult {
    pub url: String,
    pub latency_ms: Option<u32>,
}

/// Test mirrors for a repo without changing system config. Returns top 3 with latency (ms).
/// repo_key: "arch" | "Arch" | "cachyos" | "chaotic-aur" (others fall back to arch or N/A).
#[tauri::command]
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
    utils::run_privileged_script(&script, password, false).await
}

#[tauri::command]
pub async fn emit_sync_progress(app: AppHandle, status: String) -> Result<(), String> {
    let _ = app.emit("sync-progress", status);
    Ok(())
}

#[tauri::command]
pub async fn force_refresh_databases(
    app: AppHandle,
    password: Option<String>,
) -> Result<(), String> {
    let _ = app.emit("install-output", "Force refreshing sync databases...");
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ExecuteBatch {
            manifest: crate::models::TransactionManifest {
                refresh_db: true,
                ..Default::default()
            },
        },
        password,
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
pub async fn sync_system_databases(app: AppHandle, password: Option<String>) -> Result<(), String> {
    let _ = app.emit("sync-progress", "Updating package databases...");
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ExecuteBatch {
            manifest: crate::models::TransactionManifest {
                refresh_db: true,
                ..Default::default()
            },
        },
        password,
    )
    .await?;
    while let Some(msg) = rx.recv().await {
        let _ = app.emit("sync-progress", &msg.message);
    }
    let _ = app.emit("sync-progress", "Package databases up to date.");
    crate::repair::write_last_sync_timestamp();
    Ok(())
}

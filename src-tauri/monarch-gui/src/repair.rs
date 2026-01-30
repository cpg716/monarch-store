use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthIssue {
    pub category: String,
    pub severity: String,
    pub message: String,
    pub action_label: String,
    pub action_command: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InitializationStatus {
    pub needs_policy: bool,
    pub needs_keyring: bool,
    pub needs_migration: bool,
    pub is_healthy: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct RepairStep {
    pub name: String,
    pub status: String, // "pending", "running", "success", "failed"
    pub logs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyringStatus {
    pub healthy: bool,
    pub message: String,
}

#[tauri::command]
pub async fn check_keyring_health() -> Result<KeyringStatus, String> {
    let output = Command::new("pacman-key")
        .arg("--list-keys")
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(KeyringStatus {
            healthy: true,
            message: "Keyring appears healthy".to_string(),
        })
    } else {
        Ok(KeyringStatus {
            healthy: false,
            message: "pacman-key check failed".to_string(),
        })
    }
}

// Helper to run privileged commands and stream output
async fn run_privileged(
    app: &AppHandle,
    cmd: &str,
    args: &[&str],
    password: Option<String>,
    event_name: &str,
    bypass_helper: bool,
) -> Result<(), String> {
    use crate::error_classifier::ClassifiedError;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Acquire Lock
    let _guard = crate::utils::PRIVILEGED_LOCK.lock().await;

    let (binary, final_args) = if password.is_none() {
        // Use pkexec with helper if available
        let helper = crate::utils::MONARCH_PK_HELPER;

        if std::path::Path::new(helper).exists() && !bypass_helper {
            // Use Helper as Proxy for Polkit authorization
            let helper_cmd = crate::helper_client::HelperCommand::RunCommand {
                binary: cmd.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
            };
            let json = serde_json::to_string(&helper_cmd).unwrap_or_default();
            (
                "/usr/bin/pkexec".to_string(),
                vec![helper.to_string(), json],
            )
        } else {
            // Direct pkexec fallback (will prompt)
            let mut all_args = Vec::new();
            all_args.push(cmd.to_string());
            for a in args {
                all_args.push(a.to_string());
            }
            ("/usr/bin/pkexec".to_string(), all_args)
        }
    } else {
        // Use sudo
        let mut all_args = vec!["-S".to_string(), cmd.to_string()];
        for a in args {
            all_args.push(a.to_string());
        }
        ("/usr/bin/sudo".to_string(), all_args)
    };

    let mut command = Command::new(&binary);
    command.args(&final_args);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    if password.is_some() {
        command.stdin(Stdio::piped());
    }

    let mut child = command.spawn().map_err(|e| e.to_string())?;

    if let Some(pwd) = password {
        if let Some(mut stdin) = child.stdin.take() {
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut stdin, format!("{}\n", pwd).as_bytes())
                    .await;
            let _ = tokio::io::AsyncWriteExt::flush(&mut stdin).await;
            let _ = tokio::io::AsyncWriteExt::shutdown(&mut stdin).await;
        }
    }

    let error_buffer: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Stream Output
    if let Some(stdout) = child.stdout.take() {
        let app = app.clone();
        let event = event_name.to_string();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app.emit(&event, line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let app = app.clone();
        let event = event_name.to_string();
        let err_buf = error_buffer.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                {
                    let mut buf = err_buf.lock().await;
                    buf.push(line.clone());
                }
                let _ = app.emit(&event, format!("ERROR: {}", line));
            }
        });
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        let errors = error_buffer.lock().await;
        let combined_output = errors.join("\n");

        if let Some(classified) = ClassifiedError::from_output(&combined_output) {
            let _ = app.emit("repair-error-classified", &classified);
            Err(format!("{}: {}", classified.title, classified.description))
        } else {
            Err(format!(
                "Operation failed (Exit Code: {:?}). Output: {}",
                status.code(),
                combined_output
            ))
        }
    }
}

#[tauri::command]
pub async fn repair_unlock_pacman(app: AppHandle, password: Option<String>) -> Result<(), String> {
    let _ = app.emit("repair-log", "🔓 Unlocking Pacman DB...");

    // SECURITY: Safe Lock Removal — use Helper RemoveLock when one-click (no password)
    // so Polkit authorizes the helper; RunCommand(rm) is rejected by the helper.
    if password.is_none() {
        let mut rx = crate::helper_client::invoke_helper(
            &app,
            crate::helper_client::HelperCommand::RemoveLock,
            None,
        )
        .await?;
        let mut last = None;
        while let Ok(msg) = rx.recv().await {
            let _ = app.emit("repair-log", &msg.message);
            last = Some(msg);
        }
        if let Some(m) = last {
            if m.progress == 0 && m.message.starts_with("Error:") {
                return Err(m.message);
            }
        }
        let _ = app.emit("repair-log", "✅ Pacman DB unlocked.");
        return Ok(());
    }

    // Sudo path: safe lock removal with pgrep check, then rm via run_privileged
    let is_running = std::process::Command::new("pgrep")
        .arg("-x")
        .arg("pacman")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if is_running {
        let _ = app.emit(
            "repair-log",
            "❌ Safety Check Failed: Pacman is currently running.",
        );
        return Err("Cannot remove lock: Pacman process detected.".to_string());
    }

    run_privileged(
        &app,
        "rm",
        &["-f", "/var/lib/pacman/db.lck"][..],
        password,
        "repair-log",
        false,
    )
    .await?;

    let _ = app.emit("repair-log", "✅ Pacman DB unlocked.");
    Ok(())
}

#[tauri::command]
pub async fn fix_keyring_issues(app: AppHandle, password: Option<String>) -> Result<(), String> {
    let _ = app.emit(
        "repair-log",
        "🔑 Starting Unified Keyring & Security Repair...",
    );

    let script = r#"
        echo "--- Resetting GPG Keyring (Deep Clean) ---"
        
        # 1. Kill locking agents
        killall gpg-agent dirmngr 2>/dev/null || true
        
        # 2. Re-Install Security Policy (Ensures standard access)
        mkdir -p /usr/share/polkit-1/actions
        cat <<'EOF' > /usr/share/polkit-1/actions/com.monarch.store.policy
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
<policyconfig>
  <vendor>MonARCH Store</vendor>
  <action id="com.monarch.store.package-manage">
    <description>Manage system packages</description>
    <message>Authentication required</message>
    <defaults>
      <allow_any>auth_admin</allow_any>
      <allow_inactive>auth_admin</allow_inactive>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.exec.path">/usr/lib/monarch-store/monarch-helper</annotate>
    <annotate key="org.freedesktop.policykit.exec.allow_gui">false</annotate>
  </action>
</policyconfig>
EOF
        chmod 644 /usr/share/polkit-1/actions/com.monarch.store.policy

        # 3. Nuke and Pave GPG files
        rm -rf /etc/pacman.d/gnupg
        pacman-key --init
        
        # Distro-aware population
        if [ -f /etc/manjaro-release ]; then
            pacman-key --populate manjaro archlinux
        else
            pacman-key --populate archlinux
        fi
        
        # 4. Sync Keyring Packages (use -Syu to avoid partial upgrade)
        if [ -f /etc/manjaro-release ]; then
            pacman -Syu --noconfirm manjaro-keyring archlinux-keyring
        else
            pacman -Syu --noconfirm archlinux-keyring
        fi
        
        # 5. Import Third Party Keys
        echo "Importing Chaotic-AUR, CachyOS, and Garuda Keys..."
        # Chaotic
        pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com && pacman-key --lsign-key 3056513887B78AEB
        # CachyOS
        pacman-key --recv-key F3B607488DB35A47 --keyserver keyserver.ubuntu.com && pacman-key --lsign-key F3B607488DB35A47
        # Garuda
        pacman-key --recv-key 349BC7808577C592 --keyserver keyserver.ubuntu.com && pacman-key --lsign-key 349BC7808577C592

        echo "✅ Keyring & Policy Reset Complete."
    "#;

    run_privileged(
        &app,
        "bash",
        &["-c", script][..],
        password,
        "repair-log",
        true, // Bypass to overwrite broken policy/helper
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn initialize_system(
    app: AppHandle,
    state: tauri::State<'_, crate::repo_manager::RepoManager>,
    password: Option<String>,
) -> Result<String, String> {
    // This is the "God Mode" init - it does Bootstrap + Keyring if needed
    // But we'll mostly use it to call bootstrap_system which is already robustly built

    let one_click = state.inner().is_one_click_enabled().await;

    // We reuse bootstrap_system but ensured it's called with ONE password
    crate::repo_setup::bootstrap_system(app, state, password, Some(one_click)).await
}

#[tauri::command]
pub async fn repair_emergency_sync(app: AppHandle, password: Option<String>) -> Result<(), String> {
    let _ = app.emit("repair-log", "🚑 Starting Emergency Sync (pacman -Syu)...");

    // This is the big one.
    run_privileged(
        &app,
        "pacman",
        &["-Syu", "--noconfirm"][..],
        password,
        "repair-log",
        false, // Normal update is fine with helper
    )
    .await?;

    let _ = app.emit("repair-log", "✅ Emergency Sync Complete.");
    Ok(())
}

#[tauri::command]
pub async fn check_initialization_status() -> Result<InitializationStatus, String> {
    let mut reasons = Vec::new();

    // 1. Check Policy
    let has_policy =
        std::path::Path::new("/usr/share/polkit-1/actions/com.monarch.store.policy").exists();
    let has_helper = std::path::Path::new(crate::utils::MONARCH_PK_HELPER).exists();
    let needs_policy = !has_policy || !has_helper;

    if !has_policy {
        reasons.push("Security policy is missing or not installed properly.".to_string());
    }
    if !has_helper {
        reasons.push(
            "Authentication helper binary is missing from /usr/lib/monarch-store/.".to_string(),
        );
    }

    // 2. Check Keyring (Quick check for existence of gnupg dir)
    let needs_keyring = !std::path::Path::new("/etc/pacman.d/gnupg").exists();
    if needs_keyring {
        reasons.push("System GPG keyring is missing or uninitialized.".to_string());
    }

    // 3. Check Migration (Modular Include)
    let conf = std::fs::read_to_string("/etc/pacman.conf").unwrap_or_default();
    let needs_migration = !conf.contains("/etc/pacman.d/monarch/*.conf");
    if needs_migration {
        reasons.push("System is not using the MonARCH modular pacman configuration.".to_string());
    }

    let is_healthy = !needs_policy && !needs_keyring && !needs_migration;

    Ok(InitializationStatus {
        needs_policy,
        needs_keyring,
        needs_migration,
        is_healthy,
        reasons,
    })
}

#[tauri::command]
pub async fn check_system_health() -> Result<Vec<HealthIssue>, String> {
    let mut issues = Vec::new();

    // 1. Check for pacman executable
    if !std::path::Path::new("/usr/bin/pacman").exists() {
        issues.push(HealthIssue {
            category: "System".to_string(),
            severity: "Critical".to_string(),
            message: "Pacman package manager not found.".to_string(),
            action_label: "Install Pacman".to_string(),
            action_command: None,
        });
    }

    // 2. Hardware/CPU Check
    let opt_level = if crate::utils::is_cpu_znver4_compatible() {
        "Zen 4/5 (Extreme)"
    } else if crate::utils::is_cpu_v4_compatible() {
        "v4 (AVX-512)"
    } else if crate::utils::is_cpu_v3_compatible() {
        "v3 (AVX2)"
    } else {
        "v1 (Standard x86-64)"
    };
    issues.push(HealthIssue {
        category: "Hardware".to_string(),
        severity: "Info".to_string(),
        message: format!("Hardware Optimization Level: {}", opt_level),
        action_label: "View Optimization Guide".to_string(),
        action_command: None,
    });

    // 3. Critical Keyring Check
    if !std::path::Path::new("/etc/pacman.d/gnupg").exists() {
        issues.push(HealthIssue {
            category: "Keyring".to_string(),
            severity: "Critical".to_string(),
            message: "System Keyring is corrupted or uninitialized.".to_string(),
            action_label: "Initialize Keyring".to_string(),
            action_command: Some("keyring".to_string()),
        });
    }

    // 4. Check dependencies (Non-root)
    let deps = ["base-devel", "git", "checkupdates"];
    for dep in deps {
        let has_dep = std::process::Command::new("pacman")
            .args(["-Qq", dep])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !has_dep {
            issues.push(HealthIssue {
                category: "Dependency".to_string(),
                severity: "Critical".to_string(),
                message: format!("Missing essential build dependency: {}", dep),
                action_label: format!("Install {}", dep),
                action_command: Some(format!("install_dep_{}", dep)), // We'll handle this in the frontend or add a generic install_dep
            });
        }
    }

    Ok(issues)
}

#[tauri::command]
pub async fn fix_keyring_issues_alias(
    app: AppHandle,
    password: Option<String>,
) -> Result<(), String> {
    fix_keyring_issues(app, password).await
}

#[tauri::command]
pub async fn check_pacman_lock() -> bool {
    std::path::Path::new("/var/lib/pacman/db.lck").exists()
}


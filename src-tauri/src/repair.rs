use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct RepairStep {
    pub name: String,
    pub status: String, // "pending", "running", "success", "failed"
    pub logs: Vec<String>,
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
    // Acquire Lock
    let _guard = crate::utils::PRIVILEGED_LOCK.lock().await;

    let (binary, final_args) = if password.is_none() {
        // Use pkexec with helper if available
        let helper = "/usr/bin/monarch-pk-helper";
        let mut all_args = Vec::new();

        if std::path::Path::new(helper).exists() && !bypass_helper {
            all_args.push(helper.to_string());
        }
        all_args.push(cmd.to_string());
        for a in args {
            all_args.push(a.to_string());
        }
        ("/usr/bin/pkexec".to_string(), all_args)
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
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app.emit(&event, line);
            }
        });
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Command failed with exit code: {:?}",
            status.code()
        ))
    }
}

#[tauri::command]
pub async fn repair_unlock_pacman(app: AppHandle, password: Option<String>) -> Result<(), String> {
    let _ = app.emit("repair-log", "🔓 Unlocking Pacman DB...");

    // remove /var/lib/pacman/db.lck
    run_privileged(
        &app,
        "rm",
        &["-f", "/var/lib/pacman/db.lck"][..],
        password,
        "repair-log",
        false, // Standard rm is safe with helper
    )
    .await?;

    let _ = app.emit("repair-log", "✅ Pacman DB unlocked.");
    Ok(())
}

#[tauri::command]
pub async fn repair_reset_keyring(app: AppHandle, password: Option<String>) -> Result<(), String> {
    let _ = app.emit(
        "repair-log",
        "🔑 Starting Unified Keyring & Security Repair...",
    );

    let script = r#"
        echo "--- Resetting GPG Keyring (Deep Clean) ---"
        
        # 1. Kill locking agents
        killall gpg-agent dirmngr 2>/dev/null || true
        
        # 2. Re-Install Security Policy & Helper (Ensures standard access)
        mkdir -p /usr/share/polkit-1/actions
        cat <<'EOF' > /usr/bin/monarch-pk-helper
#!/bin/bash
case "${1##*/}" in
  pacman|pacman-key|yay|paru|aura|rm|cat|mkdir|chmod|killall|cp|sed|bash|ls|grep|touch|checkupdates)
    exec "$@" ;;
  *)
    echo "Unauthorized: $1"; exit 1 ;;
esac
EOF
        chmod +x /usr/bin/monarch-pk-helper

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
      <allow_active>yes</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.exec.path">/usr/bin/monarch-pk-helper</annotate>
    <annotate key="org.freedesktop.policykit.exec.allow_gui">false</annotate>
  </action>
</policyconfig>
EOF
        chmod 644 /usr/share/polkit-1/actions/com.monarch.store.policy

        # 3. Nuke and Pave GPG files
        rm -rf /etc/pacman.d/gnupg
        pacman-key --init
        pacman-key --populate archlinux
        
        # 4. Sync Keyring Packages
        pacman -Sy --noconfirm archlinux-keyring
        
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
pub async fn check_pacman_lock() -> bool {
    std::path::Path::new("/var/lib/pacman/db.lck").exists()
}

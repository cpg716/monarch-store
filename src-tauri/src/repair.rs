use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
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
) -> Result<(), String> {
    let (binary, final_args) = if password.is_none() {
        // Use pkexec
        let mut all_args = vec![cmd.to_string()];
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
        &["-f", "/var/lib/pacman/db.lck"],
        password,
        "repair-log",
    )
    .await?;

    let _ = app.emit("repair-log", "✅ Pacman DB unlocked.");
    Ok(())
}

#[tauri::command]
pub async fn repair_reset_keyring(app: AppHandle, password: Option<String>) -> Result<(), String> {
    let _ = app.emit("repair-log", "🔑 Resetting GPG Keyring...");

    // 1. Remove trusted.gpg.d and gnupg dir (Backup logic could be added but usually better to nuke for repair)
    // We follow arch_fix.sh logic largely
    let _ = app.emit("repair-log", "Cleaning old GPG files...");
    run_privileged(
        &app,
        "rm",
        &["-rf", "/etc/pacman.d/gnupg"],
        password.clone(),
        "repair-log",
    )
    .await?;

    // 2. Init
    let _ = app.emit("repair-log", "Initializing Keyring...");
    run_privileged(
        &app,
        "pacman-key",
        &["--init"],
        password.clone(),
        "repair-log",
    )
    .await?;

    // 3. Populate
    let _ = app.emit("repair-log", "Populating Arch Keys...");
    run_privileged(
        &app,
        "pacman-key",
        &["--populate", "archlinux"],
        password.clone(),
        "repair-log",
    )
    .await?;

    // 4. Refresh keys (Optional, often slow, omitted in fast repair usually, but maybe good)
    // arch_fix.sh doesn't explicitly refresh-keys, just populates.

    let _ = app.emit("repair-log", "✅ Keyring Reset Complete.");
    Ok(())
}

#[tauri::command]
pub async fn repair_emergency_sync(app: AppHandle, password: Option<String>) -> Result<(), String> {
    let _ = app.emit("repair-log", "🚑 Starting Emergency Sync (pacman -Syu)...");

    // This is the big one.
    run_privileged(
        &app,
        "pacman",
        &["-Syu", "--noconfirm"],
        password,
        "repair-log",
    )
    .await?;

    let _ = app.emit("repair-log", "✅ Emergency Sync Complete.");
    Ok(())
}

#[tauri::command]
pub async fn check_pacman_lock() -> bool {
    std::path::Path::new("/var/lib/pacman/db.lck").exists()
}

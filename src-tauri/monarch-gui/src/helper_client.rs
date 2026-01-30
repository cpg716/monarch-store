use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "command", content = "payload")]
pub enum HelperCommand {
    // ✅ NEW: Full ALPM Transactions
    AlpmInstall {
        packages: Vec<String>,
        sync_first: bool,
        enabled_repos: Vec<String>,
        cpu_optimization: Option<String>,
    },
    AlpmUninstall {
        packages: Vec<String>,
        remove_deps: bool,
    },
    AlpmUpgrade {
        packages: Option<Vec<String>>,
        enabled_repos: Vec<String>,
    },
    AlpmSync {
        enabled_repos: Vec<String>,
    },
    AlpmInstallFiles {
        paths: Vec<String>,
    },
    // Legacy commands (kept for compatibility)
    InstallTargets {
        packages: Vec<String>,
    },
    InstallFiles {
        paths: Vec<String>,
    },
    Sysupgrade,
    Refresh,
    Initialize,
    UninstallTargets {
        packages: Vec<String>,
    },
    RemoveOrphans,
    ClearCache {
        keep: u32,
    },
    RemoveLock,
    ConfigureRepo {
        name: String,
        enabled: bool,
        url: String,
    },
    WriteFile {
        path: String,
        content: String,
    },
    RemoveFile {
        path: String,
    },
    WriteFiles {
        files: Vec<(String, String)>,
    },
    RemoveFiles {
        paths: Vec<String>,
    },
    RunCommand {
        binary: String,
        args: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProgressMessage {
    pub progress: u8,
    pub message: String,
}

/// Temp file prefix for helper command (helper deletes after reading).
const CMD_FILE_PREFIX: &str = "monarch-cmd-";

pub async fn invoke_helper(
    app: &AppHandle,
    cmd: HelperCommand,
    password: Option<String>,
) -> Result<tokio::sync::mpsc::Receiver<ProgressMessage>, String> {
    let json = serde_json::to_string(&cmd).map_err(|e| e.to_string())?;

    // Pass command via temp file in /tmp so root (pkexec) can read it; helper reads and deletes the file.
    let cmd_path = {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::path::Path::new("/tmp").join(format!("{}{}.json", CMD_FILE_PREFIX, ts));
        std::fs::write(&path, json.as_bytes())
            .map_err(|e| format!("Failed to write command file: {}", e))?;
        path
    };

    // Phase 1: Production Path Priority — Polkit only respects the path in the .policy file.
    // Check FIRST: use /usr/lib/monarch-store/monarch-helper if it exists (Prod/Hybrid → passwordless).
    // If not, fallback to local target/debug (Pure Dev → accept password prompt).
    let mut helper_bin = crate::utils::MONARCH_PK_HELPER.to_string();
    let production_path = std::path::Path::new(crate::utils::MONARCH_PK_HELPER);

    if production_path.exists() {
        // Always use production path so pkexec matches the policy; enables passwordless rules.
        helper_bin = crate::utils::MONARCH_PK_HELPER.to_string();
    } else {
        // Pure Dev: no installed helper; use local binary (path won't match policy → password prompt).
        if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
            let dev_helper = std::path::Path::new(&target_dir).join("debug").join("monarch-helper");
            if dev_helper.exists() {
                helper_bin = dev_helper.to_string_lossy().to_string();
            }
        }
        if helper_bin == crate::utils::MONARCH_PK_HELPER {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let dev_helper = exe_dir.join("monarch-helper");
                    if dev_helper.exists() {
                        helper_bin = dev_helper.to_string_lossy().to_string();
                    }
                }
            }
        }
        if helper_bin == crate::utils::MONARCH_PK_HELPER {
            let fallbacks = [
                "src-tauri/target/debug/monarch-helper",
                "./src-tauri/target/debug/monarch-helper",
                "../target/debug/monarch-helper",
                "./target/debug/monarch-helper",
            ];
            for path in fallbacks {
                if std::path::Path::new(path).exists() {
                    if let Ok(canon) = std::path::Path::new(path).canonicalize() {
                        helper_bin = canon.to_string_lossy().to_string();
                        break;
                    }
                }
            }
        }
    }

    let _ = app.emit(
        "helper-output",
        format!("[Client]: Seeking helper at: {}", helper_bin),
    );

    // Pass path to command file so helper reads JSON from file (avoids argv length/escaping).
    let cmd_path_str = cmd_path.to_string_lossy().to_string();
    let mut command = if password.is_some() {
        let mut c = tokio::process::Command::new("sudo");
        c.args(["-S", &helper_bin, &cmd_path_str]);
        c
    } else {
        let mut c = tokio::process::Command::new("pkexec");
        c.args([&helper_bin, &cmd_path_str]);
        c
    };

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            // SECURITY: Delete temp command file on spawn failure so it does not linger with payload
            let _ = std::fs::remove_file(&cmd_path);
            format!(
                "Failed to spawn monarch-helper ({}): {}. Ensure Polkit policy is installed.",
                helper_bin, e
            )
        })?;

    let (tx, rx) = tokio::sync::mpsc::channel(100);

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        // Only write password to stdin (for sudo -S). Command is passed via argv.
        if let Some(pwd) = &password {
            stdin.write_all(format!("{}\n", pwd).as_bytes()).await.map_err(|e| e.to_string())?;
        }
        stdin.shutdown().await.ok();
    }

    if let Some(stdout) = child.stdout.take() {
        let a = app.clone();
        let tx_stdout = tx.clone();
        tokio::spawn(async move {
            let reader = TokioBufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.starts_with('{') {
                    // Try to parse as AlpmProgressEvent first (new structured events)
                    if let Ok(event) = serde_json::from_str::<crate::alpm_progress::AlpmProgressEvent>(&line) {
                        // Emit structured ALPM event
                        let _ = a.emit("alpm-progress", &event);
                        // Also convert to ProgressMessage for backward compatibility
                        let msg = ProgressMessage {
                            progress: event.percent.unwrap_or(0),
                            message: event.message,
                        };
                        let _ = tx_stdout.send(msg).await;
                    } else if let Ok(msg) = serde_json::from_str::<ProgressMessage>(&line) {
                        // Legacy ProgressMessage format
                        let _ = tx_stdout.send(msg).await;
                    }
                }
                let _ = a.emit("helper-output", format!("[Helper]: {}", line));
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let a = app.clone();
        tokio::spawn(async move {
            let reader = TokioBufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = a.emit("helper-output", format!("[Helper Error]: {}", line));
            }
        });
    }

    tokio::spawn(async move {
        let status = child.wait().await;
        if let Ok(s) = status {
            if !s.success() {
                let _ = tx
                    .send(ProgressMessage {
                        progress: 0,
                        message: format!("Error: Helper process exited with status {}", s),
                    })
                    .await;
            }
        }
    });

    Ok(rx)
}

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};

/// Minimum interval between helper invocations (debounce) to mitigate DoS from rapid/spam invokes.
const HELPER_DEBOUNCE: Duration = Duration::from_millis(800);

static LAST_HELPER_INVOKE: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));

#[cfg(test)]
mod tests {
    use super::HelperCommand;
    use serde_json;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_helper_command_serialization_matches_helper() {
        let cmd = HelperCommand::AlpmInstall {
            packages: vec!["firefox".to_string()],
            sync_first: true,
            enabled_repos: vec!["core".to_string(), "chaotic-aur".to_string()],
            cpu_optimization: Some("v3".to_string()),
            target_repo: None,
        };

        let json = serde_json::to_string(&cmd).expect("Should serialize");
        assert!(json.contains("\"command\":\"AlpmInstall\""));
        assert!(json.contains("firefox"));
        assert!(json.contains("chaotic-aur"));
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }

    #[test]
    fn test_command_file_write_simulation() {
        let cmd = HelperCommand::AlpmInstall {
            packages: vec!["test-pkg".to_string()],
            sync_first: false,
            enabled_repos: vec!["cachyos".to_string()],
            cpu_optimization: None,
            target_repo: None,
        };

        let json = serde_json::to_string(&cmd).expect("Should serialize");
        let mut file = NamedTempFile::new().expect("Should create temp file");
        file.write_all(json.as_bytes()).expect("Should write");
        file.flush().expect("Should flush");

        let contents = std::fs::read_to_string(file.path()).expect("Should read");
        assert!(!contents.trim().is_empty());
        assert_eq!(contents.trim(), json);

        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&contents.trim());
        assert!(parsed.is_ok(), "File content should be valid JSON");
    }

    #[test]
    fn test_no_raw_strings_in_commands() {
        let repos = vec!["cachyos".to_string()];
        let cmd = HelperCommand::AlpmInstall {
            packages: vec!["pkg".to_string()],
            sync_first: true,
            enabled_repos: repos,
            cpu_optimization: None,
            target_repo: None,
        };

        let json = serde_json::to_string(&cmd).expect("Should serialize");
        assert_ne!(json.trim(), "\"cachyos\"");
        assert!(json.starts_with('{'));
        assert!(json.contains("AlpmInstall"));
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "command", content = "payload")]
pub enum HelperCommand {
    // ✅ NEW: Full ALPM Transactions
    AlpmInstall {
        packages: Vec<String>,
        sync_first: bool,
        enabled_repos: Vec<String>,
        cpu_optimization: Option<String>,
        target_repo: Option<String>,
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
    ForceRefreshDb,
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
/// Use /var/tmp so both the app and root (sudo) see the same path.
const CMD_FILE_DIR: &str = "/var/tmp";

/// When password is provided: use sudo -S so user entered password once (e.g. onboarding "reduce prompts").
/// When password is None: use pkexec so Polkit policy applies (one system prompt per call, or none if rules allow).
pub async fn invoke_helper(
    app: &AppHandle,
    cmd: HelperCommand,
    password: Option<String>,
) -> Result<tokio::sync::mpsc::Receiver<ProgressMessage>, String> {
    // SECURITY: Debounce to limit rapid helper invocations (mitigates DoS from malformed/spam calls).
    let wait_duration = {
        let mut guard = LAST_HELPER_INVOKE.lock().map_err(|e| e.to_string())?;
        let now = Instant::now();
        let wait = if let Some(prev) = *guard {
            let elapsed = prev.elapsed();
            if elapsed < HELPER_DEBOUNCE {
                Some(HELPER_DEBOUNCE - elapsed)
            } else {
                *guard = Some(now);
                None
            }
        } else {
            *guard = Some(now);
            None
        };
        wait
    };
    if let Some(wait) = wait_duration {
        tokio::time::sleep(wait).await;
        if let Ok(mut g) = LAST_HELPER_INVOKE.lock() {
            *g = Some(Instant::now());
        }
    }

    let json = serde_json::to_string(&cmd).map_err(|e| e.to_string())?;

    let use_stdin = password.is_none();
    let cmd_path_opt: Option<std::path::PathBuf> = if use_stdin {
        None
    } else {
        if let Err(e) = std::fs::create_dir_all(CMD_FILE_DIR) {
            return Err(format!(
                "Failed to create command directory {}: {}",
                CMD_FILE_DIR, e
            ));
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::path::Path::new(CMD_FILE_DIR).join(format!("{}{}.json", CMD_FILE_PREFIX, ts));
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&path)
                .map_err(|e| format!("Failed to create command file {}: {}", path.display(), e))?;
            file.write_all(json.as_bytes())
                .map_err(|e| format!("Failed to write JSON to file: {}", e))?;
            file.sync_all()
                .map_err(|e| format!("Failed to sync file to disk: {}", e))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
                .map_err(|e| format!("Failed to set file permissions: {}", e))?;
        }
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to verify command file: {}", e))?;
        if contents.trim().is_empty() {
            let _ = std::fs::remove_file(&path);
            return Err("Command file was empty after write".to_string());
        }
        if contents.trim() != json.trim() {
            let _ = std::fs::remove_file(&path);
            return Err("Command file content mismatch".to_string());
        }
        let canonical = path.canonicalize().unwrap_or(path);
        Some(canonical)
    };

    // Helper selection:
    // - In debug builds, hard-lock to the dev helper to avoid version mismatches.
    // - In release builds, MONARCH_USE_PRODUCTION_HELPER=1 forces /usr/lib/monarch-store/monarch-helper.
    // - Otherwise, prefer dev helper when it exists, then fall back to production path.
    let mut helper_bin = crate::utils::MONARCH_PK_HELPER.to_string();
    let production_path = std::path::Path::new(crate::utils::MONARCH_PK_HELPER);
    let force_production = std::env::var("MONARCH_USE_PRODUCTION_HELPER").as_deref() == Ok("1");

    // Single source of truth: utils::get_dev_helper_path() (same resolution as onboarding deployment).
    let dev_helper_path = crate::utils::get_dev_helper_path()
        .map(|p| p.to_string_lossy().to_string());

    if cfg!(debug_assertions) {
        if let Some(dev) = dev_helper_path {
            helper_bin = dev;
        } else {
            let cwd = std::env::current_dir().unwrap_or_default();
            let exe = std::env::current_exe().unwrap_or_default();
            return Err(format!(
                "Dev helper not found. Build it first: run 'npm run tauri dev' from the project root (builds the helper), or manually: cd src-tauri && cargo build -p monarch-helper. Expected at src-tauri/target/debug/monarch-helper (cwd={}, exe={})",
                cwd.display(),
                exe.display()
            ));
        }
    } else if force_production && production_path.exists() {
        helper_bin = crate::utils::MONARCH_PK_HELPER.to_string();
    } else if let Some(dev) = dev_helper_path {
        helper_bin = dev;
    } else if production_path.exists() {
        helper_bin = crate::utils::MONARCH_PK_HELPER.to_string();
    }

    let _ = app.emit(
        "helper-output",
        format!(
            "[Client]: Seeking helper at: {} (command via {})",
            helper_bin,
            if use_stdin {
                "stdin, Polkit"
            } else {
                "file, sudo (one password)"
            }
        ),
    );

    let mut command = if let Some(ref cmd_path) = cmd_path_opt {
        if !cmd_path.exists() {
            return Err(format!(
                "Command file does not exist: {}",
                cmd_path.display()
            ));
        }
        let mut c = tokio::process::Command::new("sudo");
        c.env("MONARCH_CMD_JSON", &json);
        c.env("MONARCH_CMD_FILE", cmd_path.to_string_lossy().as_ref());
        c.args(["-E", "-S", &helper_bin, cmd_path.to_string_lossy().as_ref()]);
        c
    } else {
        let mut c = tokio::process::Command::new("pkexec");
        c.arg("--disable-internal-agent");
        c.arg(&helper_bin);
        c
    };

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if let Some(ref p) = cmd_path_opt {
                let _ = std::fs::remove_file(p);
            }
            format!(
                "Failed to spawn monarch-helper ({}): {}. {}",
                helper_bin,
                e,
                if use_stdin {
                    "Ensure Polkit policy is installed."
                } else {
                    "Check sudo access."
                }
            )
        })?;

    let (tx, rx) = tokio::sync::mpsc::channel(100);

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        if use_stdin {
            stdin
                .write_all(json.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            stdin.write_all(b"\n").await.map_err(|e| e.to_string())?;
        } else {
            if let Some(pwd) = &password {
                stdin
                    .write_all(format!("{}\n", pwd).as_bytes())
                    .await
                    .map_err(|e| e.to_string())?;
            }
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
                    if let Ok(event) =
                        serde_json::from_str::<crate::alpm_progress::AlpmProgressEvent>(&line)
                    {
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

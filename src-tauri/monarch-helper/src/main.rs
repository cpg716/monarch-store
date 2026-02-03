mod alpm_errors;
mod logger;
mod progress;
mod safe_transaction;
mod self_healer;
mod transactions;

#[cfg(test)]
mod command_tests {
    use super::HelperCommand;
    use serde_json;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_command_serialization_alpm_install() {
        let cmd = HelperCommand::AlpmInstall {
            packages: vec!["firefox".to_string(), "vlc".to_string()],
            sync_first: true,
            enabled_repos: vec![
                "core".to_string(),
                "extra".to_string(),
                "chaotic-aur".to_string(),
            ],
            cpu_optimization: Some("v3".to_string()),
            target_repo: None,
        };

        let json = serde_json::to_string(&cmd).expect("Should serialize");
        assert!(json.contains("AlpmInstall"));
        assert!(json.contains("firefox"));
        assert!(json.contains("chaotic-aur"));
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));

        let parsed: HelperCommand = serde_json::from_str(&json).expect("Should deserialize");
        match parsed {
            HelperCommand::AlpmInstall {
                packages,
                sync_first,
                enabled_repos,
                cpu_optimization,
                target_repo,
            } => {
                assert_eq!(packages.len(), 2);
                assert!(sync_first);
                assert_eq!(enabled_repos.len(), 3);
                assert_eq!(cpu_optimization, Some("v3".to_string()));
                assert_eq!(target_repo, None);
            }
            _ => panic!("Wrong command variant"),
        }
    }

    #[test]
    fn test_command_serialization_with_cachyos_repo() {
        let cmd = HelperCommand::AlpmInstall {
            packages: vec!["anydesk-bin".to_string()],
            sync_first: true,
            enabled_repos: vec![
                "cachyos".to_string(),
                "cachyos-v3".to_string(),
                "chaotic-aur".to_string(),
            ],
            cpu_optimization: Some("v3".to_string()),
            target_repo: None,
        };

        let json = serde_json::to_string(&cmd).expect("Should serialize");
        assert!(json.contains("cachyos"));
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        assert!(!json.trim().eq("\"cachyos\""));

        let parsed: HelperCommand = serde_json::from_str(&json).expect("Should deserialize");
        match parsed {
            HelperCommand::AlpmInstall { enabled_repos, .. } => {
                assert!(enabled_repos.contains(&"cachyos".to_string()));
            }
            _ => panic!("Wrong command variant"),
        }
    }

    #[test]
    fn test_reject_raw_string_as_command() {
        let raw_string = "cachyos";
        let result: Result<HelperCommand, _> = serde_json::from_str(raw_string);
        assert!(
            result.is_err(),
            "Raw string should not parse as HelperCommand"
        );

        let quoted = "\"cachyos\"";
        let result2: Result<HelperCommand, _> = serde_json::from_str(quoted);
        assert!(
            result2.is_err(),
            "Quoted string should not parse as HelperCommand"
        );
    }

    #[test]
    fn test_command_file_format() {
        let cmd = HelperCommand::AlpmInstall {
            packages: vec!["test-pkg".to_string()],
            sync_first: false,
            enabled_repos: vec!["core".to_string()],
            cpu_optimization: None,
            target_repo: None,
        };

        let json = serde_json::to_string(&cmd).expect("Should serialize");
        let mut file = NamedTempFile::new().expect("Should create temp file");
        file.write_all(json.as_bytes()).expect("Should write");
        file.flush().expect("Should flush");

        let contents = std::fs::read_to_string(file.path()).expect("Should read");
        assert_eq!(contents.trim(), json);

        let parsed: HelperCommand = serde_json::from_str(&contents.trim()).expect("Should parse");
        match parsed {
            HelperCommand::AlpmInstall { packages, .. } => {
                assert_eq!(packages[0], "test-pkg");
            }
            _ => panic!("Wrong variant"),
        }
    }
}

use alpm::Question;
use alpm::{Alpm, SigLevel};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
    // ✅ NEW: Atomic Batch Transaction (Operation Silent Guard)
    ExecuteBatch {
        manifest: transactions::TransactionManifest,
    },
    CheckUpdatesSafe {
        enabled_repos: Vec<String>,
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
}

// Struct for legacy or simple progress messages if ever needed again
// #[derive(Debug, Serialize)]
// struct ProgressMessage {
//     progress: u8,
//     message: String,
// }

fn emit_progress(progress: u32, message: &str) {
    let event = transactions::AlpmProgressEvent {
        event_type: "progress".to_string(),
        package: None,
        percent: Some(progress as u8),
        downloaded: None,
        total: None,
        message: message.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&event) {
        progress::send_progress_line(json);
    }
}

/// Emit a structured error event so the GUI can show recovery actions (Unlock, Repair Keys, etc.).
fn emit_classified_error(e: &str) {
    let classified = alpm_errors::classify_alpm_error(e);
    let message = serde_json::to_string(&classified).unwrap_or_else(|_| e.to_string());
    let event = transactions::AlpmProgressEvent {
        event_type: "error".to_string(),
        package: None,
        percent: None,
        downloaded: None,
        total: None,
        message,
    };
    if let Ok(json) = serde_json::to_string(&event) {
        progress::send_progress_line(json);
    }
}

// Top-level callbacks to ensure 'static lifetime
// ALPM helpers remain for read-only queries if needed.

/// Real user ID when run via pkexec (pkexec strips env; Polkit sets this).
/// Use for audit logs or GPG keyring path when not relying on $HOME/$USER.
fn calling_uid() -> Option<u32> {
    std::env::var("PKEXEC_UID")
        .ok()
        .and_then(|s| s.parse().ok())
}

/// Paths for App Store–style cancel: GUI creates CANCEL_FILE, helper watches and exits.
const HELPER_PID_FILE: &str = "/var/tmp/monarch-helper.pid";
const CANCEL_FILE: &str = "/var/tmp/monarch-cancel";

/// Remove PID file and cancel file on exit so next run is clean.
struct PidFileGuard;
impl Drop for PidFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(HELPER_PID_FILE);
        let _ = std::fs::remove_file(CANCEL_FILE);
    }
}

/// Spawn a thread that watches for CANCEL_FILE. When the GUI creates it (user clicked Cancel),
/// we remove it and exit so the install stops and the lock is released.
fn spawn_cancel_watcher() {
    std::thread::spawn(|| {
        let cancel_path = std::path::Path::new(CANCEL_FILE);
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if cancel_path.exists() {
                let _ = std::fs::remove_file(cancel_path);
                let _ = std::fs::remove_file(HELPER_PID_FILE);
                logger::info("Cancel requested by user; exiting.");
                std::process::exit(0);
            }
        }
    });
}

use std::os::unix::io::FromRawFd;

fn redirect_streams() -> Result<std::fs::File, String> {
    use std::os::unix::io::AsRawFd;

    // 1. Prepare log directory and file
    let log_path = std::path::Path::new("/var/log/monarch/helper.log");
    if let Some(parent) = log_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create log dir: {}", e))?;
        }
    }

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| format!("Failed to open log file: {}", e))?;

    // 2. Duplicate stdout (FD 1) to a new FD. This new FD will be our IPC channel.
    // We must do this BEFORE redirecting stdout.
    let ipc_fd = unsafe { libc::dup(1) };
    if ipc_fd < 0 {
        return Err("Failed to duplicate stdout for IPC channel".to_string());
    }

    // 3. Create a File from the duplicated FD so we can write to it nicely.
    let ipc_pipe = unsafe { std::fs::File::from_raw_fd(ipc_fd) };

    // 4. Redirect stdout (1) and stderr (2) to the log file.
    // This ensures any "noise" from pacman loops/hooks goes to the log, not the pipe.
    let fd = log_file.as_raw_fd();
    unsafe {
        libc::dup2(fd, 1); // stdout -> log
        libc::dup2(fd, 2); // stderr -> log
    }

    Ok(ipc_pipe)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Effective UID check: helper must run as root. Exit before touching ALPM.
    #[cfg(unix)]
    {
        let euid = unsafe { libc::geteuid() };
        if euid != 0 {
            let msg = format!(
                "monarch-helper must run as root (euid={}). Use pkexec or Polkit.",
                euid
            );
            logger::error(&msg);
            std::process::exit(125);
        }
    }

    // STREAM SEGREGATION:
    // Redirect stdout/stderr to log file so ALPM hooks don't corrupt the JSON IPC pipe.
    // We keep the original stdout as 'ipc_pipe' for progress updates.
    let ipc_pipe = redirect_streams()?;
    progress::init(ipc_pipe);

    if let Some(uid) = calling_uid() {
        logger::info(&format!("monarch-helper starting (invoker UID={})", uid));
    } else {
        logger::info("monarch-helper starting");
    }

    // App Store–style cancel: write PID so GUI can request cancel; watch for cancel file.
    let _pid_guard = PidFileGuard;
    if std::fs::write(HELPER_PID_FILE, std::process::id().to_string()).is_err() {
        logger::trace("Could not write PID file (non-fatal)");
    }
    spawn_cancel_watcher();
    let mut alpm = Alpm::new("/", "/var/lib/pacman")?;

    // Phase 4: Performance - Set Parallel Downloads
    let _ = alpm.set_parallel_downloads(5);

    // App Store grade: auto-answer questions (NOCONFIRM behavior) so GUI never hangs
    alpm.set_question_cb((), |question, _: &mut ()| match question.question() {
        Question::SelectProvider(mut q) => {
            q.set_index(0);
            logger::trace("Auto-resolved provider conflict: chose option 1 (repository default)");
        }
        Question::Replace(q) => {
            q.set_replace(true);
            logger::trace("Auto-resolved replace: chose to replace");
        }
        Question::ImportKey(mut q) => q.set_import(true),
        Question::InstallIgnorepkg(mut q) => {
            logger::warn("IgnorePkg respected: skipping requested upgrade for ignored package.");
            q.set_install(false);
        }
        Question::RemovePkgs(mut q) => q.set_skip(false),
        Question::Conflict(mut q) => q.set_remove(false),
        Question::Corrupted(mut q) => q.set_remove(true),
    });

    // Set log callback to suppress noise (set_log_cb(data, FnMut(LogLevel, &str, &mut T))
    alpm.set_log_cb((), |level, msg, _: &mut ()| {
        if level.bits() >= alpm::LogLevel::WARNING.bits() {
            logger::warn(&format!("[ALPM {:?}] {}", level, msg));
        }
    });

    // Improved Repository Registration: Use pacman-conf to get accurate DB locations and servers
    if let Err(e) = register_repositories(&mut alpm) {
        emit_progress(
            0,
            &format!(
                "Warning: Failed to register repositories via pacman-conf: {}",
                e
            ),
        );
        // Fail gracefully if pacman-conf fails
    }
    // Remove any syncdb that has no servers (avoids "no servers configured for repository" during sync)
    remove_syncdbs_with_no_servers(&mut alpm);

    let args: Vec<String> = std::env::args().collect();
    logger::info(&format!(
        "Helper started with {} args: {:?}",
        args.len(),
        args
    ));

    // Check for command in environment variable first (used when password is provided via sudo -S)
    if let Ok(env_json) = std::env::var("MONARCH_CMD_JSON") {
        logger::info(&format!(
            "Found command in MONARCH_CMD_JSON environment variable (length: {})",
            env_json.len()
        ));
        if !env_json.trim().is_empty() {
            match serde_json::from_str::<HelperCommand>(&env_json) {
                Ok(cmd) => {
                    logger::info("Successfully parsed command from env var");
                    execute_command(cmd, &mut alpm);
                    logger::info("monarch-helper exiting normally");
                    return Ok(());
                }
                Err(e) => {
                    logger::error(&format!("Failed to parse command from env var: {}", e));
                    let preview: String = env_json.chars().take(100).collect();
                    emit_progress(0, &format!("Error: Invalid JSON command in environment variable: {}. Preview: {:?}", e, preview));
                    // Fall through to try file path backup
                }
            }
        } else {
            logger::warn("MONARCH_CMD_JSON is set but empty, falling back to file path");
        }
    } else {
        logger::info("MONARCH_CMD_JSON not found in environment");
    }

    // Fallback: Check for file path in environment variable (backup when env var JSON fails)
    if let Ok(file_path) = std::env::var("MONARCH_CMD_FILE") {
        logger::info(&format!(
            "Found command file path in MONARCH_CMD_FILE: {}",
            file_path
        ));
        if let Ok(json_str) = std::fs::read_to_string(&file_path) {
            let trimmed = json_str.trim();
            if !trimmed.is_empty() {
                logger::info(&format!("Read {} bytes from command file", trimmed.len()));
                match serde_json::from_str::<HelperCommand>(trimmed) {
                    Ok(cmd) => {
                        logger::info("Successfully parsed command from file");
                        let _ = std::fs::remove_file(&file_path);
                        execute_command(cmd, &mut alpm);
                        logger::info("monarch-helper exiting normally");
                        return Ok(());
                    }
                    Err(e) => {
                        logger::error(&format!("Failed to parse command from file: {}", e));
                        emit_progress(
                            0,
                            &format!("Error: Invalid JSON command in file {}: {}", file_path, e),
                        );
                        let _ = std::fs::remove_file(&file_path);
                        return Ok(());
                    }
                }
            }
        }
    }

    if args.len() > 1 {
        // Handle command: args[1] may be path to temp JSON file (GUI/Update flow) or inline JSON (repair/AUR PACMAN wrapper).
        // Try reading as file first for any path-like arg (handles /tmp and /var/tmp).
        let arg1 = args[1].trim();
        logger::info(&format!("Processing argument: {}", arg1));
        let path_to_try = std::path::Path::new(arg1);
        let path_with_json = if arg1.ends_with(".json") {
            path_to_try.to_path_buf()
        } else if arg1.contains("monarch-cmd") {
            std::path::Path::new(arg1).with_extension("json")
        } else {
            path_to_try.to_path_buf()
        };
        let path_to_try_var_tmp = if arg1.starts_with("/tmp/") {
            std::path::Path::new("/var/tmp").join(path_to_try.file_name().unwrap_or_default())
        } else {
            path_to_try.to_path_buf()
        };
        // Detect if this looks like a command file path (very lenient - any absolute path is considered a file path)
        let looks_like_cmd_file =
            arg1.starts_with("/") || arg1.contains("/") || arg1.contains("\\");

        let read_from_path = |p: &std::path::Path| -> Option<String> {
            logger::info(&format!("Attempting to read from path: {}", p.display()));
            if !p.exists() {
                logger::info(&format!("Path does not exist: {}", p.display()));
                return None;
            }
            if !p.is_file() {
                logger::info(&format!("Path is not a file: {}", p.display()));
                return None;
            }
            // SECURITY: When invoked via pkexec, command file must be owned by the invoking user (prevents TOCTOU/race).
            #[cfg(unix)]
            if let Some(expect_uid) = calling_uid() {
                if let Ok(meta) = std::fs::metadata(p) {
                    let file_uid = meta.uid();
                    if file_uid != expect_uid {
                        logger::error(&format!(
                            "Command file ownership violation: file uid={}, expected {}",
                            file_uid, expect_uid
                        ));
                        emit_progress(0, "Error: Command file must be owned by the invoking user (security check).");
                        return None;
                    }
                }
            }
            if let Ok(metadata) = std::fs::metadata(p) {
                logger::info(&format!(
                    "File metadata: permissions={:?}, size={}",
                    metadata.permissions(),
                    metadata.len()
                ));
            }
            match std::fs::read_to_string(p) {
                Ok(s) => {
                    let trimmed = s.trim();
                    logger::info(&format!(
                        "Successfully read {} bytes from file",
                        trimmed.len()
                    ));
                    if trimmed.is_empty() {
                        emit_progress(0, &format!("Error: Command file {} is empty", p.display()));
                        let _ = std::fs::remove_file(p);
                        return None;
                    }
                    let _ = std::fs::remove_file(p);
                    Some(trimmed.to_string())
                }
                Err(e) => {
                    logger::error(&format!("Failed to read file {}: {}", p.display(), e));
                    emit_progress(
                        0,
                        &format!("Error: Failed to read command file {}: {}", p.display(), e),
                    );
                    None
                }
            }
        };

        let json_str: String = match read_from_path(path_to_try)
            .or_else(|| read_from_path(&path_with_json))
            .or_else(|| read_from_path(&path_to_try_var_tmp))
        {
            Some(s) => {
                if s.trim().is_empty() {
                    emit_progress(0, &format!("Error: Command file {} is empty", arg1));
                    return Ok(());
                }
                s
            }
            None if looks_like_cmd_file => {
                emit_progress(
                    0,
                    &format!(
                        "Error: Command file not found: {}. Tried: {:?}, {:?}, {:?}. Reinstall monarch-store so helper and GUI both use /var/tmp.",
                        arg1, path_to_try, path_with_json, path_to_try_var_tmp
                    ),
                );
                return Ok(());
            }
            None if arg1.is_empty() => {
                emit_progress(
                    0,
                    "Error: No command argument. Expected path to JSON file or inline JSON.",
                );
                return Ok(());
            }
            None => {
                // If it looks like a file path (contains / or starts with /), don't try to parse as JSON
                if arg1.starts_with('/') || arg1.contains("/") || arg1.contains("\\") {
                    // Definitely a file path that we couldn't read
                    logger::error(&format!("File path provided but could not read: {}", arg1));
                    emit_progress(
                        0,
                        &format!(
                            "Error: Command file not found or not readable: {}. Tried paths: {:?}, {:?}, {:?}. Check permissions and ensure file exists.",
                            arg1, path_to_try, path_with_json, path_to_try_var_tmp
                        ),
                    );
                    return Ok(());
                }
                // Try to parse arg1 as inline JSON (fallback for repair commands that pass JSON directly)
                // CRITICAL: Only accept if it's valid JSON structure (starts with { and ends with })
                if arg1.starts_with('{') && arg1.ends_with('}') {
                    // Validate it's actually JSON by attempting to parse
                    if serde_json::from_str::<serde_json::Value>(arg1).is_ok() {
                        logger::info("Treating argument as inline JSON");
                        arg1.to_string()
                    } else {
                        logger::error(&format!(
                            "Argument looks like JSON but failed to parse: {}",
                            arg1
                        ));
                        emit_progress(
                            0,
                            &format!(
                                "Error: Invalid JSON in argument: {}. Expected valid JSON command.",
                                arg1
                            ),
                        );
                        return Ok(());
                    }
                } else {
                    // Reject any non-JSON, non-file-path argument (e.g., raw repo names like "cachyos")
                    logger::error(&format!(
                        "Argument doesn't look like a file path or JSON: {}",
                        arg1
                    ));
                    emit_progress(
                        0,
                        &format!(
                            "Error: Invalid command argument: {}. Expected file path or JSON command. Got raw string (possibly a repo name).",
                            arg1
                        ),
                    );
                    return Ok(());
                }
            }
        };

        logger::info(&format!(
            "Parsing JSON command (length: {})",
            json_str.len()
        ));
        match serde_json::from_str::<HelperCommand>(&json_str) {
            Ok(cmd) => {
                logger::info("Successfully parsed command");
                execute_command(cmd, &mut alpm)
            }
            Err(e) => {
                let err_str = e.to_string();
                logger::error(&format!("JSON parse error: {}", err_str));
                let is_outdated_helper = err_str.contains("unknown variant")
                    && (json_str.contains("AlpmInstall")
                        || json_str.contains("AlpmUpgrade")
                        || json_str.contains("AlpmUninstall"));
                if is_outdated_helper {
                    emit_progress(
                        0,
                        "Error: The installed monarch-helper is outdated and does not support ALPM install/update. Please update monarch-store: run 'pacman -Syu monarch-store' or reinstall the package so the helper matches this app version.",
                    );
                } else {
                    let preview: String = json_str
                        .chars()
                        .take(80)
                        .collect::<String>()
                        .replace('\n', " ");
                    emit_progress(
                        0,
                        &format!(
                            "Error: Invalid JSON command: {}. Input length: {}. Preview: {:?}. Full input starts with: {:?}",
                            err_str,
                            json_str.len(),
                            if preview.is_empty() { "(empty)" } else { &preview },
                            json_str.chars().take(200).collect::<String>()
                        ),
                    );
                }
            }
        }
    } else {
        // Primary path for GUI (pkexec with no args): command is sent as a single JSON line on stdin.
        // BUT: If MONARCH_CMD_FILE is set, we're in password mode (sudo -S) and stdin contains the password, not the command.
        // In that case, we should have already read from the file above. If we get here, something went wrong.
        if std::env::var("MONARCH_CMD_FILE").is_ok() {
            logger::error(
                "MONARCH_CMD_FILE is set but command file read failed. This should not happen.",
            );
            emit_progress(
                0,
                "Error: Command file was specified but could not be read. Check permissions and ensure the file exists.",
            );
            return Ok(());
        }

        // No file path set, so we're in pkexec mode: command is on stdin (not password)
        logger::info("Reading command from stdin (pkexec mode, no password)");
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(line) = line {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue; // Skip empty lines
                }
                // CRITICAL: Validate that input looks like JSON before attempting to parse
                // Reject raw strings (e.g., repo names like "cachyos") that aren't JSON
                if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
                    logger::error(&format!(
                        "Received non-JSON input on stdin: {:?}",
                        trimmed.chars().take(80).collect::<String>()
                    ));
                    emit_progress(
                        0,
                        &format!(
                            "Error: Invalid input on stdin. Expected JSON command, got raw string: {:?}. This may indicate a serialization bug in the GUI.",
                            trimmed.chars().take(80).collect::<String>()
                        ),
                    );
                    continue; // Skip this line and try next
                }
                match serde_json::from_str::<HelperCommand>(trimmed) {
                    Ok(cmd) => execute_command(cmd, &mut alpm),
                    Err(e) => {
                        let err_str = e.to_string();
                        let is_outdated_helper = err_str.contains("unknown variant")
                            && (trimmed.contains("AlpmInstall")
                                || trimmed.contains("AlpmUpgrade")
                                || trimmed.contains("AlpmUninstall"));
                        if is_outdated_helper {
                            emit_progress(
                                0,
                                "Error: The installed monarch-helper is outdated and does not support ALPM install/update. Please update monarch-store: run 'pacman -Syu monarch-store' or reinstall the package so the helper matches this app version.",
                            );
                        } else {
                            let preview: String = trimmed.chars().take(80).collect();
                            emit_progress(
                                0,
                                &format!(
                                    "Error: Failed to parse command JSON: {}. Payload preview: {:?}",
                                    err_str, preview
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    logger::info("monarch-helper exiting normally");
    Ok(())
}

fn main() {
    let result = std::panic::catch_unwind(|| {
        run().map_err(|e| {
            logger::error(&e.to_string());
            e.to_string()
        })
    });
    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            emit_progress(0, &format!("Error: {}", e));
        }
        Err(panic_payload) => {
            let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "Helper crashed (unknown panic)".to_string()
            };
            logger::panic_msg(&msg);
            emit_progress(0, &format!("Error: {}", msg));
            // Release ALPM lock on panic so a zombie lockfile doesn't break the system.
            if std::path::Path::new(self_healer::DB_LOCK_PATH).exists() {
                let _ = std::fs::remove_file(self_healer::DB_LOCK_PATH);
            }
        }
    }
}

/// Ensure we can lock the DB.
fn ensure_db_ready() -> Result<(), String> {
    if !std::path::Path::new(self_healer::DB_LOCK_PATH).exists() {
        return Ok(());
    }
    if self_healer::is_db_lock_stale() {
        self_healer::remove_stale_db_lock()?;
        return Ok(());
    }
    Err(self_healer::db_lock_busy_message().to_string())
}

// --- SELF-HEALING RETRY LOOP ---
fn execute_with_healing<F>(mut action: F)
where
    F: FnMut() -> Result<(), String>,
{
    // Attempt 1
    if let Err(e) = action() {
        // Check for signature/keyring errors
        let err_lower = e.to_lowercase();
        let is_sig_error = err_lower.contains("invalid or corrupted package")
            || err_lower.contains("invalid signature")
            || err_lower.contains("unknown trust")
            || err_lower.contains("signature from")
            || err_lower.contains("corrupted package");

        if is_sig_error {
            emit_progress(
                0,
                "Identified signature error. Attempting self-repair (resetting keys)...",
            );
            logger::warn(&format!("Self-Heal Triggered: {}", e));

            // Repair Action: pacman-key --init && pacman-key --populate
            let heal_res = (|| -> Result<(), String> {
                std::process::Command::new("pacman-key")
                    .arg("--init")
                    .output()
                    .map_err(|e| format!("Init failed: {}", e))?;

                std::process::Command::new("pacman-key")
                    .arg("--populate")
                    .output()
                    .map_err(|e| format!("Populate failed: {}", e))?;
                Ok(())
            })();

            if let Err(heal_err) = heal_res {
                emit_progress(0, &format!("Self-repair failed: {}", heal_err));
            // Verify if we should still return the original error?
            // Yes, fall through to emit the original failure or a new one.
            } else {
                emit_progress(10, "Keys reset. Retrying operation...");
                if let Err(retry_e) = action() {
                    emit_classified_error(&retry_e);
                    emit_progress(0, &format!("Error (Persistent): {}", retry_e));
                } else {
                    emit_progress(100, "Recovered successfully!");
                }
                return;
            }
        }

        // If not sig error or repair failed/didn't help: emit both legacy line and structured error for GUI recovery UI
        emit_classified_error(&e);
        emit_progress(0, &format!("Error: {}", e));
    }
}

fn execute_command(cmd: HelperCommand, alpm: &mut Alpm) {
    match cmd {
        // ✅ NEW: Full ALPM Transactions
        HelperCommand::AlpmInstall {
            packages,
            sync_first,
            enabled_repos: _,
            cpu_optimization,
            target_repo,
        } => {
            execute_with_healing(|| {
                if let Err(e) = ensure_db_ready() {
                    return Err(e);
                }
                transactions::execute_alpm_install(
                    packages.clone(),
                    sync_first,
                    cpu_optimization.clone(),
                    target_repo.clone(),
                    alpm,
                )
            });
        }
        HelperCommand::CheckUpdatesSafe { enabled_repos: _ } => {
            // Safe Check: Does NOT require ensure_db_ready() because it uses a temp DB path
            // and does not lock the main pacman lock.
            transactions::execute_alpm_check_updates_safe(alpm);
        }
        HelperCommand::AlpmUninstall {
            packages,
            remove_deps,
        } => {
            // Uninstall usually doesn't involve signatures, but db lock might need check.
            // We can use simple execution or healing if we suspect DB lock issues?
            // Prompt only requested self-healing for "Invalid Signature". Uninstall won't verify sigs.
            if let Err(e) = ensure_db_ready() {
                emit_classified_error(&e);
                emit_progress(0, &e);
                return;
            }
            if let Err(e) = transactions::execute_alpm_uninstall(packages, remove_deps, alpm) {
                emit_classified_error(&e);
                emit_progress(0, &format!("Error: {}", e));
            }
        }
        HelperCommand::AlpmUpgrade {
            packages,
            enabled_repos: _,
        } => {
            execute_with_healing(|| {
                if let Err(e) = ensure_db_ready() {
                    return Err(e);
                }
                let mut trans = safe_transaction::SafeUpdateTransaction::new(alpm);
                if let Some(targets) = packages.clone() {
                    trans = trans.with_targets(targets);
                }
                trans.execute()
            });
        }
        HelperCommand::AlpmSync { enabled_repos } => {
            // Sync verifies DB signatures!
            execute_with_healing(|| transactions::execute_alpm_sync(enabled_repos.clone(), alpm));
        }
        HelperCommand::AlpmInstallFiles { paths } => {
            execute_with_healing(|| {
                if let Err(e) = ensure_db_ready() {
                    return Err(e);
                }
                // SECURITY: Only allow paths under /tmp/monarch-install/
                const ALLOWED_INSTALL_PREFIX: &str = "/tmp/monarch-install";
                let prefix = std::fs::canonicalize(ALLOWED_INSTALL_PREFIX)
                    .unwrap_or_else(|_| std::path::PathBuf::from(ALLOWED_INSTALL_PREFIX));
                let mut allowed_paths = Vec::new();
                for p in &paths {
                    match std::fs::canonicalize(p) {
                        Ok(canon) => {
                            if canon.starts_with(&prefix) {
                                allowed_paths.push(canon.to_string_lossy().to_string());
                            } else {
                                return Err("Error: Unauthorized path for AlpmInstallFiles (only /tmp/monarch-install/ allowed)".to_string());
                            }
                        }
                        Err(_) => {
                            return Err(format!("Error: Path not found or invalid: {}", p));
                        }
                    }
                }
                if allowed_paths.is_empty() {
                    return Err("Error: No valid paths for AlpmInstallFiles".to_string());
                }
                transactions::execute_alpm_install_files(allowed_paths, alpm)
            });
        }
        HelperCommand::ExecuteBatch { manifest } => {
            // Operation "Silent Guard": Execute all steps under ONE lock acquisition

            // 0a. Remove Stale Lock (Pre-transaction maintenance)
            if manifest.remove_lock {
                let _ = remove_lock(); // Special logic in remove_lock handles safety
            }

            // 0b. Clear Cache (Maintenance)
            if manifest.clear_cache {
                let _ = clear_cache(alpm, 0); // Clear everything
            }

            // 1. Refresh DB
            if manifest.refresh_db {
                if let Err(e) = transactions::force_refresh_sync_dbs(alpm) {
                    emit_progress(0, &format!("Error refreshing databases: {}", e));
                    return;
                }
            }

            // 2. System Upgrade
            if manifest.update_system {
                if let Err(e) = transactions::execute_alpm_upgrade(None, alpm) {
                    emit_progress(0, &format!("Error upgrading system: {}", e));
                    return;
                }
            }

            // 3. Remove Targets
            if !manifest.remove_targets.is_empty() {
                if let Err(e) = transactions::execute_alpm_uninstall(
                    manifest.remove_targets.clone(),
                    true,
                    alpm,
                ) {
                    emit_progress(0, &format!("Error removing packages: {}", e));
                    return;
                }
            }

            // 4. Install Targets (Repo + Local)
            // Note: ALPM allows installing repo pkgs and local files in one transaction?
            // The current helper functions are separate. We can call them sequentially since we hold the lock.
            // The main() function holds the ALPM handle which implies the lock is held (if configured).
            // Actually ALPM lock matches the handle lifetime or transaction lifetime.
            // Our helper is short-lived process. One invocation = one ALPM instance = one lock.
            // So calling these sequentially IS atomic regarding the lock.

            let mut installed_anything = false;

            if !manifest.install_targets.is_empty() {
                // sync_first false because we handled it in step 1 if needed
                // cpu strictness default (None)
                if let Err(e) = transactions::execute_alpm_install(
                    manifest.install_targets.clone(),
                    false,
                    None,
                    None,
                    alpm,
                ) {
                    emit_progress(0, &format!("Error installing repo packages: {}", e));
                    return;
                }
                installed_anything = true;
            }

            // 4b. Install Local Files (Built AUR packages)
            if !manifest.local_paths.is_empty() {
                if let Err(e) =
                    transactions::execute_alpm_install_files(manifest.local_paths.clone(), alpm)
                {
                    emit_progress(0, &format!("Error installing local packages: {}", e));
                    return;
                }
                installed_anything = true;
            }

            if !installed_anything
                && !manifest.update_system
                && !manifest.refresh_db
                && manifest.remove_targets.is_empty()
            {
                emit_progress(100, "Transaction successful (No actions required)");
            } else {
                emit_progress(100, "Batch Transaction Complete");
            }
        }
    }
}

fn remove_syncdbs_with_no_servers(alpm: &mut Alpm) {
    let mut names_to_remove = Vec::new();
    for db in alpm.syncdbs().iter() {
        if db.servers().iter().next().is_none() {
            names_to_remove.push(db.name().to_string());
        }
    }

    for name in names_to_remove {
        // We use a separate loop and find the DB again to avoid iterator invalidation
        let mut found_db_to_unregister = None;
        for db in alpm.syncdbs_mut() {
            if db.name() == name {
                found_db_to_unregister = Some(db);
                break;
            }
        }
        if let Some(db) = found_db_to_unregister {
            logger::warn(&format!(
                "Unregistering repo '{}' because it has no servers.",
                name
            ));
            let _ = db.unregister();
        }
    }
}

/// Use `pacman-conf` to retrieve the list of configured repositories and their servers.
/// This avoids manual parsing of `pacman.conf` and `Include` directives, ensuring we see exactly what pacman sees.
fn register_repositories(alpm: &mut Alpm) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    // 1. Get list of repo names
    let output = Command::new("pacman-conf").arg("--repo-list").output()?;

    if !output.status.success() {
        return Err(format!(
            "pacman-conf failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let repo_list_str = String::from_utf8_lossy(&output.stdout);
    let repo_names: Vec<&str> = repo_list_str
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    emit_progress(
        5,
        &format!("Detected {} system repositories.", repo_names.len()),
    );

    for repo_name in repo_names {
        logger::trace(&format!("Querying details for repo: {}", repo_name));
        // 2. Get details for each repo
        let details_out = Command::new("pacman-conf")
            .arg("--repo")
            .arg(repo_name)
            .output()?;

        if !details_out.status.success() {
            logger::warn(&format!("Failed to get details for repo '{}'", repo_name));
            continue;
        }

        let details_str = String::from_utf8_lossy(&details_out.stdout);
        let mut servers = Vec::new();
        let mut siglevel = SigLevel::USE_DEFAULT;
        let mut usage = alpm::Usage::ALL;

        for line in details_str.lines() {
            let line = line.trim();
            if line.contains("Server = ") || line.contains("Server=") {
                let val = if line.contains("Server = ") {
                    line.splitn(2, "Server = ").nth(1).unwrap_or("")
                } else {
                    line.splitn(2, "Server=").nth(1).unwrap_or("")
                }
                .trim();

                let server = val.split('#').next().unwrap_or(val).trim();
                if !server.is_empty() {
                    servers.push(server.to_string());
                }
            } else if line.contains("SigLevel = ") || line.contains("SigLevel=") {
                let val = if line.contains("SigLevel = ") {
                    line.splitn(2, "SigLevel = ").nth(1).unwrap_or("")
                } else {
                    line.splitn(2, "SigLevel=").nth(1).unwrap_or("")
                }
                .trim();
                let val_lower = val.to_lowercase();
                if val_lower.contains("never") {
                    siglevel = SigLevel::NONE;
                } else if val_lower.contains("taroptional") || val_lower.contains("packageoptional")
                {
                    siglevel = SigLevel::PACKAGE_OPTIONAL;
                } else if val_lower.contains("required") {
                    siglevel = SigLevel::USE_DEFAULT;
                }
            } else if line.contains("Usage = ") || line.contains("Usage=") {
                let val = if line.contains("Usage = ") {
                    line.splitn(2, "Usage = ").nth(1).unwrap_or("")
                } else {
                    line.splitn(2, "Usage=").nth(1).unwrap_or("")
                }
                .trim();

                if val.contains("Sync") {
                    usage |= alpm::Usage::SYNC;
                }
                if val.contains("Search") {
                    usage |= alpm::Usage::SEARCH;
                }
                if val.contains("Install") {
                    usage |= alpm::Usage::INSTALL;
                }
                if val.contains("Upgrade") {
                    usage |= alpm::Usage::UPGRADE;
                }
                if val.contains("All") {
                    usage = alpm::Usage::ALL;
                }
            }
        }

        if !servers.is_empty() {
            emit_progress(
                5,
                &format!(
                    "Registering {} ({} mirrors found)...",
                    repo_name,
                    servers.len()
                ),
            );

            // Register or find existing
            let already_exists = alpm.syncdbs().iter().any(|db| db.name() == repo_name);
            if !already_exists {
                let _ = alpm.register_syncdb(repo_name.to_string(), siglevel)?;
            }

            // We still need to find it mutably after registration to add servers/usage
            // because of borrow checker constraints in a loop.
            for db in alpm.syncdbs_mut() {
                if db.name() == repo_name {
                    let _ = db.set_usage(usage);
                    for server in servers {
                        let _ = db.add_server(server);
                    }
                    break;
                }
            }
        } else {
            emit_progress(
                5,
                &format!("Warning: No mirrors found for repository '{}'.", repo_name),
            );
            logger::warn(&format!("No servers found for repo: {}", repo_name));
        }
    }

    Ok(())
}

fn clear_cache(_alpm: &mut Alpm, _keep: u32) -> Result<(), String> {
    emit_progress(0, "Clearing package cache...");
    let cache_dir = "/var/cache/pacman/pkg";
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .map(|s| s == "zst" || s == "xz")
                .unwrap_or(false)
            {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    emit_progress(100, "Cache cleared");
    Ok(())
}

fn remove_lock() -> Result<(), String> {
    let lock_path = "/var/lib/pacman/db.lck";
    if std::path::Path::new(lock_path).exists() {
        // SECURITY: Check if pacman is running first
        let is_running = std::process::Command::new("pgrep")
            .arg("-x")
            .arg("pacman")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if is_running {
            return Err("Cannot remove lock: Pacman process is active.".to_string());
        }

        std::fs::remove_file(lock_path).map_err(|e| e.to_string())?;
        emit_progress(100, "Lock removed successfully");
    } else {
        emit_progress(100, "No lock file found");
    }
    Ok(())
}

mod transactions;
mod alpm_errors;
mod logger;
mod self_healer;

use alpm::{Alpm, SigLevel};
use alpm::Question;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", content = "payload")]
enum HelperCommand {
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
    // Legacy commands (deprecated but kept for compatibility)
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

#[derive(Debug, Serialize)]
struct ProgressMessage {
    progress: u8,
    message: String,
}

fn emit_progress(progress: u32, message: &str) {
    use std::io::Write;
    let progress = ProgressMessage {
        progress: progress as u8,
        message: message.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&progress) {
        let _ = writeln!(std::io::stdout(), "{}", json);
        let _ = std::io::stdout().flush();
    }
}

// Top-level callbacks to ensure 'static lifetime
// ALPM helpers remain for read-only queries if needed.

fn run() -> Result<(), Box<dyn std::error::Error>> {
    logger::info("monarch-helper starting");
    let mut alpm = Alpm::new("/", "/var/lib/pacman")?;

    // Phase 4: Performance - Set Parallel Downloads
    let _ = alpm.set_parallel_downloads(5);

    // App Store grade: auto-answer questions (NOCONFIRM behavior) so GUI never hangs
    alpm.set_question_cb((), |question, _: &mut ()| {
        match question.question() {
            Question::SelectProvider(mut q) => {
                q.set_index(0);
                logger::trace("Auto-resolved provider conflict: chose option 1 (repository default)");
            }
            Question::Replace(q) => {
                q.set_replace(true);
                logger::trace("Auto-resolved replace: chose to replace");
            }
            Question::ImportKey(mut q) => q.set_import(true),
            Question::InstallIgnorepkg(mut q) => q.set_install(true),
            Question::RemovePkgs(mut q) => q.set_skip(false),
            Question::Conflict(mut q) => q.set_remove(false),
            Question::Corrupted(mut q) => q.set_remove(true),
        }
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
        // Fallback to basic registration if pacman-conf fails
        if let Ok(conf) = std::fs::read_to_string("/etc/pacman.conf") {
            for line in conf.lines() {
                let line = line.trim();
                if line.starts_with('[') && line.ends_with(']') {
                    let section = &line[1..line.len() - 1];
                    if section != "options" {
                        let _ = alpm.register_syncdb(section.as_bytes().to_vec(), SigLevel::PACKAGE_OPTIONAL);
                    }
                }
            }
        }
    }

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        // Handle command: args[1] may be path to JSON file (preferred) or inline JSON
        let arg1 = &args[1];
        let json_str: String = if std::path::Path::new(arg1).is_file() {
            match std::fs::read_to_string(arg1) {
                Ok(s) => {
                    let _ = std::fs::remove_file(arg1);
                    s
                }
                Err(e) => {
                    emit_progress(
                        0,
                        &format!("Error: Failed to read command file: {}", e),
                    );
                    return Ok(());
                }
            }
        } else {
            arg1.clone()
        };

        match serde_json::from_str::<HelperCommand>(&json_str) {
            Ok(cmd) => execute_command(cmd, &mut alpm),
            Err(e) => {
                emit_progress(
                    0,
                    &format!(
                        "Error: Invalid JSON command: {}. Input length: {}",
                        e,
                        json_str.len()
                    ),
                );
            }
        }
    } else {
        // Handle commands from stdin (e.g. from invoke_helper)
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(line) = line {
                match serde_json::from_str::<HelperCommand>(&line) {
                    Ok(cmd) => execute_command(cmd, &mut alpm),
                    Err(e) => {
                        emit_progress(
                            0,
                            &format!(
                                "Error: Failed to parse command JSON: {}. Payload: {}",
                                e, line
                            ),
                        );
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
        }
    }
}

/// Ensures db.lck is not held (or removes stale lock). Call before any modifying transaction.
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

fn execute_command(cmd: HelperCommand, alpm: &mut Alpm) {
    match cmd {
        // ✅ NEW: Full ALPM Transactions
        HelperCommand::AlpmInstall {
            packages,
            sync_first,
            enabled_repos,
            cpu_optimization,
        } => {
            if let Err(e) = ensure_db_ready() {
                emit_progress(0, &e);
                return;
            }
            if let Err(e) = transactions::execute_alpm_install(
                packages,
                sync_first,
                enabled_repos,
                cpu_optimization,
                alpm,
            ) {
                emit_progress(0, &format!("Error: {}", e));
            }
        }
        HelperCommand::AlpmUninstall {
            packages,
            remove_deps,
        } => {
            if let Err(e) = ensure_db_ready() {
                emit_progress(0, &e);
                return;
            }
            if let Err(e) = transactions::execute_alpm_uninstall(packages, remove_deps, alpm) {
                emit_progress(0, &format!("Error: {}", e));
            }
        }
        HelperCommand::AlpmUpgrade {
            packages,
            enabled_repos,
        } => {
            if let Err(e) = ensure_db_ready() {
                emit_progress(0, &e);
                return;
            }
            if let Err(e) = transactions::execute_alpm_upgrade(packages, enabled_repos, alpm) {
                emit_progress(0, &format!("Error: {}", e));
            }
        }
        HelperCommand::AlpmSync { enabled_repos } => {
            if let Err(e) = transactions::execute_alpm_sync(enabled_repos, alpm) {
                emit_progress(0, &format!("Error: {}", e));
            }
        }
        HelperCommand::AlpmInstallFiles { paths } => {
            if let Err(e) = ensure_db_ready() {
                emit_progress(0, &e);
                return;
            }
            // SECURITY: Only allow paths under /tmp/monarch-install/ (canonicalized) to prevent
            // a compromised GUI from installing arbitrary package files from other locations.
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
                            emit_progress(0, "Error: Unauthorized path for AlpmInstallFiles (only /tmp/monarch-install/ allowed)");
                            return;
                        }
                    }
                    Err(_) => {
                        emit_progress(0, &format!("Error: Path not found or invalid: {}", p));
                        return;
                    }
                }
            }
            if allowed_paths.is_empty() {
                emit_progress(0, "Error: No valid paths for AlpmInstallFiles");
                return;
            }
            if let Err(e) = transactions::execute_alpm_install_files(allowed_paths, alpm) {
                emit_progress(0, &format!("Error: {}", e));
            }
        }
        // Legacy commands (deprecated but kept for compatibility)
        HelperCommand::InstallTargets { .. } => {
            emit_progress(
                0,
                "Error: InstallTargets is deprecated. Use AlpmInstall instead.",
            );
        }
        HelperCommand::InstallFiles { .. } => {
            emit_progress(
                0,
                "Error: InstallFiles is deprecated. Use AlpmInstallFiles instead.",
            );
        }
        HelperCommand::Sysupgrade => {
            if let Err(e) = ensure_db_ready() {
                emit_progress(0, &e);
                return;
            }
            let enabled_repos: Vec<String> = alpm.syncdbs().iter().map(|db| db.name().to_string()).collect();
            if let Err(e) = transactions::execute_alpm_upgrade(None, enabled_repos, alpm) {
                emit_progress(0, &format!("Error: {}", e));
            }
        }
        HelperCommand::Refresh => {
            emit_progress(
                0,
                "Error: Refresh is deprecated. Use AlpmSync instead.",
            );
        }
        HelperCommand::UninstallTargets { .. } => {
            emit_progress(
                0,
                "Error: UninstallTargets is deprecated. Use AlpmUninstall instead.",
            );
        }
        HelperCommand::RemoveOrphans => {
            emit_progress(
                0,
                "Error: RemoveOrphans is deprecated in Helper. Use GUI Shell Wrapper.",
            );
        }
        HelperCommand::ClearCache { keep } => {
            if let Err(e) = clear_cache(alpm, keep) {
                emit_progress(0, &format!("Error: {}", e));
            }
        }
        HelperCommand::RemoveLock => {
            if let Err(e) = remove_lock() {
                emit_progress(0, &format!("Error: {}", e));
            }
        }
        HelperCommand::ConfigureRepo { name, enabled, url } => {
            if let Err(e) = configure_repo(name, enabled, url) {
                emit_progress(0, &format!("Error: {}", e));
            }
        }
        HelperCommand::WriteFile { path, content } => {
            // SECURITY: Only allow writing to /etc/pacman.d/monarch for now
            if path.starts_with("/etc/pacman.d/monarch/") || path == "/etc/pacman.conf" {
                if let Err(e) = std::fs::write(&path, content) {
                    emit_progress(0, &format!("Error: {}", e.to_string()));
                } else {
                    emit_progress(100, &format!("Wrote {}", path));
                }
            } else {
                emit_progress(0, "Error: Unauthorized path");
            }
        }
        HelperCommand::RemoveFile { path } => {
            // SECURITY: Only allow removing from /etc/pacman.d/monarch for now
            if path.starts_with("/etc/pacman.d/monarch/") {
                if let Err(e) = std::fs::remove_file(&path) {
                    emit_progress(0, &format!("Error: {}", e.to_string()));
                } else {
                    emit_progress(100, &format!("Removed {}", path));
                }
            } else {
                emit_progress(0, "Error: Unauthorized path");
            }
        }
        HelperCommand::WriteFiles { files } => {
            for (path, content) in files {
                if path.starts_with("/etc/pacman.d/monarch/") || path == "/etc/pacman.conf" {
                    if let Err(e) = std::fs::write(&path, content) {
                        emit_progress(0, &format!("Error writing {}: {}", path, e));
                    }
                }
            }
            emit_progress(100, "Batch write complete");
        }
        HelperCommand::RemoveFiles { paths } => {
            for path in paths {
                if path.starts_with("/etc/pacman.d/monarch/") {
                    let _ = std::fs::remove_file(&path);
                }
            }
            emit_progress(100, "Batch remove complete");
        }
        HelperCommand::RunCommand { binary, args } => {
            use std::io::Write;
            use std::os::unix::process::CommandExt;
            // SECURITY: Whitelist RunCommand to pacman and pacman-key only. A compromised GUI
            // must not be able to execute arbitrary binaries as root.
            let allowed_binaries = ["pacman", "pacman-key"];
            let bin_name = std::path::Path::new(&binary)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if !allowed_binaries.contains(&bin_name) {
                emit_progress(0, &format!("Error: RunCommand only allows pacman and pacman-key, got: {}", binary));
                return;
            }
            // Only allow absolute path to the actual binary (avoid PATH abuse)
            let safe_binary = if bin_name == "pacman" {
                "/usr/bin/pacman"
            } else {
                "/usr/bin/pacman-key"
            };
            emit_progress(0, &format!("Proxying execution to {}...", safe_binary));
            let _ = std::io::stdout().flush();

            let mut cmd = std::process::Command::new(safe_binary);
            cmd.args(args);

            let err = cmd.exec();
            emit_progress(0, &format!("Error: Failed to exec proxy: {}", err));
        }
        HelperCommand::Initialize => {
            emit_progress(10, "Initializing MonARCH system directories...");
            let _ = std::fs::create_dir_all("/etc/pacman.d/monarch");
            let _ = std::fs::create_dir_all("/var/lib/monarch/dbs");

            emit_progress(50, "Checking /etc/pacman.conf...");
            if let Ok(content) = std::fs::read_to_string("/etc/pacman.conf") {
                if !content.contains("/etc/pacman.d/monarch/*.conf") {
                    // Add Include before [core]
                    let mut new_content = String::new();
                    let mut added = false;
                    for line in content.lines() {
                        if !added && line.trim() == "[core]" {
                            new_content.push_str("# MonARCH Managed Repositories\nInclude = /etc/pacman.d/monarch/*.conf\n\n");
                            added = true;
                        }
                        new_content.push_str(line);
                        new_content.push_str("\n");
                    }
                    if !added {
                        // Fallback to append
                        new_content.push_str("\nInclude = /etc/pacman.d/monarch/*.conf\n");
                    }
                    let _ = std::fs::write("/etc/pacman.conf", new_content);
                    emit_progress(100, "MonARCH integrated with pacman.conf");
                } else {
                    emit_progress(100, "MonARCH already integrated");
                }
            }
        }
    }
}

fn register_repositories(alpm: &mut Alpm) -> Result<(), Box<dyn std::error::Error>> {
    let conf_path = "/etc/pacman.conf";
    if !std::path::Path::new(conf_path).exists() {
        return Err("pacman.conf not found".into());
    }

    // 1. Register base pacman.conf
    let conf = std::fs::read_to_string(conf_path)?;
    parse_and_register_conf(alpm, &conf, None)?;

    // 2. Register MonARCH modular configs (Hardcoded sync dir)
    if let Ok(entries) = std::fs::read_dir("/etc/pacman.d/monarch") {
        for entry in entries.flatten() {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                parse_and_register_conf(alpm, &content, None)?;
            }
        }
    }

    Ok(())
}

fn parse_and_register_conf(
    alpm: &mut Alpm,
    content: &str,
    current_repo_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut current_repo_name = current_repo_name;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            if section != "options" {
                current_repo_name = Some(section.to_string());
                let _ = alpm.register_syncdb(section.as_bytes().to_vec(), SigLevel::PACKAGE_OPTIONAL);
            } else {
                current_repo_name = None;
            }
        } else if let Some(repo_name) = &current_repo_name {
            if line.contains("Server =") {
                if let Some(server) = line.split('=').nth(1) {
                    for db in alpm.syncdbs_mut() {
                        if db.name() == repo_name {
                            let _ = db.add_server(server.trim());
                        }
                    }
                }
            } else if line.contains("Include =") {
                if let Some(path) = line.split('=').nth(1) {
                    let path = path.trim();
                    if let Ok(include_content) = std::fs::read_to_string(path) {
                        parse_and_register_conf(alpm, &include_content, Some(repo_name.clone()))?;
                    }
                }
            }
        }
    }
    Ok(())
}

// State-changing functions now use run_pacman_command

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

fn configure_repo(name: String, enabled: bool, url: String) -> Result<(), String> {
    let conf_path = "/etc/pacman.conf";
    let content = std::fs::read_to_string(conf_path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    let mut found_index = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == format!("[{}]", name) || trimmed == format!("#[{}]", name) {
            found_index = Some(i);
            break;
        }
    }

    if enabled {
        if let Some(idx) = found_index {
            if lines[idx].trim().starts_with('#') {
                lines[idx] = lines[idx].replacen('#', "", 1);
                let mut j = idx + 1;
                while j < lines.len() && !lines[j].trim().starts_with('[') {
                    if lines[j].trim().starts_with('#') {
                        let trimmed = lines[j].trim();
                        if trimmed.contains("Server")
                            || trimmed.contains("SigLevel")
                            || trimmed.contains("Include")
                        {
                            lines[j] = lines[j].replacen('#', "", 1);
                        }
                    }
                    j += 1;
                }
            }
        } else {
            lines.push("".to_string());
            lines.push(format!("[{}]", name));
            lines.push("SigLevel = PackageOptional".to_string());
            lines.push(format!("Server = {}", url));
        }
    } else {
        if let Some(idx) = found_index {
            if !lines[idx].trim().starts_with('#') {
                lines[idx] = format!("#{}", lines[idx]);
                let mut j = idx + 1;
                while j < lines.len() && !lines[j].trim().starts_with('[') {
                    if !lines[j].trim().starts_with('#') && !lines[j].trim().is_empty() {
                        lines[j] = format!("#{}", lines[j]);
                    }
                    j += 1;
                }
            }
        }
    }

    std::fs::write(conf_path, lines.join("\n")).map_err(|e| e.to_string())?;
    Ok(())
}

use crate::{aur_api, commands::utils as cmd_utils, models, repo_manager::RepoManager};
use serde::Serialize;
use std::process::Stdio;
use tauri::{AppHandle, Emitter, State};
use tempfile;
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
use tokio::sync::Mutex;

lazy_static::lazy_static! {
    static ref ACTIVE_INSTALL_PROCESS: Mutex<Option<tokio::process::Child>> = Mutex::new(None);
}

#[derive(Serialize, Clone)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub install_date: Option<String>,
    pub size: Option<String>,
    pub url: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct PackageInstallStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub repo: Option<String>,
    pub source: Option<models::PackageSource>,
    pub actual_package_name: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct PendingUpdate {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
    pub repo: String,
}

#[tauri::command]
pub async fn abort_installation(app: AppHandle) -> Result<(), String> {
    let mut active = ACTIVE_INSTALL_PROCESS.lock().await;
    if let Some(mut child) = active.take() {
        let _ = app.emit("install-output", "--- Installation Aborted by User ---");
        let _ = child.kill().await;
        let _ = app.emit("install-complete", "failed");
        Ok(())
    } else {
        // Fallback: kill any stray pacman/yay processes if we lost track
        let _ = app.emit(
            "install-output",
            "Clean-up: Killing any stray pacman/yay processes...",
        );
        let _ = tokio::process::Command::new("sudo")
            .args(["killall", "-9", "pacman", "yay", "paru", "makepkg"])
            .status()
            .await;
        let _ = app.emit("install-complete", "failed");
        Ok(())
    }
}

#[tauri::command]
pub async fn install_package(
    _app: AppHandle,
    _state_repo: State<'_, RepoManager>,
    app_handle: AppHandle,
    name: String,
    source: models::PackageSource,
    password: Option<String>,
    repo_name: Option<String>,
) -> Result<(), String> {
    install_package_core(
        &app_handle,
        &*_state_repo,
        &name,
        source,
        &password,
        repo_name,
    )
    .await
}

pub async fn install_package_core(
    app: &AppHandle,
    _repo_manager: &RepoManager,
    name: &str,
    source: models::PackageSource,
    password: &Option<String>,
    repo_name: Option<String>,
) -> Result<(), String> {
    // VECTOR 5: INPUT SANITIZATION
    crate::utils::validate_package_name(name)?;

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

    // PILLAR 6: Manjaro Stability Guard (Refined)
    // Block Pre-built binaries from Arch-based repos (Chaotic/CachyOS) on Manjaro due to glibc/python mismatches.
    let distro = crate::distro_context::DistroContext::new();
    if distro.id == crate::distro_context::DistroId::Manjaro {
        if matches!(
            source,
            models::PackageSource::Chaotic | models::PackageSource::CachyOS
        ) {
            let msg = "Manjaro Stability Guard: Installing pre-built binaries (Chaotic/CachyOS) is blocked on Manjaro to prevent system breakage. Please use the AUR (Native Build) version instead.".to_string();
            let _ = app.emit("install-output", &msg);
            let _ = app.emit("install-complete", "failed");
            return Err(msg);
        }
    }

    // Pre-flight check: Database Lock
    if crate::repair::check_pacman_lock().await {
        let _ = app.emit(
            "install-output",
            "Error: Pacman database is locked (/var/lib/pacman/db.lck).",
        );
        let _ = app.emit("install-complete", "failed");
        return Err("Pacman database is locked".to_string());
    }

    let native_builder_needed = match source {
        models::PackageSource::Aur => {
            let helpers = ["/usr/bin/paru", "/usr/bin/yay", "/usr/bin/aura"];
            let mut found = None;
            for h in helpers {
                if std::path::Path::new(h).exists() {
                    found = Some(h);
                    break;
                }
            }
            found.is_none()
        }
        _ => false,
    };

    if native_builder_needed {
        // LOCK for AUR Build
        let _guard = crate::utils::PRIVILEGED_LOCK.lock().await;
        match build_aur_package(app, name, password).await {
            Ok(_) => {
                let _ = app.emit("install-complete", "success");
                return Ok(());
            }
            Err(e) => {
                let _ = app.emit("install-output", format!("Build Error: {}", e));
                let _ = app.emit("install-complete", "failed");
                return Err(e);
            }
        }
    }

    let (binary, args) = match source {
        models::PackageSource::Aur => {
            let helpers = ["/usr/bin/paru", "/usr/bin/yay", "/usr/bin/aura"];
            let mut found = None;
            for h in helpers {
                if std::path::Path::new(h).exists() {
                    found = Some(h);
                    break;
                }
            }
            let h = found.unwrap_or_else(|| "/usr/bin/paru");
            (
                h.to_string(),
                vec![
                    "-S".to_string(),
                    "--noconfirm".to_string(),
                    "--overwrite".to_string(),
                    "*".to_string(),
                    "--".to_string(),
                    name.to_string(),
                ],
            )
        }
        _ => {
            // PILLAR 2: Smart Sync & PILLAR 6: Safety Core
            let metadata = std::fs::metadata("/var/lib/pacman/sync/core.db");
            let is_recent_sync = if let Ok(m) = metadata {
                if let Ok(mod_time) = m.modified() {
                    if let Ok(elapsed) = mod_time.elapsed() {
                        elapsed.as_secs() < 3600 // 1 Hour
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            let mut args = Vec::new();
            if is_recent_sync {
                let _ = app.emit(
                    "install-output",
                    "Smart Sync: Database is fresh (<1h). Instant Start...",
                );
                args.push("-S");
                args.push("--needed");
            } else {
                let _ = app.emit(
                    "install-output",
                    "Smart Sync: Database outdated. Running Full Upgrade (-Syu)...",
                );
                args.push("-Syu");
            }

            args.push("--noconfirm");
            args.push("--");

            // Handle Repo Targeting & Switch Safety
            let target_string; // Keep alive
            if let Some(r_name) = &repo_name {
                target_string = format!("{}/{}", r_name, name);
                args.push("--overwrite");
                args.push("*");
                args.push(&target_string);
            } else {
                args.push(name);
            }

            cmd_utils::build_pacman_cmd(&args, password)
        }
    };

    // ACQUIRE LOCK for Execution
    let _guard = crate::utils::PRIVILEGED_LOCK.lock().await;

    let _ = app.emit(
        "install-output",
        format!("Executing: {} {:?}", binary, args),
    );

    if matches!(source, models::PackageSource::Aur) && password.is_some() {
        if let Some(pwd) = password {
            if let Ok(mut c) = tokio::process::Command::new("sudo")
                .arg("-S")
                .arg("-v")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut s) = c.stdin.take() {
                    let _ = tokio::io::AsyncWriteExt::write_all(
                        &mut s,
                        format!("{}\n", pwd).as_bytes(),
                    )
                    .await;
                }
                let _ = c.wait().await;
            }
        }
    }

    let mut child = tokio::process::Command::new(binary);
    for arg in &args {
        child.arg(arg);
    }

    // Capture logs for analysis
    let captured_logs;
    let (log_tx, mut log_rx) = tokio::sync::mpsc::unbounded_channel();

    match child
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            if !matches!(source, models::PackageSource::Aur) {
                if let Some(pwd) = password {
                    if let Some(mut s) = child.stdin.take() {
                        let _ = tokio::io::AsyncWriteExt::write_all(
                            &mut s,
                            format!("{}\n", pwd).as_bytes(),
                        )
                        .await;
                    }
                }
            }
            if let Some(out) = child.stdout.take() {
                let tx = log_tx.clone();
                let a = app.clone();
                tokio::spawn(async move {
                    let reader = TokioBufReader::new(out);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = a.emit("install-output", &line);
                        let _ = tx.send(line);
                    }
                });
            }
            if let Some(err) = child.stderr.take() {
                let tx = log_tx.clone();
                let a = app.clone();
                tokio::spawn(async move {
                    let reader = TokioBufReader::new(err);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = a.emit("install-output", &line);
                        let _ = tx.send(line);
                    }
                });
            }

            // Track process for aborting
            {
                let mut active = ACTIVE_INSTALL_PROCESS.lock().await;
                *active = Some(child);
            }

            // Re-acquire child for waiting
            let mut child = {
                let mut active = ACTIVE_INSTALL_PROCESS.lock().await;
                active.take().unwrap()
            };

            // Collecting logs in background
            let log_collection = tokio::spawn(async move {
                let mut logs = Vec::new();
                while let Some(line) = log_rx.recv().await {
                    logs.push(line);
                }
                logs
            });

            let status = match child.wait().await {
                Ok(s) => s,
                Err(e) => {
                    let _ = app.emit("install-output", format!("Process wait failed: {}", e));
                    let _ = app.emit("install-complete", "failed");
                    return Err(e.to_string());
                }
            };

            // Wait for logs to finish draining
            drop(log_tx); // close sender
            captured_logs = log_collection.await.unwrap_or_default();

            let success = status.success();

            // SAFE STORE: Error Analysis
            if !success {
                let full_log = captured_logs.join("\n");
                // Check for 404 / Retrieve errors indicating stale DB
                if full_log.contains("404 Not Found")
                    || full_log.contains("failed retrieving file")
                    || full_log.contains("target not found")
                    || full_log.contains("unsatisfiable dependency")
                    || full_log.contains("cannot resolve")
                {
                    let _ = app.emit("install-complete", "failed_update_required");
                    return Err("SystemUpdateRequired".to_string());
                }
            }

            let _ = app.emit(
                "install-complete",
                if success { "success" } else { "failed" },
            );

            use tauri_plugin_notification::NotificationExt;
            if success {
                let _ = app
                    .notification()
                    .builder()
                    .title("✨ MonArch: Installation Complete")
                    .body(format!("Successfully installed '{}'", name))
                    .show();
            }

            if success {
                // TELEMETRY: Track Successful Install
                crate::utils::track_event_safe(
                    app,
                    "install_package",
                    Some(serde_json::json!({
                        "pkg": name,
                        "source": format!("{:?}", source),
                        "from_repo": repo_name.is_some()
                    })),
                )
                .await;

                Ok(())
            } else {
                Err("Installation failed".to_string())
            }
        }
        Err(e) => {
            let _ = app.emit(
                "install-output",
                format!("Failed to spawn installer: {}", e),
            );
            let _ = app.emit("install-complete", "failed");
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn uninstall_package(
    app: AppHandle,
    name: String,
    password: Option<String>,
) -> Result<(), String> {
    // SUICIDE PREVENTION: Protect critical system packages
    let protected = [
        "base",
        "base-devel",
        "linux",
        "linux-lts",
        "linux-zen",
        "glibc",
        "systemd",
        "pacman",
        "sudo",
        "monarch-store",
        "monarch-pk-helper",
    ];

    if protected.contains(&name.as_str()) {
        let _ = app.emit("install-complete", "failed");
        return Err(format!(
            "PITAL ERROR: '{}' is a protected system package. Uninstallation is forbidden.",
            name
        ));
    }
    // Acquire global lock
    let _guard = crate::utils::PRIVILEGED_LOCK.lock().await;

    let _ = app.emit(
        "install-output",
        format!("Preparing to uninstall '{}'...", name),
    );
    let (binary, args) =
        cmd_utils::build_pacman_cmd(&["-Rns", "--noconfirm", "--", &name][..], &password);
    let mut child = tokio::process::Command::new(binary);
    for arg in &args {
        child.arg(arg);
    }

    match child
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
                    let reader = TokioBufReader::new(out);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = a.emit("install-output", line);
                    }
                });
            }
            match child.wait().await {
                Ok(s) if s.success() => {
                    let _ = app.emit("install-complete", "success");
                    Ok(())
                }
                _ => {
                    let _ = app.emit("install-complete", "failed");
                    Err("Uninstall failed".to_string())
                }
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

pub async fn build_aur_package(
    app: &AppHandle,
    name: &str,
    password: &Option<String>,
) -> Result<(), String> {
    // Audit dependencies
    audit_aur_builder_deps(app)
        .map_err(|e| format!("Build environment verification failed: {}", e))?;

    let mut resolved = Vec::new();
    let mut visited = std::collections::HashSet::new();

    resolve_aur_dependencies(app, name, &mut resolved, &mut visited).await?;

    if resolved.len() > 1 {
        let _ = app.emit(
            "install-output",
            format!("Building {} AUR dependencies...", resolved.len() - 1),
        );
    }

    for pkg_name in resolved {
        build_aur_package_single(app, &pkg_name, password).await?;
    }

    Ok(())
}

async fn build_aur_package_single(
    app: &AppHandle,
    name: &str,
    password: &Option<String>,
) -> Result<(), String> {
    let temp_dir = tempfile::tempdir().map_err(|e: std::io::Error| e.to_string())?;
    let pkg_path = temp_dir.path();

    let _ = app.emit("install-output", format!("Cloning {} from AUR...", name));
    let clone_status = tokio::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            &format!("https://aur.archlinux.org/{}.git", name),
        ])
        .current_dir(pkg_path)
        .status()
        .await
        .map_err(|e| e.to_string())?;

    // Prime sudo credentials if password is provided
    if let Some(pwd) = password {
        let _ = app.emit("install-output", "Refreshing privileged credentials...");
        let mut child = tokio::process::Command::new("sudo")
            .args(["-S", "-v"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn sudo refresh: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut stdin, format!("{}\n", pwd).as_bytes())
                    .await;
        }
        let status = child.wait().await.map_err(|e| e.to_string())?;
        if !status.success() {
            let _ = app.emit(
                "install-output",
                "Warning: Sudo refresh failed. Build might prompt for password.",
            );
        }
    }

    if !clone_status.success() {
        return Err(format!("Failed to clone {} from AUR", name));
    }

    let pkg_dir = pkg_path.join(name);

    // SECURITY: Root Check
    // We must ensure we are NOT running as root before invoking makepkg
    #[cfg(target_os = "linux")]
    {
        let is_root = std::process::Command::new("id")
            .arg("-u")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
            .unwrap_or(false);

        if is_root {
            return Err(
                "Security Violation: Attempted to run makepkg as root. This is forbidden."
                    .to_string(),
            );
        }
    }

    let _ = app.emit("install-output", format!("Building {} (makepkg)...", name));

    let mut makepkg = tokio::process::Command::new("makepkg");
    makepkg
        .args(["-si", "--noconfirm", "--needed"])
        .current_dir(&pkg_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = makepkg.spawn().map_err(|e| e.to_string())?;

    if let Some(pwd) = password {
        if let Some(mut stdin) = child.stdin.take() {
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut stdin, format!("{}\n", pwd).as_bytes())
                    .await;
        }
    }

    let mut missing_key = None;

    if let Some(out) = child.stdout.take() {
        let a = app.clone();
        tokio::spawn(async move {
            let reader = TokioBufReader::new(out);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = a.emit("install-output", line);
            }
        });
    }

    if let Some(err) = child.stderr.take() {
        let a = app.clone();
        let mut reader = TokioBufReader::new(err).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = a.emit("install-output", format!("MAKEPKG: {}", line));

            // Detect GPG errors
            if line.contains("unknown public key") || line.contains("not found in keychain") {
                // Extract key ID (usually the last word or in quotes)
                if let Some(pos) = line.find("key ") {
                    let key = &line[pos + 4..].trim_matches(|c: char| !c.is_alphanumeric());
                    if key.len() >= 8 {
                        missing_key = Some(key.to_string());
                    }
                }
            }

            if line.contains("MAKEPKG: ERROR:") {
                // error logged above
            }
        }
    }

    let exit_status = child.wait().await.map_err(|e| e.to_string())?;

    if !exit_status.success() {
        if let Some(key) = missing_key {
            import_gpg_key(app, &key).await?;
            // Retry ONCE
            let _ = app.emit("install-output", "Retrying build after key import...");
            return Box::pin(build_aur_package_single(app, name, password)).await;
        }
        return Err(format!("makepkg failed for {}", name));
    }

    Ok(())
}

use futures::future::{BoxFuture, FutureExt};

pub fn resolve_aur_dependencies<'a>(
    app: &'a AppHandle,
    name: &'a str,
    resolved: &'a mut Vec<String>,
    visited: &'a mut std::collections::HashSet<String>,
) -> BoxFuture<'a, Result<(), String>> {
    async move {
        if visited.contains(name) {
            return Ok(());
        }
        visited.insert(name.to_string());

        let _ = app.emit(
            "install-output",
            format!("Checking dependencies for {}...", name),
        );

        // Fetch AUR info
        let names = [name];
        let info = aur_api::get_multi_info(&names[..]).await?;
        let pkg = match info.first() {
            Some(p) => p,
            _ => return Err(format!("Package {} not found in AUR", name)),
        };

        let mut all_deps: Vec<String> = Vec::new();
        if let Some(deps) = &pkg.depends {
            all_deps.extend(deps.clone());
        }
        if let Some(deps) = &pkg.make_depends {
            all_deps.extend(deps.clone());
        }

        for dep_entry in all_deps {
            // Strip version constraints: "libfoo>=1.0" -> "libfoo"
            let dep_name = dep_entry
                .split(['=', '>', '<'])
                .next()
                .unwrap_or(&dep_entry)
                .trim();

            if is_package_satisfied(dep_name).await {
                continue;
            }

            // Check if it's in official repos (we skip this if pacman can find it)
            if is_in_official_repos(dep_name).await {
                continue;
            }

            // If not official and not satisfied, assume it's AUR
            resolve_aur_dependencies(app, dep_name, resolved, visited).await?;
        }

        if !resolved.contains(&name.to_string()) {
            resolved.push(name.to_string());
        }

        Ok(())
    }
    .boxed()
}

async fn is_package_satisfied(name: &str) -> bool {
    // Check if package or something providing it is installed
    let status = tokio::process::Command::new("pacman")
        .args(["-Qq", name])
        .output()
        .await;

    if let Ok(o) = status {
        if o.status.success() {
            return true;
        }
    }

    // Check if it's provided by someone else
    let _status = tokio::process::Command::new("pacman")
        .args(["-Qq", "-p", name]) // This isn't quite right for provides
        .output()
        .await;

    // Better: pacman -T checks if dependencies are satisfied
    let t_status = tokio::process::Command::new("pacman")
        .args(["-T", name])
        .status()
        .await;

    match t_status {
        Ok(s) => s.success(), // pacman -T returns 0 if satisfied
        Err(_) => false,
    }
}

async fn is_in_official_repos(name: &str) -> bool {
    // Check if pacman can find it in sync databases
    let status = tokio::process::Command::new("pacman")
        .args(["-Si", name])
        .output()
        .await;

    match status {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

async fn import_gpg_key(app: &AppHandle, key_id: &str) -> Result<(), String> {
    let _ = app.emit(
        "install-output",
        format!("Auto-importing PGP key: {}...", key_id),
    );

    let status = tokio::process::Command::new("gpg")
        .args(["--recv-keys", key_id])
        .status()
        .await
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        // Try alternate keyserver
        let status = tokio::process::Command::new("gpg")
            .args(["--keyserver", "keyserver.ubuntu.com", "--recv-keys", key_id])
            .status()
            .await
            .map_err(|e| e.to_string())?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Failed to import GPG key {}", key_id))
        }
    }
}

pub fn audit_aur_builder_deps(app: &AppHandle) -> Result<(), String> {
    let deps = ["base-devel", "git"];
    for dep in deps {
        let has_dep = std::process::Command::new("pacman")
            .args(["-Qq", dep])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !has_dep {
            let _ = app.emit(
                "install-output",
                format!(
                    "Error: Missing BUILD dependency: {}. Please install it first.",
                    dep
                ),
            );
            return Err(format!("Missing {}", dep));
        }
    }
    Ok(())
}

#[derive(Serialize, Clone)]
pub struct UpdateProgress {
    pub phase: String,
    pub progress: u32,
    pub message: String,
}

#[tauri::command]
pub async fn perform_system_update(
    app: AppHandle,
    password: Option<String>,
) -> Result<String, String> {
    // Acquire global lock
    let _guard = crate::utils::PRIVILEGED_LOCK.lock().await;

    let _ = app.emit("install-output", "--- Starting Global System Update ---");
    let _ = app.emit(
        "update-progress",
        UpdateProgress {
            phase: "start".to_string(),
            progress: 0,
            message: "Starting update...".to_string(),
        },
    );

    let (binary, args) = cmd_utils::build_pacman_cmd(&["-Syu", "--noconfirm"][..], &password);
    let mut child = tokio::process::Command::new(binary);
    for arg in &args {
        child.arg(arg);
    }

    match child
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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

            // Progress tracking variables
            let app_prog = app.clone();
            let mut current_progress = 0;

            if let Some(out) = child.stdout.take() {
                let a = app.clone();
                tokio::spawn(async move {
                    let reader = TokioBufReader::new(out);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = a.emit("install-output", &line);

                        // Parse Progress
                        let lower = line.to_lowercase();

                        if lower.contains("synchronizing package databases") {
                            current_progress = 10;
                            let _ = app_prog.emit(
                                "update-progress",
                                UpdateProgress {
                                    phase: "sync".to_string(),
                                    progress: 10,
                                    message: "Synchronizing databases...".to_string(),
                                },
                            );
                        } else if lower.contains("starting full system upgrade") {
                            current_progress = 20;
                            let _ = app_prog.emit(
                                "update-progress",
                                UpdateProgress {
                                    phase: "upgrade".to_string(),
                                    progress: 20,
                                    message: "Calculating upgrade...".to_string(),
                                },
                            );
                        } else if lower.contains("downloading") {
                            // downloading x...
                            // Slowly increment if stuck
                            if current_progress < 50 {
                                current_progress += 2;
                            }
                            let _ = app_prog.emit(
                                "update-progress",
                                UpdateProgress {
                                    phase: "download".to_string(),
                                    progress: current_progress,
                                    message: "Downloading packages...".to_string(),
                                },
                            );
                        } else if lower.contains("checking keys")
                            || lower.contains("checking package integrity")
                        {
                            current_progress = 60;
                            let _ = app_prog.emit(
                                "update-progress",
                                UpdateProgress {
                                    phase: "verify".to_string(),
                                    progress: 60,
                                    message: "Verifying packages...".to_string(),
                                },
                            );
                        } else if lower.contains("installing") || lower.contains("upgrading") {
                            if current_progress < 90 {
                                current_progress += 1;
                            }
                            let _ = app_prog.emit(
                                "update-progress",
                                UpdateProgress {
                                    phase: "install".to_string(),
                                    progress: current_progress,
                                    message: "Installing updates...".to_string(),
                                },
                            );
                        }
                    }
                });
            }
            match child.wait().await {
                Ok(s) if s.success() => {
                    // TELEMETRY: Track System Update
                    crate::utils::track_event_safe(&app, "system_update", None).await;

                    let _ = app.emit("install-complete", "success");
                    Ok("System updated successfully".to_string())
                }
                _ => {
                    let _ = app.emit("install-complete", "failed");
                    Err("System update failed".to_string())
                }
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn fetch_pkgbuild(pkg_name: String) -> Result<String, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        pkg_name
    );
    let resp = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        resp.text().await.map_err(|e| e.to_string())
    } else {
        Err(format!("Failed to fetch PKGBUILD: {}", resp.status()))
    }
}

#[tauri::command]
pub async fn get_installed_packages(
    state: tauri::State<'_, crate::metadata::MetadataState>,
) -> Result<Vec<InstalledPackage>, String> {
    let output = std::process::Command::new("pacman")
        .arg("-Qei")
        .output()
        .map_err(|e| e.to_string())?;
    let content = String::from_utf8_lossy(&output.stdout);
    let all_pkgs = parse_pacman_qi(&content);

    // Filter for "Apps" (must have icon or app_id) to hide libs
    let mut apps = Vec::new();
    if let Ok(loader) = state.inner().0.lock() {
        for pkg in all_pkgs {
            // Check if it's an app
            let has_icon = loader.find_icon_heuristic(&pkg.name).is_some();
            let has_id = loader.find_app_id(&pkg.name).is_some();

            // Heuristic: If description says "Application" or "Game" or similar?
            // For now, rely on Metadata presence.
            if has_icon || has_id {
                apps.push(pkg);
            } else {
                // Keep if specifically whitelisted or looks like a GUI app?
                // Fallback: If it's explicitly installed, maybe we show it if we are unsure?
                // The user complained about "system apps".
                // Let's be strict for now.
            }
        }
    } else {
        // If lock fails, return all (fallback)
        return Ok(all_pkgs);
    }

    Ok(apps)
}

#[tauri::command]
pub async fn check_for_updates() -> Result<Vec<PendingUpdate>, String> {
    let output = std::process::Command::new("checkupdates")
        .output()
        .map_err(|e| e.to_string())?;
    let content = String::from_utf8_lossy(&output.stdout);
    Ok(parse_checkupdates(&content))
}

#[tauri::command]
pub async fn get_orphans() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("pacman")
        .args(["-Qtdq"])
        .output()
        .map_err(|e| e.to_string())?;
    let content = String::from_utf8_lossy(&output.stdout);
    Ok(content.lines().map(|s| s.to_string()).collect())
}

#[tauri::command]
pub async fn remove_orphans(orphans: Vec<String>) -> Result<(), String> {
    if orphans.is_empty() {
        return Ok(());
    }
    let mut args = vec!["-Rns", "--noconfirm"];
    args.extend(orphans.iter().map(|s| s.as_str()));
    let args_str: Vec<&str> = args.iter().map(|s| *s).collect();
    let (binary, args) = cmd_utils::build_pacman_cmd(&args_str[..], &Option::None);
    let status = std::process::Command::new(binary)
        .args(args)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Failed to remove orphans".to_string())
    }
}

fn parse_pacman_qi(content: &str) -> Vec<InstalledPackage> {
    let mut packages = Vec::new();
    let mut current = InstalledPackage {
        name: "".to_string(),
        version: "".to_string(),
        description: "".to_string(),
        install_date: None,
        size: None,
        url: None,
    };
    for line in content.lines() {
        if line.is_empty() {
            if !current.name.is_empty() {
                packages.push(current.clone());
            }
            current = InstalledPackage {
                name: "".to_string(),
                version: "".to_string(),
                description: "".to_string(),
                install_date: None,
                size: None,
                url: None,
            };
            continue;
        }
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() == 2 {
            let key = parts[0].trim();
            let val = parts[1].trim().to_string();
            match key {
                "Name" => current.name = val,
                "Version" => current.version = val,
                "Description" => current.description = val,
                "Install Date" => current.install_date = Some(val),
                "Installed Size" => current.size = Some(val),
                "URL" => current.url = Some(val),
                _ => {}
            }
        }
    }
    if !current.name.is_empty() {
        packages.push(current);
    }
    packages
}

fn parse_checkupdates(content: &str) -> Vec<PendingUpdate> {
    content
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                Some(PendingUpdate {
                    name: parts[0].to_string(),
                    old_version: parts[1].to_string(),
                    new_version: parts[3].to_string(),
                    repo: "official".to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}
#[tauri::command]
pub async fn check_installed_status(
    state: State<'_, crate::metadata::MetadataState>,
    name: String,
) -> Result<PackageInstallStatus, String> {
    let mut current_name = name;
    let mut resolved_via_fallback = false;

    // Use a loop to handle fallbacks without recursion (avoids Send/Sync issues with MutexGuards)
    for _ in 0..2 {
        // 1. Resolve App ID to package name if needed
        let resolved_name = {
            let loader = state.inner().0.lock().unwrap();
            loader.resolve_package_name(&current_name)
        };

        let output = std::process::Command::new("pacman")
            .env("LC_ALL", "C") // Force English output
            .args(["-Qi", &resolved_name, "--color", "never"]) // Disable ANSI colors
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut version = None;
            let mut packager = None;

            let mut repo_name = None;

            for line in stdout.lines() {
                if line.starts_with("Version") {
                    if let Some(v) = line.split(':').nth(1) {
                        version = Some(v.trim().to_string());
                    }
                }
                if line.starts_with("Repository") {
                    if let Some(r) = line.split(':').nth(1) {
                        let trimmed = r.trim().to_string();
                        println!(
                            "DEBUG: Found Repository raw: '{}' -> trimmed: '{}'",
                            r, trimmed
                        );
                        repo_name = Some(trimmed);
                    }
                }
                // Handle "Repository      : core" vs "Repository : core"

                if line.starts_with("Packager") {
                    if let Some(p) = line.split(':').nth(1) {
                        packager = Some(p.trim().to_string().to_lowercase());
                    }
                }
            }

            // Robust Source Inference:
            // 1. Try Repository Name first (if present)
            // 2. Fallback to Packager if Repository is missing or generic (local/unknown)

            let mut inferred_source = None;

            // Check Repository Field
            if let Some(r) = &repo_name {
                let r_lower = r.to_lowercase();
                if r_lower == "chaotic-aur" || r_lower.contains("chaotic") {
                    inferred_source = Some(models::PackageSource::Chaotic);
                } else if r_lower.starts_with("cachyos") {
                    inferred_source = Some(models::PackageSource::CachyOS);
                } else if r_lower == "core"
                    || r_lower == "extra"
                    || r_lower == "multilib"
                    || r_lower == "community"
                {
                    inferred_source = Some(models::PackageSource::Official);
                } else if r_lower == "garuda" {
                    inferred_source = Some(models::PackageSource::Garuda);
                } else if r_lower == "endeavouros" {
                    inferred_source = Some(models::PackageSource::Endeavour);
                } else if r_lower.contains("manjaro") {
                    inferred_source = Some(models::PackageSource::Manjaro);
                } else if r_lower == "aur" {
                    inferred_source = Some(models::PackageSource::Aur);
                }
            }

            // If still unknown (Repo missing or local), check Packager
            if inferred_source.is_none() {
                if let Some(p) = &packager {
                    // Check for Known Maintainer Signatures
                    if p.contains("archlinux.org") {
                        inferred_source = Some(models::PackageSource::Official);
                    } else if p.contains("chaotic") {
                        inferred_source = Some(models::PackageSource::Chaotic);
                    } else if p.contains("cachyos") {
                        // Added CachyOS signature check
                        inferred_source = Some(models::PackageSource::CachyOS);
                    } else if p.contains("manjaro") {
                        inferred_source = Some(models::PackageSource::Manjaro);
                    } else if p.contains("garuda") {
                        inferred_source = Some(models::PackageSource::Garuda);
                    } else if p.contains("endeavouros") {
                        inferred_source = Some(models::PackageSource::Endeavour);
                    }
                }
            }

            // Final Fallback: If repo is explicitly 'local' or 'unknown' and we still don't know, assume AUR.
            // But if repo is None (missing line), and we couldn't infer from Packager, it's truly unknown (or local built without signature).
            // We'll default to AUR for missing/local repos if no other signature matches, as that's the safe bet for "User installed this manually".
            if inferred_source.is_none() {
                if let Some(r) = &repo_name {
                    let r_lower = r.to_lowercase();
                    if r_lower == "local" || r_lower == "unknown" {
                        inferred_source = Some(models::PackageSource::Aur);
                    }
                } else {
                    // Case: Repository line MISSING (e.g. broken DB).
                    // If we have a version, it's installed. Assume AUR/Local to allow "Update" checks if name matches.
                    inferred_source = Some(models::PackageSource::Aur);
                }
            }

            // Fix: If repo_name is missing but we inferred source (e.g. CachyOS), backfill repo_name for UI
            if repo_name.is_none() {
                if let Some(src) = &inferred_source {
                    repo_name = Some(format!("{:?}", src).to_lowercase());
                } else {
                    repo_name = Some("unknown".to_string());
                }
            }

            return Ok(PackageInstallStatus {
                installed: true,
                version,
                repo: repo_name,
                source: inferred_source,
                actual_package_name: Some(resolved_name),
            });
        }

        // 2. ULTIMATE FALLBACK: If Qi failed, check if we already found it via scanner OR scan now
        if !output.status.success() {
            // If we already resolved it via fallback (meaning it IS in -Qq), but -Qi failed again...
            // This happens on corrupted DBs where -Qi fails but package is installed.
            if resolved_via_fallback {
                return Ok(PackageInstallStatus {
                    installed: true,
                    version: None, // Can't get metadata from broken DB
                    repo: None,
                    source: None,
                    actual_package_name: Some(current_name),
                });
            }

            let mut found_pkg = None;
            if let Ok(output_q) = std::process::Command::new("pacman").arg("-Qq").output() {
                let stdout = String::from_utf8_lossy(&output_q.stdout);
                {
                    let loader = state.inner().0.lock().unwrap();
                    for pkg_name in stdout.lines() {
                        let pkg_clean = pkg_name.trim();

                        // 1. Direct Name Match
                        if pkg_clean.eq_ignore_ascii_case(&current_name) {
                            found_pkg = Some(pkg_clean.to_string());
                            break;
                        }

                        // 2. App ID Match
                        if let Some(found_id) = loader.find_app_id(pkg_clean) {
                            if found_id.to_lowercase() == current_name.to_lowercase() {
                                found_pkg = Some(pkg_clean.to_string());
                                break;
                            }
                        }
                    }
                }
            }

            if let Some(pkg_name) = found_pkg {
                current_name = pkg_name;
                resolved_via_fallback = true;
                continue; // Try one more time with correct name (will likely fail -Qi again, but catched above)
            }
        }

        break;
    }

    Ok(PackageInstallStatus {
        installed: false,
        version: None,
        repo: None,
        source: None,
        actual_package_name: None,
    })
}

#[tauri::command]
pub async fn get_essentials_list(
    state_repo: State<'_, RepoManager>,
) -> Result<Vec<String>, String> {
    // PILLAR 7: Essentials Smart Curation

    // 1. CachyOS Spotlight
    let mut essentials = vec![];
    if state_repo.inner().is_repo_enabled("cachyos").await {
        essentials.extend(vec![
            "cachyos-settings",
            "linux-cachyos",
            "cachyos-browser",
            "cachyos-fish-config",
            "paru",
        ]);
    }

    // 2. The Core Essentials (Official Arch)
    essentials.extend(vec![
        "firefox",
        "vlc",
        "neofetch",
        "htop",
        "gimp",
        "libreoffice-fresh",
        "visual-studio-code-bin",
        "spotify",
        "discord",
        "obs-studio",
        "steam",
        "qbittorrent",
        "mpv",
        "kitty",
        "fish",
        "obsidian",
        "thunderbird",
        "thunar",
        "ark",
        "partitionmanager",
        "btop",
        // Add more popular ones
        "google-chrome",
        "slack-desktop",
        "zoom",
        "telegram-desktop-bin",
        "brave-bin",
    ]);

    // 3. Dynamic DB Override (if exists, it PREPENDS or REPLACES? Let's say it supplements)
    // Actually, strict file logic says "if path exists, return lines".
    // We should probably keep that behavior for power users who customized valid paths.
    let path = std::path::Path::new("/var/lib/monarch/dbs/essentials.db");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            let custom_lines: Vec<String> = content
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect();

            if !custom_lines.is_empty() {
                // Return custom listing instead of default
                return Ok(custom_lines);
            }
        }
    }

    // Deduplicate just in case
    let mut unique = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for pkg in essentials {
        if seen.insert(pkg) {
            unique.push(pkg.to_string());
        }
    }

    Ok(unique)
}

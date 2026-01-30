use crate::{aur_api, models, repo_manager::RepoManager, helper_client};
use serde::Serialize;
use std::path::Path;
use std::process::Stdio;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_notification::NotificationExt;
use tempfile;
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
use tokio::sync::Mutex;

/// Zone 4: Copy built .pkg.tar.zst to shared temp so root helper can read them.
const MONARCH_INSTALL_DIR: &str = "/tmp/monarch-install";

pub async fn copy_paths_to_monarch_install(paths: Vec<String>) -> Result<Vec<String>, String> {
    tokio::fs::create_dir_all(MONARCH_INSTALL_DIR)
        .await
        .map_err(|e| format!("Could not create {}: {}", MONARCH_INSTALL_DIR, e))?;
    let mut out = Vec::with_capacity(paths.len());
    for src in paths {
        let src_path = Path::new(&src);
        let name = src_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("Invalid path: {}", src))?;
        let dest = format!("{}/{}", MONARCH_INSTALL_DIR, name);
        tokio::fs::copy(&src, &dest)
            .await
            .map_err(|e| format!("Could not copy {} to {}: {}", src, dest, e))?;
        out.push(dest);
    }
    Ok(out)
}

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
    pub repository: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct PackageInstallStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub repo: Option<String>,
    pub source: Option<models::PackageSource>,
    pub actual_package_name: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
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
        // SECURITY: Do NOT use killall as fallback - it could kill unrelated pacman processes
        // and potentially corrupt the package database. Instead, inform the user.
        let _ = app.emit(
            "install-output",
            "Warning: No tracked installation process found. If an operation is stuck, please wait for it to complete or manually close any package manager windows.",
        );
        let _ = app.emit("install-complete", "failed");
        Err(
            "No active installation to abort. If pacman is locked, use the Repair tool to unlock."
                .to_string(),
        )
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
    _repo_name: Option<String>,
) -> Result<(), String> {
    install_package_core(
        &app_handle,
        &*_state_repo,
        &name,
        source,
        &password,
        _repo_name,
    )
    .await
}

pub async fn install_package_core(
    app: &AppHandle,
    repo_manager: &RepoManager,
    name: &str,
    source: models::PackageSource,
    password: &Option<String>,
    _repo_name: Option<String>,
) -> Result<(), String> {
    // VECTOR 5: INPUT SANITIZATION
    crate::utils::validate_package_name(name)?;

    // No conflicting-process check here: rely on db.lck and helper failure if another
    // package manager is running. The check caused false positives (e.g. our own
    // pacman -Q verification, or CachyOS updater) and broke installs for users who
    // "never had an issue before". Real conflicts still surface as database locked.

    // ✅ DISTRO-AWARE: Manjaro Stability Guard (Refined)
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

    // ✅ HARDWARE OPTIMIZATION DETECTION
    let cpu_optimization = if crate::utils::is_cpu_znver4_compatible() {
        Some("znver4".to_string())
    } else if crate::utils::is_cpu_v4_compatible() {
        Some("v4".to_string())
    } else if crate::utils::is_cpu_v3_compatible() {
        Some("v3".to_string())
    } else {
        None
    };

    // ✅ GET ENABLED REPOS (Soft Disable support)
    let enabled_repos: Vec<String> = repo_manager
        .get_all_repos()
        .await
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.name.clone())
        .collect();

    // Acquire global lock
    let _guard = crate::utils::PRIVILEGED_LOCK.lock().await;

    match source {
        models::PackageSource::Aur => {
            // ✅ AUR: Build with makepkg, install with ALPM
            let _ = app.emit(
                "install-output",
                "--- Starting Secure AUR Build-Install Pipeline ---",
            );
            let built_paths = build_aur_package(app, name, password).await?;
            let install_paths = copy_paths_to_monarch_install(built_paths).await?;

            // ✅ NEW: Install built packages via ALPM transaction (paths in /tmp/monarch-install for root)
            let _ = app.emit("install-output", "Installing built AUR package(s)...");
            
            let mut rx = helper_client::invoke_helper(
                app,
                helper_client::HelperCommand::AlpmInstallFiles {
                    paths: install_paths,
                },
                password.clone(),
            )
            .await
            .map_err(|e| format!("Failed to invoke helper: {}", e))?;

            // Stream progress events
            while let Some(msg) = rx.recv().await {
                let _ = app.emit("install-output", &msg.message);
            }
        }
        _ => {
            // ✅ REPO: Full ALPM transaction with all features
            let _ = app.emit(
                "install-output",
                "--- Starting ALPM Transaction (with system sync) ---",
            );

            // Use ALPM transaction instead of shell command
            let mut rx = helper_client::invoke_helper(
                app,
                helper_client::HelperCommand::AlpmInstall {
                    packages: vec![name.to_string()],
                    sync_first: true, // -Syu behavior (sync + install in one transaction)
                    enabled_repos,
                    cpu_optimization,
                },
                password.clone(),
            )
            .await
            .map_err(|e| format!("Failed to invoke helper: {}", e))?;

            // Stream progress events
            while let Some(msg) = rx.recv().await {
                let _ = app.emit("install-output", &msg.message);
            }
        }
    }

    // ✅ POST-INSTALL VERIFICATION (using shell command for accuracy - no ALPM cache staleness)
    let verification = tokio::task::spawn_blocking({
        let pkg_name = name.to_string();
        move || {
            std::process::Command::new("pacman")
                .args(["-Q", &pkg_name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
    })
    .await
    .map_err(|e| format!("Verification task failed: {}", e))?;
    
    if !verification {
        let _ = app.emit("install-complete", "failed");
        return Err(format!(
            "Installation reported success but package '{}' is not installed. Check logs for details.",
            name
        ));
    }

    let _ = app.emit("install-complete", "success");

    // Process notification & telemetry
    // Only send system notification if enabled
    if repo_manager.is_notifications_enabled().await {
        let _ = app
            .notification()
            .builder()
            .title("✨ MonArch: Installation Complete")
            .body(format!("Successfully installed '{}'", name))
            .show();
    }

    crate::utils::track_event_safe(
        app,
        "install_package",
        Some(serde_json::json!({
            "pkg": name, "source": format!("{:?}", source)
        })),
    )
    .await;

    Ok(())
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
    ];

    if protected.contains(&name.as_str()) {
        let _ = app.emit("install-complete", "failed");
        return Err(format!(
            "CRITICAL ERROR: '{}' is a protected system package. Uninstallation is forbidden.",
            name
        ));
    }
    
    // Acquire global lock
    let _guard = crate::utils::PRIVILEGED_LOCK.lock().await;

    let _ = app.emit(
        "install-output",
        format!("Preparing to uninstall '{}'...", name),
    );

    // ✅ NEW: Use ALPM transaction instead of shell command
    let mut rx = helper_client::invoke_helper(
        &app,
        helper_client::HelperCommand::AlpmUninstall {
            packages: vec![name.clone()],
            remove_deps: true, // -Rns behavior
        },
        password.clone(),
    )
    .await
    .map_err(|e| format!("Failed to invoke helper: {}", e))?;

    // Stream progress events
    while let Some(msg) = rx.recv().await {
        let _ = app.emit("install-output", &msg.message);
    }

    // ✅ POST-UNINSTALL VERIFICATION (using shell command for accuracy)
    let verification = tokio::task::spawn_blocking({
        let pkg_name = name.clone();
        move || {
            std::process::Command::new("pacman")
                .args(["-Q", &pkg_name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
    })
    .await
    .map_err(|e| format!("Verification task failed: {}", e))?;
    
    if verification {
        let _ = app.emit("install-complete", "failed");
        return Err(format!(
            "Uninstallation reported success but package '{}' is still installed. Check for dependency conflicts.",
            name
        ));
    }

    let _ = app.emit("install-complete", "success");
    Ok(())
}

pub async fn build_aur_package(
    app: &AppHandle,
    name: &str,
    password: &Option<String>,
) -> Result<Vec<String>, String> {
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

    let mut built_paths = Vec::new();
    for pkg_name in resolved {
        let path = build_aur_package_single(app, &pkg_name, password).await?;
        built_paths.push(path);
    }

    Ok(built_paths)
}

async fn build_aur_package_single(
    app: &AppHandle,
    name: &str,
    password: &Option<String>,
) -> Result<String, String> {
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

    // 3. Create transient Sudo Askpass script if password is provided
    let mut askpass_path = None;
    if let Some(pwd) = password {
        let script_path = pkg_path.join("askpass.sh");
        let script_content = format!("#!/bin/sh\necho '{}'", pwd);
        std::fs::write(&script_path, script_content).map_err(|e| e.to_string())?;

        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path)
                .map_err(|e| e.to_string())?
                .permissions();
            perms.set_mode(0o700);
            std::fs::set_permissions(&script_path, perms).map_err(|e| e.to_string())?;
        }
        askpass_path = Some(script_path);
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

    let _ = app.emit(
        "install-output",
        format!("Building {} from AUR (makepkg)...", name),
    );

    let mut makepkg = tokio::process::Command::new("makepkg");
    makepkg
        .args(["-s", "--noconfirm", "--needed"]) // REMOVED -i (install), we build only
        .env("MAKEFLAGS", format!("-j{}", num_cpus::get()))
        .env("PKGEXT", ".pkg.tar.zst")
        .current_dir(&pkg_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Inject Askpass redirection or fallback to pkexec
    if let Some(ref ap) = askpass_path {
        makepkg.env("SUDO_ASKPASS", ap);
        makepkg.env("PACMAN", "sudo -A pacman");
    } else {
        // Fallback: If no password provided, use helper as Proxy if available
        let helper = crate::utils::MONARCH_PK_HELPER;
        if std::path::Path::new(helper).exists() {
            let wrapper_path = pkg_dir.join("pacman-helper.sh");
            let wrapper_content = format!(
                "#!/bin/sh\n/usr/bin/pkexec {} \"{{\\\"command\\\": \\\"RunCommand\\\", \\\"payload\\\": {{\\\"binary\\\": \\\"pacman\\\", \\\"args\\\": [\\\"$@\\\"]}}}}\"",
                helper
            );
            std::fs::write(&wrapper_path, wrapper_content).map_err(|e| e.to_string())?;

            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&wrapper_path)
                    .map_err(|e| e.to_string())?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&wrapper_path, perms).map_err(|e| e.to_string())?;
            }
            makepkg.env("PACMAN", wrapper_path.to_string_lossy().to_string());
        } else {
            makepkg.env("PACMAN", "pkexec pacman");
        }
    }

    let mut child = makepkg.spawn().map_err(|e| e.to_string())?;

    if let Some(pwd) = password {
        if let Some(mut stdin) = child.stdin.take() {
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut stdin, format!("{}\n", pwd).as_bytes())
                    .await;
        }
    }

    let missing_keys = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
    let build_errors = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));

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

    let missing_keys_clone = missing_keys.clone();
    let build_errors_clone = build_errors.clone();
    if let Some(err) = child.stderr.take() {
        let a = app.clone();
        let mut reader = TokioBufReader::new(err).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = a.emit("install-output", format!("MAKEPKG: {}", line));

            // Detect GPG key errors and extract key IDs
            if line.contains("unknown public key")
                || line.contains("not found in keychain")
                || line.contains("FAILED (unknown public key")
                || line.contains("could not be verified")
            {
                // Extract key ID using regex-like pattern matching
                // Common formats: "key ABCD1234", "FAILED (unknown public key ABCD1234)"
                let words: Vec<&str> = line.split_whitespace().collect();
                for (i, word) in words.iter().enumerate() {
                    // Look for hex-like key IDs (8+ alphanumeric characters)
                    let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
                    if clean.len() >= 8 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
                        let mut keys = missing_keys_clone.lock().await;
                        if !keys.contains(&clean.to_string()) {
                            keys.push(clean.to_string());
                        }
                    }
                    // Also check if previous word was "key"
                    if *word == "key" || word.ends_with("key") {
                        if let Some(next) = words.get(i + 1) {
                            let clean = next.trim_matches(|c: char| !c.is_alphanumeric());
                            if clean.len() >= 8 {
                                let mut keys = missing_keys_clone.lock().await;
                                if !keys.contains(&clean.to_string()) {
                                    keys.push(clean.to_string());
                                }
                            }
                        }
                    }
                }
            }

            // Collect actual errors
            if line.contains("ERROR:") {
                let mut errs = build_errors_clone.lock().await;
                errs.push(line.clone());
            }
        }
    }

    let exit_status = child.wait().await.map_err(|e| e.to_string())?;

    // Check if build failed due to PGP keys
    if !exit_status.success() {
        let keys = missing_keys.lock().await;

        if !keys.is_empty() {
            // Attempt automatic key import
            let _ = app.emit("install-output", "");
            let _ = app.emit("install-output", "--- PGP KEY RECOVERY ---");
            let _ = app.emit(
                "install-output",
                format!(
                    "Detected {} missing PGP key(s). Attempting automatic import...",
                    keys.len()
                ),
            );

            let mut imported_any = false;
            for key_id in keys.iter() {
                let _ = app.emit("install-output", format!("Importing key: {}...", key_id));

                // Try multiple keyservers in order of reliability
                let keyservers = ["keyserver.ubuntu.com", "keys.openpgp.org", "pgp.mit.edu"];

                let mut key_imported = false;
                for server in keyservers {
                    let import_result = tokio::process::Command::new("gpg")
                        .args(["--keyserver", server, "--recv-keys", key_id])
                        .output()
                        .await;

                    if let Ok(output) = import_result {
                        if output.status.success() {
                            let _ = app.emit(
                                "install-output",
                                format!("✓ Key {} imported from {}", key_id, server),
                            );
                            key_imported = true;
                            imported_any = true;
                            break;
                        }
                    }
                }

                if !key_imported {
                    let _ = app.emit(
                        "install-output",
                        format!("⚠ Could not import key {} from any keyserver", key_id),
                    );
                }
            }

            if imported_any {
                // Retry the build after importing keys
                let _ = app.emit("install-output", "");
                let _ = app.emit(
                    "install-output",
                    "--- RETRYING BUILD WITH IMPORTED KEYS ---",
                );

                // Clean previous build artifacts
                let _ = tokio::process::Command::new("rm")
                    .args(["-rf", "src", "pkg"])
                    .current_dir(&pkg_dir)
                    .status()
                    .await;

                // Retry makepkg
                let mut retry_makepkg = tokio::process::Command::new("makepkg");
                retry_makepkg
                    .args(["-s", "--noconfirm", "--needed"])
                    .env("MAKEFLAGS", format!("-j{}", num_cpus::get()))
                    .env("PKGEXT", ".pkg.tar.zst")
                    .current_dir(&pkg_dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                if let Some(ref ap) = askpass_path {
                    retry_makepkg.env("SUDO_ASKPASS", ap);
                    retry_makepkg.env("PACMAN", "sudo -A pacman");
                } else {
                    retry_makepkg.env("PACMAN", "pkexec pacman");
                }

                let mut retry_child = retry_makepkg.spawn().map_err(|e| e.to_string())?;

                // Stream retry output
                if let Some(out) = retry_child.stdout.take() {
                    let a = app.clone();
                    tokio::spawn(async move {
                        let reader = TokioBufReader::new(out);
                        let mut lines = reader.lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = a.emit("install-output", line);
                        }
                    });
                }

                if let Some(err) = retry_child.stderr.take() {
                    let a = app.clone();
                    tokio::spawn(async move {
                        let reader = TokioBufReader::new(err);
                        let mut lines = reader.lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = a.emit("install-output", format!("MAKEPKG: {}", line));
                        }
                    });
                }

                let retry_status = retry_child.wait().await.map_err(|e| e.to_string())?;

                if !retry_status.success() {
                    let errs = build_errors.lock().await;
                    let err_summary = if errs.is_empty() {
                        "Build failed after key import. Check logs for details.".to_string()
                    } else {
                        errs.last()
                            .cloned()
                            .unwrap_or_else(|| "Unknown build error".to_string())
                    };
                    return Err(err_summary);
                }

                let _ = app.emit("install-output", "✓ Build succeeded after key import!");
            } else {
                return Err(format!(
                    "PGP verification failed. Could not import required keys: {}. You may need to import them manually.",
                    keys.join(", ")
                ));
            }
        } else {
            // Non-PGP build failure
            let errs = build_errors.lock().await;
            let err_summary = if errs.is_empty() {
                "makepkg build failed. Check logs for details.".to_string()
            } else {
                errs.last()
                    .cloned()
                    .unwrap_or_else(|| "Unknown build error".to_string())
            };
            return Err(err_summary);
        }
    }

    // Find the resulting package file
    let mut dir = tokio::fs::read_dir(&pkg_dir)
        .await
        .map_err(|e| e.to_string())?;
    while let Some(entry) = dir.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "zst" && path.to_string_lossy().contains(".pkg.tar.") {
                return Ok(path.to_string_lossy().to_string());
            }
        }
    }

    Err(format!("Could not find built package in {:?}", pkg_dir))
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
    let native_pkgs = crate::alpm_read::get_installed_packages_native();
    let mut apps = Vec::new();

    if let Ok(loader) = state.inner().0.lock() {
        for pkg in native_pkgs {
            // Check if it's an app
            let has_icon = loader.find_icon_heuristic(&pkg.name).is_some();
            let has_id = loader.find_app_id(&pkg.name).is_some();

            if has_icon || has_id {
                apps.push(InstalledPackage {
                    name: pkg.name,
                    version: pkg.version,
                    description: pkg.description,
                    install_date: None,
                    size: pkg
                        .installed_size
                        .map(|s| format!("{} MB", s / (1024 * 1024))),
                    url: None,
                    repository: None,
                });
            }
        }
    }

    Ok(apps)
}

#[tauri::command]
pub async fn check_for_updates(
    _app: AppHandle,
    _state: tauri::State<'_, crate::metadata::MetadataState>,
) -> Result<Vec<PendingUpdate>, String> {
    // 1. Get Official updates from 'checkupdates' utility (unprivileged)
    let mut all_updates = tokio::task::spawn_blocking(|| {
        let output = std::process::Command::new("checkupdates")
            .output()
            .map_err(|e| e.to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut updates = Vec::new();

        // Parse line by line: "package-name 0.1-1 -> 0.2-1"
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 && parts[2] == "->" {
                updates.push(PendingUpdate {
                    name: parts[0].to_string(),
                    old_version: parts[1].to_string(),
                    new_version: parts[3].to_string(),
                    repo: "official".to_string(),
                });
            }
        }
        Ok::<Vec<PendingUpdate>, String>(updates)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;

    // 2. Get AUR updates locally (unprivileged)
    if let Ok(aur_updates) = check_aur_updates().await {
        all_updates.extend(aur_updates);
    }

    Ok(all_updates)
}

async fn check_aur_updates() -> Result<Vec<PendingUpdate>, String> {
    // Use spawn_blocking to avoid blocking the Tokio runtime with std::process::Command
    let (installed_aur, names) = tokio::task::spawn_blocking(|| {
        let output = std::process::Command::new("pacman")
            .args(["-Qm"])
            .output()
            .map_err(|e| e.to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut installed_aur = std::collections::HashMap::new();
        let mut names = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 {
                let name = parts[0];

                // Distro-Aware Priority: Check if the package now exists in a Repo
                // This prevents building linux-cachyos from AUR if the CachyOS repo is enabled.
                let in_repo = std::process::Command::new("pacman")
                    .args(["-Sp", name])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);

                if !in_repo {
                    installed_aur.insert(name.to_string(), parts[1].to_string());
                    names.push(name.to_string());
                }
            }
        }

        Ok::<_, String>((installed_aur, names))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;

    if names.is_empty() {
        return Ok(vec![]);
    }

    // Query AUR RPC for info
    let names_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let aur_info = aur_api::get_multi_info(&names_refs[..]).await?;

    let mut pending = Vec::new();
    for pkg in aur_info {
        if let Some(installed_ver) = installed_aur.get(&pkg.name) {
            // Basic version mismatch check
            if pkg.version != *installed_ver {
                pending.push(PendingUpdate {
                    name: pkg.name,
                    old_version: installed_ver.clone(),
                    new_version: pkg.version,
                    repo: "aur".to_string(),
                });
            }
        }
    }

    Ok(pending)
}

#[tauri::command]
pub async fn get_orphans() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(|| {
        let output = std::process::Command::new("pacman")
            .args(["-Qtdq"])
            .output()
            .map_err(|e| e.to_string())?;
        let content = String::from_utf8_lossy(&output.stdout);
        Ok(content.lines().map(|s| s.to_string()).collect())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn remove_orphans(app: AppHandle, orphans: Vec<String>) -> Result<(), String> {
    if orphans.is_empty() {
        return Ok(());
    }
    // Validate all package names to prevent injection
    for name in &orphans {
        crate::utils::validate_package_name(name)?;
    }
    let mut args = vec!["-Rns".to_string(), "--noconfirm".to_string()];
    args.extend(orphans);
    crate::utils::run_pacman_command_transparent(app.clone(), args, None).await?;
    Ok(())
}

#[tauri::command]
pub async fn check_installed_status(
    state: State<'_, crate::metadata::MetadataState>,
    name: String,
) -> Result<PackageInstallStatus, String> {
    // 1. Resolve App ID to package name if needed
    let resolved_name = state
        .inner()
        .0
        .lock()
        .ok()
        .map(|loader| loader.resolve_package_name(&name))
        .unwrap_or_else(|| name.clone());

    if let Some(pkg) = crate::alpm_read::get_package_native(&resolved_name) {
        return Ok(PackageInstallStatus {
            installed: pkg.installed,
            version: Some(pkg.version),
            repo: None, // ALPM doesn't always expose repo name directly in syncdb loops without effort
            source: Some(pkg.source),
            actual_package_name: Some(resolved_name),
        });
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

#[tauri::command]
pub async fn check_reboot_required() -> Result<bool, String> {
    let running_kernel = std::process::Command::new("uname")
        .arg("-r")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .map_err(|e| e.to_string())?;

    if running_kernel.is_empty() {
        return Ok(false);
    }

    let modules_dir = format!("/usr/lib/modules/{}", running_kernel);
    if !std::path::Path::new(&modules_dir).exists() {
        // Kernel updated and old modules removed
        return Ok(true);
    }

    Ok(false)
}

#[tauri::command]
pub async fn get_pacnew_warnings() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("find")
        .args(["/etc", "-name", "*.pacnew"])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

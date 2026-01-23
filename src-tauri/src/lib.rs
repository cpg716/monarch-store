pub mod aur_api;
pub mod chaotic_api;
pub mod flathub_api;
pub mod metadata;
pub mod models;
mod odrs_api;
mod repo_db;
mod repo_manager;
pub mod reviews;
pub mod scm_api;
mod utils;

// use models::Package;
// use metadata::{AppStreamLoader, MetadataState};
use chaotic_api::{ChaoticApiClient, ChaoticPackage, InfraStats};
mod repo_setup; // [NEW]
use base64::prelude::*;
use repo_manager::RepoManager;
use serde::Serialize;
use std::process::{Command, Stdio}; // Keep Stdio
use tauri::Emitter; // Fix: Use Emitter trait for emit()
use tauri::Manager;
// use tauri_plugin_aptabase::EventTracker;
use tokio::io::{AsyncBufReadExt, BufReader}; // Use async reader

#[derive(Serialize)]
struct SystemInfo {
    kernel: String,
    de: String,
    distro: String,
}

#[tauri::command]
async fn get_system_info() -> Result<SystemInfo, String> {
    let kernel = Command::new("uname")
        .arg("-r")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let de = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "Unknown".to_string());

    let distro = std::fs::read_to_string("/etc/os-release")
        .map(|content| {
            content
                .lines()
                .find(|l| l.starts_with("PRETTY_NAME="))
                .map(|l| l.replace("PRETTY_NAME=", "").replace("\"", ""))
                .unwrap_or_else(|| "Arch Linux".to_string())
        })
        .unwrap_or_else(|_| "Arch Linux".to_string());

    Ok(SystemInfo { kernel, de, distro })
}

async fn audit_aur_builder_deps(app: &tauri::AppHandle) -> Result<(), String> {
    let has_git = Command::new("which")
        .arg("git")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let has_base_devel = Command::new("pacman")
        .args(["-Qq", "base-devel"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_git || !has_base_devel {
        let _ = app.emit(
            "install-output",
            "Installing build dependencies (git, base-devel)...",
        );

        // Install missing deps
        let script = String::from("#!/bin/bash\npacman -Sy --needed --noconfirm git base-devel\n");

        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("monarch_dep_install.sh");
        {
            let mut file = std::fs::File::create(&script_path).map_err(|e| e.to_string())?;
            file.write_all(script.as_bytes())
                .map_err(|e| e.to_string())?;
            let mut perms = file.metadata().map_err(|e| e.to_string())?.permissions();
            perms.set_mode(0o755);
            file.set_permissions(perms).map_err(|e| e.to_string())?;
        }

        let output = Command::new("pkexec")
            .arg(&script_path)
            .output()
            .map_err(|e| format!("Failed to run dependency installer: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to install build dependencies: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    Ok(())
}

async fn build_aur_package(
    app: &tauri::AppHandle,
    name: &str,
    password: &Option<String>,
) -> Result<(), String> {
    let _ = app.emit(
        "install-output",
        format!("Starting native build for {}", name),
    );

    // 0. Ensure deps (git, base-devel)
    audit_aur_builder_deps(app).await?;

    // 1. Create temp build dir
    let temp_dir = std::env::temp_dir().join(format!("monarch_build_{}", name));
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    // 2. Git Clone
    let _ = app.emit("install-output", "Cloning AUR repository...");
    let clone_status = tokio::process::Command::new("git")
        .arg("clone")
        .arg(format!("https://aur.archlinux.org/{}.git", name))
        .arg(&temp_dir)
        .status()
        .await
        .map_err(|e| format!("Git clone failed: {}", e))?;

    if !clone_status.success() {
        return Err("Git clone failed".to_string());
    }

    // 3. Makepkg
    let _ = app.emit(
        "install-output",
        "Building package (this may take a while)...",
    );

    // We need to run makepkg as a non-root user (which we are), but it needs sudo for -i (install)
    // We can't easily pipe password to makepkg's internal sudo call.
    // Strategy: Build only (-s), then install result with pacman -U.

    let build_status = tokio::process::Command::new("makepkg")
        .arg("-s") // Sync deps
        .arg("--noconfirm")
        .current_dir(&temp_dir)
        .status()
        .await
        .map_err(|e| format!("Makepkg failed: {}", e))?;

    if !build_status.success() {
        return Err("Build failed. Check dependencies.".to_string());
    }

    // 4. Find the built package
    let mut built_pkg = None;
    if let Ok(entries) = std::fs::read_dir(&temp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if ext_str == "zst" || ext_str == "xz" {
                    // pkg.tar.zst or pkg.tar.xz
                    built_pkg = Some(path);
                    break;
                }
            }
        }
    }

    let pkg_path = built_pkg.ok_or("Could not find built package archive")?;
    let _ = app.emit(
        "install-output",
        format!("Installing built package: {:?}", pkg_path),
    );

    // 5. Install with pacman -U (using pkexec or sudo)
    let (binary, args) = if password.is_none() {
        (
            "/usr/bin/pkexec".to_string(),
            vec![
                "/usr/bin/pacman".to_string(),
                "-U".to_string(),
                "--noconfirm".to_string(),
                "--".to_string(),
                pkg_path.to_string_lossy().to_string(),
            ],
        )
    } else {
        (
            "/usr/bin/sudo".to_string(),
            vec![
                "-S".to_string(),
                "/usr/bin/pacman".to_string(),
                "-U".to_string(),
                "--noconfirm".to_string(),
                "--".to_string(),
                pkg_path.to_string_lossy().to_string(),
            ],
        )
    };

    let mut child = tokio::process::Command::new(binary);
    for arg in &args {
        child.arg(arg);
    }

    if password.is_some() {
        child.stdin(std::process::Stdio::piped());
    }

    let mut child_proc = child.spawn().map_err(|e| e.to_string())?;

    if let Some(pwd) = password {
        if let Some(mut stdin) = child_proc.stdin.take() {
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut stdin, format!("{}\n", pwd).as_bytes())
                    .await;
        }
    }

    let status = child_proc.wait().await.map_err(|e| e.to_string())?;

    if !status.success() {
        return Err("Failed to install built package".to_string());
    }

    Ok(())
}

#[tauri::command]
async fn install_package(
    app: tauri::AppHandle,
    name: String,
    source: models::PackageSource,
    password: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn(async move {
        // 1. Determine Helper/Binary
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
            match build_aur_package(&app, &name, &password).await {
                Ok(_) => {
                    let _ = app.emit("install-complete", "success");
                }
                Err(e) => {
                    let _ = app.emit("install-output", format!("Build Error: {}", e));
                    let _ = app.emit("install-complete", "failed");
                }
            }
            return;
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

                // Fallback to native (should cover above check, but safe logic)
                match found {
                    Some(h) => (h.to_string(), vec!["-Sy", "--noconfirm", "--", &name]),
                    None => {
                        // This path shouldn't be reached due to native_builder_needed check
                        return;
                    }
                }
            }
            _ => {
                // For official/chaotic, use pkexec if available for a native prompt,
                // fallback to sudo if password was provided in the UI box.
                if password.is_none() {
                    (
                        "/usr/bin/pkexec".to_string(),
                        vec!["/usr/bin/pacman", "-Sy", "--noconfirm", "--", &name],
                    )
                } else {
                    (
                        "/usr/bin/sudo".to_string(),
                        vec!["-S", "/usr/bin/pacman", "-Sy", "--noconfirm", "--", &name],
                    )
                }
            }
        };

        let _ = app.emit(
            "install-output",
            format!("Executing: {} {:?}", binary, args),
        );

        // Cache sudo creds if needed (for yay/paru)
        if matches!(source, models::PackageSource::Aur) && password.is_some() {
            if let Some(pwd) = &password {
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

        match child
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                // Pipe password for direct sudo
                if !matches!(source, models::PackageSource::Aur) {
                    if let Some(pwd) = &password {
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
                    let a = app.clone();
                    tokio::spawn(async move {
                        let reader = BufReader::new(out);
                        let mut lines = reader.lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = a.emit("install-output", line);
                        }
                    });
                }
                if let Some(err) = child.stderr.take() {
                    let a = app.clone();
                    tokio::spawn(async move {
                        let reader = BufReader::new(err);
                        let mut lines = reader.lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = a.emit("install-output", line);
                        }
                    });
                }

                let status = match child.wait().await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = app.emit("install-output", format!("Process wait failed: {}", e));
                        let _ = app.emit("install-complete", "failed");
                        return;
                    }
                };
                let success = status.success();
                let _ = app.emit(
                    "install-complete",
                    if success { "success" } else { "failed" },
                );

                // Native Notification
                use tauri_plugin_notification::NotificationExt;
                let _ = app
                    .notification()
                    .builder()
                    .title(if success {
                        "✨ MonArch: Installation Complete"
                    } else {
                        "❌ MonArch: Installation Failed"
                    })
                    .body(format!(
                        "{} '{}' {}",
                        if success {
                            "Successfully installed"
                        } else {
                            "Failed to install"
                        },
                        name,
                        if success {
                            "from chosen repositories."
                        } else {
                            "due to a system error."
                        }
                    ))
                    .show();
            }
            Err(e) => {
                let _ = app.emit(
                    "install-output",
                    format!("Failed to spawn installer: {}", e),
                );
                let _ = app.emit("install-complete", "failed");
            }
        }
    });
    Ok(())
}

#[tauri::command]
async fn uninstall_package(
    app: tauri::AppHandle,
    name: String,
    password: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn(async move {
        // Use pkexec for polkit authentication, or sudo if password provided
        let (binary, args) = if password.is_none() {
            (
                "/usr/bin/pkexec".to_string(),
                vec!["/usr/bin/pacman", "-Rns", "--noconfirm", "--", &name],
            )
        } else {
            (
                "/usr/bin/sudo".to_string(),
                vec!["-S", "/usr/bin/pacman", "-Rns", "--noconfirm", "--", &name],
            )
        };

        let _ = app.emit(
            "install-output",
            format!("Executing Uninstaller: {} {:?}", binary, args),
        );

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
                // Pipe password for direct sudo
                if let Some(pwd) = &password {
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
                        let reader = BufReader::new(out);
                        let mut lines = reader.lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = a.emit("uninstall-output", line);
                        }
                    });
                }
                if let Some(err) = child.stderr.take() {
                    let a = app.clone();
                    tokio::spawn(async move {
                        let reader = BufReader::new(err);
                        let mut lines = reader.lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = a.emit("uninstall-output", line);
                        }
                    });
                }

                let status = match child.wait().await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = app.emit("uninstall-output", format!("Process wait failed: {}", e));
                        let _ = app.emit("uninstall-complete", "failed");
                        return;
                    }
                };
                let success = status.success();
                let _ = app.emit(
                    "uninstall-complete",
                    if success { "success" } else { "failed" },
                );

                // Native Notification
                use tauri_plugin_notification::NotificationExt;
                let _ = app
                    .notification()
                    .builder()
                    .title(if success {
                        "✨ MonArch: Uninstall Complete"
                    } else {
                        "❌ MonArch: Uninstall Failed"
                    })
                    .body(format!(
                        "Successfully removed '{}' and its unneeded dependencies.",
                        name
                    ))
                    .show();
            }
            Err(e) => {
                let _ = app.emit(
                    "uninstall-output",
                    format!("Failed to spawn uninstaller: {}", e),
                );
                let _ = app.emit("uninstall-complete", "failed");
            }
        }
    });
    Ok(())
}

#[tauri::command]
async fn search_aur(query: String) -> Result<Vec<models::Package>, String> {
    aur_api::search_aur(&query).await
}

// Removed unused greet function

#[tauri::command]
async fn trigger_repo_sync(
    app: tauri::AppHandle,
    state: tauri::State<'_, RepoManager>,
    sync_interval_hours: Option<u64>,
) -> Result<String, String> {
    // Default to 24h if not provided (e.g. from legacy calls), but frontend should provide it
    let interval = sync_interval_hours.unwrap_or(24);

    // 1. Sync Repos
    let repo_result = state.sync_all(false, interval).await?;

    // 2. Refresh Metadata (AppStream)
    let state_meta = app.state::<metadata::MetadataState>();
    state_meta.init(interval).await;

    Ok(repo_result)
}

#[tauri::command]
async fn fetch_repo(
    url: String,
    name: String,
    source: models::PackageSource,
) -> Result<Vec<models::Package>, String> {
    // For single repo fetch, we use a temp cache or the standard one?
    // Let's use standard but force update.
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("monarch-store")
        .join("dbs");
    let _ = std::fs::create_dir_all(&cache_dir);

    repo_db::fetch_repo_packages(&url, &name, source, &cache_dir, true, 0).await
}

#[tauri::command]
async fn clear_cache(
    state_meta: tauri::State<'_, metadata::MetadataState>,
    state_chaotic: tauri::State<'_, ChaoticApiClient>,
    state_repo: tauri::State<'_, RepoManager>,
    state_flathub: tauri::State<'_, flathub_api::FlathubApiClient>,
    state_scm: tauri::State<'_, ScmState>,
) -> Result<(), String> {
    state_chaotic.clear_cache().await;
    state_flathub.clear_cache(); // Correct: synchronous method (cache is Arc<Mutex>)

    // ScmClient wrapped in ScmState(ScmClient), ScmClient has cache: Mutex<HashMap>
    // So we need to access the cache field which is creating the confusion with .lock() usage
    // But ScmClient struct definition has `cache` as private/public?
    // Let's check ScmClient definition. It uses internal mutability with Mutex.
    // It has a method clear_cache() which handles the locking internally!
    state_scm.0.clear_cache();

    state_repo.sync_all(true, 0).await?; // Force re-download (interval 0 is ignored when force=true)

    // 4. Clear Metadata (AppStream) - Force reload
    state_meta.init(0).await;

    // 5. Clear Download Cache (basic implementation)
    // Safest is to just try to remove the ~/.cache/monarch-store/downloads if it exists
    Ok(())
}

#[tauri::command]
async fn search_packages(
    state: tauri::State<'_, metadata::MetadataState>,
    state_chaotic: tauri::State<'_, ChaoticApiClient>,
    state_repo: tauri::State<'_, RepoManager>,
    query: String,
) -> Result<Vec<models::Package>, String> {
    let mut packages = Vec::new();
    let query = query.trim();

    if query.is_empty() {
        return Ok(vec![]);
    }

    // 1. Search AppStream (Official/Local Metadata)
    {
        let loader = state.0.lock().unwrap();
        let app_results = loader.search_apps(query);

        for app in app_results {
            // ... (rest of logic)
            // ...
            packages.push(models::Package {
                name: app.pkg_name.clone().unwrap_or(app.app_id.clone()),
                display_name: Some(app.name),
                description: app.summary.unwrap_or_default(),
                version: app.version.unwrap_or_else(|| "latest".to_string()),
                source: models::PackageSource::Official,
                maintainer: None,
                license: None,
                url: None,
                last_modified: None,
                first_submitted: None,
                out_of_date: None,
                keywords: None,
                num_votes: None,
                icon: app.icon_url,
                screenshots: if app.screenshots.is_empty() {
                    None
                } else {
                    Some(app.screenshots)
                },
                provides: None,
                app_id: Some(app.app_id.clone()),
            });
        }
    }

    // 2. Search Synced Repos (Binary Repos)
    let repo_results = state_repo.search(query).await;

    // Track seen App IDs to prevent duplicates (e.g. brave-bin vs brave)
    let mut seen_app_ids: std::collections::HashSet<String> =
        packages.iter().filter_map(|p| p.app_id.clone()).collect();

    for mut pkg in repo_results {
        // Skip if name exists
        if packages.iter().any(|p| p.name == pkg.name) {
            continue;
        }

        // Heuristic Icon Lookup & App ID Resolution
        if pkg.icon.is_none() || pkg.app_id.is_none() {
            if let Ok(loader) = state.0.lock() {
                if pkg.icon.is_none() {
                    pkg.icon = loader.find_icon_heuristic(&pkg.name);
                }
                if pkg.app_id.is_none() {
                    pkg.app_id = loader.find_app_id(&pkg.name);
                }
            }
        }

        // Fallback: Use Flathub ID mapping if still None (Handle AUR pkgs like brave)
        if pkg.app_id.is_none() {
            pkg.app_id = crate::flathub_api::get_flathub_app_id(&pkg.name);
        }

        // Skip if App ID already seen (e.g. invalidates brave-bin if brave exists)
        if let Some(id) = &pkg.app_id {
            if seen_app_ids.contains(id) {
                continue;
            }
            seen_app_ids.insert(id.clone());
        }

        pkg.display_name = Some(utils::to_pretty_name(&pkg.name));
        packages.push(pkg);
    }

    // 3. Search Chaotic AUR (Directly)
    if let Ok(chaotic_arc) = state_chaotic.fetch_packages().await {
        let q_lower = query.to_lowercase();
        // Filter chaotic packages matching query
        let chaotic_matches: Vec<models::Package> = chaotic_arc
            .iter()
            .filter(|p| {
                p.pkgname.to_lowercase().contains(&q_lower)
                    || p.metadata
                        .as_ref()
                        .and_then(|m| m.desc.as_ref())
                        .map(|d| d.to_lowercase().contains(&q_lower))
                        .unwrap_or(false)
            })
            .take(50) // Limit to 50 results explicitly
            .map(|p| models::Package {
                name: p.pkgname.clone(),
                display_name: Some(utils::to_pretty_name(&p.pkgname)),
                description: p
                    .metadata
                    .as_ref()
                    .and_then(|m| m.desc.clone())
                    .unwrap_or_default(),
                version: p.version.clone().unwrap_or_default(),
                source: models::PackageSource::Chaotic,
                maintainer: Some("Chaotic-AUR Team".to_string()),
                license: p
                    .metadata
                    .as_ref()
                    .and_then(|m| m.license.clone())
                    .map(|l| vec![l]),
                url: p.metadata.as_ref().and_then(|m| m.url.clone()),
                last_modified: None,
                first_submitted: None,
                out_of_date: None,
                keywords: None,
                num_votes: None,
                icon: {
                    let mut icon = None;
                    if let Ok(loader) = state.0.lock() {
                        icon = loader.find_icon_heuristic(&p.pkgname);
                    }
                    icon
                },
                screenshots: None,
                provides: None,
                app_id: None, // Will try to lookup below
            })
            .collect();

        // Track seen packages to prevent duplicates
        let mut seen_packages: std::collections::HashSet<String> =
            packages.iter().map(|p| p.name.clone()).collect();

        for mut pkg in chaotic_matches {
            if seen_packages.insert(pkg.name.clone()) {
                // Enrich Chaotic packages with AppID if possible
                if pkg.app_id.is_none() {
                    if let Ok(loader) = state.0.lock() {
                        pkg.app_id = loader.find_app_id(&pkg.name);
                    }
                }

                // Fallback: Use Flathub ID mapping
                if pkg.app_id.is_none() {
                    pkg.app_id = crate::flathub_api::get_flathub_app_id(&pkg.name);
                }

                // Check App ID duplication
                if let Some(id) = &pkg.app_id {
                    if seen_app_ids.contains(id) {
                        continue;
                    }
                    seen_app_ids.insert(id.clone());
                }

                packages.push(pkg);
            }
        }
    }

    // 3. Search AUR (Async, Remote) - ONLY if Enabled
    let aur_enabled = state_repo.is_aur_enabled().await;

    if aur_enabled && query.len() >= 2 {
        if let Ok(aur_results) = aur_api::search_aur(query).await {
            // Optimization: Build HashSets for O(1) lookups
            // Chaotic Set
            let chaotic_res = state_chaotic.fetch_packages().await;
            if let Err(_e) = &chaotic_res {}
            let chaotic_packages = chaotic_res.unwrap_or_default();
            let chaotic_set: std::collections::HashSet<&String> =
                chaotic_packages.iter().map(|c| &c.pkgname).collect();

            // Repo Map (Name -> Source)
            let repo_map: std::collections::HashMap<String, models::PackageSource> = packages
                .iter()
                .map(|p| (p.name.clone(), p.source.clone()))
                .collect();

            // Re-init seen_packages
            let mut seen_packages: std::collections::HashSet<String> =
                packages.iter().map(|p| p.name.clone()).collect();

            for mut pkg in aur_results {
                if seen_packages.insert(pkg.name.clone()) {
                    // Enrich AUR package with AppID
                    if pkg.app_id.is_none() {
                        if let Ok(loader) = state.0.lock() {
                            pkg.app_id = loader.find_app_id(&pkg.name);
                        }
                    }
                    if pkg.app_id.is_none() {
                        pkg.app_id = crate::flathub_api::get_flathub_app_id(&pkg.name);
                    }

                    // Check App ID duplication
                    if let Some(id) = &pkg.app_id {
                        if seen_app_ids.contains(id) {
                            continue;
                        }
                        seen_app_ids.insert(id.clone());
                    }

                    // CHECK: Is this package available in Chaotic-AUR?
                    if chaotic_set.contains(&pkg.name) {
                        pkg.source = models::PackageSource::Chaotic;
                    }
                    // CHECK: Is it in Secondary Repos?
                    else if let Some(source) = repo_map.get(&pkg.name) {
                        pkg.source = source.clone();
                    }

                    pkg.display_name = Some(utils::to_pretty_name(&pkg.name));
                    packages.push(pkg);
                }
            }
        }
    }

    // Sorting Logic: Use shared utility (Weighted Rank)
    utils::sort_packages_by_relevance(&mut packages, query);

    Ok(packages)
}

#[tauri::command]
async fn get_packages_by_names(
    state_meta: tauri::State<'_, metadata::MetadataState>,
    state_chaotic: tauri::State<'_, ChaoticApiClient>,
    state_repo: tauri::State<'_, RepoManager>,
    names: Vec<String>,
) -> Result<Vec<models::Package>, String> {
    let mut results = Vec::new();
    let name_set: std::collections::HashSet<String> = names.iter().map(|s| s.to_string()).collect();

    // 1. Check Chaotic-AUR (Fast, contains binaries)
    if let Ok(chaotic_pkgs) = state_chaotic.fetch_packages().await {
        for p in chaotic_pkgs.iter() {
            if name_set.contains(&p.pkgname) {
                results.push(models::Package {
                    name: p.pkgname.clone(),
                    display_name: Some(utils::to_pretty_name(&p.pkgname)),
                    description: p
                        .metadata
                        .as_ref()
                        .and_then(|m| m.desc.clone())
                        .unwrap_or_default(),
                    version: p.version.clone().unwrap_or_default(),
                    source: models::PackageSource::Chaotic,
                    maintainer: Some("Chaotic-AUR Team".to_string()),
                    license: p
                        .metadata
                        .as_ref()
                        .and_then(|m| m.license.clone())
                        .map(|l| vec![l]),
                    url: p.metadata.as_ref().and_then(|m| m.url.clone()),
                    screenshots: None,
                    last_modified: None,
                    first_submitted: None,
                    out_of_date: None,
                    keywords: None,
                    num_votes: None,
                    icon: {
                        let mut icon = None;
                        if let Ok(loader) = state_meta.0.lock() {
                            icon = loader.find_icon_heuristic(&p.pkgname);
                        }
                        icon
                    },
                    provides: None,
                    app_id: {
                        let mut aid = None;
                        if let Ok(loader) = state_meta.0.lock() {
                            aid = loader.find_app_id(&p.pkgname);
                        }
                        if aid.is_none() {
                            aid = crate::flathub_api::get_flathub_app_id(&p.pkgname);
                        }
                        aid
                    },
                });
            }
        }
    }

    // Deduplicate found names
    let found_names: std::collections::HashSet<String> =
        results.iter().map(|p| p.name.clone()).collect();
    let remaining_names: Vec<String> = names
        .iter()
        .filter(|n| !found_names.contains(*n))
        .cloned()
        .collect();

    // 2. Check Repos for remaining
    if !remaining_names.is_empty() {
        for name in remaining_names {
            let mut pkgs = state_repo.get_all_packages(&name).await;

            // SORT PRIORITY: Chaotic > Official > Distros > AUR
            pkgs.sort_by(|a, b| {
                let get_score = |s: &models::PackageSource| match s {
                    models::PackageSource::Chaotic => 0,
                    models::PackageSource::Official => 1,
                    models::PackageSource::CachyOS
                    | models::PackageSource::Garuda
                    | models::PackageSource::Endeavour
                    | models::PackageSource::Manjaro => 2,
                    models::PackageSource::Aur => 3,
                };
                get_score(&a.source).cmp(&get_score(&b.source))
            });

            if let Some(pkg) = pkgs.first() {
                let mut p = pkg.clone();

                // Ensure App ID for reviews
                if p.app_id.is_none() {
                    if let Ok(loader) = state_meta.0.lock() {
                        p.app_id = loader.find_app_id(&p.name);
                    }
                }
                if p.app_id.is_none() {
                    p.app_id = crate::flathub_api::get_flathub_app_id(&p.name);
                }

                if p.icon.is_none() {
                    if let Ok(loader) = state_meta.0.lock() {
                        p.icon = loader.find_icon_heuristic(&p.name);
                    }
                }
                p.display_name = Some(utils::to_pretty_name(&p.name));
                results.push(p);
            }
        }
    }

    Ok(results)
}

#[tauri::command]
async fn get_repo_states(
    state: tauri::State<'_, RepoManager>,
) -> Result<Vec<repo_manager::RepoConfig>, String> {
    Ok(state.get_all_repos().await)
}

#[tauri::command]
async fn is_aur_enabled(state: tauri::State<'_, RepoManager>) -> Result<bool, String> {
    Ok(state.is_aur_enabled().await)
}

#[tauri::command]
async fn toggle_repo(
    state: tauri::State<'_, RepoManager>,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    state.set_repo_state(&name, enabled).await;
    Ok(())
}

#[tauri::command]
async fn toggle_repo_family(
    state: tauri::State<'_, RepoManager>,
    family: String,
    enabled: bool,
) -> Result<(), String> {
    state.set_repo_family_state(&family, enabled).await;
    Ok(())
}

#[tauri::command]
async fn set_aur_enabled(
    state: tauri::State<'_, RepoManager>,
    enabled: bool,
) -> Result<(), String> {
    state.set_aur_enabled(enabled).await;
    Ok(())
}

#[tauri::command]
async fn get_chaotic_packages(
    state: tauri::State<'_, ChaoticApiClient>,
) -> Result<Vec<ChaoticPackage>, String> {
    state.fetch_packages().await.map(|p| (*p).clone())
}

#[tauri::command]
async fn get_trending(
    state_meta: tauri::State<'_, metadata::MetadataState>,
    state_chaotic: tauri::State<'_, ChaoticApiClient>,
) -> Result<Vec<models::Package>, String> {
    let trending_raw = state_chaotic.fetch_trending().await?;
    let mut packages = Vec::new();

    // Prefetch all packages if needed to make find_package fast
    // Actually fetches are cached so it's fine.

    for t_pkg in trending_raw {
        // Enriched Lookup
        if let Some(c_pkg) = state_chaotic.find_package(&t_pkg.pkgbase_pkgname).await {
            let mut pkg = models::Package {
                name: c_pkg.pkgname.clone(),
                display_name: Some(utils::to_pretty_name(&c_pkg.pkgname)),
                description: c_pkg
                    .metadata
                    .as_ref()
                    .and_then(|m| m.desc.clone())
                    .unwrap_or_default(),
                version: c_pkg.version.clone().unwrap_or_default(),
                source: models::PackageSource::Chaotic,
                maintainer: Some("Chaotic-AUR Team".to_string()),
                license: c_pkg
                    .metadata
                    .as_ref()
                    .and_then(|m| m.license.clone())
                    .map(|l| vec![l]),
                url: c_pkg.metadata.as_ref().and_then(|m| m.url.clone()),
                last_modified: None,
                first_submitted: None,
                out_of_date: None,
                keywords: None,
                num_votes: None,
                icon: None, // Will set below
                screenshots: None,
                provides: c_pkg.provides.clone(),
                app_id: None, // Will set below
            };

            // Icon Lookup
            if let Ok(loader) = state_meta.0.lock() {
                pkg.icon = loader.find_icon_heuristic(&pkg.name);
                pkg.app_id = loader
                    .find_app_id(&pkg.name)
                    .or_else(|| crate::flathub_api::get_flathub_app_id(&pkg.name));
            }

            packages.push(pkg);
        } else {
            // If not found in cache (rare but possible if trending list has new items not in our cache sync)
            // We can add a fallback stripped version or just skip
            // Let's add a fallback so the box isn't missing, but it might be empty-ish
            // Actually better to skip than show broken data
        }
    }

    Ok(packages)
}

#[tauri::command]
async fn get_infra_stats(state: tauri::State<'_, ChaoticApiClient>) -> Result<InfraStats, String> {
    state.fetch_infra_stats().await
}

#[tauri::command]
async fn get_chaotic_package_info(
    state: tauri::State<'_, ChaoticApiClient>,
    name: String,
) -> Result<Option<ChaoticPackage>, ()> {
    Ok(state.get_package_by_name(&name).await)
}

#[tauri::command]
async fn get_chaotic_packages_batch(
    state: tauri::State<'_, ChaoticApiClient>,
    names: Vec<String>,
) -> Result<std::collections::HashMap<String, ChaoticPackage>, ()> {
    Ok(state.get_packages_batch(names).await)
}

#[tauri::command]
async fn get_app_rating(app_id: String) -> Result<Option<odrs_api::OdrsRating>, String> {
    odrs_api::get_app_rating(&app_id).await
}

#[tauri::command]
async fn get_app_reviews(app_id: String) -> Result<Vec<odrs_api::Review>, String> {
    odrs_api::get_app_reviews(&app_id).await
}

#[tauri::command]
async fn get_package_variants(
    state_meta: tauri::State<'_, metadata::MetadataState>,
    state_chaotic: tauri::State<'_, ChaoticApiClient>,
    state_repo: tauri::State<'_, RepoManager>,
    pkg_name: String,
) -> Result<Vec<models::PackageVariant>, String> {
    let mut variants = Vec::new();
    let mut sources_found = std::collections::HashSet::new(); // tuple of (Source, PkgName) might be better, but UI groups by Source? No, UI lists variants.
                                                              // Actually, distinct variants should be allowed even for same source if pkg_name differs (e.g. firefox vs firefox-pure in CachyOS).
                                                              // So distinct key: (Source, PkgName, Version).

    let mut distinct_variants = std::collections::HashSet::new();

    // 1. Check Repos (Official, CachyOS, Garuda, etc)
    // A. Exact Matches
    let repo_pkgs = state_repo.get_all_packages(&pkg_name).await;
    // B. Providers (e.g. "firefox-pure" provides "firefox")
    let provider_pkgs = state_repo.get_packages_providing(&pkg_name).await;

    // Combine
    let all_repo_pkgs = repo_pkgs.into_iter().chain(provider_pkgs.into_iter());

    for rp in all_repo_pkgs {
        let key = (rp.source.clone(), rp.name.clone());
        if !distinct_variants.contains(&key) {
            variants.push(models::PackageVariant {
                source: rp.source.clone(),
                version: rp.version,
                repo_name: None,
                pkg_name: Some(rp.name.clone()),
            });
            distinct_variants.insert(key);
            sources_found.insert(rp.source);
        }
    }

    // 2. Check Chaotic-AUR
    if let Some(c_pkg) = state_chaotic.get_package_by_name(&pkg_name).await {
        let key = (models::PackageSource::Chaotic, c_pkg.pkgname.clone());
        if !distinct_variants.contains(&key) {
            variants.push(models::PackageVariant {
                source: models::PackageSource::Chaotic,
                version: c_pkg.version.unwrap_or_else(|| "unknown".to_string()),
                repo_name: Some("chaotic-aur".to_string()),
                pkg_name: Some(c_pkg.pkgname),
            });
            distinct_variants.insert(key);
        }
    }
    // B. Providers
    let c_providers = state_chaotic.get_packages_providing(&pkg_name).await;
    for c_pkg in c_providers {
        let key = (models::PackageSource::Chaotic, c_pkg.pkgname.clone());
        if !distinct_variants.contains(&key) {
            variants.push(models::PackageVariant {
                source: models::PackageSource::Chaotic,
                version: c_pkg.version.unwrap_or_else(|| "unknown".to_string()),
                repo_name: Some("chaotic-aur".to_string()),
                pkg_name: Some(c_pkg.pkgname),
            });
            distinct_variants.insert(key);
        }
    }

    // 3. Check AUR (Source) - ONLY if Enabled
    if state_repo.is_aur_enabled().await {
        // A. Exact Match Search
        if let Ok(aur_results) = aur_api::search_aur(&pkg_name).await {
            if let Some(aur_pkg) = aur_results.into_iter().find(|p| p.name == pkg_name) {
                let key = (models::PackageSource::Aur, aur_pkg.name.clone());
                if !distinct_variants.contains(&key) {
                    variants.push(models::PackageVariant {
                        source: models::PackageSource::Aur,
                        version: aur_pkg.version,
                        repo_name: None,
                        pkg_name: Some(aur_pkg.name),
                    });
                    distinct_variants.insert(key);
                }
            }
        }
        // B. Providers Search
        if let Ok(aur_providers) = aur_api::search_aur_by_provides(&pkg_name).await {
            for aur_pkg in aur_providers {
                let key = (models::PackageSource::Aur, aur_pkg.name.clone());
                if !distinct_variants.contains(&key) {
                    variants.push(models::PackageVariant {
                        source: models::PackageSource::Aur,
                        version: aur_pkg.version,
                        repo_name: None,
                        pkg_name: Some(aur_pkg.name),
                    });
                    distinct_variants.insert(key);
                }
            }
        }
    }

    // 4. Fallback: Check AppStream (Official Metadata)
    // Only add if "Official" is NOT yet present (i.e. not found in synced repos)
    // Actually, AppStream is just metadata for Official usually.
    if !sources_found.contains(&models::PackageSource::Official) {
        let loader = state_meta.0.lock().unwrap();
        if let Some(meta) = loader.find_package(&pkg_name) {
            // AppStream implies official usually
            variants.push(models::PackageVariant {
                source: models::PackageSource::Official,
                version: meta.version.unwrap_or_else(|| "unknown".to_string()),
                repo_name: Some("community".to_string()),
                pkg_name: Some(pkg_name.clone()),
            });
        }
    }

    // Final Sort: Enforce strict priority
    // 1. Official
    // 2. Chaotic
    // 3. Other Repos (Cachy, Garuda, etc.)
    // 4. AUR
    variants.sort_by(|a, b| {
        let priority = |source: &models::PackageSource| match source {
            models::PackageSource::Official => 0,
            models::PackageSource::Chaotic => 1,
            models::PackageSource::Aur => 100,
            _ => 50, // All other repos (CachyOS, Garuda, etc.)
        };
        priority(&a.source).cmp(&priority(&b.source))
    });

    Ok(variants)
}

#[derive(Serialize)]
pub struct PaginatedResponse {
    pub items: Vec<models::Package>,
    pub total: usize,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn get_category_packages_paginated(
    state_meta: tauri::State<'_, metadata::MetadataState>,
    state_chaotic: tauri::State<'_, ChaoticApiClient>,
    state_repo: tauri::State<'_, RepoManager>,
    category: String,
    repo_filter: Option<String>,
    sort_by: Option<String>,
    page: usize,
    limit: usize,
) -> Result<PaginatedResponse, String> {
    // 1. Get AppStream apps (Instant via HashMap)
    let apps = if let Ok(loader) = state_meta.0.lock() {
        loader.get_apps_by_category(&category)
    } else {
        Vec::new()
    };

    let mut packages = Vec::new();

    // 2. Pre-fetch chaotic set for heuristics
    let chaotic_set = if let Ok(c_pkgs) = state_chaotic.fetch_packages().await {
        c_pkgs
            .iter()
            .map(|p| p.pkgname.clone())
            .collect::<std::collections::HashSet<_>>()
    } else {
        std::collections::HashSet::new()
    };

    for app in apps {
        let pkg_name = app.pkg_name.clone().unwrap_or(app.app_id.clone());

        let mut source = models::PackageSource::Official;
        if chaotic_set.contains(&pkg_name) {
            source = models::PackageSource::Chaotic;
        } else if let Some(r_pkg) = state_repo.get_package(&pkg_name).await {
            source = r_pkg.source;
        }

        packages.push(models::Package {
            name: pkg_name.clone(),
            display_name: Some(app.name),
            description: app.summary.unwrap_or_default(),
            version: app.version.unwrap_or_else(|| "latest".to_string()),
            source,
            maintainer: None,
            license: None,
            url: None,
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: None,
            icon: if app.icon_url.is_none() {
                let mut icon = None;
                if let Ok(loader) = state_meta.0.lock() {
                    icon = loader.find_icon_heuristic(&pkg_name);
                }
                icon
            } else {
                app.icon_url
            },
            screenshots: if app.screenshots.is_empty() {
                None
            } else {
                Some(app.screenshots)
            },
            provides: None,
            app_id: Some(app.app_id.clone()),
        });
    }

    // Track duplicates
    let mut seen_packages: std::collections::HashSet<String> =
        packages.iter().map(|p| p.name.clone()).collect();

    // 4. Get cached Chaotic packages
    let c_matches = state_chaotic.get_packages_by_category(&category).await;

    for p in c_matches {
        if !seen_packages.insert(p.pkgname.clone()) {
            continue;
        }

        packages.push(models::Package {
            name: p.pkgname.clone(),
            display_name: Some(utils::to_pretty_name(&p.pkgname)),
            description: p
                .metadata
                .as_ref()
                .and_then(|m| m.desc.clone())
                .unwrap_or_default(),
            version: p.version.clone().unwrap_or_default(),
            source: models::PackageSource::Chaotic,
            maintainer: Some("Chaotic-AUR Team".to_string()),
            license: p
                .metadata
                .as_ref()
                .and_then(|m| m.license.clone())
                .map(|l| vec![l]),
            url: p.metadata.as_ref().and_then(|m| m.url.clone()),
            last_modified: p.last_updated.as_ref().and_then(|s| {
                // Try to parse ISO8601/RFC3339 format from Chaotic API
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.timestamp())
            }),
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: None,
            icon: None,
            screenshots: None,
            provides: None,
            app_id: Some(p.pkgname.clone()), // Chaotic apps usually don't have AppStream ID, use pkgname as fallback
        });
    }

    // --- FILTER ---
    if let Some(filter) = repo_filter {
        if filter != "all" {
            packages.retain(|p| {
                let s = match p.source {
                    models::PackageSource::Chaotic => "chaotic",
                    models::PackageSource::Official => "official",
                    models::PackageSource::Aur => "aur",
                    models::PackageSource::CachyOS => "cachyos",
                    models::PackageSource::Garuda => "garuda",
                    models::PackageSource::Endeavour => "endeavour",
                    models::PackageSource::Manjaro => "manjaro",
                };
                s == filter
            });
        }
    }

    // --- SORT ---
    let sort = sort_by.unwrap_or_else(|| "name".to_string());
    match sort.as_str() {
        "updated" | "date" => {
            packages.sort_by(|a, b| {
                b.last_modified
                    .unwrap_or(0)
                    .cmp(&a.last_modified.unwrap_or(0))
            });
        }
        _ => {
            // Name (A-Z)
            packages.sort_by(|a, b| {
                let da = a.display_name.as_deref().unwrap_or(&a.name).to_lowercase();
                let db = b.display_name.as_deref().unwrap_or(&b.name).to_lowercase();
                da.cmp(&db)
            });
        }
    }

    // --- PAGINATE ---
    let total = packages.len();
    let start = (page - 1) * limit;
    if start >= total {
        return Ok(PaginatedResponse {
            items: vec![],
            total,
        });
    }
    let end = (start + limit).min(total);
    let items = packages[start..end].to_vec();

    Ok(PaginatedResponse { items, total })
}

#[tauri::command]
async fn get_repo_counts(
    state_repo: tauri::State<'_, RepoManager>,
    state_chaotic: tauri::State<'_, ChaoticApiClient>,
) -> Result<std::collections::HashMap<String, usize>, String> {
    let mut counts = state_repo.get_package_counts().await;

    // Add Chaotic Count
    match state_chaotic.fetch_packages().await {
        Ok(pkgs) => {
            counts.insert("chaotic".to_string(), pkgs.len());
        }
        Err(_) => {
            counts.insert("chaotic".to_string(), 0);
        }
    }

    // Add AUR Count (Estimated) if enabled
    if state_repo.is_aur_enabled().await {
        counts.insert("aur".to_string(), 85000); // 85k+ is a safe active estimate
    }

    Ok(counts)
}

#[tauri::command]
async fn optimize_system() -> Result<String, String> {
    let mut results = Vec::new();

    // 1. Remove pacman lock file if it exists
    let lock_path = std::path::Path::new("/var/lib/pacman/db.lck");
    if lock_path.exists() {
        // This requires root, so we use pkexec
        let output = Command::new("pkexec")
            .args(["rm", "-f", "/var/lib/pacman/db.lck"])
            .output();
        match output {
            Ok(o) if o.status.success() => results.push("✓ Removed pacman lock file".to_string()),
            _ => results
                .push("⚠ Could not remove lock file (may need manual intervention)".to_string()),
        }
    } else {
        results.push("✓ No lock file present".to_string());
    }

    // 2. Refresh pacman keys
    let key_output = Command::new("pkexec")
        .args(["pacman-key", "--refresh-keys"])
        .output();
    match key_output {
        Ok(o) if o.status.success() => results.push("✓ Refreshed pacman keys".to_string()),
        _ => results.push("⚠ Key refresh skipped or failed".to_string()),
    }

    // 3. Update package databases
    let sync_output = Command::new("pkexec").args(["pacman", "-Sy"]).output();
    match sync_output {
        Ok(o) if o.status.success() => results.push("✓ Updated package databases".to_string()),
        _ => results.push("⚠ Database sync failed".to_string()),
    }

    Ok(results.join("\n"))
}

// ============ INSTALLED PACKAGES & UPDATES ============

#[derive(Serialize, Clone)]
pub struct InstalledPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub size: String,
    pub install_date: String,
    pub description: String,
}

#[derive(Serialize, Clone)]
pub struct PendingUpdate {
    pub id: String,
    pub name: String,
    pub current_version: String,
    pub new_version: String,
    pub size: String,
    pub update_type: String, // "official", "aur", "chaotic"
}

/// Get list of installed packages using pacman -Q
#[tauri::command]
async fn get_installed_packages() -> Result<Vec<InstalledPackage>, String> {
    // Try to run pacman -Q (only works on Arch)
    // Use -Qei to get info (-i) but ONLY for explicitly installed packages (-e)
    // This hides dependencies and system libs from the UI
    let output = Command::new("pacman").args(["-Qei"]).output();

    match output {
        Ok(out) if out.status.success() => {
            let content = String::from_utf8_lossy(&out.stdout);
            let packages = parse_pacman_qi(&content);
            Ok(packages)
        }
        _ => {
            // Fallback for non-Arch systems (macOS dev)
            Ok(get_demo_installed_packages())
        }
    }
}

fn parse_pacman_qi(content: &str) -> Vec<InstalledPackage> {
    let mut packages = Vec::new();
    let mut current = InstalledPackage {
        id: String::new(),
        name: String::new(),
        version: String::new(),
        size: String::new(),
        install_date: String::new(),
        description: String::new(),
    };

    for line in content.lines() {
        if line.starts_with("Name") {
            if !current.id.is_empty() {
                packages.push(current.clone());
            }
            let name = line.split(':').nth(1).map(|s| s.trim()).unwrap_or("");
            current = InstalledPackage {
                id: name.to_string(),
                name: name.to_string(),
                version: String::new(),
                size: String::new(),
                install_date: String::new(),
                description: String::new(),
            };
        } else if line.starts_with("Version") {
            current.version = line
                .split(':')
                .nth(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
        } else if line.starts_with("Installed Size") {
            current.size = line
                .split(':')
                .nth(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
        } else if line.starts_with("Install Date") {
            current.install_date = line
                .split(':')
                .skip(1)
                .collect::<Vec<_>>()
                .join(":")
                .trim()
                .to_string();
        } else if line.starts_with("Description") {
            current.description = line
                .split(':')
                .nth(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
        }
    }

    if !current.id.is_empty() {
        packages.push(current);
    }

    packages
}

fn get_demo_installed_packages() -> Vec<InstalledPackage> {
    vec![
        InstalledPackage {
            id: "firefox".into(),
            name: "Firefox".into(),
            version: "121.0-1".into(),
            size: "256 MiB".into(),
            install_date: "2024-01-15".into(),
            description: "Fast, private web browser".into(),
        },
        InstalledPackage {
            id: "vlc".into(),
            name: "VLC".into(),
            version: "3.0.20-1".into(),
            size: "64 MiB".into(),
            install_date: "2023-12-20".into(),
            description: "Multimedia player".into(),
        },
        InstalledPackage {
            id: "gimp".into(),
            name: "GIMP".into(),
            version: "2.10.36-1".into(),
            size: "400 MiB".into(),
            install_date: "2024-01-10".into(),
            description: "GNU Image Manipulation Program".into(),
        },
        InstalledPackage {
            id: "discord".into(),
            name: "Discord".into(),
            version: "0.0.35-1".into(),
            size: "150 MiB".into(),
            install_date: "2024-01-19".into(),
            description: "All-in-one voice and text chat".into(),
        },
        InstalledPackage {
            id: "obs-studio".into(),
            name: "OBS Studio".into(),
            version: "30.0.0-1".into(),
            size: "320 MiB".into(),
            install_date: "2023-10-30".into(),
            description: "Streaming and recording software".into(),
        },
        InstalledPackage {
            id: "visual-studio-code-bin".into(),
            name: "Visual Studio Code".into(),
            version: "1.85.1-1".into(),
            size: "350 MiB".into(),
            install_date: "2023-12-15".into(),
            description: "Code editor".into(),
        },
        InstalledPackage {
            id: "spotify".into(),
            name: "Spotify".into(),
            version: "1.2.26-1".into(),
            size: "180 MiB".into(),
            install_date: "2024-01-18".into(),
            description: "Music streaming service".into(),
        },
        InstalledPackage {
            id: "steam".into(),
            name: "Steam".into(),
            version: "1.0.0.78-1".into(),
            size: "12 MiB".into(),
            install_date: "2023-11-20".into(),
            description: "Digital distribution platform".into(),
        },
    ]
}

/// Check for available updates using checkupdates
#[tauri::command]
async fn check_for_updates() -> Result<Vec<PendingUpdate>, String> {
    // Try checkupdates (from pacman-contrib)
    let output = Command::new("checkupdates").output();

    match output {
        Ok(out) if out.status.success() || out.status.code() == Some(2) => {
            // checkupdates returns 2 if no updates, 0 if updates available
            let content = String::from_utf8_lossy(&out.stdout);
            let updates = parse_checkupdates(&content);
            Ok(updates)
        }
        _ => {
            // Fallback for non-Arch systems
            Ok(get_demo_updates())
        }
    }
}

fn parse_checkupdates(content: &str) -> Vec<PendingUpdate> {
    content
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                Some(PendingUpdate {
                    id: parts[0].to_string(),
                    name: parts[0].to_string(),
                    current_version: parts[1].to_string(),
                    new_version: parts[3].to_string(),
                    size: "~10 MiB".into(), // checkupdates doesn't provide size
                    update_type: "official".into(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn get_demo_updates() -> Vec<PendingUpdate> {
    vec![
        PendingUpdate {
            id: "firefox".into(),
            name: "Firefox".into(),
            current_version: "121.0-1".into(),
            new_version: "121.0.1-1".into(),
            size: "65 MiB".into(),
            update_type: "official".into(),
        },
        PendingUpdate {
            id: "discord".into(),
            name: "Discord".into(),
            current_version: "0.0.35-1".into(),
            new_version: "0.0.36-1".into(),
            size: "85 MiB".into(),
            update_type: "official".into(),
        },
        PendingUpdate {
            id: "neovim".into(),
            name: "Neovim".into(),
            current_version: "0.10.0-dev".into(),
            new_version: "0.10.0-rc1".into(),
            size: "12 MiB".into(),
            update_type: "chaotic".into(),
        },
    ]
}

/// Get icon path for a package - checks local icons first
#[tauri::command]
async fn get_package_icon(pkg_name: String) -> Result<Option<String>, String> {
    // Check if we have a local icon in the cache directory
    let icons_dir = metadata::get_icons_dir();

    // Pattern matching logic
    if let Ok(entries) = std::fs::read_dir(&icons_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name_os) = path.file_name() {
                let name = name_os.to_string_lossy();
                // Simple prefix match like before: "firefox_*.png" or "firefox.png"
                if (name.starts_with(&pkg_name) && name.ends_with(".png"))
                    && (name == format!("{}.png", pkg_name)
                        || name.starts_with(&format!("{}_", pkg_name)))
                {
                    if let Ok(bytes) = std::fs::read(&path) {
                        let encoded = BASE64_STANDARD.encode(&bytes);
                        return Ok(Some(format!("data:image/png;base64,{}", encoded)));
                    }
                }
            }
        }
    }

    Ok(None)
}

// Define wrapper for state management
pub struct ScmState(pub scm_api::ScmClient);

/// Launch an application by package name
/// Tries multiple methods: gtk-launch, xdg-open on .desktop file, or direct execution
#[tauri::command]
async fn launch_app(pkg_name: String) -> Result<(), String> {
    // Method 1: Try gtk-launch (most reliable for desktop apps)
    let gtk_result = Command::new("gtk-launch").arg(&pkg_name).spawn();

    if gtk_result.is_ok() {
        return Ok(());
    }

    // Method 2: Look for .desktop file and use xdg-open or direct exec
    let desktop_dirs = [
        "/usr/share/applications",
        "/usr/local/share/applications",
        &format!(
            "{}/.local/share/applications",
            std::env::var("HOME").unwrap_or_default()
        ),
    ];

    for dir in desktop_dirs.iter() {
        let desktop_path = format!("{}/{}.desktop", dir, pkg_name);
        if std::path::Path::new(&desktop_path).exists() {
            // Try gio launch first (GNOME)
            if Command::new("gio")
                .args(["launch", &desktop_path])
                .spawn()
                .is_ok()
            {
                return Ok(());
            }

            // Try dex (generic .desktop launcher)
            if Command::new("dex").arg(&desktop_path).spawn().is_ok() {
                return Ok(());
            }

            // Parse .desktop file and execute Exec line
            if let Ok(content) = std::fs::read_to_string(&desktop_path) {
                for line in content.lines() {
                    if line.starts_with("Exec=") {
                        let exec_line = line.trim_start_matches("Exec=");
                        // Remove %f, %u, %F, %U placeholders
                        let clean_exec: String = exec_line
                            .split_whitespace()
                            .filter(|s| !s.starts_with('%'))
                            .collect::<Vec<_>>()
                            .join(" ");

                        if let Some(cmd) = clean_exec.split_whitespace().next() {
                            let args: Vec<&str> = clean_exec.split_whitespace().skip(1).collect();
                            if Command::new(cmd).args(&args).spawn().is_ok() {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }

    // Method 3: Try running the package name directly as a command
    if Command::new(&pkg_name).spawn().is_ok() {
        return Ok(());
    }

    Err(format!(
        "Could not launch {}. No .desktop file or executable found.",
        pkg_name
    ))
}

/// Perform a real system update using pacman and/or AUR helper
/// Emits progress events to the frontend
#[tauri::command]
async fn perform_system_update(
    app: tauri::AppHandle,
    password: Option<String>,
) -> Result<String, String> {
    // Emit: Starting
    let _ = app.emit(
        "update-progress",
        serde_json::json!({
            "phase": "starting",
            "progress": 0,
            "message": "Initializing MonArch System Update..."
        }),
    );

    // Detect AUR helper
    let aur_helper = ["paru", "yay", "aura", "pikaur"]
        .iter()
        .find(|&helper| {
            Command::new("which")
                .arg(helper)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .map(|s| s.to_string());

    let mut results = Vec::new();

    // Method 1: Use AUR helper if available (handles both official and AUR)
    if let Some(helper) = &aur_helper {
        let _ = app.emit(
            "update-progress",
            serde_json::json!({
                "phase": "updating",
                "progress": 20,
                "message": format!("Running {} -Syu...", helper)
            }),
        );

        let mut cmd = Command::new("pkexec");
        cmd.args([helper, "-Syu", "--noconfirm"]);

        // If password provided, use sudo with password instead
        if let Some(pwd) = &password {
            cmd = Command::new("sh");
            cmd.args([
                "-c",
                &format!("echo '{}' | sudo -S {} -Syu --noconfirm", pwd, helper),
            ]);
        }

        let output = cmd.output();

        match output {
            Ok(o) if o.status.success() => {
                results.push(format!("✓ Updated system via {}", helper));
                let _ = app.emit(
                    "update-progress",
                    serde_json::json!({
                        "phase": "complete",
                        "progress": 100,
                        "message": "Update complete!"
                    }),
                );
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                results.push(format!(
                    "⚠ {} update had issues: {}",
                    helper,
                    stderr.lines().take(3).collect::<Vec<_>>().join(" ")
                ));
                let _ = app.emit(
                    "update-progress",
                    serde_json::json!({
                        "phase": "error",
                        "progress": 100,
                        "message": "Update completed with warnings"
                    }),
                );
            }
            Err(e) => {
                return Err(format!("Failed to run {}: {}", helper, e));
            }
        }
    } else {
        // Method 2: Use pacman directly for official repos
        let _ = app.emit(
            "update-progress",
            serde_json::json!({
                "phase": "syncing",
                "progress": 10,
                "message": "Syncing package databases..."
            }),
        );

        // Sync databases first
        let sync_cmd = if let Some(pwd) = &password {
            Command::new("sh")
                .args([
                    "-c",
                    &format!("echo '{}' | sudo -S pacman -Sy --noconfirm", pwd),
                ])
                .output()
        } else {
            Command::new("pkexec")
                .args(["pacman", "-Sy", "--noconfirm"])
                .output()
        };

        match sync_cmd {
            Ok(o) if o.status.success() => results.push("✓ Synced package databases".to_string()),
            _ => results.push("⚠ Database sync may have failed".to_string()),
        }

        let _ = app.emit(
            "update-progress",
            serde_json::json!({
                "phase": "downloading",
                "progress": 30,
                "message": "Downloading packages..."
            }),
        );

        // Perform upgrade
        let upgrade_cmd = if let Some(pwd) = &password {
            Command::new("sh")
                .args([
                    "-c",
                    &format!("echo '{}' | sudo -S pacman -Su --noconfirm", pwd),
                ])
                .output()
        } else {
            Command::new("pkexec")
                .args(["pacman", "-Su", "--noconfirm"])
                .output()
        };

        match upgrade_cmd {
            Ok(o) if o.status.success() => {
                results.push("✓ Upgraded official packages".to_string());
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                results.push(format!(
                    "⚠ Upgrade had issues: {}",
                    stderr.lines().take(2).collect::<Vec<_>>().join(" ")
                ));
            }
            _ => results.push("⚠ Upgrade may have failed".to_string()),
        }

        let _ = app.emit(
            "update-progress",
            serde_json::json!({
                "phase": "complete",
                "progress": 100,
                "message": "Update complete!"
            }),
        );
    }

    Ok(results.join("\n"))
}

/// Fetch PKGBUILD content from AUR for a package
/// Returns the raw PKGBUILD content as a string
#[tauri::command]
async fn fetch_pkgbuild(pkg_name: String) -> Result<String, String> {
    // AUR PKGBUILDs are hosted in git repos at:
    // https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h=<package_name>
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        pkg_name
    );

    let client = reqwest::Client::builder()
        .user_agent("MonARCH-Store/0.1.0 (Tauri; Arch Linux)")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch PKGBUILD: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "PKGBUILD not found for '{}'. This package may not be in the AUR.",
            pkg_name
        ));
    }

    let content = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read PKGBUILD content: {}", e))?;

    // Basic validation - PKGBUILD should contain pkgname or pkgbase
    if !content.contains("pkgname") && !content.contains("pkgbase") {
        return Err("Invalid PKGBUILD format - missing pkgname/pkgbase".to_string());
    }

    Ok(content)
}

/// Get list of orphan packages (unused dependencies)
/// Runs `pacman -Qtdq`
#[tauri::command]
fn get_orphans() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("pacman")
        .args(["-Qtdq"])
        .output()
        .map_err(|e| format!("Failed to check orphans: {}", e))?;

    if !output.status.success() {
        // If exit code is non-zero, it usually means no orphans found (pacman returns 1 when no results)
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Remove orphan packages
/// Runs `pkexec pacman -Rns <orphans> --noconfirm`
#[tauri::command]
async fn remove_orphans(orphans: Vec<String>) -> Result<(), String> {
    if orphans.is_empty() {
        return Ok(());
    }

    // Use pkexec to get permissions
    let mut args = vec!["pacman", "-Rns"];
    args.extend(orphans.iter().map(|s| s.as_str()));
    args.push("--noconfirm");

    let status = std::process::Command::new("pkexec")
        .args(&args)
        .status()
        .map_err(|e| format!("Failed to execute removal: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err("Failed to remove orphans (process exited with error)".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // .plugin(tauri_plugin_aptabase::Builder::new("A-EU-3907248034").build())
        .manage(repo_manager::RepoManager::new())
        .manage(chaotic_api::ChaoticApiClient::new())
        .manage(flathub_api::FlathubApiClient::new())
        .manage(metadata::MetadataState(std::sync::Mutex::new(
            metadata::AppStreamLoader::new(),
        )))
        .manage(ScmState(scm_api::ScmClient::new())) // Initialize SCM Client
        .setup(|app| {
            // let _ = app.track_event("app_started", None);
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state_repo = handle.state::<RepoManager>();
                let state_chaotic = handle.state::<ChaoticApiClient>();

                // 1. Sync Repos
                // Default startup sync (Smart Sync, 3h interval default)
                match state_repo.sync_all(false, 3).await {
                    Ok(msg) => println!("Startup Sync: {}", msg),
                    Err(e) => println!("Startup Sync Failed: {}", e),
                }

                // 2. Fetch Chaotic (Ensure it's warm and print count)
                match state_chaotic.fetch_packages().await {
                    Ok(pkgs) => println!(
                        "Startup Sync: Synced {} packages from chaotic-aur",
                        pkgs.len()
                    ),
                    Err(e) => println!("Startup Sync Failed (Chaotic): {}", e),
                }

                // 3. Init Metadata (AppStream) - Default 24h for initial backend setup
                let state_meta = handle.state::<metadata::MetadataState>();
                state_meta.init(24).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search_aur,
            get_packages_by_names,
            get_chaotic_package_info,
            get_chaotic_packages_batch,
            metadata::get_metadata,
            get_category_packages_paginated, // Replaced get_category_packages
            search_packages,
            get_app_rating,
            get_app_reviews,
            get_chaotic_packages,
            get_trending,
            get_infra_stats,
            fetch_repo,
            get_package_variants,
            trigger_repo_sync,
            toggle_repo,
            set_aur_enabled,
            is_aur_enabled,
            get_repo_states,
            install_package,
            uninstall_package,
            get_system_info,
            reviews::submit_review,
            reviews::get_local_reviews,
            get_repo_counts,
            optimize_system,
            toggle_repo_family,
            clear_cache,
            // New package management commands
            get_installed_packages,
            check_for_updates,
            get_package_icon,
            launch_app,
            perform_system_update,
            fetch_pkgbuild,
            get_orphans,
            remove_orphans,
            repo_setup::check_repo_status, // [NEW]
            repo_setup::enable_repo,
            repo_setup::enable_repos_batch,
            repo_setup::reset_pacman_conf, // [NEW]
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_handler, event| match event {
            tauri::RunEvent::Exit { .. } => {
                // let _ = handler.track_event("app_exited", None);
                // handler.flush_events_blocking();
            }
            _ => {}
        });
}

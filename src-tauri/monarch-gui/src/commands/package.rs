use crate::{aur_api, helper_client, models, repo_manager::RepoManager};
use moka::future::Cache;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::{Duration as StdDuration, Instant};
use tauri::{AppHandle, Emitter, State};
use tempfile;
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
use tokio::sync::Mutex;

/// Global PID of an active AUR build (makepkg) so abort_installation can kill it.
static ACTIVE_AUR_BUILD_PID: AtomicU32 = AtomicU32::new(0);

use crate::models::FullPackageDetails;

// Short-lived details cache to dedupe StrictMode/dev duplicate invokes and rapid re-opens.
static FULL_DETAILS_CACHE: Lazy<Cache<String, FullPackageDetails>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(std::time::Duration::from_secs(600))
        .max_capacity(256)
        .build()
});
static FULL_DETAILS_GATE: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static CACHE_STATS_CACHE: Lazy<StdMutex<Option<(Instant, models::CacheStats)>>> =
    Lazy::new(|| StdMutex::new(None));
type InstalledCatalogEntry = (Instant, Vec<models::Package>);
static INSTALLED_CATALOG_CACHE: Lazy<StdMutex<Option<InstalledCatalogEntry>>> =
    Lazy::new(|| StdMutex::new(None));
static INSTALLED_CATALOG_GATE: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
const CACHE_STATS_TTL: StdDuration = StdDuration::from_secs(5);
const INSTALLED_CATALOG_TTL: StdDuration = StdDuration::from_secs(20);

pub(crate) fn invalidate_installed_catalog_cache() {
    if let Ok(mut cache) = INSTALLED_CATALOG_CACHE.lock() {
        *cache = None;
    }
}

fn maintainer_fallback_for_source(source: &models::PackageSource) -> Option<String> {
    let id = source.id.to_lowercase();
    match source.source_type.as_str() {
        "repo" => {
            if id.contains("cachyos") {
                Some("CachyOS Packaging Team".to_string())
            } else if id.contains("chaotic") {
                Some("Chaotic-AUR Team".to_string())
            } else if id.contains("manjaro") || id.contains("garuda") || id.contains("endeavour") {
                Some("Distribution Packaging Team".to_string())
            } else {
                Some("Arch Linux Packager".to_string())
            }
        }
        _ => None,
    }
}

fn derive_developer_name_for_details(pkg: &models::Package) -> Option<String> {
    if let Some(maintainer) = pkg.maintainer.as_ref().map(|v| v.trim()).filter(|v| !v.is_empty()) {
        return Some(maintainer.to_string());
    }

    let url = pkg.url.as_ref().map(|v| v.trim()).filter(|v| !v.is_empty())?;
    let host = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .trim_start_matches("www.");
    let first = host.split('.').next().unwrap_or(host).trim();
    if first.is_empty() {
        return None;
    }

    let mut chars = first.chars();
    let head = chars.next()?;
    Some(format!(
        "{}{}",
        head.to_uppercase(),
        chars.collect::<String>()
    ))
}

fn derive_donation_url_for_details(pkg: &models::Package) -> Option<String> {
    pkg.url
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
}

fn build_security_summary(
    source: Option<&models::PackageSource>,
    maintainer_known: bool,
) -> models::PackageSecuritySummary {
    let fallback = models::PackageSource::new("repo", "core", "latest", "Arch Official");
    let source = source.unwrap_or(&fallback);
    let id = source.id.to_lowercase();

    let (trust_tier, system_access, verification_note) = match source.source_type.as_str() {
        "flatpak" => (
            "sandboxed",
            "scoped",
            "Runs with sandboxed permissions, which may vary by app.",
        ),
        "aur" => (
            "community_build",
            "full",
            "Built from community-provided packaging scripts on your machine.",
        ),
        "repo" if id.contains("chaotic") => (
            "third_party_repo",
            "full",
            "Provided by a third-party binary repository.",
        ),
        "repo" if id.contains("cachyos")
            || id.contains("manjaro")
            || id.contains("garuda")
            || id.contains("endeavour") => (
            "distro_native",
            "full",
            "Provided by your distribution's repositories.",
        ),
        _ => (
            "official",
            "full",
            "Provided by the system package repositories.",
        ),
    };

    let user_action_note = if maintainer_known {
        "Review the package source before installing or updating."
    } else {
        "This source did not publish a maintainer. Verify the source before installing."
    };

    models::PackageSecuritySummary {
        trust_tier: trust_tier.to_string(),
        system_access: system_access.to_string(),
        maintainer_known,
        verification_note: verification_note.to_string(),
        user_action_note: user_action_note.to_string(),
    }
}

fn repo_family_key(source_id: &str) -> String {
    let id = source_id.to_lowercase();
    if id.contains("cachyos") {
        "cachyos".to_string()
    } else if id.contains("manjaro") {
        "manjaro".to_string()
    } else if id.contains("garuda") {
        "garuda".to_string()
    } else if id.contains("endeavour") {
        "endeavouros".to_string()
    } else if id.contains("chaotic") {
        "chaotic-aur".to_string()
    } else if matches!(id.as_str(), "core" | "extra" | "community" | "multilib" | "official") {
        "arch-official".to_string()
    } else {
        id
    }
}

fn same_source_identity_or_family(
    a: &models::PackageSource,
    b: &models::PackageSource,
) -> bool {
    if a.source_type != b.source_type {
        return false;
    }

    if a.id == b.id {
        return true;
    }

    if a.source_type == "repo" {
        return repo_family_key(&a.id) == repo_family_key(&b.id);
    }

    false
}

fn canonicalize_to_known_variant_source(
    preferred: Option<models::PackageSource>,
    variants: &[models::PackageVariant],
) -> Option<models::PackageSource> {
    let preferred = preferred?;
    variants
        .iter()
        .find(|variant| same_source_identity_or_family(&variant.source, &preferred))
        .map(|variant| variant.source.clone())
        .or(Some(preferred))
}

fn reorder_variants_for_selected_source(
    variants: &mut [models::PackageVariant],
    selected_source: Option<&models::PackageSource>,
    preferred_default: Option<&models::PackageSource>,
) {
    variants.sort_by_key(|variant| {
        if selected_source
            .map(|selected| same_source_identity_or_family(&variant.source, selected))
            .unwrap_or(false)
        {
            0
        } else if preferred_default
            .map(|preferred| same_source_identity_or_family(&variant.source, preferred))
            .unwrap_or(false)
        {
            1
        } else {
            2
        }
    });
}

fn resolve_authoritative_selected_source(
    package: Option<&models::Package>,
    install_status: &PackageInstallStatus,
    all_installed_variants: &[PackageInstallStatus],
    all_variants: &mut [models::PackageVariant],
) -> Option<models::PackageSource> {
    let package_default = package.map(|pkg| pkg.source.clone());
    let installed_candidate = all_installed_variants
        .iter()
        .find_map(|status| status.source.clone())
        .or_else(|| install_status.source.clone());

    let selected = if install_status.installed {
        canonicalize_to_known_variant_source(installed_candidate, all_variants)
            .or_else(|| canonicalize_to_known_variant_source(package_default.clone(), all_variants))
    } else {
        canonicalize_to_known_variant_source(package_default.clone(), all_variants)
            .or_else(|| all_variants.first().map(|variant| variant.source.clone()))
    };

    reorder_variants_for_selected_source(all_variants, selected.as_ref(), package_default.as_ref());
    selected
}

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

#[derive(Serialize, Clone, Type)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub install_date: Option<String>,
    pub install_date_unix: Option<i64>,
    pub size: Option<String>,
    pub size_bytes: Option<u64>,
    pub url: Option<String>,
    pub repository: Option<String>,
    pub source_label: Option<String>,
    pub resolved_source: Option<models::PackageSource>,
    pub display_name: Option<String>,
    pub launchable: bool,

    // Optimizing "The Storm": Serve icon directly to avoid N+1 requests
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
pub struct PackageInstallStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub repo: Option<String>,
    pub source: Option<models::PackageSource>,
    pub actual_package_name: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Type)]
pub struct PendingUpdate {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
    pub repo: String,
}

#[tauri::command]
#[specta::specta]
pub async fn abort_installation(app: AppHandle) -> Result<(), String> {
    let mut aborted = false;

    // 1. Kill active AUR build (makepkg) if running via PID tracker (Zone 1/2)
    let aur_pid = ACTIVE_AUR_BUILD_PID.swap(0, Ordering::SeqCst);
    if aur_pid > 0 {
        let _ = app.emit("install-output", "--- Killing AUR build process ---");
        // Send SIGTERM to the process group to kill makepkg and its children
        let _ = tokio::process::Command::new("kill")
            .args(["-TERM", &format!("-{}", aur_pid)])
            .status()
            .await;
        aborted = true;
    }

    // 2. Kill any GUI-tracked child process (redundant but safe)
    let mut active = ACTIVE_INSTALL_PROCESS.lock().await;
    if let Some(mut child) = active.take() {
        let _ = app.emit("install-output", "--- Aborting local process ---");
        let _ = child.kill().await;
        aborted = true;
    }
    drop(active);

    // 3. Signal the Root Helper (ALPM/Repo/System transactions)
    if helper_client::abort_helper().await.is_ok() {
        aborted = true;
    }

    // 4. Signal and kill any active Flatpak process
    if crate::flathub_api::abort_flatpak().await.is_ok() {
        aborted = true;
    }

    // 4. Create the Cancel File (helper heartbeat fallback)
    const CANCEL_FILE: &str = "/var/tmp/monarch-cancel";
    let _ = std::fs::write(CANCEL_FILE, "1");

    if aborted {
        let _ = app.emit("install-output", "--- Installation Aborted by User ---");
        let _ = app.emit("install-complete", "failed");
        Ok(())
    } else {
        // Fallback: emit failure anyway to reset UI
        let _ = app.emit("install-complete", "failed");
        Ok(()) // Return Ok so UI logic doesn't show a second error popup
    }
}

#[tauri::command]
#[specta::specta]
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
        &_state_repo,
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
    if distro.id == crate::distro_context::DistroId::Manjaro
        && (source.id == "chaotic-aur" || source.id == "cachyos")
    {
        let msg = "Manjaro Stability Guard: Installing pre-built binaries (Chaotic/CachyOS) is blocked on Manjaro to prevent system breakage. Please use the AUR (Native Build) version instead.".to_string();
        let _ = app.emit("install-output", &msg);
        let _ = app.emit("install-complete", "failed");
        return Err(msg);
    }

    // Pre-flight check: Database Lock - try to unlock if stale
    if crate::repair::check_pacman_lock().await {
        let _ = app.emit(
            "install-output",
            "Database is locked. Checking if lock is stale...",
        );
        // Always use helper (Polkit) for unlock so we don't run sudo with a password that may be
        // empty or wrong; the helper RemoveLock does the same safe rm and avoids "sudo: no password was provided".
        let one_click = repo_manager.is_one_click_enabled().await;
        match crate::repair::repair_unlock_pacman_impl(app, None, one_click).await {
            Ok(_) => {
                let _ = app.emit(
                    "install-output",
                    "✓ Stale lock removed. Proceeding with installation...",
                );
            }
            Err(e) => {
                let _ = app.emit(
                    "install-output",
                    &format!("Error: Database is locked by another process: {}", e),
                );
                let _ = app.emit("install-complete", "failed");
                return Err(format!("Pacman database is locked: {}", e));
            }
        }
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

    let one_click = repo_manager.is_one_click_enabled().await;

    // Use ALL enabled repos for the transaction so dependencies can be resolved (e.g. vlc-git from chaotic needs deps from core/extra/community).
    // Always include system repos (core, extra, community, multilib) so ALPM can resolve dependencies even if UI state is stale.
    let all_repos = repo_manager.get_all_repos().await;
    let mut enabled_repos: Vec<String> = all_repos
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.name.clone())
        .collect();
    for sys in ["core", "extra", "community", "multilib"] {
        if !enabled_repos.contains(&sys.to_string()) {
            enabled_repos.push(sys.to_string());
        }
    }

    let mut saw_unknown_variant = false;
    let mut saw_corrupt_db = false;
    // Buffer last install-output lines to surface real ALPM errors (e.g. "not found in any enabled repository")
    let mut install_log: Vec<String> = Vec::new();
    const LOG_CAP: usize = 50;

    match source.source_type.as_str() {
        "aur" => {
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
                one_click,
            )
            .await
            .map_err(|e| format!("Failed to invoke helper: {}", e))?;

            // Stream progress events
            while let Some(msg) = rx.recv().await {
                let _ = app.emit("install-output", &msg.message);
            }
        }
        "flatpak" => {
            let is_beta = source.id == "flathub-beta";
            let _ = app.emit(
                "install-output",
                format!(
                    "Installing {} from Flathub{}...",
                    name,
                    if is_beta { " Beta" } else { "" }
                ),
            );
            crate::flathub_api::install_flatpak(
                app.clone(),
                name.to_string(),
                Some(source.id.as_str()),
            )
            .await?;
            // Flatpak success: skip ALPM verification (package is not in pacman DB). Emit success and return.
            invalidate_installed_catalog_cache();
            let _ = app.emit("install-complete", "success");
            if repo_manager.is_notifications_enabled().await {
                crate::commands::system::show_desktop_notification_safe(
                    app,
                    "✨ MonARCH: Installation Complete".to_string(),
                    format!("Successfully installed '{}'", name),
                )
                .await;
            }
            crate::utils::track_event_safe(
                app,
                "install_package",
                Some(serde_json::json!({
                    "pkg": name,
                    "source": if is_beta { "flatpak_beta" } else { "flatpak" },
                    "success": true,
                })),
            )
            .await;
            return Ok(());
        }
        _ => {
            // Sync databases so helper sees latest repo state (host-adaptive: repos are
            // discovered from pacman.conf, not injected).
            let is_monarch_repo = matches!(
                source.id.as_str(),
                "chaotic-aur" | "cachyos" | "garuda" | "endeavour" | "manjaro"
            );
            if is_monarch_repo {
                repo_manager
                    .apply_os_config(app, password.clone())
                    .await
                    .map_err(|e| format!("Repository sync failed. {}", e))?;
            }

            let _ = app.emit("install-output", "--- Starting ALPM Transaction ---");

            let sync_first = false;

            // ✅ GHOST FIX: Pass selected repo so helper installs from THAT repo ONLY (not first match).
            // We prioritize source.id if source_type is "repo", otherwise fallback to Legacy _repo_name.
            let target_repo = if source.source_type == "repo"
                && !source.id.is_empty()
                && source.id != "id_unknown"
            {
                Some(source.id.clone())
            } else {
                _repo_name.clone()
            };

            let mut rx = helper_client::invoke_helper(
                app,
                helper_client::HelperCommand::AlpmInstall {
                    packages: vec![name.to_string()],
                    sync_first,
                    enabled_repos: enabled_repos.clone(),
                    cpu_optimization: cpu_optimization.clone(),
                    target_repo: target_repo.clone(),
                },
                password.clone(),
                one_click,
            )
            .await
            .map_err(|e| format!("Failed to invoke helper: {}", e))?;

            let mut saw_download_error = false;
            while let Some(msg) = rx.recv().await {
                if !msg.is_structured {
                    let _ = app.emit("install-output", &msg.message);
                }
                install_log.push(msg.message.clone());
                if install_log.len() > LOG_CAP {
                    install_log.remove(0);
                }
                if (msg.message.contains("unknown variant") && msg.message.contains("AlpmInstall"))
                    || (msg.message.contains("expected one of")
                        && msg.message.contains("ExecuteBatch"))
                    || msg.message.contains("outdated and does not support ALPM")
                {
                    saw_unknown_variant = true;
                }
                if msg.message.contains("Unrecognized archive format")
                    || msg.message.contains("could not open database")
                {
                    saw_corrupt_db = true;
                }
                // Detect 404/Download failures (stale DB)
                if msg.message.to_lowercase().contains("failed retrieving")
                    || msg.message.to_lowercase().contains("404")
                    || msg.message.contains("unexpected error: package")
                // generic alpm error?
                {
                    saw_download_error = true;
                }
            }

            // ✅ AUTO-RETRY: If download failed, database is likely stale.
            // Retry with sync_first=true. The helper will ENFORCE full system upgrade to be safe on Arch.
            if saw_download_error && !saw_corrupt_db {
                let _ = app.emit(
                    "install-output",
                    "⚠ Download failed (likely stale database).",
                );
                let _ = app.emit(
                    "install-output",
                    "System update required before installation can continue.",
                );
                let _ = app.emit(
                    "install-output",
                    "Select “Update & Install” to perform a full upgrade (-Syu) and retry safely.",
                );
                let _ = app.emit("install-complete", "failed_update_required");
                return Err("SystemUpdateRequired: Package database is out of date.".to_string());
            }

            if saw_unknown_variant {
                let _ = app.emit(
                    "install-output",
                    "Installed helper is outdated; syncing and installing with legacy path.",
                );
                let _ = app.emit(
                    "install-output",
                    "To fix permanently: run from source (npm run tauri dev), complete Onboarding once, or reinstall: pacman -Syu monarch-store",
                );
                let mut rx_refresh = helper_client::invoke_helper(
                    app,
                    helper_client::HelperCommand::ExecuteBatch {
                        manifest: crate::models::TransactionManifest {
                            refresh_db: true,
                            ..Default::default()
                        },
                    },
                    password.clone(),
                    one_click,
                )
                .await
                .map_err(|e| format!("Failed to invoke helper (refresh): {}", e))?;
                while let Some(msg) = rx_refresh.recv().await {
                    let _ = app.emit("install-output", &msg.message);
                }

                let mut rx_install = helper_client::invoke_helper(
                    app,
                    helper_client::HelperCommand::ExecuteBatch {
                        manifest: crate::models::TransactionManifest {
                            install_targets: vec![name.to_string()],
                            ..Default::default()
                        },
                    },
                    password.clone(),
                    one_click,
                )
                .await
                .map_err(|e| format!("Failed to invoke helper (install): {}", e))?;
                while let Some(msg) = rx_install.recv().await {
                    let _ = app.emit("install-output", &msg.message);
                    install_log.push(msg.message.clone());
                    if install_log.len() > LOG_CAP {
                        install_log.remove(0);
                    }
                    if msg.message.contains("Unrecognized archive format")
                        || msg.message.contains("could not open database")
                    {
                        saw_corrupt_db = true;
                    }
                }
            }
        }
    }

    // ✅ POST-INSTALL VERIFICATION (ALPM read-only; no shell)
    let mut verification = tokio::task::spawn_blocking({
        let pkg_name = name.to_string();
        move || crate::alpm_read::is_package_installed(&pkg_name)
    })
    .await
    .map_err(|e| format!("Verification task failed: {}", e))?;

    // Only retry with sync when failure suggests missing/stale package (sync might help).
    // Do NOT retry with sync for "could not satisfy dependencies" — that's a dependency resolution failure; syncing again won't fix it and wastes several minutes (user already synced at startup).
    let is_dependency_failure = install_log.iter().any(|m| {
        m.contains("could not satisfy dependencies") || m.contains("could not satisfy dependency")
    });
    let might_need_sync = install_log.iter().any(|m| {
        m.contains("not found in any enabled repository")
            || m.contains("target not found")
            || m.contains("no such package")
            || m.contains("could not find")
    });

    if !verification && source.source_type != "aur" && !saw_unknown_variant && is_dependency_failure
    {
        let _ = app.emit(
            "install-output",
            "Dependency resolution failed (sync already done at startup; skipping duplicate sync).",
        );
    }

    if !verification
        && source.source_type != "aur"
        && !saw_unknown_variant
        && might_need_sync
        && !is_dependency_failure
    {
        // DBs may be stale (e.g. sync at launch skipped). Retry once with sync.
        let _ = app.emit(
            "install-output",
            "Package not found; syncing databases and retrying...",
        );
        let all_repos_retry = repo_manager.get_all_repos().await;
        let enabled_repos_retry: Vec<String> = all_repos_retry
            .iter()
            .filter(|r| r.enabled)
            .map(|r| r.name.clone())
            .collect();
        let target_repo_retry = if source.source_type == "aur" {
            None
        } else {
            _repo_name.clone()
        };
        let mut rx_install = helper_client::invoke_helper(
            app,
            helper_client::HelperCommand::AlpmInstall {
                packages: vec![name.to_string()],
                sync_first: true,
                enabled_repos: enabled_repos_retry,
                cpu_optimization: cpu_optimization.clone(),
                target_repo: target_repo_retry,
            },
            password.clone(),
            one_click,
        )
        .await
        .map_err(|e| format!("Failed to invoke helper (install): {}", e))?;
        while let Some(msg) = rx_install.recv().await {
            let _ = app.emit("install-output", &msg.message);
            install_log.push(msg.message.clone());
            if install_log.len() > LOG_CAP {
                install_log.remove(0);
            }
            if msg.message.contains("Unrecognized archive format")
                || msg.message.contains("could not open database")
            {
                saw_corrupt_db = true;
            }
        }
        verification = tokio::task::spawn_blocking({
            let pkg_name = name.to_string();
            move || crate::alpm_read::is_package_installed(&pkg_name)
        })
        .await
        .map_err(|e| format!("Verification task failed: {}", e))?;
    }

    if !verification {
        let _ = app.emit("install-complete", "failed");
        if saw_corrupt_db {
            return Err("Sync databases are corrupt (Unrecognized archive format). Use Settings → System Management → Refresh Databases, then retry. If it still fails, run 'sudo pacman -Syy' once.".to_string());
        }
        // Surface the real ALPM error when package is not in any enabled repo
        let not_in_repo = install_log
            .iter()
            .find(|m| m.contains("not found in any enabled repository"));
        if let Some(msg) = not_in_repo {
            return Err(format!(
                "{} Try enabling Chaotic-AUR or another repo that provides this package, or install from AUR.",
                msg.trim()
            ));
        }
        if is_dependency_failure {
            // Surface the exact ALPM line (e.g. "Transaction preparation failed: ..." or "unable to satisfy dependency 'X' required by Y")
            let detail = install_log.iter().find(|m| {
                m.contains("Transaction preparation failed")
                    || m.contains("could not satisfy")
                    || m.contains("unable to satisfy")
                    || m.contains("breaks dependency")
            });
            let detail_str = detail
                .map(|s| s.trim().trim_start_matches("Error: ").to_string())
                .filter(|s| !s.is_empty());
            return Err(if let Some(d) = detail_str {
                format!(
                    "Dependencies could not be satisfied for '{}': {}. Try enabling more repos (e.g. multilib, Chaotic-AUR) or install the missing dependency first.",
                    name, d
                )
            } else {
                format!(
                    "Dependencies could not be satisfied for '{}'. A required dependency may be missing from your enabled repos, or there may be a version conflict. Check the log above or try: pacman -S {}",
                    name, name
                )
            });
        }
        return Err(format!(
            "Package '{}' could not be installed. Check the log above for details.",
            name
        ));
    }

    invalidate_installed_catalog_cache();
    let _ = app.emit("install-complete", "success");

    // Process notification & telemetry
    // Only send system notification if enabled
    if repo_manager.is_notifications_enabled().await {
        crate::commands::system::show_desktop_notification_safe(
            app,
            "✨ MonArch: Installation Complete".to_string(),
            format!("Successfully installed '{}'", name),
        )
        .await;
    }

    crate::utils::track_event_safe(
        app,
        "install_package",
        Some(serde_json::json!({
            "pkg": name,
            "source": format!("{:?}", source),
            "success": true,
        })),
    )
    .await;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn uninstall_package(
    app: AppHandle,
    state_repo: State<'_, RepoManager>,
    name: String,
    source: Option<models::PackageSource>,
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

    // ✅ Flatpak Support: emit install-complete so UI leaves "running" state
    if let Some(src) = &source {
        if src.source_type == "flatpak" {
            match crate::flathub_api::remove_flatpak(app.clone(), name.clone()).await {
                Ok(()) => {
                    invalidate_installed_catalog_cache();
                    let _ = app.emit("install-complete", "success");
                    crate::utils::track_event_safe(
                        &app,
                        "uninstall_package",
                        Some(serde_json::json!({
                            "pkg": name,
                            "success": true,
                        })),
                    )
                    .await;
                    return Ok(());
                }
                Err(e) => {
                    let _ = app.emit("install-output", format!("Flatpak uninstall failed: {}", e));
                    let _ = app.emit("install-complete", "failed");
                    return Err(e);
                }
            }
        }
    }

    let one_click = state_repo.inner().is_one_click_enabled().await;
    // ✅ Native ALPM Support
    let mut rx = helper_client::invoke_helper(
        &app,
        helper_client::HelperCommand::AlpmUninstall {
            packages: vec![name.clone()],
            remove_deps: true, // -Rns behavior
        },
        password.clone(),
        one_click,
    )
    .await
    .map_err(|e| format!("Failed to invoke helper: {}", e))?;

    // Stream progress events
    while let Some(msg) = rx.recv().await {
        let _ = app.emit("install-output", &msg.message);
    }

    // ✅ POST-UNINSTALL VERIFICATION (ALPM read-only; no shell)
    let verification = tokio::task::spawn_blocking({
        let pkg_name = name.clone();
        move || crate::alpm_read::is_package_installed(&pkg_name)
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

    invalidate_installed_catalog_cache();
    let _ = app.emit("install-complete", "success");

    crate::utils::track_event_safe(
        &app,
        "uninstall_package",
        Some(serde_json::json!({
            "pkg": name,
            "success": true,
        })),
    )
    .await;

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
    let mut stack = std::collections::HashSet::new();

    resolve_aur_dependencies(app, name, &mut resolved, &mut visited, &mut stack, 0).await?;

    if resolved.len() > 1 {
        let _ = app.emit(
            "install-output",
            format!("Building {} AUR dependencies...", resolved.len() - 1),
        );
    }

    let mut built_paths = Vec::new();
    for pkg_name in resolved {
        let paths = build_aur_package_single(app, &pkg_name, password).await?;
        built_paths.extend(paths);
    }

    Ok(built_paths)
}

async fn build_aur_package_single(
    app: &AppHandle,
    name: &str,
    password: &Option<String>,
) -> Result<Vec<String>, String> {
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

    // 3. Create transient Sudo Askpass script if password is provided.
    // Password lives only in this temp dir and is removed when the function returns (temp dir dropped).
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

    // SECURITY (AUR / Arch Packaging): makepkg must NEVER run as root (instant ban risk).
    // We explicitly refuse if effective UID is root; we do not "drop" privileges because
    // the GUI runs as the user—only root would trigger this check.
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
    // When no password: close stdin so makepkg never blocks on read (e.g. prompts).
    let stdin_mode = if password.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    makepkg
        .args(["-s", "-r", "--noconfirm", "--needed"]) // -r: remove make-deps after build (avoid orphan build libs)
        .env("MAKEFLAGS", format!("-j{}", num_cpus::get()))
        .env("PKGEXT", ".pkg.tar.zst")
        .current_dir(&pkg_dir)
        .stdin(stdin_mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Inject Askpass redirection or pkexec for pacman (makepkg installs build deps as root).
    // We use pkexec pacman directly; monarch-helper does not support RunCommand, and the
    // wrapper path would fail with "unknown variant". Polkit will prompt once per build.
    if let Some(ref ap) = askpass_path {
        makepkg.env("SUDO_ASKPASS", ap);
        makepkg.env("PACMAN", "sudo -A pacman");
    } else {
        makepkg.env("PACMAN", "pkexec pacman");
    }

    let mut child = makepkg.spawn().map_err(|e| e.to_string())?;

    // Track PID so abort_installation can kill AUR builds
    if let Some(pid) = child.id() {
        ACTIVE_AUR_BUILD_PID.store(pid, Ordering::SeqCst);
    }

    if let Some(pwd) = password {
        if let Some(mut stdin) = child.stdin.take() {
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut stdin, format!("{}\n", pwd).as_bytes())
                    .await;
        }
    }

    let missing_keys = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
    let build_errors = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    {
        let mut active = ACTIVE_INSTALL_PROCESS.lock().await;
        *active = Some(child);
    }

    if let Some(out) = stdout {
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
    let stderr_handle = if let Some(err) = stderr {
        let a = app.clone();
        Some(tokio::spawn(async move {
            let mut reader = TokioBufReader::new(err).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = a.emit("install-output", format!("MAKEPKG: {}", line));

                if line.contains('%')
                    || (line.len() > 10
                        && line
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_digit() || c.is_whitespace()))
                {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(first) = parts.first() {
                        if let Ok(pct) = first.parse::<u8>() {
                            if pct <= 100 {
                                let _ = a.emit(
                                    "update-progress",
                                    serde_json::json!({
                                        "phase": "download",
                                        "progress": pct,
                                        "message": format!("Downloading AUR sources... {}%", pct)
                                    }),
                                );
                            }
                        }
                    }
                }

                if line.contains("unknown public key")
                    || line.contains("not found in keychain")
                    || line.contains("FAILED (unknown public key")
                    || line.contains("could not be verified")
                {
                    let words: Vec<&str> = line.split_whitespace().collect();
                    for (i, word) in words.iter().enumerate() {
                        let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
                        if clean.len() >= 8 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
                            let mut keys = missing_keys_clone.lock().await;
                            if !keys.contains(&clean.to_string()) {
                                keys.push(clean.to_string());
                            }
                        }
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

                if line.contains("ERROR:") {
                    let mut errs = build_errors_clone.lock().await;
                    errs.push(line.clone());
                }
            }
        }))
    } else {
        None
    };

    let exit_status = {
        let mut active = ACTIVE_INSTALL_PROCESS.lock().await;
        if let Some(mut c) = active.take() {
            drop(active);
            c.wait().await.map_err(|e| e.to_string())?
        } else {
            let _ = app.emit("install-output", "--- Build aborted by user ---");
            return Err("Build aborted by user.".to_string());
        }
    };

    if let Some(h) = stderr_handle {
        let _ = h.await;
    }

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

                // Retry makepkg (stdin closed so it never blocks on read)
                let mut retry_makepkg = tokio::process::Command::new("makepkg");
                retry_makepkg
                    .args(["-s", "-r", "--noconfirm", "--needed"]) // -r: remove make-deps after build
                    .env("MAKEFLAGS", format!("-j{}", num_cpus::get()))
                    .env("PKGEXT", ".pkg.tar.zst")
                    .current_dir(&pkg_dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                if let Some(ref ap) = askpass_path {
                    retry_makepkg.env("SUDO_ASKPASS", ap);
                    retry_makepkg.env("PACMAN", "sudo -A pacman");
                } else {
                    retry_makepkg.env("PACMAN", "pkexec pacman");
                }

                let mut retry_child = retry_makepkg.spawn().map_err(|e| e.to_string())?;
                let retry_stdout = retry_child.stdout.take();
                let retry_stderr = retry_child.stderr.take();
                {
                    let mut active = ACTIVE_INSTALL_PROCESS.lock().await;
                    *active = Some(retry_child);
                }
                if let Some(out) = retry_stdout {
                    let a = app.clone();
                    tokio::spawn(async move {
                        let reader = TokioBufReader::new(out);
                        let mut lines = reader.lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = a.emit("install-output", line);
                        }
                    });
                }
                if let Some(err) = retry_stderr {
                    let a = app.clone();
                    tokio::spawn(async move {
                        let reader = TokioBufReader::new(err).lines();
                        let mut lines = reader;
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = a.emit("install-output", format!("MAKEPKG: {}", line));
                        }
                    });
                }
                let retry_status = {
                    let mut active = ACTIVE_INSTALL_PROCESS.lock().await;
                    if let Some(mut c) = active.take() {
                        drop(active);
                        c.wait().await.map_err(|e| e.to_string())?
                    } else {
                        let _ = app.emit("install-output", "--- Build aborted by user ---");
                        return Err("Build aborted by user.".to_string());
                    }
                };

                if !retry_status.success() {
                    let errs = build_errors.lock().await;
                    let err_summary = if errs.is_empty() {
                        "Build failed after key import. Check logs for details.".to_string()
                    } else {
                        let last = errs.last().cloned().unwrap_or_default();
                        if last.to_lowercase().contains("unknown error has occurred") {
                            "AUR build failed: makepkg reported an unknown error. Ensure base-devel and git are installed; run scripts/monarch-permission-sanitizer.sh to fix build cache permissions.".to_string()
                        } else {
                            last
                        }
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
            // Non-PGP build failure — surface descriptive message for makepkg "unknown error"
            let errs = build_errors.lock().await;
            let err_summary = if errs.is_empty() {
                "makepkg build failed. Check logs for details.".to_string()
            } else {
                let last = errs.last().cloned().unwrap_or_default();
                if last.to_lowercase().contains("unknown error has occurred") {
                    "AUR build failed: makepkg reported an unknown error. Ensure base-devel and git are installed; run scripts/monarch-permission-sanitizer.sh to fix build cache permissions.".to_string()
                } else {
                    last
                }
            };
            ACTIVE_AUR_BUILD_PID.store(0, Ordering::SeqCst);
            return Err(err_summary);
        }
    }

    // Collect all built packages (supports split packages: multiple .pkg.tar.zst per PKGBUILD)
    let mut artifacts = Vec::new();
    let mut dir = tokio::fs::read_dir(&pkg_dir)
        .await
        .map_err(|e| e.to_string())?;
    while let Some(entry) = dir.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "zst" && path.to_string_lossy().contains(".pkg.tar.") {
                artifacts.push(path.to_string_lossy().to_string());
            }
        }
    }
    if artifacts.is_empty() {
        ACTIVE_AUR_BUILD_PID.store(0, Ordering::SeqCst);
        return Err(format!("Could not find built package in {:?}", pkg_dir));
    }
    ACTIVE_AUR_BUILD_PID.store(0, Ordering::SeqCst);
    Ok(artifacts)
}

use futures::future::{BoxFuture, FutureExt};

const AUR_DEPENDENCY_MAX_DEPTH: u32 = 64;

pub fn resolve_aur_dependencies<'a>(
    app: &'a AppHandle,
    name: &'a str,
    resolved: &'a mut Vec<String>,
    visited: &'a mut std::collections::HashSet<String>,
    stack: &'a mut std::collections::HashSet<String>,
    depth: u32,
) -> BoxFuture<'a, Result<(), String>> {
    async move {
        if depth > AUR_DEPENDENCY_MAX_DEPTH {
            return Err(
                "AUR dependency depth exceeded (max 64). Possible cycle or very deep tree."
                    .to_string(),
            );
        }
        if stack.contains(name) {
            return Err(format!(
                "Cycle detected in AUR dependencies involving '{}'.",
                name
            ));
        }
        if visited.contains(name) {
            return Ok(());
        }
        visited.insert(name.to_string());
        stack.insert(name.to_string());

        let _ = app.emit(
            "install-output",
            format!("Checking dependencies for {}...", name),
        );

        let names = [name];
        let info = aur_api::get_multi_info(&names[..]).await?;
        let pkg = match info.first() {
            Some(p) => p,
            _ => {
                stack.remove(name);
                return Err(format!("Package {} not found in AUR", name));
            }
        };

        let mut all_deps: Vec<String> = Vec::new();
        if let Some(deps) = &pkg.depends {
            all_deps.extend(deps.clone());
        }
        if let Some(deps) = &pkg.make_depends {
            all_deps.extend(deps.clone());
        }

        for dep_entry in all_deps {
            let dep_name = dep_entry
                .split(['=', '>', '<'])
                .next()
                .unwrap_or(&dep_entry)
                .trim();

            if is_package_satisfied(dep_name).await {
                continue;
            }
            if is_in_official_repos(dep_name).await {
                continue;
            }

            if let Err(e) =
                resolve_aur_dependencies(app, dep_name, resolved, visited, stack, depth + 1).await
            {
                stack.remove(name);
                return Err(e);
            }
        }

        stack.remove(name);
        if !resolved.contains(&name.to_string()) {
            resolved.push(name.to_string());
        }

        Ok(())
    }
    .boxed()
}

async fn is_package_satisfied(name: &str) -> bool {
    let name = name.to_string();
    tokio::task::spawn_blocking(move || crate::alpm_read::is_dep_satisfied(&name))
        .await
        .unwrap_or(false)
}

/// Returns true if the package exists in any sync database (official or enabled repos).
/// Used to avoid building from AUR when the package is available as pre-built in Chaotic/CachyOS/etc.
pub(crate) async fn is_in_sync_repos(name: &str) -> bool {
    let name = name.to_string();
    tokio::task::spawn_blocking(move || crate::alpm_read::is_package_in_syncdb(&name))
        .await
        .unwrap_or(false)
}

async fn is_in_official_repos(name: &str) -> bool {
    is_in_sync_repos(name).await
}

pub fn audit_aur_builder_deps(app: &AppHandle) -> Result<(), String> {
    let deps = ["base-devel", "git"];
    for dep in deps {
        let has_dep = crate::alpm_read::is_package_installed(dep);
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
#[specta::specta]
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

async fn get_installed_packages_legacy(
    state: &crate::metadata::MetadataState,
    state_registry: &crate::registry::RegistryState,
) -> Vec<InstalledPackage> {
    let native_pkgs = crate::alpm_read::get_installed_packages_native();
    let mut apps = Vec::new();

    if let Ok(loader) = state.loader.lock() {
        for pkg in native_pkgs {
            let icon = loader.find_icon_heuristic(&pkg.name);
            let has_icon = icon.is_some();
            let has_id = loader.find_app_id(&pkg.name).is_some();

            if has_icon || has_id {
                let display_name = loader
                    .find_package(&pkg.name)
                    .map(|meta| meta.name.clone())
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| Some(crate::utils::to_pretty_name(&pkg.name)));
                let source_label = crate::utils::installed_source_for_package(&pkg.name, None)
                    .map(|source| source.label)
                    .or_else(|| Some("System package".to_string()));

                apps.push(InstalledPackage {
                    name: pkg.name.clone(),
                    version: pkg.version,
                    description: pkg.description,
                    install_date: None,
                    install_date_unix: None,
                    size: pkg
                        .installed_size
                        .map(|s| format!("{} MB", s / (1024 * 1024))),
                    size_bytes: pkg.installed_size,
                    url: None,
                    repository: source_label.as_ref().map(|_| "repo".to_string()),
                    source_label,
                    resolved_source: crate::utils::installed_source_for_package(&pkg.name, None),
                    display_name,
                    launchable: true,
                    icon,
                });
            }
        }
    }

    let flatpaks = tokio::time::timeout(
        StdDuration::from_secs(3),
        crate::flathub_api::get_installed_flatpaks_detailed(),
    )
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default();

    for fp in flatpaks {
        let flatpak_version = fp.version.clone();
        let mut icon = None;
        let mut description = fp.summary.clone();
        let canonical = crate::utils::canonical_merge_key(&fp.name, Some(&fp.app_id));

        if let Ok(entries) = state_registry.manager.get_packages_by_canonical_ids(&[canonical.clone(),
            fp.app_id.to_lowercase(),
            fp.app_id.clone()]) {
            if let Some(cached) = entries.into_iter().next() {
                icon = cached.icon;
                if !cached.description.is_empty() {
                    description = cached.description;
                }
            }
        }

        if icon.is_none() {
            if let Ok(loader) = state.loader.lock() {
                icon = loader
                    .find_icon_heuristic(&fp.app_id)
                    .or_else(|| loader.find_icon_heuristic(&fp.name))
                    .or_else(|| loader.find_icon_heuristic(&canonical));
                if description.trim().is_empty() {
                    description = loader
                        .find_package(&fp.app_id)
                        .or_else(|| loader.find_package(&fp.name))
                        .or_else(|| loader.find_package(&canonical))
                        .and_then(|meta| meta.summary)
                        .unwrap_or(description);
                }
            }
        }

        apps.push(InstalledPackage {
            name: fp.app_id.clone(),
            version: flatpak_version.clone(),
            description,
            install_date: None,
            install_date_unix: None,
            size: None,
            size_bytes: None,
            url: None,
            repository: Some("flathub".to_string()),
            source_label: Some("Flatpak".to_string()),
            resolved_source: Some(models::PackageSource::new_with_name(
                "flatpak",
                "flathub",
                &flatpak_version,
                "Flatpak",
                &fp.app_id,
            )),
            display_name: Some(fp.name.clone()),
            launchable: true,
            icon,
        });
    }

    apps
}

#[tauri::command]
#[specta::specta]
pub async fn get_installed_catalog(
    state: tauri::State<'_, crate::metadata::MetadataState>,
    state_registry: tauri::State<'_, crate::registry::RegistryState>,
) -> Result<Vec<models::Package>, String> {
    if let Ok(cache) = INSTALLED_CATALOG_CACHE.lock() {
        if let Some((created_at, snapshot)) = cache.as_ref() {
            if created_at.elapsed() < INSTALLED_CATALOG_TTL {
                log::debug!(
                    "[INSTALLED-CATALOG] cache hit: {} packages",
                    snapshot.len()
                );
                let mut snapshot = snapshot.clone();
                crate::utils::finalize_packages_contract(&mut snapshot);
                return Ok(snapshot);
            }
        }
    }

    let _guard = INSTALLED_CATALOG_GATE.lock().await;
    if let Ok(cache) = INSTALLED_CATALOG_CACHE.lock() {
        if let Some((created_at, snapshot)) = cache.as_ref() {
            if created_at.elapsed() < INSTALLED_CATALOG_TTL {
                log::debug!(
                    "[INSTALLED-CATALOG] cache hit after gate: {} packages",
                    snapshot.len()
                );
                let mut snapshot = snapshot.clone();
                crate::utils::finalize_packages_contract(&mut snapshot);
                return Ok(snapshot);
            }
        }
    }

    let started = Instant::now();
    let mut packages = get_installed_packages_legacy(state.inner(), state_registry.inner())
        .await
        .into_iter()
        .map(|pkg| {
            let resolved_source = pkg.resolved_source.clone();
            let source_type = resolved_source
                .as_ref()
                .map(|source| source.source_type.clone())
                .unwrap_or_else(|| {
                    if pkg.repository.as_deref() == Some("flathub") {
                        "flatpak".to_string()
                    } else {
                        "repo".to_string()
                    }
                });
            let source_id = resolved_source
                .as_ref()
                .map(|source| source.id.clone())
                .or_else(|| pkg.repository.clone())
                .unwrap_or_else(|| "local".to_string());
            let source_label = resolved_source
                .as_ref()
                .map(|source| source.label.clone())
                .or_else(|| pkg.source_label.clone())
                .unwrap_or_else(|| "Installed".to_string());
            let source_package_name = resolved_source
                .as_ref()
                .and_then(|source| source.package_name.clone())
                .unwrap_or_else(|| pkg.name.clone());
            let package_source = models::PackageSource::new_with_name(
                &source_type,
                &source_id,
                &pkg.version,
                &source_label,
                &source_package_name,
            );
            models::Package {
                name: pkg.name.clone(),
                display_name: pkg.display_name.clone(),
                display_title: pkg.display_name.clone(),
                description: pkg.description.clone(),
                version: pkg.version.clone(),
                source: package_source.clone(),
                icon: pkg.icon.clone(),
                installed: true,
                installed_size_bytes: pkg.size_bytes,
                installed_size: pkg.size_bytes,
                available_sources: Some(vec![package_source]),
                canonical_id: crate::utils::canonical_merge_key(
                    &pkg.name,
                    if source_type == "flatpak" { Some(&pkg.name) } else { None },
                ),
                installed_sources: Some(vec![pkg.name.clone()]),
                launch_target: Some(pkg.name.clone()),
                app_id: if source_type == "flatpak" {
                    Some(pkg.name.clone())
                } else {
                    None
                },
                ..Default::default()
            }
        })
        .collect::<Vec<_>>();
    crate::utils::finalize_packages_contract(&mut packages);
    log::info!(
        "[INSTALLED-CATALOG] loaded {} packages in {} ms",
        packages.len(),
        started.elapsed().as_millis()
    );

    if let Ok(mut cache) = INSTALLED_CATALOG_CACHE.lock() {
        *cache = Some((Instant::now(), packages.clone()));
    }

    Ok(packages)
}

#[tauri::command]
#[specta::specta]
pub async fn get_installed_packages(
    state: tauri::State<'_, crate::metadata::MetadataState>,
    _state_flathub: tauri::State<'_, crate::flathub_api::FlathubApiClient>,
    state_registry: tauri::State<'_, crate::registry::RegistryState>,
) -> Result<Vec<InstalledPackage>, String> {
    Ok(get_installed_packages_legacy(state.inner(), state_registry.inner()).await)
}

#[tauri::command]
#[specta::specta]
pub async fn check_for_updates(
    _app: AppHandle,
    _state: tauri::State<'_, crate::metadata::MetadataState>,
    state_repo: State<'_, RepoManager>,
) -> Result<Vec<PendingUpdate>, String> {
    // 1. Get Official updates via Helper "Safe Check" (avoids DB lock, creates temp env)
    let mut updates = Vec::new();

    // We pass explicit repos if we want, or let helper use default config.
    // Helper expects enabled_repos. We'll use "core", "extra", "multilib" + "cachyos/chaotic" if detected.
    // But getting enabled repos from RepoManager needs async state access.
    // For now, let's pass a list of known standard repos to ensure they are checked.
    // Or, we can update the helper call to be smart.
    // Actually, passing an empty list to CheckUpdatesSafe in my implementation (transactions.rs)
    // effectively meant loop 0 times? NO, I fixed that in step 190?
    // Wait, in step 190 `extract_repos_from_config` is called if enabled_repos is empty?
    // No, `force_refresh` calls `extract`. `execute_alpm_sync` iterates input.
    // So I MUST pass the list of repos.

    // Determine enabled repos from config (best effort from Tauri side or hardcode common ones)
    // The Helper is better suited to read config, but it requires us to pass them.
    // Let's read pacman.conf here? No, redundant.
    // Let's assume standard Arch repos + common ones.
    let standard_repos = vec![
        "core".to_string(),
        "extra".to_string(),
        "multilib".to_string(),
        "cachyos".to_string(),
        "cachyos-v3".to_string(),
        "cachyos-v4".to_string(),
        "chaotic-aur".to_string(),
        "now-testing".to_string(),
    ];

    let one_click = state_repo.inner().is_one_click_enabled().await;
    // Invoke Helper
    match crate::helper_client::invoke_helper(
        &_app,
        crate::helper_client::HelperCommand::CheckUpdatesSafe {
            enabled_repos: standard_repos,
        },
        None,
        one_click,
    )
    .await
    {
        Ok(mut rx) => {
            while let Some(msg) = rx.recv().await {
                // Helper emits event_type="package_found" with message "Update available: name old -> new"
                // Parse the message string.
                if msg.message.starts_with("Update available:") {
                    // Format: "Update available: <name> <old> -> <new>"
                    let parts: Vec<&str> = msg.message.split_whitespace().collect();
                    if parts.len() >= 6 {
                        // "Update", "available:", "name", "old", "->", "new"
                        updates.push(PendingUpdate {
                            name: parts[2].to_string(),
                            old_version: parts[3].to_string(),
                            new_version: parts[5].to_string(),
                            repo: "official".to_string(),
                        });
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Safe update check failed: {}", e);
            // Fallback to empty updates or previous method?
            // Returning error is honest.
            return Err(e);
        }
    }

    let mut all_updates = updates;

    // 2. Get AUR updates locally (unprivileged)
    if let Ok(aur_updates) = check_aur_updates().await {
        all_updates.extend(aur_updates);
    }

    Ok(all_updates)
}

async fn check_aur_updates() -> Result<Vec<PendingUpdate>, String> {
    // ALPM read-only: foreign packages (not in sync DB) = AUR candidates
    let (installed_aur, names) = tokio::task::spawn_blocking(|| {
        let foreign = crate::alpm_read::get_foreign_installed_packages();
        let mut installed_aur = std::collections::HashMap::new();
        let mut names = Vec::new();
        for (name, version) in foreign {
            // Distro-Aware: exclude if package now exists in a sync repo
            if !crate::alpm_read::is_package_in_syncdb(&name) {
                installed_aur.insert(name.clone(), version);
                names.push(name);
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
#[specta::specta]
pub async fn get_orphans() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(crate::alpm_read::get_orphans_native)
        .await
        .map_err(|e| format!("Task join error: {}", e))
}

#[tauri::command]
#[specta::specta]
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
#[specta::specta]
pub async fn check_installed_status(
    state: State<'_, crate::metadata::MetadataState>,
    name: String,
) -> Result<PackageInstallStatus, String> {
    // 1. ALPM (native) check: resolve display name to package name if needed,
    // then try known repo aliases for the same canonical product.
    let resolved_name = state
        .loader
        .lock()
        .ok()
        .map(|loader| loader.resolve_package_name(&name))
        .unwrap_or_else(|| name.clone());

    let canonical = crate::utils::canonical_merge_key(&resolved_name, None);
    let mut candidate_names = vec![resolved_name.clone()];
    for alias in crate::utils::canonical_to_repo_lookup_names(&canonical) {
        let alias_name = alias.to_string();
        if !candidate_names.contains(&alias_name) {
            candidate_names.push(alias_name);
        }
    }

    for candidate in candidate_names {
        if let Some(pkg) = crate::alpm_read::get_package_native(&candidate) {
            // Only finalize here when ALPM confirms this package is installed.
            // If it's merely available in sync DBs, continue to Flatpak detection below.
            if pkg.installed {
                return Ok(PackageInstallStatus {
                    installed: true,
                    version: Some(pkg.version),
                    repo: None,
                    source: Some(pkg.source),
                    actual_package_name: Some(candidate),
                });
            }
        }
    }

    // 2. Flatpak check: so Launch and conflict UI work for Flatpak-installed apps
    if let Ok(ids) = crate::flathub_api::get_installed_flatpak_app_ids().await {
        let name_lower = name.to_lowercase();
        // If name looks like an app ID (contains dot), check exact match (case-insensitive)
        let installed_id = if name_lower.contains('.') {
            ids.iter()
                .find(|id| id.to_lowercase() == name_lower)
                .cloned()
        } else {
            // Robust check:
            // 1. Try resolving simple name directly
            // 2. Try resolving canonical name (handles -bin, -git, etc.)
            let canonical = crate::utils::canonical_merge_key(&name_lower, None);

            crate::flathub_api::get_flathub_app_id(&name_lower)
                .or_else(|| crate::flathub_api::get_flathub_app_id(&canonical))
                .and_then(|app_id| {
                    ids.iter()
                        .find(|id| id.eq_ignore_ascii_case(&app_id))
                        .cloned()
                })
        };
        if let Some(app_id) = installed_id {
            let flatpak_source = models::PackageSource::new(
                "flatpak",
                "flathub",
                "installed",
                "Flatpak (Sandboxed)",
            );
            return Ok(PackageInstallStatus {
                installed: true,
                version: None, // Could run flatpak info for version if needed
                repo: Some("flathub".to_string()),
                source: Some(flatpak_source),
                actual_package_name: Some(app_id),
            });
        }
    }

    Ok(PackageInstallStatus {
        installed: false,
        version: None,
        repo: None,
        source: None,
        actual_package_name: None,
    })
}

/// Top 32 common apps for Linux/Arch users. Used when remote list is unavailable.
/// Order: Web Browsers & Communication, Office & Productivity, Graphics & Design, Multimedia & Audio.
const DEFAULT_ESSENTIALS: &[&str] = &[
    "firefox",
    "librewolf-bin",
    "google-chrome",
    "thunderbird",
    "telegram-desktop",
    "signal-desktop",
    "discord",
    "newsflash",
    "libreoffice-fresh",
    "obsidian",
    "calibre",
    "simplenote-electron-bin",
    "okular",
    "foliate",
    "keepassxc",
    "gimp",
    "inkscape",
    "blender",
    "flameshot",
    "krita",
    "rawtherapee",
    "vlc",
    "audacity",
    "obs-studio",
    "handbrake",
    "strawberry",
    "easyeffects",
    "ardour",
    "visual-studio-code-bin",
    "git",
    "docker-desktop",
    "steam",
    "lutris",
    "heroic-games-launcher-bin",
    "timeshift",
    "bitwarden-bin",
    "gparted",
    "kdeconnect",
    "balena-etcher",
    "peazip-bin",
];

// Update max limit to accommodate slightly larger initial list
const ESSENTIALS_MAX: usize = 40;

/// URL for essentials list (updated over time without app release). Cache TTL 7 days.
const ESSENTIALS_JSON_URL: &str =
    "https://raw.githubusercontent.com/cpg716/monarch-store/main/docs/essentials.json";
const ESSENTIALS_CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60; // 7 days
/// Cap for combined essentials ∪ featured list (homepage discovery pool).
const COMBINED_ESSENTIALS_MAX: usize = 120;

/// Merges resolved essentials with all featured names (per category). Single discovery pool.
fn merge_essentials_with_featured(mut list: Vec<String>) -> Vec<String> {
    let mut seen: std::collections::HashSet<String> = list.iter().cloned().collect();
    for n in crate::discovery_manager::get_all_featured_names() {
        if seen.insert(n.clone()) {
            list.push(n);
        }
    }
    list.into_iter().take(COMBINED_ESSENTIALS_MAX).collect()
}

#[derive(serde::Serialize, serde::Deserialize)]
struct EssentialsCache {
    packages: Vec<String>,
    fetched_at: u64,
    /// When set, category view uses these for Featured; otherwise built-in lists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    featured_by_category: Option<std::collections::HashMap<String, Vec<String>>>,
}

pub(crate) async fn resolve_essentials_list(
    state_repo: &RepoManager,
) -> Result<Vec<String>, String> {
    // 1. System override: power users / distro packagers
    let db_path = std::path::Path::new("/var/lib/monarch/dbs/essentials.db");
    if db_path.exists() {
        if let Ok(content) = std::fs::read_to_string(db_path) {
            let custom_lines: Vec<String> = content
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect();
            if !custom_lines.is_empty() {
                let list: Vec<String> = custom_lines.into_iter().take(ESSENTIALS_MAX).collect();
                let raw_list = merge_essentials_with_featured(list);
                let normalized: Vec<String> = raw_list
                    .into_iter()
                    .map(|name| crate::utils::canonical_merge_key(&name, None))
                    .collect();
                return Ok(normalized);
            }
        }
    }

    // 2. Remote list with cache (updated over time without app release)
    if let Some(cache_dir) = dirs::cache_dir() {
        let cache_path = cache_dir.join("monarch-store").join("essentials_v7.json");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Use cache if fresh
        if cache_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&cache_path) {
                if let Ok(cache) = serde_json::from_str::<EssentialsCache>(&data) {
                    if now.saturating_sub(cache.fetched_at) < ESSENTIALS_CACHE_TTL_SECS {
                        let list: Vec<String> =
                            cache.packages.into_iter().take(ESSENTIALS_MAX).collect();
                        if !list.is_empty() {
                            let raw_list = merge_essentials_with_featured(list);
                            let normalized: Vec<String> = raw_list
                                .into_iter()
                                .map(|name| crate::utils::canonical_merge_key(&name, None))
                                .collect();
                            return Ok(normalized);
                        }
                    }
                }
            }
        }

        // Fetch from remote (supports flat array or { packages, featured_by_category? })
        let fetch = tokio::time::timeout(
            std::time::Duration::from_secs(4),
            reqwest::get(ESSENTIALS_JSON_URL),
        )
        .await;
        if let Ok(Ok(resp)) = fetch {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    let mut packages: Vec<String> = Vec::new();
                    let mut featured_by_category: Option<
                        std::collections::HashMap<String, Vec<String>>,
                    > = None;

                    if let Ok(arr) = serde_json::from_slice::<Vec<String>>(&bytes) {
                        packages = arr.into_iter().take(ESSENTIALS_MAX).collect();
                    } else if let Ok(obj) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let Some(arr) = obj.get("packages").and_then(|v| v.as_array()) {
                            packages = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .take(ESSENTIALS_MAX)
                                .collect();
                        }
                        if let Some(map) =
                            obj.get("featured_by_category").and_then(|v| v.as_object())
                        {
                            let mut fbc = std::collections::HashMap::new();
                            for (k, v) in map {
                                if let Some(arr) = v.as_array() {
                                    let list: Vec<String> = arr
                                        .iter()
                                        .filter_map(|x| x.as_str().map(String::from))
                                        .collect();
                                    if !list.is_empty() {
                                        fbc.insert(k.to_lowercase(), list);
                                    }
                                }
                            }
                            if !fbc.is_empty() {
                                featured_by_category = Some(fbc);
                            }
                        }
                    }

                    if !packages.is_empty() {
                        let _ = std::fs::create_dir_all(cache_path.parent().unwrap());
                        let cache = EssentialsCache {
                            packages: packages.clone(),
                            fetched_at: now,
                            featured_by_category: featured_by_category.clone(),
                        };
                        if let Ok(json) = serde_json::to_string(&cache) {
                            let _ = std::fs::write(&cache_path, json);
                        }
                        let raw_list = merge_essentials_with_featured(packages);
                        let normalized: Vec<String> = raw_list
                            .into_iter()
                            .map(|name| crate::utils::canonical_merge_key(&name, None))
                            .collect();
                        return Ok(normalized);
                    }
                }
            }
        }
    }

    // 3. Built-in default (top 24)
    let mut unique = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for pkg in DEFAULT_ESSENTIALS.iter().take(ESSENTIALS_MAX) {
        if seen.insert(*pkg) {
            unique.push((*pkg).to_string());
        }
    }

    // 4. CachyOS extras when repo enabled (append, still cap at ESSENTIALS_MAX)
    if state_repo.is_repo_enabled("cachyos").await {
        for pkg in &["cachyos-settings", "linux-cachyos", "paru"] {
            if seen.insert(*pkg) && unique.len() < ESSENTIALS_MAX {
                unique.push((*pkg).to_string());
            }
        }
    }

    // Normalize all names to canonical IDs so they match the Registry keys
    // (e.g. "telegram-desktop" -> "telegram")
    let raw_list = merge_essentials_with_featured(unique);
    let normalized: Vec<String> = raw_list
        .into_iter()
        .map(|name| crate::utils::canonical_merge_key(&name, None))
        .collect();

    Ok(normalized)
}

#[tauri::command]
#[specta::specta]
pub async fn get_essentials_list(
    state_repo: State<'_, RepoManager>,
) -> Result<Vec<String>, String> {
    resolve_essentials_list(state_repo.inner()).await
}

#[tauri::command]
#[specta::specta]
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
#[specta::specta]
pub async fn get_pacnew_warnings() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("find")
        .args(["/etc", "-name", "*.pacnew"])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn get_cache_stats() -> Result<models::CacheStats, String> {
    if let Ok(cache) = CACHE_STATS_CACHE.lock() {
        if let Some((ts, stats)) = cache.as_ref() {
            if ts.elapsed() < CACHE_STATS_TTL {
                return Ok(stats.clone());
            }
        }
    }

    log::info!("Calculating package cache stats...");
    let stats = compute_cache_stats();

    if let Ok(mut cache) = CACHE_STATS_CACHE.lock() {
        *cache = Some((Instant::now(), stats.clone()));
    }

    Ok(stats)
}

fn compute_cache_stats() -> models::CacheStats {
    let cache_dir = "/var/cache/pacman/pkg";
    let mut total_size = 0;
    let mut pkg_count = 0;

    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(meta) = entry.metadata() {
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if ext == "zst" || ext == "xz" || ext == "sig" {
                        total_size += meta.len();
                        pkg_count += 1;
                    }
                }
            }
        }
    }

    models::CacheStats {
        total_size_bytes: total_size,
        package_count: pkg_count,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn clean_package_cache(
    app: AppHandle,
    state_repo: State<'_, RepoManager>,
    password: Option<String>,
    keep_versions: u32,
) -> Result<(), String> {
    log::info!(
        "Cleaning package cache (keep_versions: {})...",
        keep_versions
    );
    let one_click = state_repo.inner().is_one_click_enabled().await;
    let mut rx = helper_client::invoke_helper(
        &app,
        helper_client::HelperCommand::AlpmCleanCache { keep_versions },
        password,
        one_click,
    )
    .await?;

    while let Some(msg) = rx.recv().await {
        app.emit("package-cache-progress", msg.clone())
            .map_err(|e| e.to_string())?;
    }

    if let Ok(mut cache) = CACHE_STATS_CACHE.lock() {
        *cache = None;
    }

    Ok(())
}
#[tauri::command]
#[specta::specta]
pub async fn check_services_restart() -> Result<Vec<String>, String> {
    log::info!("Checking for services that require restart...");
    // Attempt use needrestart if available, wrapped in a strict 10-second timeout
    let timeout_duration = std::time::Duration::from_secs(10);

    let process = tokio::process::Command::new("needrestart")
        .arg("-b") // Batch mode
        .output();

    match tokio::time::timeout(timeout_duration, process).await {
        Ok(Ok(o)) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            // Parse needrestart output
            // It usually shows NEEDRESTART-SVC: service_name
            let mut services = Vec::new();
            for line in stdout.lines() {
                if line.starts_with("NEEDRESTART-SVC:") {
                    services.push(line.replace("NEEDRESTART-SVC:", "").trim().to_string());
                }
            }
            return Ok(services);
        }
        Ok(Err(e)) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("needrestart failed to execute: {}", e);
            }
        }
        Err(_) => {
            log::warn!("needrestart timed out after 10 seconds. Skipping service scan.");
        }
    }

    // Fallback or just return empty if not installed/timed out
    Ok(Vec::new())
}

#[tauri::command]
#[specta::specta]
pub async fn restart_service(
    app: tauri::AppHandle,
    state_repo: State<'_, RepoManager>,
    password: Option<String>,
    unit: String,
) -> Result<(), String> {
    log::info!("Restarting service via helper: {}", unit);
    let one_click = state_repo.inner().is_one_click_enabled().await;
    let mut rx = helper_client::invoke_helper(
        &app,
        helper_client::HelperCommand::SystemctlRestart { unit },
        password,
        one_click,
    )
    .await?;

    while let Some(msg) = rx.recv().await {
        app.emit("service-restart-progress", msg.clone())
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_flatpak_permissions(app_id: String) -> Result<Vec<String>, String> {
    crate::flathub_api::get_flatpak_permissions(&app_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_full_package_details_by_canonical_id(
    canonical_id: String,
    state_meta: State<'_, crate::metadata::MetadataState>,
    state_repo: State<'_, RepoManager>,
    state_chaotic: State<'_, crate::chaotic_api::ChaoticApiClient>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_registry: State<'_, crate::registry::RegistryState>,
    app: AppHandle,
) -> Result<FullPackageDetails, String> {
    let requested_id = canonical_id.trim();
    if requested_id.is_empty() {
        return Err("canonical_id is required".to_string());
    }

    let lookup = state_registry
        .manager
        .get_package(requested_id)
        .ok()
        .flatten()
        .map(|pkg| pkg.name)
        .unwrap_or_else(|| requested_id.to_string());

    get_full_package_details(
        lookup,
        state_meta,
        state_repo,
        state_chaotic,
        state_flathub,
        state_registry,
        app,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_full_package_details(
    name: String,
    state_meta: State<'_, crate::metadata::MetadataState>,
    state_repo: State<'_, RepoManager>,
    state_chaotic: State<'_, crate::chaotic_api::ChaoticApiClient>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_registry: State<'_, crate::registry::RegistryState>,
    _app: AppHandle,
) -> Result<FullPackageDetails, String> {
    let request_cache_key = name.trim().to_lowercase();
    if let Some(cached) = FULL_DETAILS_CACHE.get(&request_cache_key).await {
        let mut cached = cached;
        if let Some(package) = cached.package.as_mut() {
            crate::utils::finalize_package_contract(package);
        }
        return Ok(cached);
    }
    let mut cache_alias_keys = Vec::new();
    if name.contains('.') && !name.contains(' ') {
        if let Some(mapped_name) = crate::flathub_api::get_package_name_from_app_id(&name) {
            let mapped_key = mapped_name.trim().to_lowercase();
            if mapped_key != request_cache_key {
                cache_alias_keys.push(mapped_key);
            }
        } else if let Some(last) = name.split('.').next_back() {
            let fallback_key = last.trim().to_lowercase();
            if !fallback_key.is_empty() && fallback_key != request_cache_key {
                cache_alias_keys.push(fallback_key);
            }
        }
    }
    for alias_key in &cache_alias_keys {
        if let Some(cached) = FULL_DETAILS_CACHE.get(alias_key).await {
            let mut cached = cached;
            if let Some(package) = cached.package.as_mut() {
                crate::utils::finalize_package_contract(package);
            }
            FULL_DETAILS_CACHE
                .insert(request_cache_key.clone(), cached.clone())
                .await;
            return Ok(cached);
        }
    }
    // Single-flight guard: collapse concurrent duplicate detail requests
    // (common in dev StrictMode / rapid tab transitions) into one backend fetch.
    let _details_guard = FULL_DETAILS_GATE.lock().await;
    if let Some(cached) = FULL_DETAILS_CACHE.get(&request_cache_key).await {
        let mut cached = cached;
        if let Some(package) = cached.package.as_mut() {
            crate::utils::finalize_package_contract(package);
        }
        return Ok(cached);
    }
    for alias_key in &cache_alias_keys {
        if let Some(cached) = FULL_DETAILS_CACHE.get(alias_key).await {
            let mut cached = cached;
            if let Some(package) = cached.package.as_mut() {
                crate::utils::finalize_package_contract(package);
            }
            FULL_DETAILS_CACHE
                .insert(request_cache_key.clone(), cached.clone())
                .await;
            return Ok(cached);
        }
    }

    // Heuristic: If name contains a dot and no spaces, it's likely an app_id
    let mut search_name = name.clone();
    let mut search_app_id = None;
    if name.contains('.') && !name.contains(' ') {
        search_app_id = Some(name.clone());
        if let Some(mapped_name) = crate::flathub_api::get_package_name_from_app_id(&name) {
            search_name = mapped_name;
        } else if let Some(last) = name.split('.').next_back() {
            search_name = last.to_lowercase();
        }
    }

    // 1. Fetch from Unified Backend Aggregation (replaces multiple calls / "Two Brains" issue).
    // Respect discovery toggles so details does not re-inject hidden sources into the shared registry.
    let include_flatpak = state_repo.inner().is_flatpak_enabled().await;
    let include_aur = state_repo.inner().is_aur_enabled().await;
    let include_chaotic = state_repo.inner().is_repo_enabled("chaotic-aur").await;

    let mut packages = crate::middleware::aggregation::fetch_and_merge_packages_by_names_impl(
        &state_meta,
        &state_chaotic,
        &state_repo,
        &state_flathub,
        &state_registry.manager,
        vec![(search_name, search_app_id)],
        include_flatpak,
        include_aur,
        include_chaotic,
        false, // details is a discovery surface; installed variants are resolved separately below
    )
    .await?;

    let mut primary_idx = 0;
    for (i, p) in packages.iter().enumerate() {
        if p.name == name || p.app_id.as_deref() == Some(&name) || p.canonical_id == name {
            primary_idx = i;
            break;
        }
    }

    let mut package = if !packages.is_empty() {
        Some(packages.remove(primary_idx))
    } else {
        None
    };

    let packages_iter = packages.into_iter();

    // 2. ORPHAN VARIANT & DEEP METADATA FIX
    if let Some(primary) = &mut package {
        if !primary.canonical_id.trim().is_empty() {
            if let Ok(Some(registry_pkg)) = state_registry.manager.get_package(&primary.canonical_id) {
                if primary
                    .long_description
                    .as_deref()
                    .map(|text| text.trim().is_empty())
                    .unwrap_or(true)
                    && registry_pkg
                        .long_description
                        .as_deref()
                        .map(|text| !text.trim().is_empty())
                        .unwrap_or(false)
                {
                    primary.long_description = registry_pkg.long_description.clone();
                }
                if primary
                    .screenshots
                    .as_ref()
                    .map(|shots| shots.is_empty())
                    .unwrap_or(true)
                    && registry_pkg
                        .screenshots
                        .as_ref()
                        .map(|shots| !shots.is_empty())
                        .unwrap_or(false)
                {
                    primary.screenshots = registry_pkg.screenshots.clone();
                }
                if (primary.icon.is_none()
                    || primary.icon.as_deref().unwrap_or("").trim().is_empty()
                    || primary.icon.as_deref().unwrap_or("").starts_with('/'))
                    && registry_pkg
                        .icon
                        .as_deref()
                        .map(|icon| !icon.trim().is_empty() && !icon.starts_with('/'))
                        .unwrap_or(false)
                {
                    primary.icon = registry_pkg.icon.clone();
                }
                if primary.app_id.is_none() && registry_pkg.app_id.is_some() {
                    primary.app_id = registry_pkg.app_id.clone();
                }
            }
        }
        // A. Merge disjoint variants (fixes missing dropdown / "Repo" label)
        let mut merged_sources = primary
            .available_sources
            .clone()
            .unwrap_or_else(|| vec![primary.source.clone()]);
        let primary_canonical = crate::utils::canonical_merge_key(
            &primary.name,
            primary.app_id.as_deref(),
        );

        for mut other_pkg in packages_iter {
            // CRITICAL: only merge variants that belong to the same canonical product.
            // This prevents unrelated Flatpak search hits from polluting the source selector.
            let other_canonical =
                crate::utils::canonical_merge_key(&other_pkg.name, other_pkg.app_id.as_deref());
            if other_canonical != primary_canonical {
                continue;
            }
            let other_sources = other_pkg
                .available_sources
                .take()
                .unwrap_or_else(|| vec![other_pkg.source.clone()]);
            for src in other_sources {
                if !merged_sources.iter().any(|s| {
                    // Dedup by (source_type, id, package_name) only — NOT version.
                    // This prevents libreoffice-fresh v20 and v25 from both appearing
                    // as separate "Arch Official" entries in the source dropdown.
                    s.id == src.id
                        && s.source_type == src.source_type
                        && s.package_name == src.package_name
                }) {
                    merged_sources.push(src);
                } else {
                    // Keep the higher version entry for this slot
                    if let Some(existing) = merged_sources.iter_mut().find(|s| {
                        s.id == src.id
                            && s.source_type == src.source_type
                            && s.package_name == src.package_name
                    }) {
                        if src.version > existing.version {
                            *existing = src;
                        }
                    }
                }
            }
        }
        merged_sources.sort_by(|a, b| {
            let rank = |s: &models::PackageSource| {
                let id = s.id.to_lowercase();
                match s.source_type.as_str() {
                    "repo" => {
                        if id.contains("cachyos")
                            || id.contains("manjaro")
                            || id.contains("garuda")
                            || id.contains("endeavour")
                        {
                            50
                        } else if matches!(id.as_str(), "core" | "extra" | "community" | "multilib" | "official") {
                            40
                        } else if id.contains("chaotic") {
                            30
                        } else {
                            35
                        }
                    }
                    "flatpak" => 20,
                    "aur" => 10,
                    _ => 0,
                }
            };
            rank(b)
                .cmp(&rank(a))
                .then_with(|| a.id.cmp(&b.id))
                .then_with(|| a.package_name.cmp(&b.package_name))
        });
        primary.available_sources = Some(merged_sources);

        let mut search_id = primary
            .app_id
            .clone()
            .unwrap_or_else(|| primary.name.clone());

        // PROACTIVE FIX: If the search_id is just a package name (likely from Repo/AUR),
        // try to find a known AppID mapping before falling back to search-by-name.
        if !search_id.contains('.') {
            if let Some(mapped) = crate::utils::canonical_to_flathub_id(&search_id) {
                log::info!(
                    "[DETAILS-PROXY] Mapping found for {}: -> {}",
                    search_id,
                    mapped
                );
                search_id = mapped;
            }
        }

        let needs_long_description = primary
            .long_description
            .as_deref()
            .map(|text| text.trim().is_empty())
            .unwrap_or(true);
        let needs_screenshots = primary
            .screenshots
            .as_ref()
            .map(|shots| shots.is_empty())
            .unwrap_or(true);
        let needs_icon = primary.icon.is_none() || primary.icon.as_deref().unwrap_or("").starts_with('/');
        let needs_app_id = primary.app_id.is_none();
        let needs_remote_metadata =
            needs_long_description || needs_screenshots || needs_icon || needs_app_id;

        if needs_remote_metadata {
            log::info!(
                "[DETAILS-PROXY] Fetching Flathub metadata for ID: {}",
                search_id
            );
            if let Some(fm) = state_flathub.get_metadata_for_package(&search_id).await {
                log::info!(
                    "[DETAILS-PROXY] Successfully fetched Flathub metadata for ID: {}",
                    search_id
                );
                let full_meta = crate::flathub_api::flathub_to_app_metadata(&fm, &primary.name);
                let mut enriched = false;

                if primary.app_id.is_none() {
                    primary.app_id = Some(full_meta.app_id.clone());
                    enriched = true;
                }

                if needs_long_description {
                    log::info!(
                        "[DETAILS-PROXY] Updating long_description for {}",
                        primary.name
                    );
                    primary.long_description = full_meta.description.clone();
                    enriched = true;
                }
                if needs_screenshots {
                    primary.screenshots = Some(full_meta.screenshots);
                    enriched = true;
                }
                if needs_icon {
                    primary.icon = full_meta.icon_url;
                    enriched = true;
                }

                if enriched {
                    if let Err(error) = state_registry
                        .manager
                        .bulk_upsert_packages(std::slice::from_ref(primary))
                    {
                        log::warn!(
                            "[DETAILS-PROXY] Failed to persist enriched metadata for {}: {}",
                            primary.name,
                            error
                        );
                    }
                }
            } else {
                log::warn!(
                    "[DETAILS-PROXY] Failed to fetch Flathub metadata for ID: {}",
                    search_id
                );
            }
        }

        if primary.maintainer.is_none() {
            primary.maintainer = maintainer_fallback_for_source(&primary.source);
        }
    }

    // Determine actual package name and flatpak app_id if available
    let actual_name = package
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| name.clone());

    let flathub_app_id = package.as_ref().and_then(|p| p.app_id.clone());

    // 2. Fetch Install Status
    let install_status = match check_installed_status(state_meta.clone(), actual_name.clone()).await
    {
        Ok(s) => s,
        Err(_) => PackageInstallStatus {
            installed: false,
            version: None,
            repo: None,
            source: None,
            actual_package_name: None,
        },
    };

    // 3. Fetch all completely installed variants (Repo + AUR + Flatpak)
    // Here we can re-use the available_sources from the `package` we just fetched if it exists
    let mut all_installed_variants = vec![];
    if let Some(pkg) = &package {
        if let Some(sources) = &pkg.available_sources {
            let unique_names: std::collections::HashSet<String> = sources
                .iter()
                .filter_map(|s| s.package_name.clone().or_else(|| Some(pkg.name.clone())))
                .collect();

            for uniq_n in unique_names {
                if let Ok(status) = check_installed_status(state_meta.clone(), uniq_n.clone()).await
                {
                    if status.installed {
                        all_installed_variants.push(status);
                    }
                }
            }
        }
    }
    // Fallback if the package itself was installed but variant logic didn't catch it
    if all_installed_variants.is_empty() && install_status.installed {
        all_installed_variants.push(install_status.clone());
    }

    // 4. Fetch permissions if it has a flatpak app_id
    let flatpak_permissions = match flathub_app_id {
        Some(app_id) => get_flatpak_permissions(app_id).await.ok(),
        None => None,
    };

    // 5. Build rich variant list for metadata reactivity from canonical available_sources.
    let mut all_variants = if let Some(pkg) = &package {
        let sources = pkg
            .available_sources
            .clone()
            .unwrap_or_else(|| vec![pkg.source.clone()]);
        let mut seen = std::collections::HashSet::new();
        let mut flatpak_size_cache: std::collections::HashMap<String, (Option<u64>, Option<u64>)> =
            std::collections::HashMap::new();
        let mut variants = Vec::new();

        for src in sources {
            let slot = format!(
                "{}|{}|{}",
                src.source_type,
                src.id,
                src.package_name.clone().unwrap_or_default()
            );
            if !seen.insert(slot) {
                continue;
            }

            // Reuse source-specific metadata from alternatives when available.
            let alt_match = pkg.alternatives.as_ref().and_then(|alts| {
                alts.iter().find(|a| {
                    a.source.id == src.id
                        && a.source.source_type == src.source_type
                        && a.source.package_name == src.package_name
                })
            });

            let v = alt_match.unwrap_or(pkg);
            let variant_maintainer = v
                .maintainer
                .clone()
                .or_else(|| maintainer_fallback_for_source(&src));
            let variant_security = Some(build_security_summary(
                Some(&src),
                variant_maintainer
                    .as_ref()
                    .map(|maintainer| !maintainer.trim().is_empty())
                    .unwrap_or(false),
            ));
            let mut download_size = v.download_size;
            let mut installed_size = v.installed_size;
            if src.source_type == "flatpak" && (download_size.is_none() || installed_size.is_none()) {
                if let Some(app_id) = src
                    .package_name
                    .clone()
                    .or_else(|| v.app_id.clone())
                    .or_else(|| pkg.app_id.clone())
                {
                    let sizes = if let Some(existing) = flatpak_size_cache.get(&app_id) {
                        *existing
                    } else {
                        let fetched = crate::flathub_api::get_remote_app_sizes(&app_id, &src.id)
                            .await
                            .unwrap_or((None, None));
                        flatpak_size_cache.insert(app_id.clone(), fetched);
                        flatpak_size_cache
                            .get(&app_id)
                            .cloned()
                            .unwrap_or((None, None))
                    };
                    if download_size.is_none() {
                        download_size = sizes.0;
                    }
                    if installed_size.is_none() {
                        installed_size = sizes.1;
                    }
                }
            }
            variants.push(models::PackageVariant {
                source: src.clone(),
                version: if src.version.is_empty() {
                    v.version.clone()
                } else {
                    src.version.clone()
                },
                repo_name: if src.id == "chaotic-aur" {
                    Some("chaotic-aur".to_string())
                } else {
                    None
                },
                pkg_name: src.package_name.clone().or_else(|| Some(v.name.clone())),
                download_size,
                installed_size,
                maintainer: variant_maintainer,
                license: v.license.clone(),
                description: Some(v.description.clone()),
                screenshots: v.screenshots.clone(),
                security: variant_security,
            });
        }

        variants
    } else {
        vec![]
    };

    let selected_source = resolve_authoritative_selected_source(
        package.as_ref(),
        &install_status,
        &all_installed_variants,
        &mut all_variants,
    );
    let installed_source_label = if install_status.installed {
        selected_source
            .as_ref()
            .as_ref()
            .map(|src| src.label.clone())
            .or_else(|| install_status.repo.clone())
    } else {
        None
    };
    let source_switch_notice = installed_source_label.as_ref().map(|label| {
        format!(
            "Installed from {}. To install from another source, uninstall the current app first.",
            label
        )
    });
    let maintainer_known = package
        .as_ref()
        .and_then(|pkg| pkg.maintainer.as_ref())
        .map(|m| !m.trim().is_empty())
        .unwrap_or(false);
    let security = Some(build_security_summary(
        selected_source.as_ref().or_else(|| package.as_ref().map(|pkg| &pkg.source)),
        maintainer_known,
    ));
    let developer_name = package
        .as_ref()
        .and_then(derive_developer_name_for_details);
    let donation_url = package
        .as_ref()
        .and_then(derive_donation_url_for_details);
    let presentation = package.as_ref().map(|pkg| models::PackagePresentation {
        display_title: Some(
            pkg.display_title
                .clone()
                .or_else(|| pkg.display_name.clone())
                .unwrap_or_else(|| pkg.name.clone()),
        ),
        icon: pkg.icon.clone(),
        short_description: Some(pkg.description.clone()),
        long_description: pkg.long_description.clone(),
        screenshots: pkg.screenshots.clone().unwrap_or_default(),
        app_id: pkg.app_id.clone(),
        developer_name: developer_name.clone(),
        donation_url: donation_url.clone(),
    });
    let mut response = FullPackageDetails {
        presentation,
        display_title: package
            .as_ref()
            .map(|p| p.display_name.clone().unwrap_or_else(|| p.name.clone())),
        primary_action: Some(if install_status.installed {
            "launch".to_string()
        } else {
            "install".to_string()
        }),
        primary_action_label: Some(if install_status.installed {
            "Launch".to_string()
        } else {
            "Install".to_string()
        }),
        selected_default_source: selected_source.clone(),
        source_summary: if all_variants.is_empty() {
            None
        } else if all_variants.len() == 1 {
            let src = &all_variants[0].source;
            Some(format!("Primary source: {}", src.label))
        } else {
            Some(format!("{} sources available", all_variants.len()))
        },
        security_summary: security
            .as_ref()
            .map(|summary| format!("{} {}", summary.verification_note, summary.user_action_note)),
        installed_source_label,
        source_switch_policy: Some(if install_status.installed {
            "informational_only".to_string()
        } else {
            "switch_allowed".to_string()
        }),
        source_switch_notice,
        security,
        developer_name,
        donation_url,
        package,
        installed_status: install_status,
        all_installed_variants,
        flatpak_permissions,
        all_variants,
    };
    if let Some(package) = response.package.as_mut() {
        crate::utils::finalize_package_contract(package);
    }
    log::debug!(
        "[DETAILS] name={} sources={} installed={}",
        actual_name,
        response
            .package
            .as_ref()
            .and_then(|p| p.available_sources.as_ref().map(|s| s.len()))
            .unwrap_or(0),
        response.installed_status.installed
    );
    let resolved_cache_key = response
        .package
        .as_ref()
        .map(|pkg| {
            if pkg.canonical_id.trim().is_empty() {
                crate::utils::canonical_merge_key(&pkg.name, pkg.app_id.as_deref())
            } else {
                pkg.canonical_id.to_lowercase()
            }
        })
        .unwrap_or_else(|| request_cache_key.clone());

    FULL_DETAILS_CACHE
        .insert(request_cache_key.clone(), response.clone())
        .await;
    if resolved_cache_key != request_cache_key {
        FULL_DETAILS_CACHE
            .insert(resolved_cache_key.clone(), response.clone())
            .await;
    }
    if let Some(pkg) = response.package.as_ref() {
        if let Some(app_id) = pkg.app_id.as_ref() {
            let app_id_key = app_id.trim().to_lowercase();
            if !app_id_key.is_empty()
                && app_id_key != request_cache_key
                && app_id_key != resolved_cache_key
            {
                FULL_DETAILS_CACHE.insert(app_id_key, response.clone()).await;
            }
        }
        let name_key = pkg.name.trim().to_lowercase();
        if !name_key.is_empty() && name_key != request_cache_key && name_key != resolved_cache_key {
            FULL_DETAILS_CACHE.insert(name_key, response.clone()).await;
        }
    }
    Ok(response)
}

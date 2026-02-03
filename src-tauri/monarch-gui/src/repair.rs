use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Cache "sync DB healthy" for a short time to avoid "fix your system" every other run (flaky pacman -Si).
static SYNC_DB_HEALTHY_CACHE: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));
const SYNC_DB_CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes
const MONARCH_POLKIT_POLICY: &str = include_str!("../com.monarch.store.policy");

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthIssue {
    pub category: String,
    pub severity: String,
    pub message: String,
    pub action_label: String,
    pub action_command: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InitializationStatus {
    pub needs_policy: bool,
    pub needs_keyring: bool,
    pub needs_migration: bool,
    /// Sync databases in /var/lib/pacman/sync/ are corrupt (Unrecognized archive format).
    pub needs_sync_db_repair: bool,
    pub is_healthy: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct RepairStep {
    pub name: String,
    pub status: String, // "pending", "running", "success", "failed"
    pub logs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyringStatus {
    pub healthy: bool,
    pub message: String,
}

#[tauri::command]
pub async fn check_keyring_health() -> Result<KeyringStatus, String> {
    let output = Command::new("pacman-key")
        .arg("--list-keys")
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(KeyringStatus {
            healthy: true,
            message: "Keyring appears healthy".to_string(),
        })
    } else {
        Ok(KeyringStatus {
            healthy: false,
            message: "pacman-key check failed".to_string(),
        })
    }
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
    use crate::error_classifier::ClassifiedError;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Acquire Lock
    let _guard = crate::utils::PRIVILEGED_LOCK.lock().await;

    let (binary, final_args) = if password.is_none() {
        // Use pkexec with helper if available
        let helper = crate::utils::MONARCH_PK_HELPER;

        if std::path::Path::new(helper).exists() && !bypass_helper {
            // Refactoring note: We've removed arbitrary RunCommand for security.
            // Privileged actions should now be specialized in the helper or use direct pkexec if safe.
            // For now, if we reach here without a password, we fall back to generic pkexec which will prompt.
            (
                "/usr/bin/pkexec".to_string(),
                vec![cmd.to_string()]
                    .into_iter()
                    .chain(args.iter().map(|s| s.to_string()))
                    .collect(),
            )
        } else {
            let mut all_args = vec!["--disable-internal-agent".to_string(), cmd.to_string()];
            for a in args {
                all_args.push(a.to_string());
            }
            ("/usr/bin/pkexec".to_string(), all_args)
        }
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

    let error_buffer: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

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
        let err_buf = error_buffer.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                {
                    let mut buf = err_buf.lock().await;
                    buf.push(line.clone());
                }
                // Filter out informational gpg messages that aren't actual errors
                let is_gpg_info = line.starts_with("gpg:")
                    && (line.contains("trustdb created")
                        || line.contains("no ultimately trusted keys found")
                        || line.contains("starting migration")
                        || line.contains("migration succeeded")
                        || line.contains("directory")
                        || line.contains("revocation certificate stored")
                        || line.contains("marginals needed")
                        || line.contains("depth:")
                        || line.contains("valid:")
                        || line.contains("signed:")
                        || line.contains("trust:")
                        || line.contains("next trustdb check")
                        || line.contains("Note: third-party key signatures")
                        || line.contains("setting ownertrust")
                        || line.contains("inserting ownertrust")
                        || line.contains("Total number processed")
                        || line.contains("imported:")
                        || (line.contains("keyserver receive failed")
                            && line.contains("Connection timed out")));

                if !is_gpg_info {
                    let _ = app.emit(&event, format!("ERROR: {}", line));
                } else {
                    // Emit as info instead of error for gpg informational messages
                    let _ = app.emit(&event, line);
                }
            }
        });
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        let errors = error_buffer.lock().await;
        let combined_output = errors.join("\n");

        if let Some(classified) = ClassifiedError::from_output(&combined_output) {
            let _ = app.emit("repair-error-classified", &classified);
            Err(format!("{}: {}", classified.title, classified.description))
        } else {
            Err(format!(
                "Operation failed (Exit Code: {:?}). Output: {}",
                status.code(),
                combined_output
            ))
        }
    }
}

/// App Store–style cancel: create cancel file so the helper exits, wait for it, then clear db lock.
#[tauri::command]
pub async fn cancel_install(app: AppHandle) -> Result<(), String> {
    const CANCEL_FILE: &str = "/var/tmp/monarch-cancel";
    std::fs::write(CANCEL_FILE, "1").map_err(|e| format!("Could not request cancel: {}", e))?;
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
    let _ = repair_unlock_pacman(app, None).await;
    Ok(())
}

#[tauri::command]
pub async fn repair_unlock_pacman(app: AppHandle, password: Option<String>) -> Result<(), String> {
    let _ = app.emit("repair-log", "🔓 Unlocking Pacman DB...");

    // Treat empty password as None so we use helper (Polkit) instead of sudo -S; avoids "sudo: no password was provided".
    let password = password.filter(|s| !s.trim().is_empty());

    // SECURITY: Safe Lock Removal — use Helper RemoveLock when one-click (no password)
    // so Polkit authorizes the helper; RunCommand(rm) is rejected by the helper.
    if password.is_none() {
        let mut rx = crate::helper_client::invoke_helper(
            &app,
            crate::helper_client::HelperCommand::ExecuteBatch {
                manifest: crate::models::TransactionManifest {
                    remove_lock: true,
                    ..Default::default()
                },
            },
            None,
        )
        .await?;
        let mut last = None;
        while let Some(msg) = rx.recv().await {
            let _ = app.emit("repair-log", &msg.message);
            last = Some(msg);
        }
        if let Some(m) = last {
            if m.progress == 0 && m.message.starts_with("Error:") {
                return Err(m.message);
            }
        }
        let _ = app.emit("repair-log", "✅ Pacman DB unlocked.");
        return Ok(());
    }

    // Sudo path: safe lock removal with pgrep check, then rm via run_privileged
    let is_running = std::process::Command::new("pgrep")
        .arg("-x")
        .arg("pacman")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if is_running {
        let _ = app.emit(
            "repair-log",
            "❌ Safety Check Failed: Pacman is currently running.",
        );
        return Err("Cannot remove lock: Pacman process detected.".to_string());
    }

    run_privileged(
        &app,
        "rm",
        &["-f", "/var/lib/pacman/db.lck"][..],
        password,
        "repair-log",
        false,
    )
    .await?;

    let _ = app.emit("repair-log", "✅ Pacman DB unlocked.");
    Ok(())
}

#[tauri::command]
pub async fn fix_keyring_issues(app: AppHandle, password: Option<String>) -> Result<(), String> {
    let _ = app.emit(
        "repair-log",
        "🔑 Starting Unified Keyring & Security Repair...",
    );

    let policy_escaped = MONARCH_POLKIT_POLICY.replace('{', "{{").replace('}', "}}");
    let script = format!(
        r#"
        echo "--- Resetting GPG Keyring (Deep Clean) ---"
        
        # 1. Kill locking agents
        killall gpg-agent dirmngr 2>/dev/null || true
        
        # 2. Check and remove pacman lock if stale (before any pacman operations)
        if [ -f /var/lib/pacman/db.lck ]; then
            if ! pgrep -x pacman > /dev/null; then
                echo "Removing stale pacman lock..."
                rm -f /var/lib/pacman/db.lck
            else
                echo "WARNING: Pacman is running; lock will be removed when pacman exits."
            fi
        fi
        
        # 3. Re-Install Security Policy (Ensures standard access)
        mkdir -p /usr/share/polkit-1/actions
        cat <<'EOF' > /usr/share/polkit-1/actions/com.monarch.store.policy
{}
EOF
        chmod 644 /usr/share/polkit-1/actions/com.monarch.store.policy

        # 4. Nuke and Pave GPG files (gpg writes info to stderr, which is normal)
        rm -rf /etc/pacman.d/gnupg
        pacman-key --init || true
        
        # Distro-aware population
        if [ -f /etc/manjaro-release ]; then
            pacman-key --populate manjaro archlinux || true
        else
            pacman-key --populate archlinux || true
        fi
        
        # 5. Sync Keyring Packages (use -Syu to avoid partial upgrade, skip if locked)
        if [ ! -f /var/lib/pacman/db.lck ] || ! pgrep -x pacman > /dev/null; then
            if [ -f /etc/manjaro-release ]; then
                pacman -Syu --noconfirm manjaro-keyring archlinux-keyring 2>&1 || echo "Note: Keyring packages already up to date or sync skipped."
            else
                pacman -Syu --noconfirm archlinux-keyring 2>&1 || echo "Note: Keyring packages already up to date or sync skipped."
            fi
        else
            echo "Skipping keyring package sync (pacman is locked)."
        fi
        
        # 6. Import Third Party Keys (with graceful handling of network/timeout issues)
        echo "Importing Chaotic-AUR, CachyOS, and Garuda Keys..."
        # Chaotic
        pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com || echo "Note: Chaotic key may already be imported or keyserver unavailable."
        pacman-key --lsign-key 3056513887B78AEB || true
        
        # CachyOS
        pacman-key --recv-key F3B607488DB35A47 --keyserver keyserver.ubuntu.com || echo "Note: CachyOS key may already be imported or keyserver unavailable."
        pacman-key --lsign-key F3B607488DB35A47 || true
        
        # Garuda
        pacman-key --recv-key 349BC7808577C592 --keyserver keyserver.ubuntu.com || echo "Note: Garuda key may already be imported or keyserver unavailable."
        pacman-key --lsign-key 349BC7808577C592 || true

        echo "✅ Keyring & Policy Reset Complete."
    "#,
        policy_escaped
    );

    run_privileged(
        &app,
        "bash",
        &["-c", &script][..],
        password,
        "repair-log",
        true, // Bypass to overwrite broken policy/helper
    )
    .await?;

    Ok(())
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
pub async fn check_initialization_status() -> Result<InitializationStatus, String> {
    let mut reasons = Vec::new();

    // 1. Check Policy (production path) and helper (production or dev path so dev builds don't report "missing" every launch)
    let has_policy =
        std::path::Path::new("/usr/share/polkit-1/actions/com.monarch.store.policy").exists();
    let has_helper = crate::utils::monarch_helper_available();
    let needs_policy = !has_policy || !has_helper;

    if !has_policy {
        reasons.push("Security policy is missing or not installed properly.".to_string());
    }
    if !has_helper {
        reasons.push(
            "Authentication helper binary is missing from /usr/lib/monarch-store/.".to_string(),
        );
    }

    // 2. Check Keyring (Quick check for existence of gnupg dir)
    let needs_keyring = !std::path::Path::new("/etc/pacman.d/gnupg").exists();
    if needs_keyring {
        reasons.push("System GPG keyring is missing or uninitialized.".to_string());
    }

    // 3. [REMOVED] Check Migration (Modular Include) - Host-Adaptive Model does not enforce this.
    // let conf = std::fs::read_to_string("/etc/pacman.conf").unwrap_or_default();
    // let needs_migration = !conf.contains("/etc/pacman.d/monarch/*.conf");
    let needs_migration = false;

    // 4. Check Sync DB health (read-only; detects corrupt core.db / extra.db / multilib.db)
    let needs_sync_db_repair = check_sync_db_corrupt().await;
    if needs_sync_db_repair {
        reasons.push(
            "Pacman sync databases are corrupt (Unrecognized archive format). MonARCH will attempt to fix them on launch."
                .to_string(),
        );
    }

    let is_healthy = !needs_policy && !needs_keyring && !needs_migration && !needs_sync_db_repair;

    Ok(InitializationStatus {
        needs_policy,
        needs_keyring,
        needs_migration,
        needs_sync_db_repair,
        is_healthy,
        reasons,
    })
}

/// Path for persistent "sync DB healthy until" timestamp so we don't re-probe every app launch.
fn sync_db_healthy_until_path() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|d| d.join("monarch-store").join("sync_db_healthy_until"))
}

/// Runs unprivileged: pacman -Si for a core package to trigger DB load. If sync DBs are corrupt, stderr contains "Unrecognized archive format" or "could not open database".
/// Uses: (1) persistent file so we don't re-probe every launch, (2) in-memory cache for same session, (3) pacman -Si only when needed.
const SYNC_DB_PERSISTENT_TTL_SECS: u64 = 86400; // 24 hours

async fn check_sync_db_corrupt() -> bool {
    // 1. Persistent cache: trust "healthy until" file across app restarts so we don't prompt every launch
    if let Some(p) = sync_db_healthy_until_path() {
        if p.exists() {
            if let Ok(buf) = std::fs::read_to_string(&p) {
                if let Ok(valid_until) = buf.trim().parse::<u64>() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    if now < valid_until {
                        return false; // Trust persistent "healthy until" timestamp
                    }
                }
            }
            let _ = std::fs::remove_file(&p);
        }
    }

    // 2. In-memory cache for same session
    if let Ok(guard) = SYNC_DB_HEALTHY_CACHE.lock() {
        if let Some(ok_at) = *guard {
            if ok_at.elapsed() < SYNC_DB_CACHE_TTL {
                return false;
            }
        }
    }

    // 3. Actually probe
    let output = match Command::new("pacman")
        .args(["-Si", "pacman"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => return false,
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    let corrupt = stderr.contains("Unrecognized archive format")
        || stderr.contains("could not open database");

    if !corrupt {
        if let Ok(mut guard) = SYNC_DB_HEALTHY_CACHE.lock() {
            *guard = Some(Instant::now());
        }
        if let Some(p) = sync_db_healthy_until_path() {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let valid_until = now + SYNC_DB_PERSISTENT_TTL_SECS;
            let _ = std::fs::write(&p, valid_until.to_string());
        }
    }

    corrupt
}

/// Clears the sync DB health cache (in-memory and persistent file) so the next check re-probes.
/// Call this after force_refresh_databases so the UI doesn't show a stale "corrupt" banner.
#[tauri::command]
pub fn clear_sync_db_health_cache() {
    if let Ok(mut guard) = SYNC_DB_HEALTHY_CACHE.lock() {
        *guard = None;
    }
    if let Some(p) = sync_db_healthy_until_path() {
        let _ = std::fs::remove_file(&p);
    }
}

fn last_sync_at_path() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|d| d.join("monarch-store").join("last_sync_at"))
}

/// Records that a full sync just completed so we don't run sync on every launch.
pub fn write_last_sync_timestamp() {
    if let Some(p) = last_sync_at_path() {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = std::fs::write(&p, now.to_string());
    }
}

/// Returns seconds since last sync, or None if never synced. Used to skip "sync on startup" when recently synced.
#[tauri::command]
pub fn get_last_sync_age_seconds() -> Option<u64> {
    let p = last_sync_at_path()?;
    let buf = std::fs::read_to_string(&p).ok()?;
    let then: u64 = buf.trim().parse().ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    now.checked_sub(then)
}

#[tauri::command]
pub async fn check_system_health() -> Result<Vec<HealthIssue>, String> {
    let mut issues = Vec::new();

    // 1. Check for pacman executable
    if !std::path::Path::new("/usr/bin/pacman").exists() {
        issues.push(HealthIssue {
            category: "System".to_string(),
            severity: "Critical".to_string(),
            message: "Pacman package manager not found.".to_string(),
            action_label: "Install Pacman".to_string(),
            action_command: None,
        });
    }

    // 2. Hardware/CPU Check
    let opt_level = if crate::utils::is_cpu_znver4_compatible() {
        "Zen 4/5 (Extreme)"
    } else if crate::utils::is_cpu_v4_compatible() {
        "v4 (AVX-512)"
    } else if crate::utils::is_cpu_v3_compatible() {
        "v3 (AVX2)"
    } else {
        "v1 (Standard x86-64)"
    };
    issues.push(HealthIssue {
        category: "Hardware".to_string(),
        severity: "Info".to_string(),
        message: format!("Hardware Optimization Level: {}", opt_level),
        action_label: "View Optimization Guide".to_string(),
        action_command: None,
    });

    // 3. Critical Keyring Check
    if !std::path::Path::new("/etc/pacman.d/gnupg").exists() {
        issues.push(HealthIssue {
            category: "Keyring".to_string(),
            severity: "Critical".to_string(),
            message: "System Keyring is corrupted or uninitialized.".to_string(),
            action_label: "Initialize Keyring".to_string(),
            action_command: Some("keyring".to_string()),
        });
    }

    // 4. Check dependencies (Non-root)
    let deps = ["base-devel", "git", "checkupdates"];
    for dep in deps {
        let has_dep = std::process::Command::new("pacman")
            .args(["-Qq", dep])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !has_dep {
            issues.push(HealthIssue {
                category: "Dependency".to_string(),
                severity: "Critical".to_string(),
                message: format!("Missing essential build dependency: {}", dep),
                action_label: format!("Install {}", dep),
                action_command: Some(format!("install_dep_{}", dep)), // We'll handle this in the frontend or add a generic install_dep
            });
        }
    }

    Ok(issues)
}

#[tauri::command]
pub async fn fix_keyring_issues_alias(
    app: AppHandle,
    password: Option<String>,
) -> Result<(), String> {
    fix_keyring_issues(app, password).await
}

#[tauri::command]
pub async fn repair_reset_keyring(app: AppHandle, password: Option<String>) -> Result<(), String> {
    // Alias for fix_keyring_issues for compatibility with InstallMonitor
    fix_keyring_issues(app, password).await
}

#[tauri::command]
pub async fn check_pacman_lock() -> bool {
    std::path::Path::new("/var/lib/pacman/db.lck").exists()
}

/// Returns true if a stale lock exists (db.lck present and no pacman process).
/// Frontend uses this to decide whether to show the app password dialog before unlock.
#[tauri::command]
pub async fn needs_startup_unlock() -> bool {
    if !std::path::Path::new("/var/lib/pacman/db.lck").exists() {
        return false;
    }
    let pacman_running = std::process::Command::new("pgrep")
        .arg("-x")
        .arg("pacman")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    !pacman_running
}

/// Called at app startup: if db.lck exists and no pacman process is running, remove the lock
/// so install/update/sync workflow isn't broken after a previous cancel or crash.
/// If `password` is Some (and non-empty), uses sudo path (app password box); otherwise Polkit.
#[tauri::command]
pub async fn unlock_pacman_if_stale(
    app: AppHandle,
    password: Option<String>,
) -> Result<(), String> {
    if !std::path::Path::new("/var/lib/pacman/db.lck").exists() {
        return Ok(());
    }
    let pacman_running = std::process::Command::new("pgrep")
        .arg("-x")
        .arg("pacman")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if pacman_running {
        return Ok(());
    }
    let password = password.filter(|s| !s.trim().is_empty());
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ExecuteBatch {
            manifest: crate::models::TransactionManifest {
                remove_lock: true,
                ..Default::default()
            },
        },
        password,
    )
    .await?;
    while let Some(_) = rx.recv().await {}
    Ok(())
}

/// Clear the pacman package cache on disk (/var/cache/pacman/pkg) via the Helper.
/// `keep`: number of versions to keep per package (0 = remove all). Helper may not yet honor this.
#[tauri::command]
pub async fn clear_pacman_package_cache(app: AppHandle, keep: Option<u32>) -> Result<(), String> {
    let _keep = keep.unwrap_or(0);
    let mut rx = crate::helper_client::invoke_helper(
        &app,
        crate::helper_client::HelperCommand::ExecuteBatch {
            manifest: crate::models::TransactionManifest {
                clear_cache: true,
                ..Default::default()
            },
        },
        None,
    )
    .await?;
    let mut last = None;
    while let Some(msg) = rx.recv().await {
        last = Some(msg);
    }
    if let Some(m) = last {
        if m.progress == 0 && m.message.starts_with("Error:") {
            return Err(m.message);
        }
    }
    Ok(())
}

/// Clear the native builder cache (~/.cache/monarch/build).
#[tauri::command]
pub async fn clear_build_cache() -> Result<(), String> {
    if let Some(mut cache_dir) = dirs::cache_dir() {
        cache_dir.push("monarch");
        cache_dir.push("build");
        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)
                .map_err(|e| format!("Failed to remove build cache: {}", e))?;
        }
        Ok(())
    } else {
        Err("Could not determine cache directory.".to_string())
    }
}

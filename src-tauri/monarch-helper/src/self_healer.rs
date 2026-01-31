//! Self-healing actions for common ALPM failures (keyring, db lock).
//! "Grandma Mode": fix when possible and retry; map C-errors to friendly messages.
//! Arch-compliant: stale lock = PID dead OR (lock >10 min old AND no pacman running).

use crate::logger;
use crate::progress;

pub const DB_LOCK_PATH: &str = "/var/lib/pacman/db.lck";

/// Max age (seconds) for db.lck before we consider it stale when no pacman is running.
const STALE_LOCK_AGE_SECS: u64 = 600; // 10 minutes

/// Returns true if a pacman process is currently running.
fn is_pacman_running() -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", "pacman"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Returns true if the lock file exists and is safe to remove:
/// - PID in file is dead, OR
/// - Lock file is older than 10 minutes AND no pacman process is running.
pub fn is_db_lock_stale() -> bool {
    let path = std::path::Path::new(DB_LOCK_PATH);
    if !path.exists() {
        return false;
    }
    let content = match std::fs::read_to_string(DB_LOCK_PATH) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let pid_str = content.trim();
    let pid: i32 = match pid_str.parse() {
        Ok(p) => p,
        Err(_) => return true, // invalid content -> consider stale
    };
    // On Linux, /proc/<pid> exists iff process is alive.
    let pid_alive = std::path::Path::new(&format!("/proc/{}", pid)).exists();
    if !pid_alive {
        return true; // PID dead -> stale
    }
    // PID alive but lock might be from a crashed parent: if lock file is >10 min old
    // and no pacman is running, consider stale (stale lock from previous run).
    let meta = match std::fs::metadata(DB_LOCK_PATH) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let modified = match meta.modified() {
        Ok(t) => t,
        Err(_) => return false,
    };
    let age_secs = modified
        .elapsed()
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if age_secs >= STALE_LOCK_AGE_SECS && !is_pacman_running() {
        return true;
    }
    false
}

/// Remove stale db.lck. Call only after is_db_lock_stale() is true.
pub fn remove_stale_db_lock() -> Result<(), String> {
    if !std::path::Path::new(DB_LOCK_PATH).exists() {
        return Ok(());
    }
    if !is_db_lock_stale() {
        return Err("Database lock held by live process".to_string());
    }
    std::fs::remove_file(DB_LOCK_PATH).map_err(|e| e.to_string())?;
    logger::info(&format!("Self-heal: removed stale lock {}", DB_LOCK_PATH));
    Ok(())
}

/// Max time to wait for pacman-key --refresh-keys (keyservers can hang for minutes).
const KEYRING_REFRESH_TIMEOUT_SECS: u64 = 90;

/// Run pacman-key --refresh-keys (as root) with a timeout so installs don't hang indefinitely.
/// If keyservers are slow or unreachable, we fail after KEYRING_REFRESH_TIMEOUT_SECS and the user can retry or fix keys manually.
pub fn refresh_keyring() -> Result<(), String> {
    use std::io::Read;
    use std::process::Stdio;

    logger::info(&format!(
        "Self-heal: running pacman-key --refresh-keys (timeout {}s)",
        KEYRING_REFRESH_TIMEOUT_SECS
    ));
    let mut child = std::process::Command::new("pacman-key")
        .args(["--refresh-keys"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stderr = child.stderr.take();
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(KEYRING_REFRESH_TIMEOUT_SECS);
    const PROGRESS_INTERVAL_SECS: u64 = 15;
    let mut last_progress_secs: u64 = 0;

    while start.elapsed() < timeout {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => {
                if status.success() {
                    return Ok(());
                }
                let err = if let Some(ref mut s) = stderr {
                    let mut buf = String::new();
                    let _ = s.read_to_string(&mut buf);
                    buf
                } else {
                    String::new()
                };
                return Err(format!("pacman-key failed: {}", err.trim()));
            }
            None => {
                let elapsed_secs = start.elapsed().as_secs();
                if elapsed_secs >= last_progress_secs + PROGRESS_INTERVAL_SECS {
                    last_progress_secs = elapsed_secs;
                    let json = serde_json::json!({
                        "event_type": "progress",
                        "package": null,
                        "percent": 50,
                        "downloaded": null,
                        "total": null,
                        "message": format!("Still refreshing keys... ({}s)", elapsed_secs)
                    })
                    .to_string();
                    progress::send_progress_line(json);
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    Err(format!(
        "Keyring refresh timed out after {} seconds. Try Settings → Maintenance → Fix Keys, or run 'sudo pacman-key --refresh-keys' manually.",
        KEYRING_REFRESH_TIMEOUT_SECS
    ))
}

/// User-facing message for keyring refresh.
pub fn keyring_refresh_message() -> &'static str {
    "We're refreshing security keys. Hang tight..."
}

/// User-facing message for db lock (when we can't remove it).
pub fn db_lock_busy_message() -> &'static str {
    "Another app is using the package database. Please close it and try again."
}

/// User-facing message for corrupt/missing DB.
pub fn db_open_message() -> &'static str {
    "We're repairing the package database..."
}

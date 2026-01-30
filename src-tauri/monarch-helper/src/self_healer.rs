//! Self-healing actions for common ALPM failures (keyring, db lock).
//! "Grandma Mode": fix when possible and retry; map C-errors to friendly messages.

use crate::logger;

pub const DB_LOCK_PATH: &str = "/var/lib/pacman/db.lck";

/// Returns true if the lock file exists and the PID inside is dead (or file invalid).
pub fn is_db_lock_stale() -> bool {
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
    let alive = std::path::Path::new(&format!("/proc/{}", pid)).exists();
    !alive
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

/// Run pacman-key --refresh-keys (as root). Blocks.
pub fn refresh_keyring() -> Result<(), String> {
    logger::info("Self-heal: running pacman-key --refresh-keys");
    let out = std::process::Command::new("pacman-key")
        .args(["--refresh-keys"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("pacman-key failed: {}", stderr));
    }
    Ok(())
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

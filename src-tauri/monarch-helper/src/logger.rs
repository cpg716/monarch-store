//! File logging for monarch-helper. Stdout is reserved for IPC (progress JSON),
//! so we write diagnostics to /var/log/monarch-helper.log (or fallback).

use std::fs::OpenOptions;
use std::io::Write;

const LOG_PATH: &str = "/var/log/monarch-helper.log";
const FALLBACK_LOG: &str = "/tmp/monarch-helper.log";

fn write_log(level: &str, msg: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let line = format!("[{}] {} {}\n", ts, level, msg);
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
        .or_else(|_| OpenOptions::new().create(true).append(true).open(FALLBACK_LOG));
    if let Ok(ref mut file) = f {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
}

/// Trace: every ALPM step (trans_init, add_target, prepare, commit).
pub fn trace(msg: &str) {
    write_log("TRACE", msg);
}

/// Info: high-level phase.
pub fn info(msg: &str) {
    write_log("INFO", msg);
}

/// Warning: non-fatal (e.g. sync failed but continuing).
pub fn warn(msg: &str) {
    write_log("WARN", msg);
}

/// Error: operation failed.
pub fn error(msg: &str) {
    write_log("ERROR", msg);
}

/// Log a panic message (from catch_unwind).
pub fn panic_msg(msg: &str) {
    write_log("PANIC", msg);
}

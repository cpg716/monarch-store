//! Single writer thread for all progress output to IPC (original stdout).
//! Prevents ALPM download callback (or main) from blocking on stdout and stalling the download.

use crossbeam_channel::{bounded, Sender};
use std::fs::File;
use std::io::Write;
use std::sync::OnceLock;

static SENDER: OnceLock<Sender<String>> = OnceLock::new();

/// Initialize the progress system with the IPC output stream (the original stdout).
/// This must be called BEFORE any progress messages are sent.
pub fn init(mut ipc_pipe: File) {
    let (tx, rx) = bounded::<String>(256);
    std::thread::spawn(move || {
        while let Ok(line) = rx.recv() {
            let _ = writeln!(ipc_pipe, "{}", line);
            // Unbuffered write to ensure GUI gets it immediately
            let _ = ipc_pipe.flush();
        }
    });
    let _ = SENDER.set(tx);
}

/// Send a single JSON progress line to the GUI. Non-blocking; drops if channel is full.
pub fn send_progress_line(line: String) {
    if let Some(tx) = SENDER.get() {
        let _ = tx.try_send(line);
    } else {
        // Fallback if not initialized (should not happen in prod, but maybe in tests)
        // Just print to stderr so it's visible in logs at least
        eprintln!("[Pre-Init Progress]: {}", line);
    }
}

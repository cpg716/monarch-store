//! Single writer thread for all progress output to stdout.
//! Prevents ALPM download callback (or main) from blocking on stdout and stalling the download.

use crossbeam_channel::{bounded, Sender};
use std::sync::OnceLock;

static SENDER: OnceLock<Sender<String>> = OnceLock::new();

fn sender() -> &'static Sender<String> {
    SENDER.get_or_init(|| {
        let (tx, rx) = bounded::<String>(256);
        std::thread::spawn(move || {
            let mut out = std::io::stdout();
            use std::io::Write;
            while let Ok(line) = rx.recv() {
                let _ = writeln!(out, "{}", line);
                let _ = out.flush();
            }
        });
        tx
    })
}

/// Send a single JSON progress line to the GUI. Non-blocking; drops if channel is full.
pub fn send_progress_line(line: String) {
    let _ = sender().try_send(line);
}

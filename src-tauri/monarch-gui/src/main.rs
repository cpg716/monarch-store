// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Phase 4: System Polish - Suppress noisey upstream logs (F23 key error)
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,tao=off,wry=off");
    }

    std::panic::set_hook(Box::new(|info| {
        eprintln!("Panic: {:?}", info);
    }));
    monarch_store_lib::run()
}

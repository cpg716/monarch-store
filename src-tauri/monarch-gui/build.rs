fn main() {
    // Do NOT run `cargo build -p monarch-helper` here: it deadlocks because the parent
    // cargo already holds the target dir lock. Use the npm "tauri dev" script to build
    // monarch-helper first, then run tauri dev.
    tauri_build::build()
}

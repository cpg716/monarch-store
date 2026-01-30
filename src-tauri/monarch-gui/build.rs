fn main() {
    // Ensure monarch-helper is built when we build the app (tauri dev only builds monarch-store otherwise).
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest_path = std::path::Path::new(&manifest_dir);
        // Workspace root is parent of monarch-gui, i.e. src-tauri
        if let Some(workspace_root) = manifest_path.parent() {
            let mut cmd = std::process::Command::new("cargo");
            cmd.args(["build", "-p", "monarch-helper"])
                .current_dir(workspace_root)
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit());
            if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
                cmd.env("CARGO_TARGET_DIR", target_dir);
            }
            let _ = cmd.status();
        }
    }
    tauri_build::build()
}

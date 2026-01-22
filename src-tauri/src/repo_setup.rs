use std::process::Command;
use tauri::command;

#[command]
pub fn check_repo_status(name: &str) -> bool {
    // fast check: read /etc/pacman.conf
    let conf = std::fs::read_to_string("/etc/pacman.conf").unwrap_or_default();
    // Check for [name] or [name-aur] etc.
    // simpler: check if the string "[name]" exists (case insensitive?)
    // Actually repo names are case sensitive in pacman.conf usually lowercase.
    let target = format!("[{}]", name.to_lowercase());
    conf.contains(&target)
}

#[command]
pub async fn enable_repo(_app: tauri::AppHandle, name: &str) -> Result<String, String> {
    let script = match name.to_lowercase().as_str() {
        "chaotic-aur" | "chaotic" => {
            r#"
            pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com
            pacman-key --lsign-key 3056513887B78AEB
            pacman -U 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst' 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst' --noconfirm
            if ! grep -q "\[chaotic-aur\]" /etc/pacman.conf; then
                echo -e "\n[chaotic-aur]\nInclude = /etc/pacman.d/chaotic-mirrorlist" >> /etc/pacman.conf
            fi
            pacman -Sy
            "#
        }
        _ => {
            return Err(format!(
                "Automatic setup for '{}' is not yet implemented.",
                name
            ))
        }
    };

    // Run via pkexec
    // We wrap the script in sh -c
    let status = Command::new("pkexec")
        .arg("/bin/sh")
        .arg("-c")
        .arg(script)
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(format!("Successfully enabled {}.", name))
    } else {
        Err(format!(
            "Failed to enable {}. Process exited with error.",
            name
        ))
    }
}

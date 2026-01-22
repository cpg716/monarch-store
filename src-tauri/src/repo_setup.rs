use std::process::Command;
use tauri::command;

#[command]
pub fn check_repo_status(name: &str) -> bool {
    let conf = std::fs::read_to_string("/etc/pacman.conf").unwrap_or_default();
    let target = format!("[{}]", name.to_lowercase());
    // Check if repo exists and is NOT commented out
    conf.lines().any(|l| {
        let trimmed = l.trim();
        trimmed.starts_with(&target) && !trimmed.starts_with('#')
    })
}

#[command]
pub async fn enable_repo(_app: tauri::AppHandle, name: &str) -> Result<String, String> {
    let name_lower = name.to_lowercase();

    // We handle CachyOS separately since it needs dynamic script building
    if name_lower == "cachyos" {
        let mut cachy_script = String::from(
            r#"
            pacman-key --recv-keys F3B607488DB35A47 --keyserver keyserver.ubuntu.com
            pacman-key --lsign-key F3B607488DB35A47
            pacman -U 'https://cdn77.cachyos.org/repo/x86_64/cachyos/cachyos-keyring-20240331-1-any.pkg.tar.zst' 'https://cdn77.cachyos.org/repo/x86_64/cachyos/cachyos-mirrorlist-22-1-any.pkg.tar.zst' --noconfirm
        "#,
        );

        if crate::utils::is_cpu_v3_compatible() {
            cachy_script.push_str(r#"
                if ! grep -q "\[cachyos-v3\]" /etc/pacman.conf; then
                    echo -e "\n[cachyos-v3]\nInclude = /etc/pacman.d/cachyos-v3-mirrorlist" >> /etc/pacman.conf
                    echo -e "\n[cachyos-core-v3]\nInclude = /etc/pacman.d/cachyos-v3-mirrorlist" >> /etc/pacman.conf
                    echo -e "\n[cachyos-extra-v3]\nInclude = /etc/pacman.d/cachyos-v3-mirrorlist" >> /etc/pacman.conf
                fi
            "#);
        }

        if crate::utils::is_cpu_v4_compatible() {
            cachy_script.push_str(r#"
                if ! grep -q "\[cachyos-v4\]" /etc/pacman.conf; then
                    echo -e "\n[cachyos-v4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> /etc/pacman.conf
                    echo -e "\n[cachyos-core-v4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> /etc/pacman.conf
                    echo -e "\n[cachyos-extra-v4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> /etc/pacman.conf
                fi
            "#);
        }

        if crate::utils::is_cpu_znver4_compatible() {
            cachy_script.push_str(r#"
                if ! grep -q "\[cachyos-znver4\]" /etc/pacman.conf; then
                    echo -e "\n[cachyos-znver4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> /etc/pacman.conf
                    echo -e "\n[cachyos-core-znver4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> /etc/pacman.conf
                    echo -e "\n[cachyos-extra-znver4]\nInclude = /etc/pacman.d/cachyos-v4-mirrorlist" >> /etc/pacman.conf
                fi
            "#);
        }

        cachy_script.push_str("pacman -Sy\n");
        return run_pkexec_script(&cachy_script, name).await;
    }

    let script = match name_lower.as_str() {
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
        "garuda" => {
            r#"
            pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com
            pacman-key --lsign-key 3056513887B78AEB
            if ! grep -q "\[garuda\]" /etc/pacman.conf; then
                echo -e "\n[garuda]\nInclude = /etc/pacman.d/chaotic-mirrorlist" >> /etc/pacman.conf
            fi
            pacman -Sy
            "#
        }
        "endeavouros" => {
            r#"
            pacman-key --recv-keys 428F7ECC9E192215 --keyserver keyserver.ubuntu.com
            pacman-key --lsign-key 428F7ECC9E192215
            pacman -U 'https://mirror.alpix.eu/endeavouros/repo/endeavouros/x86_64/endeavouros-keyring-20230523-1-any.pkg.tar.zst' 'https://mirror.alpix.eu/endeavouros/repo/endeavouros/x86_64/endeavouros-mirrorlist-20240105-1-any.pkg.tar.zst' --noconfirm
            if ! grep -q "\[endeavouros\]" /etc/pacman.conf; then
                echo -e "\n[endeavouros]\nSigLevel = PackageRequired\nInclude = /etc/pacman.d/endeavouros-mirrorlist" >> /etc/pacman.conf
            fi
            pacman -Sy
            "#
        }
        "manjaro" => {
            r#"
            if ! grep -q "\[manjaro-core\]" /etc/pacman.conf; then
                echo -e "\n[manjaro-core]\nSigLevel = PackageRequired\nServer = https://mirror.dkm.cz/manjaro/stable/core/\$arch" >> /etc/pacman.conf
                echo -e "\n[manjaro-extra]\nSigLevel = PackageRequired\nServer = https://mirror.dkm.cz/manjaro/stable/extra/\$arch" >> /etc/pacman.conf
            fi
            pacman -Sy
            "#
        }
        "aur" => {
            r#"
             # AUR requires base-devel and git
             pacman -S --needed base-devel git --noconfirm
             "#
        }
        _ => {
            return Err(format!(
                "Automatic setup for '{}' is not yet implemented.",
                name
            ))
        }
    };

    run_pkexec_script(script, name).await
}

async fn run_pkexec_script(script: &str, name: &str) -> Result<String, String> {
    // Add a header to the script for logging/transparency
    let full_script = format!(
        r#"
        echo "--- MonArch Repo Setup: {} ---"
        {}
        "#,
        name, script
    );

    let output = Command::new("pkexec")
        .arg("/bin/sh")
        .arg("-c")
        .arg(&full_script)
        .output()
        .map_err(|e| format!("Process Error: {}", e))?;

    if output.status.success() {
        Ok(format!("Successfully configured {}.", name))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Failed to configure {}: {}",
            name,
            stderr.lines().next().unwrap_or("Unknown Error")
        ))
    }
}

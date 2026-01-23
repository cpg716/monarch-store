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
            
            if ! grep -q "\[cachyos\]" /etc/pacman.conf; then
                echo -e "\n[cachyos]\nInclude = /etc/pacman.d/cachyos-mirrorlist" >> /etc/pacman.conf
            fi
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

        // Install mirrors and keyring from the newly added repo
        // We use curl to fetch the mirrorlist first because we can't sync without it
        cachy_script.push_str(r#"
            curl -s "https://mirror.cachyos.org/cachyos-mirrorlist" -o /etc/pacman.d/cachyos-mirrorlist
            # Copy for variants
            cp /etc/pacman.d/cachyos-mirrorlist /etc/pacman.d/cachyos-v3-mirrorlist
            cp /etc/pacman.d/cachyos-mirrorlist /etc/pacman.d/cachyos-v4-mirrorlist
            
            pacman -Sy
            pacman -S cachyos-keyring cachyos-mirrorlist --noconfirm
        "#);

        return run_pkexec_script(&cachy_script, name).await;
    }

    let script = match name_lower.as_str() {
        "chaotic-aur" | "chaotic" => {
            r#"
            pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com
            pacman-key --lsign-key 3056513887B78AEB
            
            # Bootstrap mirrorlist
            curl -s "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst" -o /tmp/chaotic-mirrorlist.pkg.tar.zst
            tar -I zstd -xf /tmp/chaotic-mirrorlist.pkg.tar.zst -C /etc/pacman.d/ --strip-components=1 etc/pacman.d/chaotic-mirrorlist

            if ! grep -q "\[chaotic-aur\]" /etc/pacman.conf; then
                echo -e "\n[chaotic-aur]\nInclude = /etc/pacman.d/chaotic-mirrorlist" >> /etc/pacman.conf
            fi
            pacman -Sy
            pacman -S chaotic-keyring chaotic-mirrorlist --noconfirm
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
            
            # Bootstrap mirrorlist
            curl -s "https://mirror.alpix.eu/endeavouros/repo/endeavouros/x86_64/endeavouros-mirrorlist" -o /etc/pacman.d/endeavouros-mirrorlist
            
            if ! grep -q "\[endeavouros\]" /etc/pacman.conf; then
                echo -e "\n[endeavouros]\nSigLevel = PackageRequired\nInclude = /etc/pacman.d/endeavouros-mirrorlist" >> /etc/pacman.conf
            fi
            pacman -Sy
            pacman -S endeavouros-keyring endeavouros-mirrorlist --noconfirm
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
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    // Create a temporary script file
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join(format!("monarch_setup_{}.sh", name));

    // Add header and shebang
    let full_script = format!(
        r#"#!/bin/bash
        set -e
        echo "--- MonArch Repo Setup: {} ---"
        {}
        "#,
        name, script
    );

    // Write script to file
    {
        let mut file = File::create(&script_path)
            .map_err(|e| format!("Failed to create temp script: {}", e))?;
        file.write_all(full_script.as_bytes())
            .map_err(|e| format!("Failed to write temp script: {}", e))?;

        // Make executable (755)
        let mut perms = file
            .metadata()
            .map_err(|e| format!("Failed to get metadata: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        file.set_permissions(perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    // Execute via pkexec
    // We point directly to the script path
    let output = Command::new("pkexec")
        .arg(script_path.to_str().unwrap())
        .output()
        .map_err(|e| format!("Process Error: {}", e))?;

    // Cleanup (best effort)
    let _ = std::fs::remove_file(script_path);

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("Setup Failed: {}", err))
    }
}

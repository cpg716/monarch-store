use std::process::Command;
use tauri::command;

#[command]
pub fn check_repo_status(name: &str) -> bool {
    let conf = std::fs::read_to_string("/etc/pacman.conf").unwrap_or_default();
    let target = format!("[{}]", name.to_lowercase());
    conf.lines().any(|l| {
        let trimmed = l.trim();
        trimmed.starts_with(&target) && !trimmed.starts_with('#')
    })
}

#[command]
pub async fn enable_repo(_app: tauri::AppHandle, name: &str) -> Result<String, String> {
    let name_lower = name.to_lowercase();

    // CachyOS Setup
    if name_lower == "cachyos" {
        let mut cachy_script = String::from(
            r#"
            echo "--- CachyOS Setup ---"
            
            # 1. Receiver Key
            echo "Receiving CachyOS keys..."
            if pacman-key --recv-keys F3B607488DB35A47 --keyserver keyserver.ubuntu.com; then
                echo "Key received from ubuntu keyserver."
            elif pacman-key --recv-keys F3B607488DB35A47 --keyserver pgp.mit.edu; then
                echo "Key received from mit keyserver."
            else
                echo "ERROR: Failed to receive CachyOS key."
                exit 1
            fi

            # 2. Sign Key
            if ! pacman-key --lsign-key F3B607488DB35A47; then
                echo "ERROR: Failed to locally sign CachyOS key."
                exit 1
            fi
            
            # 3. Fetch Mirrorlist (Robustly)
            echo "Fetching CachyOS mirrorlist..."
            if curl -f -s -L "https://mirror.cachyos.org/cachyos-mirrorlist" -o /etc/pacman.d/cachyos-mirrorlist; then
                if [ ! -s /etc/pacman.d/cachyos-mirrorlist ]; then
                     echo "ERROR: Downloaded CachyOS mirrorlist is empty."
                     rm -f /etc/pacman.d/cachyos-mirrorlist
                     exit 1
                fi
                
                # PREPEND known good CDN mirror to ensure reliability
                # This fixes issues where the first mirror in the list (e.g. nl.cachyos.org) is down
                echo "Prepending reliable CDN mirror..."
                sed -i '1s/^/## Priority Mirror\nServer = https:\/\/cdn77.cachyos.org\/repo\/$arch\/$repo\n\n/' /etc/pacman.d/cachyos-mirrorlist
            else
                echo "ERROR: Failed to download CachyOS mirrorlist."
                # Fallback: Create a minimal mirrorlist with just the CDN
                echo "Server = https://cdn77.cachyos.org/repo/$arch/$repo" > /etc/pacman.d/cachyos-mirrorlist
            fi

            # 4. Configure Pacman
            if ! grep -q "\[cachyos\]" /etc/pacman.conf; then
                echo -e "\n[cachyos]\nInclude = /etc/pacman.d/cachyos-mirrorlist" >> /etc/pacman.conf
            fi
        "#,
        );

        // Add CPU-specific optimized repos
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

        // 5. Finalize Install
        cachy_script.push_str(
            r#"
            cp /etc/pacman.d/cachyos-mirrorlist /etc/pacman.d/cachyos-v3-mirrorlist
            cp /etc/pacman.d/cachyos-mirrorlist /etc/pacman.d/cachyos-v4-mirrorlist
            
            echo "Syncing pacman..."
            if ! pacman -Sy; then
                 echo "ERROR: pacman -Sy failed."
                 exit 1
            fi
            
            echo "Installing CachyOS keyring..."
            if ! pacman -S cachyos-keyring cachyos-mirrorlist --noconfirm; then
                 echo "ERROR: Failed to install cachyos-keyring."
                 exit 1
            fi
        "#,
        );

        return run_pkexec_script(&cachy_script, name).await;
    }

    let script = match name_lower.as_str() {
        "chaotic-aur" | "chaotic" => {
            r#"
            echo "--- Chaotic-AUR Setup ---"
            echo "Receiving Chaotic-AUR keys..."
            pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com || pacman-key --recv-key 3056513887B78AEB --keyserver pgp.mit.edu

            if ! pacman-key --lsign-key 3056513887B78AEB; then
                 echo "ERROR: Failed to lsign Chaotic-AUR key."
                 exit 1
            fi
            
            echo "Installing Keyring and Mirrorlist..."
            if ! pacman -U 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst' 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst' --noconfirm; then
                echo "ERROR: Failed to install keyring/mirrorlist packages."
                exit 1
            fi

            if ! grep -q "\[chaotic-aur\]" /etc/pacman.conf; then
                echo -e "\n[chaotic-aur]\nInclude = /etc/pacman.d/chaotic-mirrorlist" >> /etc/pacman.conf
            fi
            
            echo "Syncing Repos..."
            pacman -Sy
            "#
        }
        "garuda" => {
            r#"
            echo "--- Garuda Setup ---"
            if [ ! -f /etc/pacman.d/chaotic-mirrorlist ]; then
                echo "WARNING: /etc/pacman.d/chaotic-mirrorlist missing. Fetching..."
                curl -f -s -L "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst" -o /tmp/chaotic-mirrorlist.pkg.tar.zst
                pacman -U /tmp/chaotic-mirrorlist.pkg.tar.zst --noconfirm
            fi

            if ! grep -q "\[garuda\]" /etc/pacman.conf; then
                echo -e "\n[garuda]\nInclude = /etc/pacman.d/chaotic-mirrorlist" >> /etc/pacman.conf
            fi
            pacman -Sy
            "#
        }
        "endeavouros" => {
            r#"
            echo "--- EndeavourOS Setup ---"
            echo "Receiving keys..."
            pacman-key --recv-keys 428F7ECC9E192215 --keyserver keyserver.ubuntu.com || pacman-key --recv-keys 428F7ECC9E192215 --keyserver pgp.mit.edu
            pacman-key --lsign-key 428F7ECC9E192215

            echo "Fetching Mirrorlist..."
            if ! curl -f -s -L "https://raw.githubusercontent.com/endeavouros-team/PKGBUILDS/master/endeavouros-mirrorlist/endeavouros-mirrorlist" -o /etc/pacman.d/endeavouros-mirrorlist; then
                echo "ERROR: Failed to download mirrorlist."
                exit 1
            fi

            if ! grep -q "\[endeavouros\]" /etc/pacman.conf; then
                echo -e "\n[endeavouros]\nSigLevel = PackageRequired\nInclude = /etc/pacman.d/endeavouros-mirrorlist" >> /etc/pacman.conf
            fi
            
            pacman -Sy
            pacman -S endeavouros-keyring endeavouros-mirrorlist --noconfirm
            "#
        }
        "manjaro" => {
            r#"
            echo "--- Manjaro Setup ---"
            echo "Receiving Manjaro Build Server key..."
            if ! pacman-key --recv-keys 279E7CF5D8D56EC8 --keyserver keyserver.ubuntu.com; then
                echo "ERROR: Failed to receive Manjaro key."
                exit 1
            fi
            pacman-key --lsign-key 279E7CF5D8D56EC8

            # Use robust list of mirrors instead of single hardcoded 404
            if ! grep -q "\[manjaro-core\]" /etc/pacman.conf; then
                echo -e "\n[manjaro-core]\nSigLevel = PackageRequired" >> /etc/pacman.conf
                echo "Server = https://mirror.easyname.at/manjaro/stable/core/\$arch" >> /etc/pacman.conf
                echo "Server = https://mirrors.gigenet.com/manjaro/stable/core/\$arch" >> /etc/pacman.conf
                echo "Server = https://mirror.dkm.cz/manjaro/stable/core/\$arch" >> /etc/pacman.conf
                echo "Server = https://ftp.gwdg.de/pub/linux/manjaro/stable/core/\$arch" >> /etc/pacman.conf
                
                echo -e "\n[manjaro-extra]\nSigLevel = PackageRequired" >> /etc/pacman.conf
                echo "Server = https://mirror.easyname.at/manjaro/stable/extra/\$arch" >> /etc/pacman.conf
                echo "Server = https://mirrors.gigenet.com/manjaro/stable/extra/\$arch" >> /etc/pacman.conf
                echo "Server = https://mirror.dkm.cz/manjaro/stable/extra/\$arch" >> /etc/pacman.conf
                echo "Server = https://ftp.gwdg.de/pub/linux/manjaro/stable/extra/\$arch" >> /etc/pacman.conf
            fi
            
            pacman -Sy
            if pacman -S manjaro-keyring --noconfirm; then
                pacman-key --populate manjaro
            fi
            "#
        }
        "aur" => {
            r#"
            echo "--- AUR Setup ---"
            echo "Syncing repositories..."
            if ! pacman -Sy; then
                echo "ERROR: Failed to sync repositories."
                exit 1
            fi
            
            echo "Installing build tools..."
            if ! pacman -S --needed base-devel git --noconfirm; then
                echo "ERROR: Failed to install base-devel or git."
                exit 1
            fi
             "#
        }
        _ => return Err(format!("Setup for '{}' is not implemented.", name)),
    };

    run_pkexec_script(script, name).await
}

async fn run_pkexec_script(script: &str, name: &str) -> Result<String, String> {
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join(format!("monarch_setup_{}.sh", name));

    // Added set -e for safety
    let full_script = format!(
        r#"#!/bin/bash
        set -e
        {}"#,
        script
    );

    {
        let mut file = File::create(&script_path).map_err(|e| e.to_string())?;
        file.write_all(full_script.as_bytes())
            .map_err(|e| e.to_string())?;
        let mut perms = file.metadata().map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        file.set_permissions(perms).map_err(|e| e.to_string())?;
    }

    let output = Command::new("pkexec")
        .arg(script_path.to_str().unwrap())
        .output()
        .map_err(|e| format!("Process Error: {}", e))?;

    let _ = std::fs::remove_file(script_path);

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "Setup Failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

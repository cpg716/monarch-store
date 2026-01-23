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
pub async fn reset_pacman_conf() -> Result<String, String> {
    let script = r#"
        echo "--- Resetting Pacman Config ---"
        # Backup
        cp /etc/pacman.conf /etc/pacman.conf.bak.$(date +%s)
        
        # Restore sane default (Arch Linux default)
        # We strip out custom sections: cachyos, chaotic-aur, garuda, endeavouros, manjaro
        # This uses sed to delete from a match until the next section or end of file
        
        sed -i '/\[cachyos/Q' /etc/pacman.conf 2>/dev/null || true
        sed -i '/\[chaotic-aur/Q' /etc/pacman.conf 2>/dev/null || true
        sed -i '/\[garuda/Q' /etc/pacman.conf 2>/dev/null || true
        sed -i '/\[endeavouros/Q' /etc/pacman.conf 2>/dev/null || true
        sed -i '/\[manjaro/Q' /etc/pacman.conf 2>/dev/null || true

        # Re-enable standard repos if they were messily commented out (simple check)
        # Ideally we just ensure [core], [extra] exist.
        
        echo "Pacman config reset to base. run enable_repos to re-add custom ones."
        
        # Sync to restore official DBs if they were broken
        pacman -Sy --noconfirm
    "#;

    run_pkexec_script(script, "reset_config").await
}

#[command]
pub async fn enable_repos_batch(
    _app: tauri::AppHandle,
    names: Vec<String>,
) -> Result<String, String> {
    if names.is_empty() {
        return Ok("No repos to enable.".to_string());
    }

    let mut full_script = String::from("echo '--- Starting Batch Repo Setup ---'\n");

    for name in names {
        let name_lower = name.to_lowercase();
        // Append specific script logic for each repo
        let script_part = get_repo_script(&name_lower);
        full_script.push_str(&format!("\n# Setup for {}\n{}\n", name, script_part));
    }

    full_script.push_str("\n\necho '--- Batch Setup Complete ---'\n");

    // Run all in one go
    run_pkexec_script(&full_script, "batch_setup").await
}

#[command]
pub async fn enable_repo(_app: tauri::AppHandle, name: &str) -> Result<String, String> {
    let script = get_repo_script(name);
    run_pkexec_script(&script, name).await
}

fn get_repo_script(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "cachyos" => {
            r#"
            echo "--- CachyOS Setup ---"
            
            # 1. Receiver Key
            if pacman-key --recv-keys F3B607488DB35A47 --keyserver keyserver.ubuntu.com; then
                echo "Key received."
            elif pacman-key --recv-keys F3B607488DB35A47 --keyserver pgp.mit.edu; then
                echo "Key received."
            elif pacman-key --recv-keys F3B607488DB35A47 --keyserver hkps://keys.openpgp.org; then
                echo "Key received."
            else
                echo "ERROR: Failed to receive CachyOS key."
                exit 1
            fi

            # 2. Sign Key
            if ! pacman-key --lsign-key F3B607488DB35A47; then
                echo "ERROR: Failed to locally sign CachyOS key."
                exit 1
            fi
            
            # 3. Fetch Mirrorlist
            if curl -f -s -L "https://mirror.cachyos.org/cachyos-mirrorlist" -o /etc/pacman.d/cachyos-mirrorlist; then
                # PREPEND known good CDN mirror
                sed -i '1s/^/## Priority Mirror\nServer = https:\/\/cdn77.cachyos.org\/repo\/$arch\/$repo\n\n/' /etc/pacman.d/cachyos-mirrorlist
            else
                # Fallback
                echo "Server = https://cdn77.cachyos.org/repo/$arch/$repo" > /etc/pacman.d/cachyos-mirrorlist
            fi

            # 4. Configure Pacman
            if ! grep -q "\[cachyos\]" /etc/pacman.conf; then
                echo -e "\n[cachyos]\nInclude = /etc/pacman.d/cachyos-mirrorlist" >> /etc/pacman.conf
            fi
            
            # Optimized repos
            if grep -q "avx2" /proc/cpuinfo; then # Rough check for v3
                if ! grep -q "\[cachyos-v3\]" /etc/pacman.conf; then
                    echo -e "\n[cachyos-v3]\nInclude = /etc/pacman.d/cachyos-mirrorlist" >> /etc/pacman.conf
                    echo -e "\n[cachyos-core-v3]\nInclude = /etc/pacman.d/cachyos-mirrorlist" >> /etc/pacman.conf
                    echo -e "\n[cachyos-extra-v3]\nInclude = /etc/pacman.d/cachyos-mirrorlist" >> /etc/pacman.conf
                fi
            fi
            
            pacman -S cachyos-keyring cachyos-mirrorlist --noconfirm || true
            
            # 5. Re-apply Priority Mirror (as package install overwrites it)
            if [ -f /etc/pacman.d/cachyos-mirrorlist ]; then
                # Ensure we don't double add if it's already there
                if ! grep -q "cdn77.cachyos.org" /etc/pacman.d/cachyos-mirrorlist | head -n 1; then
                    sed -i '1s/^/## Priority Mirror\nServer = https:\/\/cdn77.cachyos.org\/repo\/$arch\/$repo\n\n/' /etc/pacman.d/cachyos-mirrorlist
                fi
            fi
            "#
        }
        "chaotic-aur" | "chaotic" => {
            r#"
            echo "--- Chaotic-AUR Setup ---"
            pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com || pacman-key --recv-key 3056513887B78AEB --keyserver pgp.mit.edu || pacman-key --recv-key 3056513887B78AEB --keyserver hkps://keys.openpgp.org
            pacman-key --lsign-key 3056513887B78AEB
            
            pacman -U 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst' 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst' --noconfirm

            if ! grep -q "\[chaotic-aur\]" /etc/pacman.conf; then
                echo -e "\n[chaotic-aur]\nInclude = /etc/pacman.d/chaotic-mirrorlist" >> /etc/pacman.conf
            fi
            "#
        }
        "garuda" => {
            r#"
            echo "--- Garuda Setup ---"
            
            # 1. Chaotic-AUR is a prerequisite for Garuda
            if [ ! -f /etc/pacman.d/chaotic-mirrorlist ]; then
                # Reuse Chaotic Setup Logic (inlined for safety)
                pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com || pacman-key --recv-key 3056513887B78AEB --keyserver hkps://keys.openpgp.org
                pacman-key --lsign-key 3056513887B78AEB
                pacman -U 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst' 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst' --noconfirm
            fi

            # 2. Add [garuda] repo
            if ! grep -q "\[garuda\]" /etc/pacman.conf; then
                # We try to use the geo mirror, fallback to chaotic-mirrorlist inclusion if needed
                echo -e "\n[garuda]\nInclude = /etc/pacman.d/chaotic-mirrorlist" >> /etc/pacman.conf
                echo "Server = https://geo.mirror.garudalinux.org/repos/\$repo/\$arch" >> /etc/pacman.conf
                echo "Server = https://remote.garudalinux.org/\$repo/\$arch" >> /etc/pacman.conf
            fi
            
            # 3. Install Garuda Keyring (from chaotic or garuda repo)
            pacman -Sy --noconfirm
            pacman -S garuda-keyring --noconfirm || echo "Warning: garuda-keyring not found yet."
            pacman-key --populate garuda || true
            "#
        }
        "endeavouros" => {
            r#"
            echo "--- EndeavourOS Setup ---"
            pacman-key --recv-keys 428F7ECC9E192215 --keyserver keyserver.ubuntu.com || pacman-key --recv-keys 428F7ECC9E192215 --keyserver pgp.mit.edu || pacman-key --recv-keys 428F7ECC9E192215 --keyserver hkps://keys.openpgp.org
            pacman-key --lsign-key 428F7ECC9E192215

            curl -f -s -L "https://raw.githubusercontent.com/endeavouros-team/PKGBUILDS/master/endeavouros-mirrorlist/endeavouros-mirrorlist" -o /etc/pacman.d/endeavouros-mirrorlist

            if ! grep -q "\[endeavouros\]" /etc/pacman.conf; then
                echo -e "\n[endeavouros]\nSigLevel = PackageRequired\nInclude = /etc/pacman.d/endeavouros-mirrorlist" >> /etc/pacman.conf
            fi
            pacman -S endeavouros-keyring endeavouros-mirrorlist --noconfirm || true
            "#
        }
        "arch" | "official" => {
             r#"
             echo "--- Arch Official Repos Setup ---"
             # 1. Core
             if ! grep -q "\[core\]" /etc/pacman.conf; then
                 echo -e "\n[core]\nInclude = /etc/pacman.d/mirrorlist" >> /etc/pacman.conf
             fi
             # 2. Extra (includes community now)
             if ! grep -q "\[extra\]" /etc/pacman.conf; then
                 echo -e "\n[extra]\nInclude = /etc/pacman.d/mirrorlist" >> /etc/pacman.conf
             fi
             # 3. Multilib (64-bit only)
             if [ "$(uname -m)" = "x86_64" ]; then
                 if ! grep -q "\[multilib\]" /etc/pacman.conf; then
                     echo -e "\n[multilib]\nInclude = /etc/pacman.d/mirrorlist" >> /etc/pacman.conf
                 fi
             fi
             
             # Ensure mirrorlist exists/is populated if empty? 
             # For now, verify we have at least one valid mirror or restore default
             if [ ! -s /etc/pacman.d/mirrorlist ]; then
                 echo "Server = https://geo.mirror.pkgbuild.com/$repo/os/$arch" > /etc/pacman.d/mirrorlist
             fi
             
             pacman -Sy --noconfirm
             "#
        }
        "manjaro" => {
            r#"
            echo "--- Manjaro Setup ---"
            pacman-key --recv-keys 279E7CF5D8D56EC8 --keyserver keyserver.ubuntu.com || true
            pacman-key --lsign-key 279E7CF5D8D56EC8 || true

            if ! grep -q "\[manjaro-core\]" /etc/pacman.conf; then
                echo -e "\n[manjaro-core]\nSigLevel = PackageRequired" >> /etc/pacman.conf
                echo "Server = https://mirror.easyname.at/manjaro/stable/core/\$arch" >> /etc/pacman.conf
                echo "Server = https://mirrors.gigenet.com/manjaro/stable/core/\$arch" >> /etc/pacman.conf
                echo "Server = https://ftp.halifax.rwth-aachen.de/manjaro/stable/core/\$arch" >> /etc/pacman.conf
                echo "Server = https://mirror.funami.org/manjaro/stable/core/\$arch" >> /etc/pacman.conf
                
                echo -e "\n[manjaro-extra]\nSigLevel = PackageRequired" >> /etc/pacman.conf
                echo "Server = https://mirror.easyname.at/manjaro/stable/extra/\$arch" >> /etc/pacman.conf
                echo "Server = https://mirrors.gigenet.com/manjaro/stable/extra/\$arch" >> /etc/pacman.conf
                echo "Server = https://ftp.halifax.rwth-aachen.de/manjaro/stable/extra/\$arch" >> /etc/pacman.conf
                echo "Server = https://mirror.funami.org/manjaro/stable/extra/\$arch" >> /etc/pacman.conf
            fi
            
            pacman -S manjaro-keyring --noconfirm || true
            pacman-key --populate manjaro || true
            "#
        }
        "aur" => {
            r#"
            echo "--- AUR Setup ---"
            pacman -S --needed base-devel git --noconfirm
            "#
        }
        _ => "",
    }.to_string()
}

async fn run_pkexec_script(script: &str, name: &str) -> Result<String, String> {
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join(format!("monarch_setup_{}.sh", name));

    // Combine sync for efficiency at the end
    let full_script = format!(
        r#"#!/bin/bash
        set -e
        {}
        
        # Final Sync of all databases
        echo "Syncing all databases..."
        if ! pacman -Sy; then
             echo "WARNING: pacman -Sy failed. Check network or mirrors."
             # Do not exit failure here, as we might have successfully added repos
             # and the user can retry sync later.
             exit 0 
        fi
        "#,
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

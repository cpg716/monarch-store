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
        echo "--- Resetting MonARCH Infrastructure ---"
        # 1. Backup Pacman Config
        cp /etc/pacman.conf /etc/pacman.conf.bak.reset.$(date +%s)
        
        # 2. Cleanup legacy direct injections (best effort)
        # Target: chaotic-aur, cachyos, garuda, endeavouros, manjaro
        sed -i '/\[chaotic-aur\]/,/^\s*$/{d}' /etc/pacman.conf
        sed -i '/\[cachyos\]/,/^\s*$/{d}' /etc/pacman.conf
        sed -i '/\[garuda\]/,/^\s*$/{d}' /etc/pacman.conf
        sed -i '/\[endeavouros\]/,/^\s*$/{d}' /etc/pacman.conf
        sed -i '/\[manjaro/D' /etc/pacman.conf

        # 3. Wipe MonARCH modular configs
        rm -f /etc/pacman.d/monarch/*.conf
        mkdir -p /etc/pacman.d/monarch
        
        # 4. Ensure Modular Include is present (Infrastructure 2.0)
        if ! grep -q "/etc/pacman.d/monarch/\*.conf" /etc/pacman.conf; then
            echo -e "\n# MonARCH Managed Repositories\nInclude = /etc/pacman.d/monarch/*.conf" >> /etc/pacman.conf
        fi

        echo "Reset complete. System is now clean and ready for fresh setup."
        pacman -Sy --noconfirm
    "#;

    run_pkexec_script(script, "reset_config").await
}

#[command]
pub async fn set_repo_priority(order: Vec<String>) -> Result<String, String> {
    // Pacman reads files in alphabetical order in Include directories.
    // We achieve priority by renaming existing .conf files with numerical prefixes.
    // name examples: "chaotic-aur", "cachyos"

    let mut script = String::from("echo '--- Applying Repository Priorities ---'\n");
    script.push_str("cd /etc/pacman.d/monarch || exit 1\n");

    // First, strip existing prefixes (Best effort: remove any leading digits and dash)
    script
        .push_str("for f in *.conf; do mv \"$f\" \"${f#[0-9][0-9]-}\" 2>/dev/null || true; done\n");

    for (i, name) in order.iter().enumerate() {
        let prefix = format!("{:02}", i + 1);
        // Find the file (case insensitive-ish lookup by filename)
        script.push_str(&format!(
            "FILE=$(ls | grep -i \"^{}.conf\" | head -n 1)\n\
             if [ -n \"$FILE\" ]; then mv \"$FILE\" \"{}-$FILE\"; echo \"Priority {}: $FILE\"; fi\n",
            name, prefix, i + 1
        ));
    }

    run_pkexec_script(&script, "priority").await
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

#[command]
pub async fn bootstrap_infrastructure() -> Result<String, String> {
    let script = r#"
        echo "--- Initializing MonARCH Infrastructure 2.0 ---"
        mkdir -p /etc/pacman.d/monarch
        
        # 1. Clean up old direct injections to avoid duplicates
        # We target the most common ones we used: cachyos, chaotic-aur, garuda, endeavouros, manjaro
        cp /etc/pacman.conf /etc/pacman.conf.bak.inf2
        
        # This sed command deletes sections starting with these names until the next section or EOF
        # It's a "Best Effort" cleanup.
        sed -i '/\[cachyos\]/,/^\s*$/{d}' /etc/pacman.conf
        sed -i '/\[chaotic-aur\]/,/^\s*$/{d}' /etc/pacman.conf
        sed -i '/\[garuda\]/,/^\s*$/{d}' /etc/pacman.conf
        sed -i '/\[endeavouros\]/,/^\s*$/{d}' /etc/pacman.conf
        sed -i '/\[manjaro-core\]/,/^\s*$/{d}' /etc/pacman.conf
        sed -i '/\[manjaro-extra\]/,/^\s*$/{d}' /etc/pacman.conf

        # 2. Add the Modular Include line if missing
        if ! grep -q "/etc/pacman.d/monarch/\*.conf" /etc/pacman.conf; then
            echo -e "\n# MonARCH Managed Repositories\nInclude = /etc/pacman.d/monarch/*.conf" >> /etc/pacman.conf
            echo "Modular Include added."
        else
            echo "Modular Include already present."
        fi

        echo "Bootstrap complete. Using /etc/pacman.d/monarch/ for modular configs."
    "#;

    run_pkexec_script(script, "bootstrap").await
}

fn get_repo_script(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "cachyos" => {
            r#"
            echo "--- CachyOS Modular Setup ---"
            pacman -U "https://mirror.cachyos.org/repo/x86_64/cachyos/cachyos-keyring-20240331-1-any.pkg.tar.zst" --noconfirm || true
            CONF="/etc/pacman.d/monarch/cachyos.conf"
            echo "[cachyos]" > $CONF
            echo "Include = /etc/pacman.d/cachyos-mirrorlist" >> $CONF
            
            # v3 Optimization (AVX2)
            if grep -q "avx2" /proc/cpuinfo; then
                echo "[cachyos-v3]" >> $CONF
                echo "Include = /etc/pacman.d/cachyos-v3-mirrorlist" >> $CONF
                echo "[cachyos-core-v3]" >> $CONF
                echo "Include = /etc/pacman.d/cachyos-v3-mirrorlist" >> $CONF
                echo "[cachyos-extra-v3]" >> $CONF
                echo "Include = /etc/pacman.d/cachyos-v3-mirrorlist" >> $CONF
            fi

            # v4 Optimization (AVX512)
            if grep -q "avx512f" /proc/cpuinfo; then
                echo "[cachyos-v4]" >> $CONF
                echo "Include = /etc/pacman.d/cachyos-v4-mirrorlist" >> $CONF
                echo "[cachyos-core-v4]" >> $CONF
                echo "Include = /etc/pacman.d/cachyos-v4-mirrorlist" >> $CONF
                echo "[cachyos-extra-v4]" >> $CONF
                echo "Include = /etc/pacman.d/cachyos-v4-mirrorlist" >> $CONF
            fi

            # znver4 Optimization (AMD Zen 4)
            # This is a bit trickier via flags, but checking for 'avx512_fp16' is a good indicator for Zen 4
            if grep -q "avx512_fp16" /proc/cpuinfo; then
                echo "[cachyos-core-znver4]" >> $CONF
                echo "Include = /etc/pacman.d/cachyos-v4-mirrorlist" >> $CONF
                echo "[cachyos-extra-znver4]" >> $CONF
                echo "Include = /etc/pacman.d/cachyos-v4-mirrorlist" >> $CONF
            fi

            pacman -Sy --noconfirm
            "#
        }
        "chaotic-aur" | "chaotic" => {
            r#"
            echo "--- Chaotic-AUR Modular Setup ---"
            pacman -U 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst' 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst' --noconfirm
            echo "[chaotic-aur]" > /etc/pacman.d/monarch/chaotic-aur.conf
            echo "Include = /etc/pacman.d/chaotic-mirrorlist" >> /etc/pacman.d/monarch/chaotic-aur.conf
            pacman -Sy --noconfirm
            "#
        }
        "garuda" => {
            r#"
            echo "--- Garuda Modular Setup ---"
            pacman -U https://builds.garudalinux.org/repos/garuda/x86_64/garuda-keyring-20240128-1-any.pkg.tar.zst --noconfirm || true
            CONF="/etc/pacman.d/monarch/garuda.conf"
            echo "[garuda]" > $CONF
            echo "Server = https://builds.garudalinux.org/repos/garuda/\$arch" >> $CONF
            pacman -Sy --noconfirm
            "#
        }
        "endeavouros" => {
            r#"
            echo "--- EndeavourOS Modular Setup ---"
            curl -f -s -L "https://raw.githubusercontent.com/endeavouros-team/mirrors/master/mirrorlist" -o /etc/pacman.d/endeavouros-mirrorlist
            CONF="/etc/pacman.d/monarch/endeavouros.conf"
            echo "[endeavouros]" > $CONF
            echo "SigLevel = PackageRequired" >> $CONF
            echo "Include = /etc/pacman.d/endeavouros-mirrorlist" >> $CONF
            pacman -Sy endeavouros-keyring --noconfirm || true
            "#
        }
        "arch" | "official" => {
             r#"
             echo "--- Arch Official Modular Setup ---"
             CONF="/etc/pacman.d/monarch/arch-official.conf"
             echo "[core]" > $CONF
             echo "Include = /etc/pacman.d/mirrorlist" >> $CONF
             echo "" >> $CONF
             echo "[extra]" >> $CONF
             echo "Include = /etc/pacman.d/mirrorlist" >> $CONF
             if [ "$(uname -m)" = "x86_64" ]; then
                echo "" >> $CONF
                echo "[multilib]" >> $CONF
                echo "Include = /etc/pacman.d/mirrorlist" >> $CONF
             fi
             if [ ! -s /etc/pacman.d/mirrorlist ]; then
                 echo "Server = https://geo.mirror.pkgbuild.com/\$repo/os/\$arch" > /etc/pacman.d/mirrorlist
             fi
             pacman -Sy --noconfirm
             "#
        }
        "manjaro" => {
            r#"
            echo "--- Manjaro Modular Setup ---"
            pacman -U https://mirror.init7.net/manjaro/stable/core/x86_64/manjaro-keyring-20251003-1-any.pkg.tar.zst --noconfirm || true
            CONF="/etc/pacman.d/monarch/manjaro.conf"
            echo "[core]" > $CONF
            echo "SigLevel = PackageRequired" >> $CONF
            echo "Server = https://mirror.init7.net/manjaro/stable/core/\$arch" >> $CONF
            echo "" >> $CONF
            echo "[extra]" >> $CONF
            echo "SigLevel = PackageRequired" >> $CONF
            echo "Server = https://mirror.init7.net/manjaro/stable/extra/\$arch" >> $CONF
            pacman -Sy --noconfirm
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

async fn run_pkexec_script(script: &str, _name: &str) -> Result<String, String> {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;

    // Use /bin/bash -s to read script from stdin
    // This avoids the insecure temporary file creation (TOCTOU vulnerability)
    let mut child = tokio::process::Command::new("pkexec")
        .arg("/bin/bash")
        .arg("-s") // Read commands from stdin
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn pkexec: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(script.as_bytes()).await {
            return Err(format!("Failed to write script to stdin: {}", e));
        }
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("Failed to wait on pkexec: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "Setup Failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

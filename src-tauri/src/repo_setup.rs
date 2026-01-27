use tauri::command;

#[command]
pub fn check_repo_status(name: &str) -> bool {
    let path = std::path::Path::new("/etc/pacman.d/monarch").join(format!("50-{}.conf", name));
    path.exists()
}

#[command]
pub async fn reset_pacman_conf(password: Option<String>) -> Result<String, String> {
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
        touch /etc/pacman.d/monarch/00-monarch-placeholder.conf
        
        # 4. Ensure Modular Include is present (Infrastructure 2.0)
        if ! grep -q "/etc/pacman.d/monarch/\*.conf" /etc/pacman.conf; then
            # Insert before [core] for high priority
            sed -i '/\[core\]/i # MonARCH Managed Repositories\nInclude = /etc/pacman.d/monarch/*.conf\n' /etc/pacman.conf
        fi

        echo "Reset complete. System is now clean and ready for fresh setup."
        pacman -Sy --noconfirm
    "#;

    crate::utils::run_privileged_script(script, password, false).await
}

#[command]
pub async fn set_repo_priority(
    order: Vec<String>,
    password: Option<String>,
) -> Result<String, String> {
    // Pacman reads files in alphabetical order in Include directories.
    // We achieve priority by renaming existing .conf files with numerical prefixes.
    // name examples: "chaotic-aur", "cachyos"

    let mut script = String::from("echo '--- Applying Repository Priorities ---'\n");
    script.push_str("cd /etc/pacman.d/monarch || exit 1\n");

    // First, strip existing prefixes (Best effort: remove any leading digits and dash)
    script
        .push_str("for f in *.conf; do mv \"$f\" \"${f#[0-9][0-9]-}\" 2>/dev/null || true; done\n");

    for (i, name) in order.iter().enumerate() {
        // SECURITY: Validate input to prevent command injection
        if let Err(e) = crate::utils::validate_package_name(name) {
            return Err(e);
        }

        let prefix = format!("{:02}", i + 1);
        // Find the file (case insensitive-ish lookup by filename)
        script.push_str(&format!(
            "FILE=$(ls | grep -i \"^{}.conf\" | head -n 1)\n\
             if [ -n \"$FILE\" ]; then mv \"$FILE\" \"{}-$FILE\"; echo \"Priority {}: $FILE\"; fi\n",
            name, prefix, i + 1
        ));
    }

    crate::utils::run_privileged_script(&script, password, false).await
}

#[command]
pub async fn enable_repos_batch(
    _app: tauri::AppHandle,
    names: Vec<String>,
    password: Option<String>,
) -> Result<String, String> {
    if names.is_empty() {
        return Ok("No repos to enable.".to_string());
    }

    let mut full_script = String::from("echo '--- Starting Batch Repo Setup ---'\n");

    for name in names {
        if let Err(e) = crate::utils::validate_package_name(&name) {
            return Err(e);
        }

        let name_lower = name.to_lowercase();
        // Append specific script logic for each repo
        let script_part = get_repo_script(&name_lower);
        full_script.push_str(&format!("\n# Setup for {}\n{}\n", name, script_part));
    }

    full_script.push_str("\n\necho '--- Batch Setup Complete ---'\n");

    // Run all in one go
    crate::utils::run_privileged_script(&full_script, password, false).await
}

#[command]
pub async fn enable_repo(
    _app: tauri::AppHandle,
    name: &str,
    password: Option<String>,
) -> Result<String, String> {
    let script = get_repo_script(name);
    crate::utils::run_privileged_script(&script, password, false).await
}

#[command]
pub async fn set_one_click_control(
    state: tauri::State<'_, crate::repo_manager::RepoManager>,
    enabled: bool,
    password: Option<String>,
) -> Result<String, String> {
    let allow_active = if enabled { "yes" } else { "auth_admin_keep" };
    let script = r#"
        echo "Updating MonARCH Access Control..."
        cat <<'EOF' > /usr/share/polkit-1/actions/com.monarch.store.policy
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
<policyconfig>
  <vendor>MonARCH Store</vendor>
  <action id="com.monarch.store.package-manage">
    <description>Manage system packages</description>
    <message>Authentication required</message>
    <defaults>
      <allow_any>auth_admin</allow_any>
      <allow_inactive>auth_admin</allow_inactive>
      <allow_active>{{ALLOW_ACTIVE}}</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.exec.path">/usr/bin/monarch-pk-helper</annotate>
    <annotate key="org.freedesktop.policykit.exec.allow_gui">false</annotate>
  </action>
</policyconfig>
EOF
        echo "Rules updated."
    "#
    .replace("{{ALLOW_ACTIVE}}", allow_active);

    let res = crate::utils::run_privileged_script(&script, password, false).await?;
    state.inner().set_one_click_enabled(enabled).await;
    Ok(res)
}

#[command]
pub async fn bootstrap_system(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::repo_manager::RepoManager>,
    password: Option<String>,
    one_click: Option<bool>,
) -> Result<String, String> {
    let is_one_click = one_click.unwrap_or(false);
    let allow_active = if is_one_click {
        "yes"
    } else {
        "auth_admin_keep"
    };

    let script = r##"
        echo "--- Initializing MonARCH Infrastructure 2.1 (Full Keyring & DB Reset) ---"
        
        # 0. Forced Cleanup
        echo "Clearing locks and corrupted databases..."
        rm -f /var/lib/pacman/db.lck 2>/dev/null || true
        rm -rf /var/lib/pacman/sync/* 2>/dev/null || true
        
        # 0.5 Permission Repair (Critical)
        echo "Repairing filesystem permissions..."
        chown root:root /etc/pacman.d/gnupg 2>/dev/null || true
        chmod 700 /etc/pacman.d/gnupg 2>/dev/null || true
        mkdir -p /etc/pacman.d/monarch
        chmod 755 /etc/pacman.d/monarch
        
        # Infrastructure 2.1: Nuke any existing monarch configs to prevent "Database already registered"
        rm -f /etc/pacman.d/monarch/*.conf
        
        # Ensure placeholder exists to satisfy glob
        echo "# Placeholder" > /etc/pacman.d/monarch/00-monarch-placeholder.conf
        
        if [ ! -f /etc/pacman.d/monarch/00-monarch-placeholder.conf ]; then
             echo "CRITICAL ERROR: Failed to create placeholder conf!"
             exit 1
        fi

        # 1. Nuke and Pave GPG Keyring
        echo "Resetting GPG Keyring..."
        killall gpg-agent dirmngr 2>/dev/null || true
        rm -rf /etc/pacman.d/gnupg
        
        pacman-key --init
        pacman-key --populate archlinux
        
        # 2. Update Official Keyring Package
        echo "Syncing archlinux-keyring..."
        pacman -Sy --noconfirm archlinux-keyring

        # 2.5 Force Re-Import CachyOS Keys (Fix invalid signature)
        echo "Refreshing CachyOS Keys..."
        pacman-key --recv-key F3B607488DB35A47 --keyserver keyserver.ubuntu.com
        pacman-key --lsign-key F3B607488DB35A47
        
        # 3. Import Chaotic Keys
        echo "Refreshing Chaotic Keys..."
        pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com
        pacman-key --lsign-key 3056513887B78AEB
        
        # 4. Cleanup old direct injections
        sed -i '/\[cachyos/d' /etc/pacman.conf
        sed -i '/\[chaotic-aur/d' /etc/pacman.conf
        sed -i '/\[garuda/d' /etc/pacman.conf
        sed -i '/\[endeavouros/d' /etc/pacman.conf
        sed -i '/\[manjaro/d' /etc/pacman.conf

        # 5. Add the Modular Include line
        if ! grep -q "/etc/pacman.d/monarch/\*.conf" /etc/pacman.conf; then
            # Insert before [core] for high priority
            sed -i '/\[core\]/i # MonARCH Managed Repositories\nInclude = /etc/pacman.d/monarch/*.conf\n' /etc/pacman.conf
        fi

        # 6. Install MonARCH Polkit Policy
        echo "Configuring Seamless Auth Helper..."
        cat <<'EOF' > /usr/bin/monarch-pk-helper
#!/bin/bash
case "${1##*/}" in
  pacman|pacman-key|yay|paru|aura|rm|cat|mkdir|chmod|killall|cp|sed|bash|ls|grep|touch|checkupdates)
    exec "$@" ;;
  *)
    echo "Unauthorized: $1"; exit 1 ;;
esac
EOF
        chmod +x /usr/bin/monarch-pk-helper
        
        cat <<'EOF' > /usr/share/polkit-1/actions/com.monarch.store.policy
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
<policyconfig>
  <vendor>MonARCH Store</vendor>
  <action id="com.monarch.store.package-manage">
    <description>Manage system packages</description>
    <message>Authentication required</message>
    <defaults>
      <allow_any>auth_admin</allow_any>
      <allow_inactive>auth_admin</allow_inactive>
      <allow_active>{{ALLOW_ACTIVE}}</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.exec.path">/usr/bin/monarch-pk-helper</annotate>
    <annotate key="org.freedesktop.policykit.exec.allow_gui">false</annotate>
  </action>
</policyconfig>
EOF
        echo "Final Database Sync..."
        pacman -Sy --noconfirm

        echo "Bootstrap complete. Keys & Databases healthy ({{ALLOW_ACTIVE}})."
    "##
    .replace("{{ALLOW_ACTIVE}}", allow_active);

    let res = crate::utils::run_privileged_script_with_progress(
        app,
        "bootstrap-progress",
        &script,
        password,
        true, // MUST bypass helper for bootstrap/repair to work
    )
    .await;
    if res.is_ok() {
        state.inner().set_one_click_enabled(is_one_click).await;
    }
    res
}

fn get_repo_script(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "cachyos" => {
            r#"
            echo "--- CachyOS Trust & Mirror Setup ---"
            pacman-key --recv-key F3B607488DB35A47 --keyserver keyserver.ubuntu.com
            pacman-key --lsign-key F3B607488DB35A47
            
            # CachyOS requires mirrorlists for their standard Include structure
            if [ ! -f /etc/pacman.d/cachyos-mirrorlist ]; then
                echo "Server = https://mirror.cachyos.org/repo/x86_64/\$repo" > /etc/pacman.d/cachyos-mirrorlist
            fi
            if [ ! -f /etc/pacman.d/cachyos-v4-mirrorlist ]; then
                echo "Server = https://mirror.cachyos.org/repo/x86_64_v4/\$repo" > /etc/pacman.d/cachyos-v4-mirrorlist
            fi
            if [ ! -f /etc/pacman.d/cachyos-znver4-mirrorlist ]; then
                echo "Server = https://mirror.cachyos.org/repo/x86_64_v4/\$repo" > /etc/pacman.d/cachyos-znver4-mirrorlist
            fi
            "#
        }
        "chaotic-aur" | "chaotic" => {
            r#"
            echo "--- Chaotic-AUR Trust Setup ---"
            pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com
            pacman-key --lsign-key 3056513887B78AEB

            echo "Installing Chaotic-AUR support packages..."
            pacman -U 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst' 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst' --noconfirm || true
            "#
        }
        "garuda" => {
            r#"
            echo "--- Garuda Trust Setup ---"
            pacman-key --recv-key DD499313D4057A27 --keyserver keyserver.ubuntu.com
            pacman-key --lsign-key DD499313D4057A27
            "#
        }
        "endeavouros" => {
            r#"
            echo "--- EndeavourOS Trust Setup ---"
            if [ ! -f /etc/pacman.d/endeavouros-mirrorlist ]; then
                curl -f -s -L "https://raw.githubusercontent.com/endeavouros-team/mirrors/master/mirrorlist" -o /etc/pacman.d/endeavouros-mirrorlist
            fi
            "#
        }
        "manjaro" => {
            r#"
            echo "--- Manjaro Trust & Security Setup ---"
            # Ensure Manjaro keyring is present first
            pacman -Sy --needed manjaro-keyring --noconfirm || true
            pacman-key --init
            pacman-key --populate manjaro
            
            pacman-key --recv-key 279E7CF5D8D56EC8 --keyserver keyserver.ubuntu.com
            pacman-key --lsign-key 279E7CF5D8D56EC8
            "#
        }
        "aur" => {
            r#"
            echo "--- AUR Build Environment Setup ---"
            pacman -S --needed base-devel git --noconfirm
            "#
        }
        _ => "",
    }.to_string()
}

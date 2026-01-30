use std::process::Stdio;
use tokio::io::AsyncWriteExt;

pub const MONARCH_PK_HELPER: &str = "/usr/lib/monarch-store/monarch-helper";

lazy_static::lazy_static! {
    pub static ref PRIVILEGED_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::new(());
}

pub fn to_pretty_name(pkg_name: &str) -> String {
    // 1. Basic cleaning and splitting
    let parts: Vec<&str> = pkg_name.split(['-', '_']).collect();

    // 2. Capitalization logic
    let pretty: Vec<String> = parts
        .into_iter()
        .map(|part| {
            match part.to_lowercase().as_str() {
                "cli" => "CLI".to_string(),
                "tui" => "TUI".to_string(),
                "gui" => "GUI".to_string(),
                "api" => "API".to_string(),
                "sdk" => "SDK".to_string(),
                "aur" => "AUR".to_string(),
                "git" => "Git".to_string(),
                "bin" => "".to_string(), // Strip common suffixes
                "" => "".to_string(),
                _ => {
                    let mut chars = part.chars();
                    match chars.next() {
                        core::option::Option::None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                }
            }
        })
        .filter(|p| !p.is_empty())
        .collect();

    if pretty.is_empty() {
        return pkg_name.to_string();
    }

    pretty.join(" ")
}

lazy_static::lazy_static! {
    static ref VALIDATE_RE: regex::Regex = regex::Regex::new(r"^[a-zA-Z0-9@._+\-]+$").unwrap();
}

static VALIDATE_CACHE: once_cell::sync::Lazy<moka::sync::Cache<String, Result<(), String>>> =
    once_cell::sync::Lazy::new(|| {
        moka::sync::Cache::builder()
            .max_capacity(2000)
            .time_to_live(std::time::Duration::from_secs(300))
            .build()
    });

fn validate_package_name_impl(name: &str) -> Result<(), String> {
    if !VALIDATE_RE.is_match(name) {
        return Err(format!(
            "Invalid package name: '{}'. Contains unsafe characters.",
            name
        ));
    }
    Ok(())
}

/// Validates package name (Arch standard). Results are memoized to avoid repeated regex checks during rapid search.
pub fn validate_package_name(name: &str) -> Result<(), String> {
    if let Some(cached) = VALIDATE_CACHE.get(name) {
        return cached;
    }
    let r = validate_package_name_impl(name);
    VALIDATE_CACHE.insert(name.to_string(), r.clone());
    r
}

use crate::models;

pub fn sort_packages_by_relevance(packages: &mut [models::Package], query: &str) {
    let q_lower = query.to_lowercase();
    let common_apps = [
        "google-chrome",
        "steam",
        "obs-studio",
        "discord",
        "spotify",
        "vlc",
        "firefox",
        "visual-studio-code-bin",
        "code",
    ];

    packages.sort_by(|a, b| {
        let rank_pkg = |pkg: &models::Package| -> u8 {
            let p_name = pkg.name.to_lowercase();

            // Rank 0: Common Apps (Rigid priority if query is close)
            if common_apps.contains(&p_name.as_str()) {
                // If query is "chrome" and pkg is "google-chrome", prioritize it!
                // Or if query matches the package name loosely
                if p_name.contains(&q_lower)
                    || q_lower.contains("chrome") && p_name == "google-chrome"
                {
                    return 0;
                }
            }

            // Rank 1: Exact Match
            if p_name == q_lower {
                return 1;
            }

            // Rank 2: Starts With
            if p_name.starts_with(&q_lower) {
                return 2;
            }

            // Rank 3: Source Priority
            // This ensures Chaotic > Official > CachyOS etc for items with same name strength
            3 + (pkg.source.priority() as u8)
        };

        let rank_a = rank_pkg(a);
        let rank_b = rank_pkg(b);

        if rank_a != rank_b {
            return rank_a.cmp(&rank_b);
        }

        // TIE BREAKER: Source Priority (e.g. Official > Chaotic > AUR)
        let prio_a = a.source.priority() as u8;
        let prio_b = b.source.priority() as u8;
        if prio_a != prio_b {
            return prio_a.cmp(&prio_b);
        }

        // Secondary Sort: Shortest Name
        if a.name.len() != b.name.len() {
            return a.name.len().cmp(&b.name.len());
        }

        // Tertiary Sort: Votes
        b.num_votes.unwrap_or(0).cmp(&a.num_votes.unwrap_or(0))
    });
}

// Checks if the CPU supports x86-64-v3 (AVX2, FMA, BMI2, etc.)
pub fn is_cpu_v3_compatible() -> bool {
    let cpuid = raw_cpuid::CpuId::new();

    // v3 requires: AVX, AVX2, BMI1, BMI2, F16C, FMA, MOVBE, XSAVE, LZCNT (ABM)
    let has_v3_base = if let Some(feat) = cpuid.get_feature_info() {
        feat.has_avx() && feat.has_fma() && feat.has_f16c() && feat.has_movbe() && feat.has_xsave()
    } else {
        false
    };

    let has_v3_ext = if let Some(ext) = cpuid.get_extended_feature_info() {
        ext.has_avx2() && ext.has_bmi1() && ext.has_bmi2()
    } else {
        false
    };

    let has_lzcnt = if let Some(ext) = cpuid.get_extended_processor_and_feature_identifiers() {
        ext.has_lzcnt()
    } else {
        false
    };

    has_v3_base && has_v3_ext && has_lzcnt
}

// Checks if the CPU supports x86-64-v4 (AVX-512 foundation and major extensions)
pub fn is_cpu_v4_compatible() -> bool {
    let cpuid = raw_cpuid::CpuId::new();

    if let Some(ext) = cpuid.get_extended_feature_info() {
        // v4 requires v3 + AVX-512F, BW, CD, DQ, VL
        ext.has_avx512f()
            && ext.has_avx512bw()
            && ext.has_avx512cd()
            && ext.has_avx512dq()
            && ext.has_avx512vl()
    } else {
        false
    }
}

// Checks if the CPU is Zen 4 or Zen 5 (optimized)
pub fn is_cpu_znver4_compatible() -> bool {
    let cpuid = raw_cpuid::CpuId::new();

    // 1. Must support v4 features
    if !is_cpu_v4_compatible() {
        return false;
    }

    // 2. Check for AuthenticAMD vendor
    let is_amd = cpuid
        .get_vendor_info()
        .map(|v| v.as_str() == "AuthenticAMD")
        .unwrap_or(false);
    if !is_amd {
        return false;
    }

    // 3. Detect Zen 4/5 via Leaf 7 Sub-leaf 1 (AVX512-VNNI, BF16, etc.)
    // Zen 4 specific: AVX512_VNNI, AVX512_BF16, AVX512_VBMI2 etc.
    if let Some(ext) = cpuid.get_extended_feature_info() {
        // We look for flags introduced in Zen 4 (AVX512-VNNI is one, but Intel has it too)
        // AuthenticAMD + AVX512F + BIT ALGORITHM/VPOPCNTDQ is a good indicator of Zen 4
        ext.has_avx512vnni() && ext.has_avx512bitalg()
    } else {
        false
    }
}

/// Strips common package suffixes like -bin, -git, -nightly
pub fn strip_package_suffix(name: &str) -> &str {
    // Ordered by length (longest first) to match specific first?
    // Actually -bin and -git are most common.
    // If strict match needed, verify with list.
    let suffixes = [
        "-bin",
        "-git",
        "-nightly",
        "-beta",
        "-dev",
        "-pure",
        "-appimage",
        "-wayland",
        "-x11",
        "-hg",
        "-svn",
        "-cn",
        "-fresh",
        "-still",
        "-native",
        "-runtime",
        "-lts",
        "-edge",
        "-stable",
    ];

    for suffix in suffixes {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped;
        }
    }
    name
}

/// Merges official/appstream packages with repository packages, handling deduplication.
/// This logic was extracted from lib.rs to allow for unit testing.
#[allow(dead_code)]
pub fn merge_and_deduplicate(
    mut base_packages: Vec<models::Package>,
    repo_results: Vec<models::Package>,
) -> Vec<models::Package> {
    // Track seen App IDs or Normalized Names to prevent duplicates
    let mut grouping_map: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for (i, p) in base_packages.iter().enumerate() {
        if let Some(id) = &p.app_id {
            grouping_map.insert(id.clone(), i);
        } else {
            // Fallback: use normalized name
            grouping_map.insert(strip_package_suffix(&p.name).to_string(), i);
        }
    }

    for mut pkg in repo_results {
        // 1. Check Exact Name Match
        if let Some(idx) = base_packages.iter().position(|p| p.name == pkg.name) {
            // Merge logic...
            if pkg.source.priority() < base_packages[idx].source.priority() {
                let mut old_primary = std::mem::replace(&mut base_packages[idx], pkg);
                let alternatives = old_primary.alternatives.take().unwrap_or_default();
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .extend(alternatives);
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .push(old_primary);
            } else {
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .push(pkg);
            }
            continue;
        }

        // 2. Check Grouping Match (App ID or Normalized Name)
        let group_key = pkg
            .app_id
            .clone()
            .unwrap_or_else(|| strip_package_suffix(&pkg.name).to_string());

        if let Some(&idx) = grouping_map.get(&group_key) {
            // Priority Swap Logic
            if pkg.source.priority() < base_packages[idx].source.priority() {
                let mut old_primary = std::mem::replace(&mut base_packages[idx], pkg);
                let alternatives = old_primary.alternatives.take().unwrap_or_default();
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .extend(alternatives);
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .push(old_primary);
            } else {
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .push(pkg);
            }
            continue;
        }

        // 3. New Entry
        pkg.display_name = Some(to_pretty_name(&pkg.name));
        pkg.alternatives = None;
        grouping_map.insert(group_key, base_packages.len());
        base_packages.push(pkg);
    }

    base_packages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Package, PackageSource};

    fn make_pkg(name: &str, source: PackageSource, votes: Option<u32>) -> Package {
        Package {
            name: name.to_string(),
            display_name: None,
            description: "".to_string(),
            version: "1.0".to_string(),
            source,
            maintainer: None,
            license: None,
            url: None,
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: votes,
            icon: None,
            screenshots: None,
            provides: None,
            app_id: None,
            is_optimized: None,
            depends: None,
            make_depends: None,
            is_featured: None,
            installed: false,
            ..Default::default()
        }
    }

    #[test]
    fn test_search_ranking() {
        let mut pkgs = vec![
            make_pkg("open-chrome", PackageSource::Aur, Some(50)),
            make_pkg("google-chrome", PackageSource::Chaotic, Some(1000)),
            make_pkg("chrome-gnome-shell", PackageSource::Official, Some(200)),
        ];

        sort_packages_by_relevance(&mut pkgs, "chrome");

        assert_eq!(pkgs[0].name, "google-chrome"); // Rank 0 (Common)
        assert_eq!(pkgs[1].name, "chrome-gnome-shell"); // Official (Rank 3)
        assert_eq!(pkgs[2].name, "open-chrome"); // Aur (Rank 4)
    }

    #[test]
    fn test_deduplication_priority_swap() {
        // Manjaro (Low Priority: 4)
        let manjaro = make_pkg("spotify", PackageSource::Manjaro, None);
        // Chaotic (High Priority: 1)
        let chaotic = make_pkg("spotify", PackageSource::Chaotic, None);

        let results = merge_and_deduplicate(vec![manjaro], vec![chaotic]);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, PackageSource::Chaotic); // Should have swapped to Chaotic
        assert_eq!(results[0].alternatives.as_ref().unwrap().len(), 1);
        assert_eq!(
            results[0].alternatives.as_ref().unwrap()[0].source,
            PackageSource::Manjaro
        );
    }
}

pub async fn run_privileged_script_with_progress(
    app: tauri::AppHandle,
    event_name: &str,
    script: &str,
    password: Option<String>,
    bypass_helper: bool,
) -> Result<String, String> {
    use tauri::Emitter;
    use tokio::io::{AsyncBufReadExt, BufReader};

    let helper_exists = std::path::Path::new(MONARCH_PK_HELPER).exists();

    // Acquire global lock to serialize privileged prompts (prevents multiple dialogs)
    let _guard = PRIVILEGED_LOCK.lock().await;

    let (program, args) = if let Some(_) = &password {
        ("sudo", vec!["-S", "bash", "-s"])
    } else if helper_exists && !bypass_helper {
        ("pkexec", vec![MONARCH_PK_HELPER, "bash", "-s"])
    } else {
        ("pkexec", vec!["/bin/bash", "-s"])
    };

    let mut child = tokio::process::Command::new(program)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", program, e))?;

    if let Some(mut stdin) = child.stdin.take() {
        if let Some(pwd) = &password {
            let _ = stdin.write_all(format!("{}\n", pwd).as_bytes()).await;
        }
        let _ = stdin.write_all(script.as_bytes()).await;
    }

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let app_clone = app.clone();
    let event_name_clone = event_name.to_string();
    let stdout_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = app_clone.emit(&event_name_clone, &line);
        }
    });

    let app_clone = app.clone();
    let event_name_clone = event_name.to_string();
    let stderr_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = app_clone.emit(&event_name_clone, format!("ERROR: {}", line));
        }
    });

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait on {}: {}", program, e))?;

    let _ = tokio::join!(stdout_handle, stderr_handle);

    if status.success() {
        Ok("Success".to_string())
    } else {
        Err("Privileged Action Failed. Check logs for details.".to_string())
    }
}

pub async fn run_privileged_script(
    script: &str,
    password: Option<String>,
    bypass_helper: bool,
) -> Result<String, String> {
    let helper_exists = std::path::Path::new(MONARCH_PK_HELPER).exists();

    // Acquire global lock to serialize privileged prompts
    let _guard = PRIVILEGED_LOCK.lock().await;

    let (program, args) = if let Some(_) = &password {
        ("sudo", vec!["-S", "bash", "-s"])
    } else if helper_exists && !bypass_helper {
        ("pkexec", vec![MONARCH_PK_HELPER, "bash", "-s"])
    } else {
        ("pkexec", vec!["/bin/bash", "-s"])
    };

    let mut child = tokio::process::Command::new(program)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", program, e))?;

    if let Some(mut stdin) = child.stdin.take() {
        if let Some(pwd) = &password {
            let _ = stdin.write_all(format!("{}\n", pwd).as_bytes()).await;
        }
        let _ = stdin.write_all(script.as_bytes()).await;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("Failed to wait on {}: {}", program, e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "Privileged Action Failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub enum InstallMode {
    System,
    Portable,
    Dev, // Useful for debugging
}

pub fn get_install_mode() -> InstallMode {
    if let Ok(exe_path) = std::env::current_exe() {
        let path_str = exe_path.to_string_lossy();

        // 1. System Install (Pacman)
        // Usually /usr/bin/monarch-store
        if path_str.starts_with("/usr/bin") || path_str.starts_with("/bin") {
            return InstallMode::System;
        }

        // 2. AppImage (Mounted)
        // Usually /tmp/.mount_monarcXXXXXX/usr/bin/monarch-store or similar
        // BUT the actual AppImage *file* is what we care about updates for.
        // However, we just need to know "Are we managed by Pacman?".
        // If we are NOT in /usr/bin, we are likely portable or dev.

        // 3. Dev Mode
        if path_str.contains("/target/debug/") || path_str.contains("/target/release/") {
            return InstallMode::Dev;
        }
    }

    // Default to Portable for AppImages, manual builds in /home, etc.
    InstallMode::Portable
}

/// Safely tracks an event ONLY if telemetry is enabled in configuration.
/// This is the "Backend Gatekeeper" ensuring privacy compliance.
pub async fn track_event_safe(
    app: &tauri::AppHandle,
    event: &str,
    payload: Option<serde_json::Value>,
) {
    use crate::repo_manager::RepoManager;
    use tauri::Manager;
    use tauri_plugin_aptabase::EventTracker;

    let state = app.state::<RepoManager>();
    if state.is_telemetry_enabled().await {
        // Log locally for debugging privacy
        #[cfg(debug_assertions)]
        println!("[Telemetry] Sending: {} {:?}", event, payload);

        // Send to Aptabase
        let _ = app.track_event(event, payload);
    } else {
        #[cfg(debug_assertions)]
        println!("[Telemetry] BLOCKED (Consent Denied): {}", event);
    }
}
pub async fn run_pacman_command_transparent(
    app: tauri::AppHandle,
    action_args: Vec<String>,
    password: Option<String>,
) -> Result<(), String> {
    use crate::distro_context::DistroContext;
    use crate::distro_context::DistroId;
    use crate::error_classifier::ClassifiedError;
    use tauri::Emitter;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // 1. Manjaro Safety Guard (Protocol v0.3.5 Phase 4)
    let distro = DistroContext::new();
    if distro.id == DistroId::Manjaro {
        let has_sy = action_args
            .iter()
            .any(|a| a.contains("-Sy") || a.contains("-Syy"));
        let has_u = action_args.iter().any(|a| a.contains("u"));
        if has_sy && !has_u {
            let msg = "Manjaro Stability Guard: Partial upgrades (-Sy without -u) are blocked to prevent system breakage.".to_string();
            let _ = app.emit("install-output", &msg);
            return Err(msg);
        }
    }

    // 2. Build the command
    let (binary, args) = crate::commands::utils::build_pacman_cmd(
        &action_args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &password,
    );

    // Acquire global lock to serialize privileged prompts
    let _guard = PRIVILEGED_LOCK.lock().await;

    let mut child = tokio::process::Command::new(binary)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn pacman command: {}", e))?;

    // 3. Handle password if using sudo
    if let Some(pwd) = password {
        if let Some(mut s) = child.stdin.take() {
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut s, format!("{}\n", pwd).as_bytes()).await;
        }
    }

    // 4. Stream Output with Error Collection
    // Collect stderr for error classification
    let error_buffer: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let app_clone = app.clone();
    let stdout = child.stdout.take().unwrap();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = app_clone.emit("install-output", line);
        }
    });

    let app_clone = app.clone();
    let stderr = child.stderr.take().unwrap();
    let error_buffer_clone = error_buffer.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            // Store for later classification
            {
                let mut buf = error_buffer_clone.lock().await;
                buf.push(line.clone());
            }
            let _ = app_clone.emit("install-output", format!("ERROR: {}", line));
        }
    });

    // 5. Wait for completion
    let status = child.wait().await.map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        // 6. Classify the error and emit structured event for UI recovery actions
        let errors = error_buffer.lock().await;
        let combined_output = errors.join("\n");
        
        if let Some(classified) = ClassifiedError::from_output(&combined_output) {
            // Emit structured error event for the UI to show recovery options
            let _ = app.emit("install-error-classified", &classified);
            Err(format!("{}: {}", classified.title, classified.description))
        } else {
            Err("Pacman operation failed. Check logs for details.".to_string())
        }
    }
}

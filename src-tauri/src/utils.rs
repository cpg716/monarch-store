use std::process::Stdio;
use tokio::io::AsyncWriteExt;

pub const MONARCH_PK_HELPER: &str = "/usr/bin/monarch-pk-helper";

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
    let required_flags = [
        "avx", "avx2", "bmi1", "bmi2", "f16c", "fma", "movbe", "xsave",
    ];

    // v3 requires all above + (lzcnt OR abm)
    if !check_cpu_flags(&required_flags[..]) {
        return false;
    }

    check_cpu_flags(&["lzcnt"][..]) || check_cpu_flags(&["abm"][..])
}

// Checks if the CPU supports x86-64-v4 (AVX512F, AVX512BW, AVX512CD, AVX512DQ, AVX512VL)
pub fn is_cpu_v4_compatible() -> bool {
    // v4 requires v3 + AVX512
    if !is_cpu_v3_compatible() {
        return false;
    }
    let required_flags = ["avx512f", "avx512bw", "avx512cd", "avx512dq", "avx512vl"];
    check_cpu_flags(&required_flags[..])
}

// Checks if the CPU is Zen 4 or Zen 5 (optimized)
pub fn is_cpu_znver4_compatible() -> bool {
    // 1. Must support v4 features (AVX512, etc)
    if !is_cpu_v4_compatible() {
        return false;
    }

    // 2. Check for Zen 4/5 specific identifiers
    if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
        let content_lower = content.to_lowercase();
        let is_amd = content_lower.contains("authenticamd");

        // Zen 4 (7000/8000/9000 series) uses AVX-512 and several specific instruction patterns
        // We look for 'avx512_bf16' or 'avx512_fp16' which are specific to newer Zen architectures
        let has_zen4_flags =
            content_lower.contains("avx512_bf16") || content_lower.contains("avx512_fp16");

        if is_amd && has_zen4_flags {
            return true;
        }

        // Fallback to model name check if flags are masked
        if is_amd && content_lower.contains("model name") {
            if content_lower.contains("7000")
                || content_lower.contains("8000")
                || content_lower.contains("9000")
            {
                return true;
            }
        }
    }
    false
}

fn check_cpu_flags(required: &[&str]) -> bool {
    if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
        if let Some(flags_line) = content.lines().find(|l| {
            l.to_lowercase().starts_with("flags") || l.to_lowercase().starts_with("features")
        }) {
            let cpu_flags = flags_line.to_lowercase();
            return required.iter().all(|flag| cpu_flags.contains(flag));
        }
    }
    false
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
    // Track seen App IDs to prevent duplicates (e.g. brave-bin vs brave)
    let mut app_id_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for (i, p) in base_packages.iter().enumerate() {
        if let Some(id) = &p.app_id {
            app_id_map.insert(id.clone(), i);
        }
    }

    for mut pkg in repo_results {
        // 1. Check Exact Name Match
        if let Some(idx) = base_packages.iter().position(|p| p.name == pkg.name) {
            // Priority Swap Logic:
            // If the incoming repo package has HIGHER priority (lower number) than the existing base package,
            // we swap them so the primary listing reflects the best source.
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

        // 2. Check App ID Match
        if let Some(id) = &pkg.app_id {
            if let Some(&idx) = app_id_map.get(id) {
                // Priority Swap Logic (Same as above)
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
            app_id_map.insert(id.clone(), base_packages.len());
        }

        pkg.display_name = Some(to_pretty_name(&pkg.name));
        pkg.alternatives = None;
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
            alternatives: None,
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

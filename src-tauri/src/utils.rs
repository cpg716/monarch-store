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
                        None => String::new(),
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

            // Rank 3: Official Source
            if pkg.source == models::PackageSource::Official {
                return 3;
            }

            // Rank 4: Others
            4
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
        "avx", "avx2", "bmi1", "bmi2", "f16c", "fma", "lzcnt", "movbe", "xsave",
    ];
    check_cpu_flags(&required_flags)
}

// Checks if the CPU supports x86-64-v4 (AVX512F, AVX512BW, AVX512CD, AVX512DQ, AVX512VL)
pub fn is_cpu_v4_compatible() -> bool {
    // v4 requires v3 + AVX512
    if !is_cpu_v3_compatible() {
        return false;
    }
    let required_flags = ["avx512f", "avx512bw", "avx512cd", "avx512dq", "avx512vl"];
    check_cpu_flags(&required_flags)
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
        if let Some(flags_line) = content
            .lines()
            .find(|l| l.starts_with("flags") || l.starts_with("Features"))
        {
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
    ];

    for suffix in suffixes {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped;
        }
    }
    name
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
}

/// Merges official/appstream packages with repository packages, handling deduplication.
/// This logic was extracted from lib.rs to allow for unit testing.
#[allow(dead_code)]
pub fn merge_and_deduplicate(
    mut base_packages: Vec<models::Package>,
    repo_results: Vec<models::Package>,
) -> Vec<models::Package> {
    // Track seen App IDs to prevent duplicates (e.g. brave-bin vs brave)
    let mut seen_app_ids: std::collections::HashSet<String> = base_packages
        .iter()
        .filter_map(|p| p.app_id.clone())
        .collect();

    for mut pkg in repo_results {
        // Skip if name exists exactly
        if base_packages.iter().any(|p| p.name == pkg.name) {
            continue;
        }

        // Note: Icon/AppID heuristics would normally go here, but they require AppStreamLoader state.
        // For this pure function, we assume AppIDs are already populated or will be handled by the caller
        // if they depend on state.
        // However, for correct deduplication testing, we must respect the app_id if present.

        if let Some(id) = &pkg.app_id {
            if seen_app_ids.contains(id) {
                continue;
            }
            seen_app_ids.insert(id.clone());
        }

        pkg.display_name = Some(to_pretty_name(&pkg.name));
        base_packages.push(pkg);
    }

    base_packages
}

use crate::{
    aur_api, chaotic_api, metadata, models, pkgstats_api, repo_manager::RepoManager, utils,
};
use serde::Serialize;
use std::collections::HashMap;
use tauri::State;

#[derive(Serialize)]
pub struct PaginatedResponse {
    pub packages: Vec<models::Package>,
    pub total: usize,
    pub page: usize,
    pub has_more: bool,
}

use crate::flathub_api::FlathubApiClient;
use crate::models::{Package, PackageSource};

// Helper to normalize names for merging (e.g. "Firefox" -> "firefox")
fn normalize_name(s: &str) -> String {
    s.trim().to_lowercase()
}

// Phase 1: The "Distro Dictionary"
// Maps specific repository names + distro ID to Friendly Labels
fn get_friendly_label(db_name: &str, distro_id: &str) -> &'static str {
    match db_name {
        // --- The Big Players ---
        "core" | "extra" | "multilib" => match distro_id {
            "manjaro" => "Manjaro Official",
            "endeavouros" => "EndeavourOS (Arch)",
            "garuda" => "Garuda (Arch)",
            "cachyos" => "CachyOS (Arch)",
            "steamos" => "SteamOS (Arch)", // SteamOS often mirrors core/extra
            "chimeraos" => "ChimeraOS (Arch)",
            "arcolinux" => "ArcoLinux (Arch)",
            "rebornos" => "RebornOS (Arch)",
            "artix" => "Artix Linux",
            "biglinux" => "BigLinux (Arch)",
            "mabox" => "Mabox (Manjaro Base)",
            _ => "Arch Official", // Default fallback
        },

        // --- SteamOS & Gaming ---
        "jupiter" | "jupiter-rel" | "jupiter-main" => "SteamOS (Jupiter)",
        "holo" | "holo-rel" | "holo-main" => "SteamOS (Holo)",
        "chimeraos" | "chimeraos-extra" => "ChimeraOS (Gaming)",
        "gamer-os" => "GamerOS",

        // --- Performance & Optimization ---
        "cachyos" | "cachyos-v3" | "cachyos-v4" => "CachyOS (Optimized)",
        "chaotic-aur" => "Chaotic-AUR (Pre-built)",

        // --- Specialized Distros ---
        "endeavouros" => "EndeavourOS Tools",
        "garuda" => "Garuda Tools",
        "arcolinux_repo" | "arcolinux_repo_3party" => "ArcoLinux Repo",
        "rebornos" => "RebornOS Repo",
        "blackarch" => "BlackArch (Security)",
        "xerolinux_repo" => "XeroLinux Repo",
        "mabox" => "Mabox Tools",
        "alg-repo" => "ArchLabs",
        "athena" => "Athena OS",
        "biglinux-stable" | "biglinux-testing" => "BigLinux Repo",
        "bluestar" => "Bluestar Linux",
        "obarun" => "Obarun",
        "parabola" => "Parabola (Libre)",
        "hyperbola" => "Hyperbola",
        "ctlos" => "CtlOS",
        "alci-repo" => "ALCI",

        // --- Universal ---
        "aur" => "AUR (Community)",
        "flatpak" => "Flatpak (Sandboxed)",
        _ => "Custom Repository", // Catch-all for obscure distros
    }
}

#[tauri::command]
pub async fn search_packages(
    state_repo: State<'_, RepoManager>,
    state_flathub: State<'_, FlathubApiClient>,
    state_metadata: State<'_, metadata::MetadataState>,
    state_distro: State<'_, crate::distro_context::DistroContext>,
    query: String,
) -> Result<Vec<Package>, String> {
    if query.len() < 2 {
        return Ok(Vec::new());
    }

    let query_lower = query.to_lowercase();
    let repo_manager = state_repo.inner();
    let flathub = state_flathub.inner();
    // aur_api is stateless/lazy_static accessible directly

    // 1. Parallel Search
    // We use tokio::join to run searches concurrently
    let (official_res, aur_res, flatpak_res) = tokio::join!(
        repo_manager.get_packages_matching(&query, state_distro.inner()),
        crate::aur_api::search_aur(&query),
        flathub.search_flathub(&query)
    );

    // 2. Merge Logic
    // We want a map of unique packages keyed by "normalized name"
    // Priority: Official > Flatpak > AUR (implicit by order of processing if we overwrite? logic needs care)
    // We will accumulate available_sources.

    let mut package_map: HashMap<String, Package> = HashMap::new();

    // A. Process Official (Highest Priority Base)
    // A. Process Official (Highest Priority Base)
    if let Ok(pkgs) = official_res {
        for mut p in pkgs {
            // "Grand Unification": Apply Distro Identity Logic
            let distro_id_str = match &state_distro.id {
                crate::distro_context::DistroId::Manjaro => "manjaro",
                crate::distro_context::DistroId::Garuda => "garuda",
                crate::distro_context::DistroId::CachyOS => "cachyos",
                crate::distro_context::DistroId::EndeavourOS => "endeavouros",
                crate::distro_context::DistroId::Arch => "arch",
                crate::distro_context::DistroId::Unknown(s) => s.as_str(),
            };

            p.source.label = get_friendly_label(&p.source.id, distro_id_str).to_string();

            // Initialize available_sources with its own source
            p.available_sources = Some(vec![p.source.clone()]);

            let key = normalize_name(&p.name);
            package_map.insert(key, p);
        }
    }

    // B. Process Flatpak
    if let Some(hits) = flatpak_res {
        for hit in hits {
            // --- SMART MERGE LOGIC (Global Fix) ---
            let direct_key = normalize_name(&hit.name);
            let flatpak_base = crate::utils::strip_package_suffix(&direct_key);

            // 1. Find Match in Map
            let mut match_key = None;

            // Priority 1: Direct Name Match
            if package_map.contains_key(&direct_key) {
                match_key = Some(direct_key.clone());
            }
            // Priority 2: Fuzzy Base Name Match (e.g. Brave -> brave == brave-bin -> brave)
            else {
                for k in package_map.keys() {
                    let repo_base = crate::utils::strip_package_suffix(k);
                    if repo_base == flatpak_base {
                        match_key = Some(k.clone());
                        break;
                    }
                }
            }

            // Priority 3: Scan for matching App ID or Suffix
            if match_key.is_none() {
                let suffix_part = hit
                    .app_id
                    .split('.')
                    .last()
                    .map(normalize_name)
                    .unwrap_or_default();

                for (k, pkg) in &package_map {
                    // Check explicit App ID match
                    if let Some(pkg_id) = &pkg.app_id {
                        if pkg_id.eq_ignore_ascii_case(&hit.app_id) {
                            match_key = Some(k.clone());
                            break;
                        }
                    }
                    // Check if App ID Suffix matches Repo Key (e.g. com.visualstudio.code -> code)
                    if !suffix_part.is_empty() && k == &suffix_part {
                        match_key = Some(k.clone());
                        break;
                    }
                }
            }

            if let Some(key) = match_key {
                // UPDATE existing
                if let Some(existing) = package_map.get_mut(&key) {
                    // Add Flatpak to sources
                    if let Some(sources) = &mut existing.available_sources {
                        if !sources.iter().any(|s| s.source_type == "flatpak") {
                            sources.push(PackageSource::new(
                                "flatpak",
                                "flathub",
                                "latest",
                                "Flatpak (Sandboxed)",
                            ));
                        }
                    }
                    // Update App ID if not set
                    if existing.app_id.is_none() {
                        existing.app_id = Some(hit.app_id);
                    }
                }
            } else {
                // NEW Flatpak-only package
                // Use direct name as key
                let p = Package {
                    name: hit.name.clone(),
                    display_name: Some(hit.name), // Flatpak names are display names
                    description: hit.summary.unwrap_or_default(),
                    version: "latest".to_string(), // Metadata fetch needed for real version?
                    source: PackageSource::new(
                        "flatpak",
                        "flathub",
                        "latest",
                        "Flatpak (Sandboxed)",
                    ),
                    maintainer: None,
                    license: None,
                    url: None, // Could link flathub
                    last_modified: None,
                    first_submitted: None,
                    out_of_date: None,
                    keywords: None,
                    num_votes: None,
                    icon: hit.icon,
                    screenshots: None,
                    provides: None,
                    app_id: Some(hit.app_id),
                    is_optimized: None,
                    depends: None,
                    make_depends: None,
                    is_featured: None,
                    installed: false, // Check installed?? (TODO: Phase 3 check)
                    download_size: None,
                    installed_size: None,
                    alternatives: None,
                    available_sources: Some(vec![PackageSource::new(
                        "flatpak",
                        "flathub",
                        "latest",
                        "Flatpak (Sandboxed)",
                    )]),
                };
                package_map.insert(direct_key, p);
            }
        }
    }

    // C. Process AUR
    // Only if enabled in settings? (We assume checking inside aur.search_aur or we check here)
    // For now we assume we always search if the module is active.
    if let Ok(pkgs) = aur_res {
        for mut p in pkgs {
            let key = normalize_name(&p.name);

            if let Some(existing) = package_map.get_mut(&key) {
                // MERGE AUR info
                if let Some(sources) = &mut existing.available_sources {
                    if !sources.iter().any(|s| s.source_type == "aur") {
                        sources.push(p.source.clone());
                    }
                }
                // If official/flatpak didn't have description, maybe AUR does? (Unlikely to be better than official)
                // But we CAN populate AUR-specific fields if we want to show them in "Details".
                // For the list item, the existing (Official/Flatpak) takes precedence.
            } else {
                // NEW AUR package
                p.available_sources = Some(vec![p.source.clone()]);
                package_map.insert(key, p);
            }
        }
    }

    // 3. Relevance Scoring & Sorting ("Smart Sort")
    let metadata_loader = state_metadata.0.lock().map_err(|e| e.to_string())?;

    // Hardcoded list of "Popular" apps to boost (Phase 2)
    let popular_apps = [
        "firefox",
        "google-chrome",
        "chromium",
        "brave-bin",
        "brave-browser",
        "steam",
        "spotify",
        "discord",
        "vlc",
        "obs-studio",
        "gimp",
        "inkscape",
        "blender",
        "visual-studio-code-bin",
        "code",
        "vscode",
        "telegram-desktop",
        "signal-desktop",
        "slack-desktop",
        "zoom",
        "teams",
        "libreoffice-fresh",
        "thunderbird",
        "lutris",
        "neovim",
        "kitty",
        "alacritty",
    ];

    let mut results: Vec<Package> = package_map.into_values().collect();

    // 4. Apply Friendly Names (The "Smart Search" Polish)
    // We update the display_name of packages using our registry
    for pkg in &mut results {
        if let Some(friendly) = metadata_loader.get_friendly_name(&pkg.name) {
            pkg.display_name = Some(friendly);
        }
    }

    results.sort_by(|a, b| {
        let score_a = calculate_relevance(a, &query_lower, &metadata_loader, &popular_apps);
        let score_b = calculate_relevance(b, &query_lower, &metadata_loader, &popular_apps);

        // Descending score
        score_b
            .cmp(&score_a)
            // If scores equal, fallback to shortest name
            .then_with(|| a.name.len().cmp(&b.name.len()))
            // Finally alphabetical
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(results)
}

fn calculate_relevance(
    pkg: &Package,
    query: &str,
    metadata: &metadata::AppStreamLoader,
    popular_apps: &[&str],
) -> u32 {
    let pkg_name_lower = pkg.name.to_lowercase();
    let display_name_lower = pkg.display_name.as_ref().map(|s| s.to_lowercase());
    let friendly_name = metadata
        .get_friendly_name(&pkg.name)
        .map(|s| s.to_lowercase());

    // 1. Exact Name Match (Score 100)
    if pkg_name_lower == query {
        return 100;
    }

    // 2. Exact Friendly Name Match
    if let Some(friendly) = &friendly_name {
        if friendly == query {
            return 100;
        }
        if friendly.contains(query) && query.len() >= 3 {
            return 95;
        }
    }

    // 3. Exact App ID Match (Score 90)
    if let Some(app_id) = &pkg.app_id {
        if app_id.to_lowercase() == query {
            return 90;
        }
    }

    // 4. Popular Flag (Score 80)
    let is_popular = popular_apps.contains(&pkg_name_lower.as_str());
    let matches_query = pkg_name_lower.contains(query)
        || display_name_lower.as_deref().unwrap_or("").contains(query)
        || pkg
            .keywords
            .as_ref()
            .map(|k| k.iter().any(|w| w.to_lowercase().contains(query)))
            .unwrap_or(false);

    if is_popular && matches_query {
        return 80;
    }

    // 5. Starts with Query (Score 50)
    if pkg_name_lower.starts_with(query) {
        return 50;
    }

    // 6. Contains Query (Score 20)
    if matches_query {
        return 20;
    }

    0
}

#[tauri::command]
pub async fn search_aur(query: String) -> Result<Vec<models::Package>, String> {
    aur_api::search_aur(&query).await
}

#[tauri::command]
pub async fn get_packages_by_names(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, RepoManager>,
    names: Vec<String>,
) -> Result<Vec<models::Package>, String> {
    let mut packages = Vec::new();
    // ALPM: empty repo list = search ALL syncdbs (core, extra, community, multilib + monarch)
    // so combined listing includes Official repos (e.g. Lutris from community).
    let names_clone = names.clone();
    let repo_pkgs = tokio::task::spawn_blocking(move || {
        crate::alpm_read::get_packages_batch(&names_clone, &[])
    })
    .await
    .map_err(|e| e.to_string())?;
    for mut pkg in repo_pkgs {
        if pkg.icon.is_none() || pkg.app_id.is_none() {
            if let Ok(loader) = state_meta.inner().0.lock() {
                if pkg.icon.is_none() {
                    pkg.icon = loader.find_icon_heuristic(&pkg.name);
                }
                if pkg.app_id.is_none() {
                    pkg.app_id = loader.find_app_id(&pkg.name);
                }
            }
        }
        pkg.display_name = Some(utils::to_pretty_name(&pkg.name));
        packages.push(pkg);
    }

    // Fetch Chaotic Packages for ALL names to allow for alternatives/deduplication
    // (Essentials need the Version Selector too!)
    let chaotic_enabled = state_repo.inner().is_repo_enabled("chaotic-aur").await;
    let chaotic_pkgs = if chaotic_enabled {
        state_chaotic
            .inner()
            .get_packages_batch(names.clone())
            .await
    } else {
        std::collections::HashMap::new()
    };

    for (name, p) in chaotic_pkgs {
        let mut pkg = models::Package {
            name: name.clone(),
            display_name: Some(utils::to_pretty_name(&name)),
            description: p
                .metadata
                .as_ref()
                .and_then(|m| m.desc.clone())
                .unwrap_or_default(),
            version: p.version.clone().unwrap_or_default(),
            source: models::PackageSource::chaotic(),
            maintainer: Some("Chaotic-AUR Team".to_string()),
            license: p
                .metadata
                .as_ref()
                .and_then(|m| m.license.clone())
                .map(|l| vec![l]),
            url: p.metadata.as_ref().and_then(|m| m.url.clone()),
            installed: false,
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: None,
            icon: {
                let mut icon = None;
                if let Ok(loader) = state_meta.inner().0.lock() {
                    icon = loader.find_icon_heuristic(&name);
                }
                icon
            },
            screenshots: None,
            provides: None,
            app_id: None,
            is_optimized: None,
            depends: None,
            make_depends: None,
            is_featured: None,
            ..Default::default()
        };

        if let Ok(loader) = state_meta.inner().0.lock() {
            pkg.app_id = loader.find_app_id(&name);
        }
        packages.push(pkg);
    }

    // 3. AUR Fallback & Local Enhancement (Crucial for Essentials and accurate labeling of foreign pkgs)
    let aur_enabled = state_repo.inner().is_aur_enabled().await;
    if aur_enabled {
        let existing_names: std::collections::HashSet<String> =
            packages.iter().map(|p| p.name.clone()).collect();
        let missing_names: Vec<String> = names
            .iter()
            .filter(|n| !existing_names.contains(*n))
            .cloned()
            .collect();

        // Also identify 'Local' packages that we should check against AUR to see if they are actually from AUR
        let local_names: Vec<String> = packages
            .iter()
            .filter(|p| p.source.source_type == "local")
            .map(|p| p.name.clone())
            .collect();

        // Combine missing and local for a single batch query
        let mut query_names = missing_names;
        query_names.extend(local_names);
        query_names.sort();
        query_names.dedup();

        if !query_names.is_empty() {
            let query_refs: Vec<&str> = query_names.iter().map(|s| s.as_str()).collect();
            if let Ok(aur_results) = aur_api::get_multi_info(&query_refs).await {
                for mut pkg in aur_results {
                    if let Some(existing) = packages.iter_mut().find(|p| p.name == pkg.name) {
                        // If it was labeled Local, but we found it in AUR, upgrade it
                        if existing.source.source_type == "local" {
                            existing.source = models::PackageSource::new(
                                "aur",
                                "aur",
                                &pkg.version,
                                "AUR (Community)",
                            );
                            existing.maintainer = pkg.maintainer;
                            existing.num_votes = pkg.num_votes;
                            existing.out_of_date = pkg.out_of_date;
                            existing.first_submitted = pkg.first_submitted;
                            existing.last_modified = pkg.last_modified;
                            if existing.url.is_none() {
                                existing.url = pkg.url;
                            }
                        }
                    } else {
                        // It's a missing package
                        if let Ok(loader) = state_meta.inner().0.lock() {
                            pkg.app_id = loader.find_app_id(&pkg.name);
                        }
                        pkg.display_name = Some(crate::utils::to_pretty_name(&pkg.name));
                        packages.push(pkg);
                    }
                }
            }
        }
    }

    // UNIFIED DEDUPLICATION
    packages = utils::merge_and_deduplicate(Vec::new(), packages);

    Ok(packages)
}

#[tauri::command]
pub async fn get_chaotic_package_info(
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    name: String,
) -> Result<Option<chaotic_api::ChaoticPackage>, String> {
    Ok(state_chaotic.inner().find_package(&name).await)
}

#[tauri::command]
pub async fn get_chaotic_packages_batch(
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    names: Vec<String>,
) -> Result<HashMap<String, chaotic_api::ChaoticPackage>, String> {
    state_chaotic.inner().get_packages_by_names(&names).await
}

#[tauri::command]
pub async fn get_trending(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, RepoManager>,
) -> Result<Vec<models::Package>, String> {
    let mut packages = Vec::new();

    // SECTION 1: "The Titans" (Static Foundation)
    // Always fetch these to ensure the section is never empty and contains high-quality apps.
    let titan_names = vec![
        "firefox",
        "vlc",
        "obs-studio",
        "discord",
        "spotify",
        "steam",
        "visual-studio-code-bin",
        "gimp",
    ];

    if let Ok(loader) = state_meta.inner().0.lock() {
        for name in &titan_names {
            if let Some(app) = loader.find_package(name) {
                packages.push(models::Package {
                    name: app.pkg_name.clone().unwrap_or(app.app_id.clone()),
                    display_name: Some(app.name),
                    description: app.summary.unwrap_or_default(),
                    version: app.version.unwrap_or_else(|| "latest".to_string()),
                    source: models::PackageSource::new("repo", "core", "latest", "Arch Official"),
                    maintainer: None,
                    license: None,
                    url: None,
                    last_modified: None,
                    first_submitted: None,
                    out_of_date: None,
                    keywords: None,
                    num_votes: None,
                    icon: app.icon_url,
                    screenshots: if app.screenshots.is_empty() {
                        None
                    } else {
                        Some(app.screenshots)
                    },
                    provides: None,
                    app_id: Some(app.app_id.clone()),
                    is_optimized: None,
                    depends: None,
                    make_depends: None,
                    is_featured: Some(true),
                    ..Default::default()
                });
            }
        }
    }

    // SECTION 2: "Arch Pulse" (Real-world Popularity)
    // Fetch top packages from pkgstats (limit to top 8 to avoid overwhelming)
    // We only take packages that aren't already in "The Titans"
    if let Ok(arch_top) = pkgstats_api::fetch_top_packages(15).await {
        for mut pkg in arch_top {
            // Dedup against Titans
            if !packages.iter().any(|p| p.name == pkg.name) {
                // Try to hydrate metadata
                if let Ok(loader) = state_meta.inner().0.lock() {
                    if let Some(meta) = loader.find_package(&pkg.name) {
                        pkg.display_name = Some(meta.name);
                        pkg.description = meta.summary.unwrap_or(pkg.description);
                        pkg.icon = meta.icon_url;
                        pkg.app_id = Some(meta.app_id);
                    }
                }

                // Only add if it looks like a "desktop" app (has icon or we want to show it)
                // For now, we trust pkgstats but limit the count
                if pkg.icon.is_some()
                    || ["git", "neovim", "vim", "htop"].contains(&pkg.name.as_str())
                {
                    packages.push(pkg);
                }
            }
        }
    }

    // SECTION 3: "CachyOS Spotlight" (Curated Performance)
    // Only if CachyOS repos are enabled
    if state_repo.inner().is_repo_enabled("cachyos").await {
        let cachy_curated = vec![
            "linux-cachyos",
            "cachyos-settings",
            "cachyos-browser",
            "cachyos-fish-config",
        ];

        // We manually construct these since they might not be in AppStream data if typical repo data is missing
        // But for "trending" we can search them or just manually stub them.
        // Better yet, let's try to find them in metadata OR just stub them if missing.
        for name in cachy_curated {
            if !packages.iter().any(|p| p.name == name) {
                // Try metadata first
                let mut found = false;
                if let Ok(loader) = state_meta.inner().0.lock() {
                    if let Some(app) = loader.find_package(name) {
                        // Add from metadata...
                        packages.push(models::Package {
                            name: app.pkg_name.clone().unwrap_or(app.app_id.clone()),
                            display_name: Some(app.name),
                            description: app.summary.unwrap_or_default(),
                            version: app.version.unwrap_or_else(|| "optimized".to_string()),
                            source: models::PackageSource::cachyos(),
                            // ... fields ...
                            maintainer: Some("CachyOS Team".to_string()),
                            license: None,
                            url: None,
                            last_modified: None,
                            first_submitted: None,
                            out_of_date: None,
                            keywords: None,
                            num_votes: None,
                            icon: app.icon_url,
                            screenshots: None,
                            provides: None,
                            app_id: Some(app.app_id.clone()),
                            is_optimized: Some(true),
                            depends: None,
                            make_depends: None,
                            is_featured: Some(true),
                            installed: false,
                            alternatives: None,
                            ..Default::default()
                        });
                        found = true;
                    }
                }

                if !found {
                    // Fallback Stub
                    packages.push(models::Package {
                        name: name.to_string(),
                        display_name: Some(utils::to_pretty_name(name)),
                        description: "High-performance CachyOS component".to_string(),
                        version: "latest".to_string(),
                        source: models::PackageSource::new(
                            "repo",
                            "cachyos",
                            "optimized",
                            "CachyOS (Optimized)",
                        ),
                        maintainer: Some("CachyOS Team".to_string()),
                        license: None,
                        url: None,
                        last_modified: None,
                        first_submitted: None,
                        out_of_date: None,
                        keywords: None,
                        num_votes: None,
                        icon: None,
                        screenshots: None,
                        provides: None,
                        app_id: None,
                        is_optimized: Some(true),
                        depends: None,
                        make_depends: None,
                        is_featured: Some(true),
                        installed: false,
                        alternatives: None,
                        ..Default::default()
                    });
                }
            }
        }
    }

    // SECTION 4: "Chaotic Heat" (Dynamic Build Stats)
    let chaotic_enabled = state_repo.inner().is_repo_enabled("chaotic-aur").await;

    if chaotic_enabled {
        if let Ok(trending_list) = state_chaotic.inner().fetch_trending().await {
            let dynamic_names: Vec<String> = trending_list
                .iter()
                .map(|t| t.pkgbase_pkgname.clone())
                .collect();
            let chaotic_pkgs = state_chaotic
                .inner()
                .get_packages_batch(dynamic_names.clone())
                .await;

            for name in dynamic_names {
                if let Some(p) = chaotic_pkgs.get(&name) {
                    if !packages.iter().any(|pkg| pkg.name == name) {
                        let mut pkg = models::Package {
                            name: name.clone(),
                            display_name: Some(utils::to_pretty_name(&name)),
                            description: p
                                .metadata
                                .as_ref()
                                .and_then(|m| m.desc.clone())
                                .unwrap_or_default(),
                            version: p.version.clone().unwrap_or_default(),
                            source: models::PackageSource::new(
                                "repo",
                                "chaotic-aur",
                                &p.version.clone().unwrap_or_default(),
                                "Chaotic-AUR (Pre-built)",
                            ),
                            maintainer: Some("Chaotic-AUR Team".to_string()),
                            license: p
                                .metadata
                                .as_ref()
                                .and_then(|m| m.license.clone())
                                .map(|l| vec![l]),
                            url: p.metadata.as_ref().and_then(|m| m.url.clone()),
                            last_modified: None,
                            first_submitted: None,
                            out_of_date: None,
                            keywords: None,
                            num_votes: None,
                            icon: {
                                let mut icon = None;
                                if let Ok(loader) = state_meta.inner().0.lock() {
                                    icon = loader.find_icon_heuristic(&name);
                                }
                                icon
                            },
                            screenshots: None,
                            provides: None,
                            app_id: None,
                            is_optimized: None,
                            depends: None,
                            make_depends: None,
                            is_featured: None,
                            installed: false,
                            alternatives: None,
                            ..Default::default()
                        };
                        if let Ok(loader) = state_meta.inner().0.lock() {
                            pkg.app_id = loader.find_app_id(&name);
                        }
                        packages.push(pkg);
                    }
                }
            }
        }
    }

    // SECTION 5: "AUR Hot List" (Curated Community Favorites)
    // Only if AUR is enabled
    if state_repo.inner().is_aur_enabled().await {
        let aur_curated = vec![
            "google-chrome",
            "slack-desktop",
            "zoom",
            "visual-studio-code-bin", // In case it wasn't found in Titans (e.g. repo issue)
            "1password",
            "dropbox",
        ];

        // Helper to filter out already existing
        let needed_aur: Vec<&str> = aur_curated
            .into_iter()
            .filter(|n| !packages.iter().any(|p| p.name == *n))
            .collect();

        if !needed_aur.is_empty() {
            if let Ok(results) = aur_api::get_multi_info(&needed_aur).await {
                for p in results {
                    packages.push(models::Package {
                        name: p.name.clone(),
                        display_name: Some(utils::to_pretty_name(&p.name)),
                        description: p.description.clone(),
                        version: p.version.clone(),
                        source: models::PackageSource::new(
                            "aur",
                            "aur",
                            &p.version,
                            "AUR (Community)",
                        ),
                        maintainer: p.maintainer,
                        license: p.license,
                        url: p.url,
                        last_modified: p.last_modified,
                        first_submitted: p.first_submitted,
                        out_of_date: p.out_of_date,
                        keywords: p.keywords,
                        num_votes: p.num_votes,
                        icon: {
                            let mut icon = None;
                            if let Ok(loader) = state_meta.inner().0.lock() {
                                icon = loader.find_icon_heuristic(&p.name);
                            }
                            icon
                        },
                        screenshots: None,
                        provides: None,
                        app_id: None,
                        is_optimized: None,
                        depends: None,
                        make_depends: None,
                        is_featured: None,
                        installed: false,
                        alternatives: None,
                        ..Default::default()
                    });
                }
            }
        }
    }

    // UNIFIED DEDUPLICATION
    packages = utils::merge_and_deduplicate(Vec::new(), packages);
    Ok(packages)
}

#[tauri::command]
pub async fn get_package_variants(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_repo: State<'_, RepoManager>,
    pkg_name: String,
) -> Result<Vec<models::PackageVariant>, String> {
    let pkg_lower = pkg_name.to_lowercase();
    let base_name = utils::strip_package_suffix(&pkg_lower);
    // Resolve Mapping (e.g. brave -> com.brave.Browser)
    let mapped_id = crate::flathub_api::get_flathub_app_id(base_name);
    let app_id = state_meta
        .inner()
        .0
        .lock()
        .ok()
        .and_then(|loader| loader.find_app_id(&pkg_name));

    let mut combined_packages = Vec::new();

    // 1. Get all packages that match the BASE name or EXACT name
    let search_names = if base_name != pkg_lower {
        vec![base_name.to_string(), pkg_lower.clone()]
    } else {
        vec![pkg_lower.clone()]
    };

    // Repo Search: use empty list to search ALL syncdbs (core, extra, community, multilib + monarch)
    // so we show Official + Chaotic + AUR variants in one listing.
    let search_names_clone = search_names.clone();
    let repo_pkgs = tokio::task::spawn_blocking(move || {
        crate::alpm_read::get_packages_batch(&search_names_clone, &[])
    })
    .await
    .map_err(|e| e.to_string())?;
    combined_packages.extend(repo_pkgs);

    // Chaotic Search
    if state_repo.inner().is_repo_enabled("chaotic-aur").await {
        if let Ok(chaotic_arc) = state_chaotic.inner().fetch_packages().await {
            let matches: Vec<models::Package> = chaotic_arc
                .iter()
                .filter(|p| {
                    let p_lower = p.pkgname.to_lowercase();
                    p_lower == pkg_lower
                        || p_lower == base_name
                        || utils::strip_package_suffix(&p_lower) == base_name
                })
                .map(|p| models::Package {
                    name: p.pkgname.clone(),
                    display_name: Some(utils::to_pretty_name(&p.pkgname)),
                    description: p
                        .metadata
                        .as_ref()
                        .and_then(|m| m.desc.clone())
                        .unwrap_or_default(),
                    version: p.version.clone().unwrap_or_default(),
                    source: models::PackageSource::new(
                        "repo",
                        "chaotic-aur",
                        &p.version.clone().unwrap_or_default(),
                        "Chaotic-AUR (Pre-built)",
                    ),
                    maintainer: Some("Chaotic-AUR Team".to_string()),
                    license: p
                        .metadata
                        .as_ref()
                        .and_then(|m| m.license.clone())
                        .map(|l| vec![l]),
                    url: p.metadata.as_ref().and_then(|m| m.url.clone()),
                    last_modified: None,
                    first_submitted: None,
                    out_of_date: None,
                    keywords: None,
                    num_votes: None,
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
                })
                .collect();
            combined_packages.extend(matches);
        }
    }

    // AUR Search (Exact and Variants)
    if state_repo.inner().is_aur_enabled().await {
        // We search for the base name to find all variants
        if let Ok(aur_results) = aur_api::search_aur(base_name).await {
            combined_packages.extend(aur_results);
        }
    }

    // Flatpak Search (Exact and Variants)
    if let Some(flatpak_results) = state_flathub.inner().search_flathub(base_name).await {
        for hit in flatpak_results {
            combined_packages.push(models::Package {
                name: hit.app_id.clone(), // CRITICAL: Use AppID as name for variants to allow flatpak install
                display_name: Some(hit.name),
                source: models::PackageSource::new(
                    "flatpak",
                    "flathub",
                    "latest",
                    "Flatpak (Sandboxed)",
                ),
                app_id: Some(hit.app_id),
                ..Default::default()
            });
        }
    }

    // 2. Filter by App ID if we have one, otherwise by Normalized Name
    let mut final_variants: Vec<models::PackageVariant> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for p in combined_packages {
        let p_source = p.source.clone();
        let p_lower = p.name.to_lowercase();
        let p_app_id = state_meta
            .inner()
            .0
            .lock()
            .ok()
            .and_then(|loader| loader.find_app_id(&p.name));

        let matches_app_id = app_id.is_some() && p_app_id == app_id;

        // DEBUG LOG
        println!("DEBUG: Variant Check - PKG='{}' Source='{:?}' AppID='{:?}' MappedID='{:?}' BaseName='{}' (MatchAppID={}, EndsWith={}, MappedMatch={})",
            p.name, p.source.source_type, p_app_id, mapped_id, base_name,
            matches_app_id,
            p.name.to_lowercase().ends_with(&format!(".{}", base_name)),
            mapped_id.as_deref().map(|id| id.eq_ignore_ascii_case(&p.name)).unwrap_or(false)
        );

        // Smart Flatpak Match: Check if App ID ends with base name OR matches explicit mapping
        let is_flatpak_match = p.source.source_type == "flatpak"
            && (p.name.to_lowercase().ends_with(&format!(".{}", base_name))
                || mapped_id
                    .as_deref()
                    .map(|id| id.eq_ignore_ascii_case(&p.name))
                    .unwrap_or(false));

        let matches_name = p_lower == pkg_lower
            || p_lower == base_name
            || utils::strip_package_suffix(&p_lower) == base_name
            || is_flatpak_match;

        if matches_app_id || matches_name {
            let key = format!("{:?}-{}", p_source, p.name);
            if !seen.contains(&key) {
                final_variants.push(models::PackageVariant {
                    source: p_source.clone(),
                    version: p.version.clone(),
                    repo_name: if p_source.id == "chaotic-aur" {
                        Some("chaotic-aur".to_string())
                    } else {
                        None
                    },
                    pkg_name: Some(p.name.clone()),
                });
                seen.insert(key);
            }
        }
    }

    Ok(final_variants)
}

#[tauri::command]
pub async fn get_category_packages_paginated(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, RepoManager>,
    category: String,
    repo_filter: Option<Vec<String>>,
    sort_by: Option<String>,
    page: usize,
    limit: usize,
) -> Result<PaginatedResponse, String> {
    let mut packages = if let Ok(loader) = state_meta.inner().0.lock() {
        loader.get_apps_by_category(&category)
    } else {
        Vec::new()
    }
    .into_iter()
    .map(|app| models::Package {
        name: app.pkg_name.clone().unwrap_or(app.app_id.clone()),
        display_name: Some(app.name),
        description: app.summary.unwrap_or_default(),
        version: app.version.unwrap_or_else(|| "latest".to_string()),
        source: models::PackageSource::new("repo", "core", "latest", "Arch Official"),
        maintainer: None,
        license: None,
        url: None,
        last_modified: app.last_updated.map(|t| t as i64),
        first_submitted: None,
        out_of_date: None,
        keywords: None,
        num_votes: None,
        icon: app.icon_url,
        screenshots: if app.screenshots.is_empty() {
            None
        } else {
            Some(app.screenshots)
        },
        provides: None,
        app_id: Some(app.app_id.clone()),
        is_optimized: None,
        depends: None,
        make_depends: None,
        is_featured: None,
        installed: false,
        ..Default::default()
    })
    .collect::<Vec<_>>();

    let c_matches = state_chaotic
        .inner()
        .get_packages_by_category(&category)
        .await;

    for p in c_matches {
        // Allow duplicates at this stage so filtering can pick the right one later
        let mut pkg = models::Package {
            name: p.pkgname.clone(),
            display_name: Some(utils::to_pretty_name(&p.pkgname)),
            description: p
                .metadata
                .as_ref()
                .and_then(|m| m.desc.clone())
                .unwrap_or_default(),
            version: p.version.clone().unwrap_or_default(),
            source: models::PackageSource::new(
                "repo",
                "chaotic-aur",
                &p.version.clone().unwrap_or_default(),
                "Chaotic-AUR (Pre-built)",
            ),
            maintainer: Some("Chaotic-AUR Team".to_string()),
            license: p
                .metadata
                .as_ref()
                .and_then(|m| m.license.clone())
                .map(|l| vec![l]),
            url: p.metadata.as_ref().and_then(|m| m.url.clone()),
            installed: false,
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: None,
            icon: {
                let mut icon = None;
                if let Ok(loader) = state_meta.inner().0.lock() {
                    icon = loader.find_icon_heuristic(&p.pkgname);
                }
                icon
            },
            screenshots: None,
            provides: None,
            app_id: None,
            is_optimized: None,
            depends: None,
            make_depends: None,
            is_featured: None,
            alternatives: None,
            ..Default::default()
        };
        if let Ok(loader) = state_meta.inner().0.lock() {
            pkg.app_id = loader.find_app_id(&p.pkgname);
        }
        packages.push(pkg);
    }

    // --- FIX: FORCE INJECT FEATURES ---
    // Ensure curated featured apps are present even if category search missed them
    let featured_names = get_featured_apps(&category);
    if !featured_names.is_empty() {
        let existing_names: std::collections::HashSet<String> =
            packages.iter().map(|p| p.name.clone()).collect();
        let missing: Vec<String> = featured_names
            .iter()
            .filter(|&&n| !existing_names.contains(&n.to_string())) // simple name check
            .map(|&s| s.to_string())
            .collect();

        if !missing.is_empty() {
            if let Ok(injected) = get_packages_by_names(
                state_meta.clone(),
                state_chaotic.clone(),
                state_repo.clone(),
                missing,
            )
            .await
            {
                for mut p in injected {
                    // Re-check existence to be safe
                    if !existing_names.contains(&p.name) {
                        p.is_featured = Some(true); // Auto-mark
                        packages.push(p);
                    }
                }
            }
        }
    }
    // ----------------------------------

    // --- FIX: REPO FILTER (BEFORE DEDUP) ---
    // Filter first so we don't dedup away the variant the user explicitly asked for.
    if let Some(repos) = repo_filter {
        let has_all = repos.iter().any(|r| r.to_lowercase() == "all");

        if !has_all && !repos.is_empty() {
            let allowed: std::collections::HashSet<String> =
                repos.iter().map(|s| s.to_lowercase()).collect();

            packages.retain(|p| {
                let p_source = match p.source.source_type.as_str() {
                    "repo" => match p.source.id.as_str() {
                        "chaotic-aur" => "chaotic-aur",
                        "cachyos" => "cachyos",
                        "garuda" => "garuda",
                        "endeavour" => "endeavour",
                        "manjaro" => "manjaro",
                        _ => "official",
                    },
                    "flatpak" => "flatpak",
                    "aur" => "aur",
                    "local" => "local",
                    _ => "other",
                };

                if p_source == "chaotic-aur"
                    && (allowed.contains("chaotic") || allowed.contains("chaotic-aur"))
                {
                    return true;
                }
                allowed.contains(p_source)
            });
        }
    }
    // ---------------------------------------

    // --- FIX: DEDUPLICATE ---
    // Prioritize Injected (Featured) items (base) over standard search results (others)
    // This prevents "Steam" (AppStream) + "Steam" (Injected) duplicates.

    let (injected, others): (Vec<_>, Vec<_>) = packages
        .into_iter()
        .partition(|p| p.is_featured == Some(true));

    // Combine them, preserving priority (Featured first)
    // We pass EMPTY base lists, so that 'injected' items also check against themselves!
    let all_sorted = injected.into_iter().chain(others).collect();
    packages = utils::merge_and_deduplicate(Vec::new(), all_sorted);

    // ------------------------

    // --- FIX: AUGMENT DATES FROM REPO DB (ALPM as single READ source) ---
    let names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    let enabled_repos: Vec<String> = state_repo
        .inner()
        .get_all_repos()
        .await
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.name.clone())
        .collect();
    let names_clone = names.clone();
    let repo_data = tokio::task::spawn_blocking(move || {
        crate::alpm_read::get_packages_batch(&names_clone, &enabled_repos)
    })
    .await
    .map_err(|e| e.to_string())?;
    let date_map: std::collections::HashMap<String, i64> = repo_data
        .into_iter()
        .filter_map(|p| p.last_modified.map(|d| (p.name, d)))
        .collect();

    for pkg in packages.iter_mut() {
        if pkg.last_modified.is_none() {
            if let Some(date) = date_map.get(&pkg.name) {
                pkg.last_modified = Some(*date);
            }
        }
    }
    // ---------------------------------------

    if let Some(ref sort) = sort_by {
        match sort.as_str() {
            "name" => packages.sort_by(|a, b| a.name.cmp(&b.name)),
            "newest" => {
                packages.sort_by(|a, b| {
                    b.last_modified
                        .unwrap_or(0)
                        .cmp(&a.last_modified.unwrap_or(0))
                });
            }
            _ => utils::sort_packages_by_relevance(&mut packages, ""),
        }
    } else {
        // Default sort by name if none provided
        packages.sort_by(|a, b| a.name.cmp(&b.name));
    }

    // FEATURED APPS HOISTING & FLAGGING
    // We lift popular apps to the top regardless of sort AND mark them
    let featured = get_featured_apps(&category);
    if !featured.is_empty() {
        // Create case-insensitive map
        let featured_map: std::collections::HashMap<String, usize> = featured
            .iter()
            .enumerate()
            .map(|(i, name)| (name.to_lowercase(), i))
            .collect();

        // Mark them as featured
        for pkg in packages.iter_mut() {
            if featured_map.contains_key(&pkg.name.to_lowercase()) {
                pkg.is_featured = Some(true);
            }
        }

        // Stable sort: Featured first (in defined order), then others
        // Only if sorting by default/featured or "name" (if we want to force featured on top)
        // User asked for specific separation, so we should always hoist if "Featured" mode is on.
        // If sort_by is "name", user might expect strict A-Z?
        // Let's assume default behavior implies Featured First unless strict sort is requested.
        let is_strict_sort = sort_by.as_deref().unwrap_or("") == "name"
            || sort_by.as_deref().unwrap_or("") == "newest";

        if !is_strict_sort {
            packages.sort_by(|a, b| {
                // Check against both name and display_name for robustness
                let a_key = a.name.to_lowercase();
                let b_key = b.name.to_lowercase();

                let a_rank = featured_map.get(&a_key).unwrap_or(&9999);
                let b_rank = featured_map.get(&b_key).unwrap_or(&9999);

                a_rank.cmp(b_rank)
            });
        }
    }

    let total = packages.len();
    // Frontend sends 1-based page index
    let page_idx = if page > 0 { page - 1 } else { 0 };
    let start = page_idx * limit;
    let end = (start + limit).min(total);
    let has_more = end < total;

    let page_items = if start < total {
        packages[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok(PaginatedResponse {
        packages: page_items,
        total,
        page,
        has_more,
    })
}

fn get_featured_apps(category: &str) -> Vec<&'static str> {
    match category.trim().to_lowercase().as_str() {
        "games" | "game" => vec![
            "steam",
            "lutris",
            "heroic-games-launcher-bin",
            "discord",
            "minecraft-launcher",
            "wine",
            "protonup-qt",
            "retroarch",
            "gamemode",
            "mangohud",
            "r2modman-bin",
            "prismlauncher",
        ],
        "internet" | "network" => vec![
            "google-chrome",
            "firefox",
            "brave-bin",
            "discord",
            "telegram-desktop",
            "signal-desktop",
            "zoom",
            "thunderbird",
            "qbittorrent",
            "transmission-gtk",
            "filezilla",
            "anydesk-bin",
        ],
        "multimedia" | "audio" | "video" | "audiovideo" => vec![
            "vlc",
            "obs-studio",
            "spotify",
            "gimp",
            "kdenlive",
            "blender",
            "audacity",
            "mpv",
            "inkscape",
            "handbrake",
            "ffmpeg",
            "krita",
        ],
        "graphics" => vec![
            "gimp",
            "blender",
            "inkscape",
            "krita",
            "darktable",
            "rawtherapee",
            "digikam",
            "glaxnimate",
        ],
        "development" => vec![
            "visual-studio-code-bin",
            "code",
            "git",
            "docker",
            "intellij-idea-community-edition",
            "pycharm-community-edition",
            "postman-bin",
            "sublime-text-4",
            "neovim",
            "vim",
            "cmake",
            "qtcreator",
        ],
        "office" => vec![
            "libreoffice-fresh",
            "obsidian",
            "notion-app-electron",
            "evince",
            "onlyoffice-bin",
            "simple-scan",
            "typora",
            "joplin",
            "okular",
        ],
        "system" => vec![
            "gparted",
            "timeshift",
            "bleachbit",
            "htop",
            "btop",
            "flatpak",
            "pacman",
            "virtualbox",
            "kvm",
            "qemu-full",
        ],
        "utility" | "utilities" => vec![
            "calculator",
            "gnome-calculator",
            "gnome-disk-utility",
            "file-roller",
            "spectacle",
            "flameshot",
            "ark",
            "kate",
            "gedit",
            "nano",
            "speedtest-cli",
            "neofetch",
            "fastfetch",
            "tree",
            "ripgrep",
            "bat",
            "eza",
            "fd",
            "fzf",
            "alacritty",
            "kitty",
        ],
        _ => vec![],
    }
}

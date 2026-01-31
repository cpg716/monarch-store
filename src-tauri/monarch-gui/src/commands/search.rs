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

#[tauri::command]
pub async fn search_packages(
    app: tauri::AppHandle,
    state_meta: State<'_, metadata::MetadataState>,
    state_repo: State<'_, RepoManager>,
    query: String,
) -> Result<Vec<models::Package>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(vec![]);
    }

    // Telemetry
    let app_handle = app.clone();
    let q_telemetry = query.clone();
    tauri::async_runtime::spawn(async move {
        utils::track_event_safe(
            &app_handle,
            "search_query",
            Some(serde_json::json!({
                "term": q_telemetry,
                "term_length": q_telemetry.len(),
                "category": "all",
            })),
        )
        .await;
    });

    // 1. Concurrent Search: ALPM (Local/Repo) + Monarch Cache + AUR (Web)
    let aur_enabled = state_repo.inner().is_aur_enabled().await;
    let query_cloned = query.clone();

    let alpm_handle =
        tokio::task::spawn_blocking(move || crate::alpm_read::search_local_dbs(&query_cloned));

    // Monarch Cache Search (Backing Store for synced 3rd party repos)
    let repo_manager = state_repo.inner().clone();
    let query_parts: Vec<String> = query.split_whitespace().map(|s| s.to_string()).collect();
    let cache_handle = tokio::spawn(async move {
        let query_regexes: Vec<regex::Regex> = query_parts
            .iter()
            .filter_map(|p| {
                regex::RegexBuilder::new(&regex::escape(p))
                    .case_insensitive(true)
                    .build()
                    .ok()
            })
            .collect();

        if query_regexes.is_empty() {
            return Vec::new();
        }

        let cache = repo_manager.cache.read().await;
        let mut results = Vec::new();
        for (repo_name, pkgs) in cache.iter() {
            for pkg in pkgs {
                let mut all_match = true;
                for re in &query_regexes {
                    if !re.is_match(&pkg.name) && !re.is_match(&pkg.description) {
                        all_match = false;
                        break;
                    }
                }

                if all_match {
                    let mut p = pkg.clone();
                    p.source = models::PackageSource::from_repo_name(repo_name);
                    results.push(p);
                }
            }
        }
        results
    });

    let aur_handle = async {
        if aur_enabled && query.len() >= 2 {
            aur_api::search_aur(&query).await.unwrap_or_default()
        } else {
            Vec::new()
        }
    };

    let (alpm_res, cache_res, aur_pkgs) = tokio::join!(alpm_handle, cache_handle, aur_handle);
    let mut packages = alpm_res.map_err(|e| e.to_string())?;
    let cache_pkgs = cache_res.map_err(|e| e.to_string())?;

    // Merge ALPM and Cache first
    packages.extend(cache_pkgs);

    // 2. Integration & Hydration
    for pkg in aur_pkgs {
        // Hydrate AUR results if they exist in ALPM (already installed or in chaotic)
        if let Some(existing) = packages.iter_mut().find(|p| p.name == pkg.name) {
            if existing.source == models::PackageSource::Local {
                existing.source = models::PackageSource::Aur;
                // Hydrate metadata from AUR if strictly better?
                existing.maintainer = pkg.maintainer.clone();
                existing.num_votes = pkg.num_votes;
                existing.out_of_date = pkg.out_of_date;
                existing.first_submitted = pkg.first_submitted;
                existing.last_modified = pkg.last_modified;
            }
        } else {
            packages.push(pkg);
        }
    }

    // 3. Metadata Hydration (AppStream Icons/AppIDs)
    if let Ok(loader) = state_meta.inner().0.lock() {
        for pkg in &mut packages {
            if pkg.icon.is_none() {
                pkg.icon = loader.find_icon_heuristic(&pkg.name);
            }
            if pkg.app_id.is_none() {
                pkg.app_id = loader.find_app_id(&pkg.name);
            }
            pkg.display_name = Some(utils::to_pretty_name(&pkg.name));
        }
    }

    // 4. Prioritization & Deduplication
    utils::sort_packages_by_relevance(&mut packages, &query);

    // Custom priority: Installed to Top.
    // We use a stable sort and return Equal for same-status to preserve the
    // relevance ranking from sort_packages_by_relevance.
    packages.sort_by(|a, b| {
        if a.installed != b.installed {
            return b.installed.cmp(&a.installed);
        }
        std::cmp::Ordering::Equal
    });

    Ok(utils::merge_and_deduplicate(Vec::new(), packages))
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
            source: models::PackageSource::Chaotic,
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
            .filter(|p| p.source == models::PackageSource::Local)
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
                        if existing.source == models::PackageSource::Local {
                            existing.source = models::PackageSource::Aur;
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
                    source: models::PackageSource::Official,
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
                            source: models::PackageSource::CachyOS,
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
                        source: models::PackageSource::CachyOS,
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
                            source: models::PackageSource::Chaotic,
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
                        source: models::PackageSource::Aur,
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
    state_repo: State<'_, RepoManager>,
    pkg_name: String,
) -> Result<Vec<models::PackageVariant>, String> {
    let pkg_lower = pkg_name.to_lowercase();
    let base_name = utils::strip_package_suffix(&pkg_lower);
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
                    source: models::PackageSource::Chaotic,
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
        let matches_name = p_lower == pkg_lower
            || p_lower == base_name
            || utils::strip_package_suffix(&p_lower) == base_name;

        if matches_app_id || matches_name {
            let key = format!("{:?}-{}", p_source, p.name);
            if !seen.contains(&key) {
                final_variants.push(models::PackageVariant {
                    source: p_source,
                    version: p.version.clone(),
                    repo_name: if matches!(p.source, models::PackageSource::Chaotic) {
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
        source: models::PackageSource::Official,
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
            source: models::PackageSource::Chaotic,
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
                let p_source = match p.source {
                    models::PackageSource::Official => "official",
                    models::PackageSource::Chaotic => "chaotic-aur",
                    models::PackageSource::Aur => "aur",
                    models::PackageSource::CachyOS => "cachyos",
                    models::PackageSource::Garuda => "garuda",
                    models::PackageSource::Endeavour => "endeavour",
                    models::PackageSource::Manjaro => "manjaro",
                    models::PackageSource::Local => "local",
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

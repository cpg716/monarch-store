use crate::{
    aur_api, chaotic_api, flathub_api, metadata, models, pkgstats_api, repo_manager::RepoManager,
    utils,
};
use serde::Serialize;
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
    state: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, RepoManager>,
    query: String,
) -> Result<Vec<models::Package>, String> {
    let mut packages = Vec::new();
    let query = query.trim();

    if query.is_empty() {
        return Ok(vec![]);
    }

    // Telemetry: Track Search Query (Privacy Guarded)
    utils::track_event_safe(
        &app,
        "search_query",
        Some(serde_json::json!({
            "term": query,
            "category": "all"
        })),
    )
    .await;

    // 1. Search AppStream (Official/Local Metadata)
    {
        let loader = state.inner().0.lock().unwrap();
        let app_results = loader.search_apps(query);

        for app in app_results {
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
                is_featured: None,
                alternatives: None,
            });
        }
    }

    // 2. Search Synced Repos (Binary Repos)
    let mut repo_results = state_repo.inner().search(query).await;

    // CRITICAL: Sort by relevance BEFORE deduplication
    utils::sort_packages_by_relevance(&mut repo_results, query);

    for mut pkg in repo_results {
        // Hydrate missing metadata using AppStream heuristic
        if pkg.icon.is_none() || pkg.app_id.is_none() {
            if let Ok(loader) = state.inner().0.lock() {
                if pkg.icon.is_none() {
                    pkg.icon = loader.find_icon_heuristic(&pkg.name);
                }
                if pkg.app_id.is_none() {
                    pkg.app_id = loader.find_app_id(&pkg.name);
                }
            }
        }

        if pkg.app_id.is_none() {
            pkg.app_id = flathub_api::get_flathub_app_id(&pkg.name);
        }

        pkg.display_name = Some(utils::to_pretty_name(&pkg.name));
        packages.push(pkg);
    }

    // 3. Search Chaotic AUR
    let chaotic_enabled = state_repo.inner().is_repo_enabled("chaotic-aur").await;

    if chaotic_enabled {
        if let Ok(chaotic_arc) = state_chaotic.inner().fetch_packages().await {
            let q_lower = query.to_lowercase();
            let chaotic_matches: Vec<models::Package> = chaotic_arc
                .iter()
                .filter(|p| {
                    p.pkgname.to_lowercase().contains(&q_lower)
                        || p.metadata
                            .as_ref()
                            .and_then(|m| m.desc.as_ref())
                            .map(|d| d.to_lowercase().contains(&q_lower))
                            .unwrap_or(false)
                })
                .take(50)
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
                    icon: {
                        let mut icon = None;
                        if let Ok(loader) = state.inner().0.lock() {
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
                })
                .collect();

            for mut pkg in chaotic_matches {
                if pkg.app_id.is_none() {
                    if let Ok(loader) = state.inner().0.lock() {
                        pkg.app_id = loader.find_app_id(&pkg.name);
                    }
                }

                if pkg.app_id.is_none() {
                    pkg.app_id = flathub_api::get_flathub_app_id(&pkg.name);
                }

                packages.push(pkg);
            }
        }
    }

    // 4. Search AUR
    let aur_enabled = state_repo.inner().is_aur_enabled().await;
    if aur_enabled && query.len() >= 2 {
        if let Ok(aur_results) = aur_api::search_aur(query).await {
            let chaotic_res = state_chaotic.inner().fetch_packages().await;
            let chaotic_packages = chaotic_res.unwrap_or_default();
            let chaotic_set: std::collections::HashSet<&String> =
                chaotic_packages.iter().map(|c| &c.pkgname).collect();

            let repo_map: std::collections::HashMap<String, models::PackageSource> = packages
                .iter()
                .map(|p| (p.name.clone(), p.source.clone()))
                .collect();

            for mut pkg in aur_results {
                if pkg.app_id.is_none() {
                    if let Ok(loader) = state.inner().0.lock() {
                        pkg.app_id = loader.find_app_id(&pkg.name);
                    }
                }
                if pkg.app_id.is_none() {
                    pkg.app_id = flathub_api::get_flathub_app_id(&pkg.name);
                }

                if chaotic_set.contains(&pkg.name) {
                    pkg.source = models::PackageSource::Chaotic;
                } else if let Some(source) = repo_map.get(&pkg.name) {
                    pkg.source = source.clone();
                }

                pkg.display_name = Some(utils::to_pretty_name(&pkg.name));
                packages.push(pkg);
            }
        }
    }

    utils::sort_packages_by_relevance(&mut packages, query);
    // UNIFIED DEDUPLICATION
    packages = utils::merge_and_deduplicate(Vec::new(), packages);
    Ok(packages)
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
    let repo_pkgs = state_repo.inner().get_packages_batch(&names).await;
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
        state_chaotic.inner().get_packages_batch(names).await
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
            alternatives: None,
        };

        if let Ok(loader) = state_meta.inner().0.lock() {
            pkg.app_id = loader.find_app_id(&name);
        }
        packages.push(pkg);
    }

    // UNIFIED DEDUPLICATION
    packages = utils::merge_and_deduplicate(Vec::new(), packages);

    Ok(packages)
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

    {
        let loader = state_meta.inner().0.lock().unwrap();
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
                    alternatives: None,
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
                            source: models::PackageSource::Official, // It's a repo package
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
                            alternatives: None,
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
                        source: models::PackageSource::Official,
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
                        alternatives: None,
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
                            alternatives: None,
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
                        alternatives: None,
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
    // Resolve App ID to package name if needed
    let resolved_pkg_name = {
        let loader = state_meta.inner().0.lock().unwrap();
        loader.resolve_package_name(&pkg_name)
    };

    let mut variants = Vec::new();

    // 1. Check all enabled repositories via Pacman
    let results = state_repo
        .inner()
        .get_all_packages_with_repos(&resolved_pkg_name)
        .await;
    variants.extend(results.into_iter().map(|(p, r)| models::PackageVariant {
        source: p.source,
        version: p.version,
        repo_name: Some(r),
        pkg_name: Some(resolved_pkg_name.clone()), // Use resolved name
    }));

    let mut sources_found: std::collections::HashSet<models::PackageSource> =
        variants.iter().map(|v| v.source.clone()).collect();

    // 2. Chaotic-AUR (API Fallback)
    if !sources_found.contains(&models::PackageSource::Chaotic) {
        if state_repo.inner().is_repo_enabled("chaotic-aur").await {
            if let Some(p) = state_chaotic
                .inner()
                .get_package_by_name(&resolved_pkg_name)
                .await
            {
                variants.push(models::PackageVariant {
                    source: models::PackageSource::Chaotic,
                    version: p.version.clone().unwrap_or_else(|| "latest".to_string()),
                    repo_name: Some("chaotic-aur".to_string()),
                    pkg_name: Some(resolved_pkg_name.clone()),
                });
                sources_found.insert(models::PackageSource::Chaotic);
            }
        }
    }
    // 3. Official / AppStream Fallback
    if !sources_found.contains(&models::PackageSource::Official) {
        let loader = state_meta.inner().0.lock().unwrap();
        if let Some(meta) = loader.find_package(&resolved_pkg_name) {
            variants.push(models::PackageVariant {
                source: models::PackageSource::Official,
                version: meta
                    .version
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                repo_name: Some("official".to_string()),
                pkg_name: Some(resolved_pkg_name.clone()),
            });
            sources_found.insert(models::PackageSource::Official);
        }
    }

    // 4. AUR (Optional)
    if state_repo.inner().is_aur_enabled().await {
        let name_str = resolved_pkg_name.as_str();
        if let Ok(aur_info) = aur_api::get_multi_info(&[name_str][..]).await {
            if let Some(p) = aur_info.first() {
                variants.push(models::PackageVariant {
                    source: models::PackageSource::Aur,
                    version: p.version.clone(),
                    repo_name: Some("aur".to_string()),
                    pkg_name: Some(resolved_pkg_name.clone()),
                });
            }
        }
    }

    Ok(variants)
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
        alternatives: None,
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

    // --- FIX: AUGMENT DATES FROM REPO DB ---
    // AppStream data often lacks recent build dates. We fetch from RepoManager to fill gaps.
    let names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    let repo_data = state_repo.inner().get_packages_batch(&names).await;
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

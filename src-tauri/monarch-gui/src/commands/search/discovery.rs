use crate::{
    chaotic_api, discovery_manager, metadata, middleware::aggregation, models,
    repo_manager::RepoManager, utils,
};
use once_cell::sync::Lazy;
use tauri::State;
use tokio::sync::Mutex;

use super::cache::{
    try_get_packages_from_cache, try_read_trending_disk, write_packages_cache, write_trending_disk,
    HOME_DISCOVERY_CACHE, TRENDING_CACHE,
};
use super::core::SearchOptions;

/// When cache is empty, run discovery fetch in this command (up to this duration) so we don't rely on background spawn.
pub(crate) const TRENDING_REFRESH_TIMEOUT_MS: u64 = 18_000;
static HOME_DISCOVERY_GATE: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fn build_registry_named_fallback(
    state_registry: &crate::registry::RegistryState,
    names: &[String],
    limit: usize,
) -> Vec<models::Package> {
    let mut lookup_ids = Vec::new();
    let mut ordered_candidates: Vec<Vec<String>> = Vec::new();

    for raw_name in names {
        let mut candidates = Vec::new();
        let canonical = utils::canonical_merge_key(raw_name, None);
        if !canonical.is_empty() {
            candidates.push(canonical);
        }
        let raw_lower = raw_name.to_lowercase();
        if !candidates.iter().any(|c| c == &raw_lower) {
            candidates.push(raw_lower);
        }
        for alias in utils::canonical_to_repo_lookup_names(raw_name) {
            let alias_str = alias.to_string();
            let alias_canonical = utils::canonical_merge_key(&alias_str, None);
            if !alias_canonical.is_empty() && !candidates.iter().any(|c| c == &alias_canonical) {
                candidates.push(alias_canonical);
            }
            if !candidates.iter().any(|c| c == &alias_str) {
                candidates.push(alias_str);
            }
        }
        for candidate in &candidates {
            if !lookup_ids.iter().any(|id| id == candidate) {
                lookup_ids.push(candidate.clone());
            }
        }
        ordered_candidates.push(candidates);
    }

    let fetched = match state_registry.manager.get_packages_by_canonical_ids(&lookup_ids) {
        Ok(packages) => packages,
        Err(_) => return Vec::new(),
    };

    let mut by_key = std::collections::HashMap::new();
    for pkg in fetched {
        by_key.insert(pkg.canonical_id.to_lowercase(), pkg.clone());
        by_key.insert(pkg.name.to_lowercase(), pkg);
    }

    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for candidates in ordered_candidates {
        for key in candidates {
            if let Some(pkg) = by_key.get(&key.to_lowercase()) {
                let canonical = if pkg.canonical_id.is_empty() {
                    utils::canonical_merge_key(&pkg.name, pkg.app_id.as_deref())
                } else {
                    pkg.canonical_id.clone()
                };
                if seen.insert(canonical) {
                    results.push(pkg.clone());
                    break;
                }
            }
        }
        if results.len() >= limit {
            break;
        }
    }

    crate::utils::finalize_packages_contract(&mut results);
    results
}

fn build_static_named_fallback(names: &[String], limit: usize) -> Vec<models::Package> {
    let mut packages = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for raw_name in names {
        let key = utils::canonical_merge_key(raw_name, None);
        if !seen.insert(key) {
            continue;
        }

        packages.push(models::Package {
            name: raw_name.clone(),
            display_name: Some(utils::to_pretty_name(raw_name)),
            display_title: Some(utils::to_pretty_name(raw_name)),
            description: "Available in your configured software sources.".to_string(),
            version: "latest".to_string(),
            source: models::PackageSource::new("repo", "core", "latest", "Arch Official"),
            icon: None,
            app_id: None,
            installed: crate::utils::is_package_or_alias_installed(raw_name),
            ..Default::default()
        });

        if packages.len() >= limit {
            break;
        }
    }

    crate::utils::finalize_packages_contract(&mut packages);
    packages
}

fn build_local_named_fallback(
    state_meta: &metadata::MetadataState,
    names: &[String],
    limit: usize,
) -> Vec<models::Package> {
    let mut packages = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let loader = match state_meta.loader.lock() {
        Ok(loader) => loader,
        Err(_) => return packages,
    };

    for raw_name in names {
        let mut candidates = Vec::with_capacity(1 + crate::utils::canonical_to_repo_lookup_names(raw_name).len());
        candidates.push(raw_name.clone());
        for alias in crate::utils::canonical_to_repo_lookup_names(raw_name) {
            let alias_name = alias.to_string();
            if !candidates.iter().any(|existing| existing == &alias_name) {
                candidates.push(alias_name);
            }
        }

        let mut built = None;
        for candidate in candidates {
            if let Some(app) = loader.find_package(&candidate) {
                let key = utils::canonical_merge_key(&candidate, Some(&app.app_id));
                if !seen.insert(key) {
                    built = Some(None);
                    break;
                }
                built = Some(Some(models::Package {
                    name: app.pkg_name.clone().unwrap_or_else(|| candidate.clone()),
                    display_name: Some(app.name.clone()),
                    display_title: Some(app.name),
                    description: app.summary.unwrap_or_else(|| {
                        "Available in your configured software sources.".to_string()
                    }),
                    version: app.version.unwrap_or_else(|| "latest".to_string()),
                    source: models::PackageSource::new("repo", "core", "latest", "Arch Official"),
                    maintainer: app.maintainer.clone(),
                    license: app.license.clone().map(|value| vec![value]),
                    icon: app.icon_url.clone(),
                    screenshots: if app.screenshots.is_empty() {
                        None
                    } else {
                        Some(app.screenshots)
                    },
                    app_id: Some(app.app_id),
                    installed: crate::utils::is_package_or_alias_installed(&candidate),
                    ..Default::default()
                }));
                break;
            }
        }

        match built {
            Some(Some(pkg)) => packages.push(pkg),
            Some(None) => {}
            None => {
                let app_id = loader.find_app_id(raw_name);
                let key = utils::canonical_merge_key(raw_name, app_id.as_deref());
                if !seen.insert(key) {
                    continue;
                }
                packages.push(models::Package {
                    name: raw_name.clone(),
                    display_name: Some(utils::to_pretty_name(raw_name)),
                    display_title: Some(utils::to_pretty_name(raw_name)),
                    description: "Available in your configured software sources.".to_string(),
                    version: "latest".to_string(),
                    source: models::PackageSource::new("repo", "core", "latest", "Arch Official"),
                    icon: loader.find_icon_heuristic(raw_name),
                    app_id,
                    installed: crate::utils::is_package_or_alias_installed(raw_name),
                    ..Default::default()
                });
            }
        }

        if packages.len() >= limit {
            break;
        }
    }

    crate::utils::finalize_packages_contract(&mut packages);
    packages
}

fn hardcoded_discovery_names(kind: &str) -> Vec<String> {
    let names = match kind {
        "essentials" => vec![
            "firefox",
            "libreoffice-fresh",
            "vlc",
            "thunderbird",
            "gimp",
            "steam",
            "discord",
            "obs-studio",
            "krita",
            "kate",
        ],
        _ => vec![
            "firefox",
            "google-chrome",
            "chromium",
            "steam",
            "discord",
            "obs-studio",
            "vlc",
            "spotify",
            "gimp",
            "kdenlive",
        ],
    };

    names.into_iter().map(|value| value.to_string()).collect()
}

async fn build_essentials_snapshot_impl(
    state_meta: &metadata::MetadataState,
    state_chaotic: &chaotic_api::ChaoticApiClient,
    state_repo: &RepoManager,
    state_flathub: &crate::flathub_api::FlathubApiClient,
    state_registry: &crate::registry::RegistryState,
) -> Result<Vec<models::Package>, String> {
    let ids = crate::commands::package::resolve_essentials_list(state_repo).await?;
    let include_flatpak = state_repo.is_flatpak_enabled().await;
    let include_aur = state_repo.is_aur_enabled().await;
    let include_chaotic = state_repo.is_repo_enabled("chaotic-aur").await;

    let mut packages = state_registry
        .get_packages_by_canonical_ids(&ids)
        .unwrap_or_default();
    if packages.len() < ids.len() {
        let mut items: Vec<(String, Option<String>)> = Vec::new();
        for id in &ids {
            items.push((id.clone(), None));
            for alias in crate::utils::canonical_to_repo_lookup_names(id) {
                let alias_name = alias.to_string();
                if !items.iter().any(|(name, _)| name == &alias_name) {
                    items.push((alias_name, None));
                }
            }
        }
        let mut rebuilt = aggregation::fetch_and_merge_packages_by_names_impl(
            state_meta,
            state_chaotic,
            state_repo,
            state_flathub,
            &state_registry.manager,
            items,
            include_flatpak,
            include_aur,
            include_chaotic,
            false,
        )
        .await
        .unwrap_or_default();
        if !rebuilt.is_empty() {
            crate::utils::finalize_packages_contract(&mut rebuilt);
            packages = rebuilt;
        }
    }
    if packages.is_empty() {
        packages = build_local_named_fallback(state_meta, &ids, 16);
    } else if packages.len() < 8 {
        let local_fallback = build_local_named_fallback(state_meta, &ids, 16);
        for pkg in local_fallback {
            let key = pkg.canonical_id.clone();
            if !packages.iter().any(|existing| existing.canonical_id == key) {
                packages.push(pkg);
            }
        }
    }
    if packages.is_empty() {
        let backup = hardcoded_discovery_names("essentials");
        packages = build_registry_named_fallback(state_registry, &backup, 16);
        if packages.is_empty() {
            packages = build_local_named_fallback(state_meta, &backup, 16);
        }
    }

    let trust_rank = |pkg: &models::Package| match pkg.trust_level.as_deref().unwrap_or("") {
        "official" => 5,
        "distro_native" => 4,
        "sandboxed" => 3,
        "third_party_repo" => 2,
        "community_build" => 1,
        _ => 0,
    };
    packages.sort_by(|a, b| {
        trust_rank(b)
            .cmp(&trust_rank(a))
            .then_with(|| b.icon.is_some().cmp(&a.icon.is_some()))
            .then_with(|| b.screenshots.as_ref().map(|v| !v.is_empty()).unwrap_or(false).cmp(&a.screenshots.as_ref().map(|v| !v.is_empty()).unwrap_or(false)))
            .then_with(|| a.name.cmp(&b.name))
    });
    packages.truncate(16);
    crate::utils::finalize_packages_contract(&mut packages);
    log::info!("[DISCOVERY] essentials snapshot returning {} packages", packages.len());
    Ok(packages)
}

fn build_quick_starts() -> Vec<models::DiscoveryIntent> {
    vec![
        ("web-browsers", "Web Browsers", "Find browsers and internet apps", Some("browser"), None),
        ("office-school", "Office & School", "Documents, mail, and study tools", Some("office suite"), None),
        ("gaming", "Gaming", "Launchers, emulators, and game clients", None, Some("Game")),
        ("chat-voice", "Chat & Voice", "Messaging and voice apps", Some("discord telegram"), None),
        ("creative-tools", "Creative Tools", "Art, design, and editing apps", None, Some("Graphics")),
        ("system-utilities", "System Utilities", "Maintenance and system tools", None, Some("System")),
    ]
    .into_iter()
    .map(|(id, label, description, query, category)| models::DiscoveryIntent {
        id: id.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        query: query.map(|v| v.to_string()),
        category: category.map(|v| v.to_string()),
    })
    .collect()
}

#[tauri::command]
#[specta::specta]
pub async fn get_trending(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, RepoManager>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_discovery: State<'_, discovery_manager::DiscoveryManager>,
    state_registry: State<'_, crate::registry::RegistryState>,
    options: Option<SearchOptions>,
) -> Result<Vec<models::Package>, String> {
    state_meta.inner().wait_until_ready().await;

    let flatpak_enabled = options.as_ref().and_then(|o| o.flatpak_enabled);
    let aur_enabled = options.as_ref().and_then(|o| o.aur_enabled);
    let chaotic_enabled = options.as_ref().and_then(|o| o.chaotic_enabled);

    let backend_flatpak_enabled = state_repo.inner().is_flatpak_enabled().await;
    let backend_aur_enabled = state_repo.inner().is_aur_enabled().await;
    let backend_chaotic_enabled = state_repo.inner().is_repo_enabled("chaotic-aur").await;
    let include_flatpak = backend_flatpak_enabled && flatpak_enabled.unwrap_or(true);
    let include_aur = backend_aur_enabled && aur_enabled.unwrap_or(true);
    let include_chaotic = backend_chaotic_enabled && chaotic_enabled.unwrap_or(true);
    let cache_key = (include_flatpak, include_aur, include_chaotic);

    state_meta.inner().wait_until_ready().await;
    if let Some(mut cached) = TRENDING_CACHE.get(&cache_key).await.filter(|items| !items.is_empty()) {
        crate::utils::finalize_packages_contract(&mut cached);
        return Ok(cached);
    }

    // Moka miss — try disk cache (warm start after restart), but restabilize it through the
    // canonical merge pipeline before returning so stale source-fragment winners do not render.
    if let Some(disk_cached) = try_read_trending_disk(include_flatpak, include_aur, include_chaotic)
    {
        let cached_items: Vec<(String, Option<String>)> = disk_cached
            .iter()
            .map(|pkg| (pkg.name.clone(), pkg.app_id.clone()))
            .collect();
        let stabilized = aggregation::fetch_and_merge_packages_by_names_impl(
            state_meta.inner(),
            state_chaotic.inner(),
            state_repo.inner(),
            state_flathub.inner(),
            &state_registry.manager,
            cached_items,
            include_flatpak,
            include_aur,
            include_chaotic,
            false,
        )
        .await
        .unwrap_or_default();

        let mut packages = if stabilized.is_empty() {
            disk_cached
        } else {
            stabilized
        };
        crate::utils::finalize_packages_contract(&mut packages);
        TRENDING_CACHE.insert(cache_key, packages.clone()).await;
        return Ok(packages);
    }

    // Official/repo packages from AppStream (so trending is not AUR-only when discovery cache is empty or Flathub fails).
    let official_packages: Vec<models::Package> = if let Ok(loader) = state_meta.loader.lock() {
        let mut apps = Vec::new();
        for cat in ["network", "office", "audiovideo", "graphics", "utility"] {
            apps.extend(loader.get_apps_by_category(cat));
        }
        apps.truncate(25);
        apps.into_iter()
            .map(|app| models::Package {
                name: app.pkg_name.clone().unwrap_or_else(|| app.app_id.clone()),
                display_name: Some(app.name),
                description: app.summary.unwrap_or_default(),
                version: app.version.unwrap_or_else(|| "latest".to_string()),
                source: models::PackageSource::new("repo", "core", "latest", "Arch Official"),
                maintainer: app.maintainer.clone(),
                license: app.license.clone().map(|l| vec![l]),
                url: None,
                last_modified: app.last_updated.map(|t| t as i64),
                first_submitted: None,
                out_of_date: None,
                keywords: None,
                num_votes: None,
                icon: app.icon_url.clone(),
                screenshots: if app.screenshots.is_empty() {
                    None
                } else {
                    Some(app.screenshots)
                },
                provides: None,
                app_id: Some(app.app_id),
                is_optimized: None,
                depends: None,
                make_depends: None,
                is_featured: None,
                installed: false,
                alternatives: None,
                ..Default::default()
            })
            .collect()
    } else {
        Vec::new()
    };

    // Dynamic discovery: top AUR by popularity; Flathub trending only when Flatpak enabled (respects on/off).
    let mut aur_packages = if include_aur {
        state_discovery.inner().get_aur_popular().await
    } else {
        Vec::new()
    };
    let mut flathub_search_results = if include_flatpak {
        state_discovery
            .inner()
            .get_flathub_popular_search_results()
            .await
    } else {
        Vec::new()
    };

    let installed_flatpaks = if include_flatpak {
        crate::flathub_api::get_installed_flatpak_app_ids()
            .await
            .unwrap_or_default()
            .into_iter()
            .collect::<std::collections::HashSet<String>>()
    } else {
        std::collections::HashSet::new()
    };

    // v0.2.41: Enrich Trending with real Flatpak versions
    let mut flathub_hits = Vec::new();
    if !flathub_search_results.is_empty() {
        let app_ids: Vec<String> = flathub_search_results
            .iter()
            .map(|h| h.app_id.clone())
            .collect();
        let versions = state_flathub
            .get_remote_versions_batch(&app_ids)
            .await
            .unwrap_or_default();
        for hit in flathub_search_results {
            let v = versions.get(&hit.app_id).cloned();
            flathub_hits.push((hit, v));
        }
    }

    // Merge: official (repo) first, then AUR, then Flathub — so trending shows a mix, not only AUR.
    let mut packages = aggregation::build_package_view_models_v2(
        official_packages.clone(),
        aur_packages.clone(),
        flathub_hits,
        &state_registry.manager,
        &installed_flatpaks,
    );
    let stable_items: Vec<(String, Option<String>)> = packages
        .iter()
        .map(|pkg| (pkg.name.clone(), pkg.app_id.clone()))
        .collect();
    let stable_packages = aggregation::fetch_and_merge_packages_by_names_impl(
        state_meta.inner(),
        state_chaotic.inner(),
        state_repo.inner(),
        state_flathub.inner(),
        &state_registry.manager,
        stable_items,
        include_flatpak,
        include_aur,
        include_chaotic,
        false,
    )
    .await
    .unwrap_or_default();
    if !stable_packages.is_empty() {
        packages = stable_packages;
    }
    aggregation::enrich_packages_ratings(&mut packages).await;

    // If cache was empty, run the fetch in this task (with timeout) so we don't depend on background spawn.
    if packages.is_empty() {
        let timeout = std::time::Duration::from_millis(TRENDING_REFRESH_TIMEOUT_MS);
        let _ = tokio::time::timeout(
            timeout,
            state_discovery
                .inner()
                .refresh_now_if_empty_or_stale(state_meta.inner()),
        )
        .await;
        aur_packages = if include_aur {
            state_discovery.inner().get_aur_popular().await
        } else {
            Vec::new()
        };
        flathub_search_results = if include_flatpak {
            state_discovery
                .inner()
                .get_flathub_popular_search_results()
                .await
        } else {
            Vec::new()
        };
        // v0.2.41: Enrich refreshed Trending with real Flatpak versions
        let mut flathub_hits = Vec::new();
        if !flathub_search_results.is_empty() {
            let app_ids: Vec<String> = flathub_search_results
                .iter()
                .map(|h| h.app_id.clone())
                .collect();
            let versions = state_flathub
                .get_remote_versions_batch(&app_ids)
                .await
                .unwrap_or_default();
            for hit in flathub_search_results {
                let v = versions.get(&hit.app_id).cloned();
                flathub_hits.push((hit, v));
            }
        }

        packages = aggregation::build_package_view_models_v2(
            official_packages,
            aur_packages,
            flathub_hits,
            &state_registry.manager,
            &installed_flatpaks,
        );
        let stable_items: Vec<(String, Option<String>)> = packages
            .iter()
            .map(|pkg| (pkg.name.clone(), pkg.app_id.clone()))
            .collect();
        let stable_packages = aggregation::fetch_and_merge_packages_by_names_impl(
            state_meta.inner(),
            state_chaotic.inner(),
            state_repo.inner(),
            state_flathub.inner(),
            &state_registry.manager,
            stable_items,
            include_flatpak,
            include_aur,
            include_chaotic,
            false,
        )
        .await
        .unwrap_or_default();
        if !stable_packages.is_empty() {
            packages = stable_packages;
        }
        aggregation::enrich_packages_ratings(&mut packages).await;
    }

    // CachyOS Spotlight: only if CachyOS repos enabled (host-adaptive, no repo injection).
    if state_repo.inner().is_repo_enabled("cachyos").await {
        let cachy_names = [
            "linux-cachyos",
            "cachyos-settings",
            "cachyos-browser",
            "cachyos-fish-config",
        ];
        for name in &cachy_names {
            if packages.iter().any(|p| p.name == *name) {
                continue;
            }
            let mut found = false;
            if let Ok(loader) = state_meta.loader.lock() {
                if let Some(app) = loader.find_package(name) {
                    packages.push(models::Package {
                        name: app.pkg_name.clone().unwrap_or_else(|| app.app_id.clone()),
                        display_name: Some(app.name),
                        description: app.summary.unwrap_or_default(),
                        version: app.version.unwrap_or_else(|| "optimized".to_string()),
                        source: models::PackageSource::cachyos(name),
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
                packages.push(models::Package {
                    name: (*name).to_string(),
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

    // Chaotic Heat: when repo enabled and distro allows (blocked on Manjaro).
    let chaotic_allowed = crate::distro_context::DistroContext::new().is_chaotic_compatible()
        || state_repo.inner().is_advanced_mode().await;
    if include_chaotic && chaotic_allowed {
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
                    if packages.iter().any(|pkg| pkg.name == name) {
                        continue;
                    }
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
                        icon: None,
                        screenshots: None,
                        provides: None,
                        app_id: None,
                        is_optimized: None,
                        depends: None,
                        make_depends: None,
                        is_featured: None,
                        installed: crate::utils::is_package_or_alias_installed(&name),
                        alternatives: None,
                        ..Default::default()
                    };
                    if let Ok(loader) = state_meta.loader.lock() {
                        pkg.icon = loader.find_icon_heuristic(&name);
                        pkg.app_id = loader.find_app_id(&name);
                    }
                    packages.push(pkg);
                }
            }
        }
    }

    // Lightweight enrichment: fix missing icons/names via Flathub (capped at 48, no full pipeline re-run).
    aggregation::enrich_packages_metadata(&mut packages, state_flathub.inner()).await;

    // SSOT Pass 2: Final local enrichment (ensures variants match local AppStream IDs)
    if let Ok(loader) = state_meta.loader.lock() {
        aggregation::enrich_with_local_metadata(&mut packages, &loader);
    }

    packages = aggregation::deduplicate_and_merge_packages(packages);
    let final_items: Vec<(String, Option<String>)> = packages
        .iter()
        .map(|pkg| (pkg.name.clone(), pkg.app_id.clone()))
        .collect();
    let final_stable = aggregation::fetch_and_merge_packages_by_names_impl(
        state_meta.inner(),
        state_chaotic.inner(),
        state_repo.inner(),
        state_flathub.inner(),
        &state_registry.manager,
        final_items,
        include_flatpak,
        include_aur,
        include_chaotic,
        false,
    )
    .await
    .unwrap_or_default();
    if !final_stable.is_empty() {
        packages = final_stable;
    }
    utils::prepare_package_descriptions_for_ui(&mut packages);
    aggregation::enrich_packages_ratings(&mut packages).await;
    log::debug!(
        "[CARD/DETAILS] get_trending returning {} packages",
        packages.len()
    );
    for (i, p) in packages.iter().take(2).enumerate() {
        log::debug!(
            "[CARD/DETAILS]   trending[{}] name={} canonical_id={} sources={} icon={}",
            i,
            p.name,
            p.canonical_id,
            p.available_sources.as_ref().map(|s| s.len()).unwrap_or(0),
            p.icon.is_some()
        );
    }

    // FINAL FALLBACK: If we still have NO packages (Discovery failed, local AppStream empty),
    // we inject a set of "Hardcoded Essentials" to ensure the Trending section is never empty.
    if packages.is_empty() {
        log::warn!("[TRENDING] All discovery methods failed. Injecting fallback from existing Featured list.");
        let mut fallback_names = discovery_manager::get_all_featured_names();
        fallback_names.truncate(20);

        let items: Vec<(String, Option<String>)> = fallback_names
            .into_iter()
            .map(|n| (n.to_string(), None))
            .collect();

        if let Ok(fallback_pkgs) =
            crate::middleware::aggregation::fetch_and_merge_packages_by_names_impl(
                &state_meta,
                &state_chaotic,
                &state_repo,
                &state_flathub,
                &state_registry.manager,
                items,
                include_flatpak,
                include_aur,
                include_chaotic,
                false,
            )
            .await
        {
            packages = fallback_pkgs;
        }
    }
    if packages.is_empty() {
        let fallback_names = discovery_manager::get_all_featured_names();
        packages = build_registry_named_fallback(state_registry.inner(), &fallback_names, 20);
        if packages.is_empty() {
            packages = build_local_named_fallback(state_meta.inner(), &fallback_names, 20);
        }
    }
    if packages.is_empty() {
        let backup = hardcoded_discovery_names("trending");
        packages = build_registry_named_fallback(state_registry.inner(), &backup, 20);
        if packages.is_empty() {
            packages = build_local_named_fallback(state_meta.inner(), &backup, 20);
        }
    }

    crate::utils::finalize_packages_contract(&mut packages);
    log::info!(
        "[DISCOVERY] trending snapshot returning {} packages (flatpak={}, aur={}, chaotic={})",
        packages.len(),
        include_flatpak,
        include_aur,
        include_chaotic
    );
    TRENDING_CACHE.insert(cache_key, packages.clone()).await;
    write_trending_disk(include_flatpak, include_aur, include_chaotic, &packages);

    // PERSIST TO REGISTRY: Seed the persistent index with these trending apps.
    let _ = state_registry.manager.bulk_upsert_packages(&packages);

    Ok(packages)
}

#[tauri::command]
#[specta::specta]
pub async fn get_trending_snapshot(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, RepoManager>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_discovery: State<'_, discovery_manager::DiscoveryManager>,
    state_registry: State<'_, crate::registry::RegistryState>,
    options: Option<SearchOptions>,
) -> Result<Vec<models::Package>, String> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        get_trending(
            state_meta.clone(),
            state_chaotic.clone(),
            state_repo.clone(),
            state_flathub.clone(),
            state_discovery.clone(),
            state_registry.clone(),
            options,
        ),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            log::warn!("[DISCOVERY] get_trending_snapshot timed out; using local fallback");
            let backup = hardcoded_discovery_names("trending");
            Ok(build_static_named_fallback(&backup, 20))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_essentials_snapshot(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, RepoManager>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_registry: State<'_, crate::registry::RegistryState>,
) -> Result<Vec<models::Package>, String> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(4),
        async {
            state_meta.inner().wait_until_ready().await;
            build_essentials_snapshot_impl(
                state_meta.inner(),
                state_chaotic.inner(),
                state_repo.inner(),
                state_flathub.inner(),
                state_registry.inner(),
            )
            .await
        },
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            log::warn!("[DISCOVERY] get_essentials_snapshot timed out; using local fallback");
            let backup = hardcoded_discovery_names("essentials");
            Ok(build_static_named_fallback(&backup, 16))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_discovery_home_snapshot(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, RepoManager>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_discovery: State<'_, discovery_manager::DiscoveryManager>,
    state_registry: State<'_, crate::registry::RegistryState>,
) -> Result<models::DiscoveryHomeSnapshot, String> {
    build_discovery_home_snapshot_impl(
        state_meta.inner(),
        state_chaotic.inner(),
        state_repo.inner(),
        state_flathub.inner(),
        state_discovery.inner(),
        state_registry.inner(),
    )
    .await
}

pub(crate) async fn build_discovery_home_snapshot_impl(
    state_meta: &metadata::MetadataState,
    state_chaotic: &chaotic_api::ChaoticApiClient,
    state_repo: &RepoManager,
    state_flathub: &crate::flathub_api::FlathubApiClient,
    _state_discovery: &discovery_manager::DiscoveryManager,
    state_registry: &crate::registry::RegistryState,
) -> Result<models::DiscoveryHomeSnapshot, String> {
    if let Some(cached) = HOME_DISCOVERY_CACHE.get(&"home").await {
        return Ok(cached);
    }

    let _guard = HOME_DISCOVERY_GATE.lock().await;
    if let Some(cached) = HOME_DISCOVERY_CACHE.get(&"home").await {
        return Ok(cached);
    }

    let essentials_fut = async {
        match tokio::time::timeout(
            std::time::Duration::from_millis(1800),
            build_essentials_snapshot_impl(
                state_meta,
                state_chaotic,
                state_repo,
                state_flathub,
                state_registry,
            ),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                log::warn!("[DISCOVERY] build_discovery_home_snapshot_impl essentials timed out; using local fallback");
                let backup = hardcoded_discovery_names("essentials");
                let mut fallback = build_registry_named_fallback(state_registry, &backup, 16);
                if fallback.is_empty() {
                    fallback = build_local_named_fallback(state_meta, &backup, 16);
                }
                if fallback.is_empty() {
                    fallback = build_static_named_fallback(&backup, 16);
                }
                Ok(fallback)
            }
        }
    };

    let trending_fut = async {
        let result: Result<Vec<models::Package>, String> = async {
        let cache_key = (true, true, true);
        if let Some(mut cached) = TRENDING_CACHE.get(&cache_key).await.filter(|items| !items.is_empty()) {
            crate::utils::finalize_packages_contract(&mut cached);
            return Ok(cached);
        }

        if let Some(mut disk_cached) = try_read_trending_disk(true, true, true) {
            crate::utils::finalize_packages_contract(&mut disk_cached);
            TRENDING_CACHE.insert(cache_key, disk_cached.clone()).await;
            return Ok(disk_cached);
        }

        log::warn!("[DISCOVERY] build_discovery_home_snapshot_impl trending cache unavailable; using static fallback");
        let backup = hardcoded_discovery_names("trending");
        let mut fallback = build_registry_named_fallback(state_registry, &backup, 20);
        if fallback.is_empty() {
            fallback = build_static_named_fallback(&backup, 20);
        }
        Ok(fallback)
        }
        .await;
        result
    };

    let (essentials_res, trending_res) = tokio::join!(essentials_fut, trending_fut);
    let mut errors = Vec::new();
    let essentials = match essentials_res {
        Ok(items) => items,
        Err(error) => {
            errors.push(format!("essentials={error}"));
            Vec::new()
        }
    };
    let trending = match trending_res {
        Ok(items) => items,
        Err(error) => {
            errors.push(format!("trending={error}"));
            Vec::new()
        }
    };

    if essentials.is_empty() && trending.is_empty() {
        let reason = if errors.is_empty() {
            "empty".to_string()
        } else {
            errors.join("; ")
        };
        return Err(format!("discovery_snapshot_unavailable:{reason}"));
    }

    let generated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0);

    let snapshot = models::DiscoveryHomeSnapshot {
        essentials,
        trending,
        quick_starts: build_quick_starts(),
        generated_at,
        stale: false,
    };

    HOME_DISCOVERY_CACHE.insert("home", snapshot.clone()).await;
    Ok(snapshot)
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)] // Tauri State + names/options/cache_context
pub async fn get_packages_by_names(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, RepoManager>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_registry: State<'_, crate::registry::RegistryState>,
    names: Vec<String>,
    options: Option<SearchOptions>,
    cache_context: Option<String>,
) -> Result<Vec<models::Package>, String> {
    state_meta.inner().wait_until_ready().await;

    let flatpak_enabled = options.as_ref().and_then(|o| o.flatpak_enabled);
    let aur_enabled = options.as_ref().and_then(|o| o.aur_enabled);
    let chaotic_enabled = options.as_ref().and_then(|o| o.chaotic_enabled);
    let backend_flatpak_enabled = state_repo.inner().is_flatpak_enabled().await;
    let backend_aur_enabled = state_repo.inner().is_aur_enabled().await;
    let backend_chaotic_enabled = state_repo.inner().is_repo_enabled("chaotic-aur").await;

    let include_flatpak = backend_flatpak_enabled && flatpak_enabled.unwrap_or(true);
    let include_aur = backend_aur_enabled && aur_enabled.unwrap_or(true);

    let for_installed_lookup = options.as_ref().and_then(|o| o.for_installed_lookup);
    let installed_lookup = for_installed_lookup == Some(true);
    let include_chaotic =
        installed_lookup || (backend_chaotic_enabled && chaotic_enabled.unwrap_or(true));

    // 1. Try Disk Cache (optimized for "Essentials" which is usually 40+ packages)
    if cache_context.as_deref() == Some("essentials") {
        if let Some(mut cached) = try_get_packages_from_cache(&names) {
            let mut to_check = std::collections::HashSet::new();
            for p in &cached {
                to_check.insert(p.name.clone());
                if let Some(base) = p.name.strip_suffix("-bin") {
                    to_check.insert(base.to_string());
                }
            }
            let names_to_check: Vec<String> = to_check.into_iter().collect();
            let installed_set = tokio::task::spawn_blocking(move || {
                let mut set = std::collections::HashSet::new();
                for name in names_to_check {
                    if crate::utils::is_package_or_alias_installed(&name) {
                        set.insert(name);
                    }
                }
                set
            })
            .await
            .map_err(|e| e.to_string())?;

            for pkg in &mut cached {
                let base = pkg.name.strip_suffix("-bin").unwrap_or(&pkg.name);
                pkg.installed = installed_set.contains(&pkg.name) || installed_set.contains(base);
            }
            crate::utils::finalize_packages_contract(&mut cached);
            return Ok(cached);
        }
    }

    // 2. Multi-threaded merge using the shared middleware implementation.
    // Pair each name with None as app_id (identity merging handles normalization).
    let items: Vec<(String, Option<String>)> = names.iter().map(|n| (n.clone(), None)).collect();

    let mut packages = crate::middleware::aggregation::fetch_and_merge_packages_by_names_impl(
        &state_meta,
        &state_chaotic,
        &state_repo,
        &state_flathub,
        &state_registry.manager,
        items,
        include_flatpak,
        include_aur,
        include_chaotic,
        installed_lookup,
    )
    .await?;

    // 3. Write to Disk Cache if context set
    crate::utils::finalize_packages_contract(&mut packages);

    if cache_context.as_deref() == Some("essentials") && !packages.is_empty() {
        write_packages_cache(&names, &packages);
    }

    Ok(packages)
}

#[tauri::command]
#[specta::specta]
pub async fn get_packages_by_canonical_ids(
    state_registry: State<'_, crate::registry::RegistryState>,
    ids: Vec<String>,
) -> Result<Vec<models::Package>, String> {
    let mut packages = state_registry.get_packages_by_canonical_ids(&ids)?;
    crate::utils::finalize_packages_contract(&mut packages);
    Ok(packages)
}

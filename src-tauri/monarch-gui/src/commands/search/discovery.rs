use crate::{
    chaotic_api, discovery_manager, metadata, middleware::aggregation, models,
    repo_manager::RepoManager, utils,
};
use tauri::State;

use super::cache::{
    try_get_packages_from_cache, try_read_trending_disk, write_packages_cache, write_trending_disk,
    TRENDING_CACHE,
};
use super::core::SearchOptions;

/// When cache is empty, run discovery fetch in this command (up to this duration) so we don't rely on background spawn.
pub(crate) const TRENDING_REFRESH_TIMEOUT_MS: u64 = 18_000;

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

    let include_flatpak = flatpak_enabled.unwrap_or(true);
    let include_aur = aur_enabled.unwrap_or(state_repo.inner().is_aur_enabled().await);
    let include_chaotic =
        chaotic_enabled.unwrap_or(state_repo.inner().is_repo_enabled("chaotic-aur").await);
    let cache_key = (include_flatpak, include_aur, include_chaotic);

    state_meta.inner().wait_until_ready().await;
    if let Some(cached) = TRENDING_CACHE.get(&cache_key).await {
        return Ok(cached);
    }

    // Moka miss — try disk cache (warm start after restart)
    if let Some(disk_cached) = try_read_trending_disk(include_flatpak, include_aur, include_chaotic)
    {
        TRENDING_CACHE.insert(cache_key, disk_cached.clone()).await;
        return Ok(disk_cached);
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
    let mut packages = aggregation::merge_search_results(
        official_packages.clone(),
        aur_packages.clone(),
        flathub_hits,
        &state_registry.manager,
        &installed_flatpaks,
    );
    packages = utils::deduplicate_by_canonical_key(packages);
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

        packages = aggregation::merge_search_results(
            official_packages,
            aur_packages,
            flathub_hits,
            &state_registry.manager,
            &installed_flatpaks,
        );
        packages = utils::deduplicate_by_canonical_key(packages);
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
    if include_chaotic && crate::distro_context::DistroContext::new().is_chaotic_compatible() {
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
                        installed: crate::alpm_read::is_package_installed(&name),
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
    // available_sources are already populated by merge_search_results above.
    aggregation::enrich_packages_metadata(&mut packages, state_flathub.inner()).await;

    packages = utils::merge_and_deduplicate(Vec::new(), packages);
    utils::prepare_package_descriptions_for_ui(&mut packages);
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

    TRENDING_CACHE.insert(cache_key, packages.clone()).await;
    write_trending_disk(include_flatpak, include_aur, include_chaotic, &packages);

    // PERSIST TO REGISTRY: Seed the persistent index with these trending apps.
    let _ = state_registry.manager.bulk_upsert_packages(&packages);

    Ok(packages)
}

#[tauri::command]
#[specta::specta]
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
    let common_chaotic_enabled = state_repo.inner().is_repo_enabled("chaotic-aur").await;

    let include_flatpak = flatpak_enabled.unwrap_or(true);
    let include_aur = aur_enabled.unwrap_or(state_repo.inner().is_aur_enabled().await);
    let include_chaotic = common_chaotic_enabled;

    let for_installed_lookup = options.as_ref().and_then(|o| o.for_installed_lookup);
    let installed_lookup = for_installed_lookup == Some(true);

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
                    if crate::alpm_read::is_package_installed(&name) {
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
            return Ok(cached);
        }
    }

    // 2. Multi-threaded merge using the shared middleware implementation.
    // Pair each name with None as app_id (identity merging handles normalization).
    let items: Vec<(String, Option<String>)> = names.iter().map(|n| (n.clone(), None)).collect();

    let packages = crate::middleware::aggregation::fetch_and_merge_packages_by_names_impl(
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
    if cache_context.as_deref() == Some("essentials") && !packages.is_empty() {
        write_packages_cache(&names, &packages);
    }

    Ok(packages)
}

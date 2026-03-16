use crate::{
    aur_api, chaotic_api, discovery_manager, metadata,
    middleware::aggregation,
    models::{self, Package},
    repo_manager::RepoManager,
    utils,
};
use std::collections::{HashMap, HashSet};
use tauri::State;
use std::time::Instant;

use super::ranking::calculate_relevance;
use crate::flathub_api::{FlathubApiClient, SearchResult};
use super::cache::{SEARCH_RESULTS_CACHE, SEARCH_SNAPSHOT_CACHE};

#[derive(serde::Deserialize, specta::Type, Debug, Clone)]
pub struct SearchOptions {
    pub flatpak_enabled: Option<bool>,
    pub aur_enabled: Option<bool>,
    pub chaotic_enabled: Option<bool>,
    pub for_installed_lookup: Option<bool>,
}

#[derive(serde::Deserialize, specta::Type, Debug, Clone)]
pub struct CategoryQuery {
    pub category: String,
    pub repo_filter: Option<Vec<String>>,
    pub sort_by: Option<String>,
    pub page: u32,
    pub limit: u32,
    pub options: Option<SearchOptions>,
}

fn normalize_search_query(raw: &str) -> String {
    raw.split_whitespace()
        .filter(|token| {
            let lower = token.to_lowercase();
            !(lower.starts_with('@') || lower.starts_with("in:") || lower.starts_with("sort:"))
        })
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn search_aliases(query: &str) -> Option<(&'static str, &'static str)> {
    match query.trim().to_lowercase().as_str() {
        "browser" | "web browser" => Some(("Popular browsers", "firefox chromium brave librewolf")),
        "chrome" => Some(("Chromium-based browsers", "google-chrome chromium ungoogled-chromium brave-bin vivaldi")),
        "photoshop" => Some(("Popular alternatives to Photoshop", "gimp krita")),
        "video editor" => Some(("Popular video editors", "kdenlive shotcut")),
        "music player" => Some(("Popular music players", "spotify vlc audacious")),
        "office" | "office suite" => Some(("Office and school apps", "libreoffice onlyoffice thunderbird")),
        "terminal" => Some(("Terminal and development tools", "wezterm kitty alacritty")),
        _ => None,
    }
}

fn build_search_suggestions(query: &str, results: &[Package]) -> Vec<models::SearchSuggestion> {
    let normalized = query.trim().to_lowercase();
    let mut suggestions = Vec::new();

    if let Some((label, alias_query)) = search_aliases(&normalized) {
        suggestions.push(models::SearchSuggestion {
            label: label.to_string(),
            query: alias_query.to_string(),
            reason: "alias".to_string(),
        });
    }

    if results.is_empty() {
        for (label, value, reason) in [
            ("Try Official Repos", format!("@official {}", normalized), "broaden"),
            ("Try Flatpak", format!("@flatpak {}", normalized), "broaden"),
            ("Browse Internet apps", "in:internet".to_string(), "category"),
        ] {
            suggestions.push(models::SearchSuggestion {
                label: label.to_string(),
                query: value,
                reason: reason.to_string(),
            });
        }
    }

    suggestions
}

fn search_alias_terms(query: &str) -> Vec<String> {
    search_aliases(query)
        .map(|(_, alias_query)| {
            alias_query
                .split_whitespace()
                .map(|token| token.trim().to_lowercase())
                .filter(|token| !token.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn package_search_haystack(pkg: &Package) -> String {
    let mut parts = vec![
        pkg.canonical_id.to_lowercase(),
        pkg.name.to_lowercase(),
        pkg.display_name.clone().unwrap_or_default().to_lowercase(),
        pkg.description.to_lowercase(),
        pkg.app_id.clone().unwrap_or_default().to_lowercase(),
    ];
    if let Some(sources) = &pkg.available_sources {
        for src in sources {
            parts.push(src.id.to_lowercase());
            parts.push(src.label.to_lowercase());
            if let Some(pkg_name) = &src.package_name {
                parts.push(pkg_name.to_lowercase());
            }
        }
    }
    parts.join(" ")
}

async fn get_or_build_search_snapshot(
    state_registry: &crate::registry::RegistryState,
) -> Result<Vec<Package>, String> {
    if let Some(cached) = SEARCH_SNAPSHOT_CACHE.get(&"global").await {
        return Ok(cached);
    }

    let mut packages = state_registry.manager.search_packages_sql("", 1500)?;
    crate::utils::finalize_packages_contract(&mut packages);
    SEARCH_SNAPSHOT_CACHE
        .insert("global", packages.clone())
        .await;
    log::info!(
        "[SEARCH] canonical snapshot built packages={}",
        packages.len()
    );
    Ok(packages)
}

fn search_packages_from_snapshot(snapshot: &[Package], query: &str) -> Vec<Package> {
    let normalized = query.trim().to_lowercase();
    if normalized.len() < 2 {
        return Vec::new();
    }

    let query_terms: Vec<String> = normalized
        .split_whitespace()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    let alias_terms = search_alias_terms(&normalized);

    let mut scored: Vec<(i32, Package)> = snapshot
        .iter()
        .filter_map(|pkg| {
            let haystack = package_search_haystack(pkg);
            let canonical = pkg.canonical_id.to_lowercase();
            let name = pkg.name.to_lowercase();
            let display = pkg.display_name.clone().unwrap_or_default().to_lowercase();
            let app_id = pkg.app_id.clone().unwrap_or_default().to_lowercase();

            let mut score = 0i32;
            if canonical == normalized || name == normalized || display == normalized || app_id == normalized {
                score += 500;
            }
            if name.contains(&normalized) || display.contains(&normalized) || app_id.contains(&normalized) {
                score += 220;
            }
            if query_terms.iter().all(|term| haystack.contains(term)) {
                score += 140;
            }
            if alias_terms.iter().any(|term| haystack.contains(term)) {
                score += 90;
            }
            if pkg.installed {
                score += 25;
            }
            if pkg.is_featured.unwrap_or(false) {
                score += 10;
            }

            if score > 0 {
                Some((score, pkg.clone()))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by(|(score_a, pkg_a), (score_b, pkg_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| pkg_a.name.len().cmp(&pkg_b.name.len()))
            .then_with(|| pkg_a.name.cmp(&pkg_b.name))
    });

    scored
        .into_iter()
        .take(50)
        .map(|(_, pkg)| pkg)
        .collect()
}

pub(crate) async fn prewarm_search_snapshot(
    state_registry: &crate::registry::RegistryState,
) -> Result<(), String> {
    let _ = get_or_build_search_snapshot(state_registry).await?;
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
#[specta::specta]
pub async fn search_packages(
    state_repo: State<'_, RepoManager>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_flathub: State<'_, FlathubApiClient>,
    state_metadata: State<'_, metadata::MetadataState>,
    state_registry: State<'_, crate::registry::RegistryState>,
    state_distro: State<'_, crate::distro_context::DistroContext>,
    state_discovery: State<'_, discovery_manager::DiscoveryManager>,
    query: String,
    options: Option<SearchOptions>,
) -> Result<Vec<Package>, String> {
    state_metadata.inner().wait_until_ready().await;

    if query.trim().len() < 2 {
        return Ok(Vec::new());
    }

    let flatpak_enabled = options.as_ref().and_then(|o| o.flatpak_enabled);
    let aur_enabled = options.as_ref().and_then(|o| o.aur_enabled);
    let chaotic_enabled = options.as_ref().and_then(|o| o.chaotic_enabled);
    let for_installed_lookup = options.as_ref().and_then(|o| o.for_installed_lookup);

    let installed_lookup = for_installed_lookup == Some(true);
    let backend_flatpak_enabled = state_repo.inner().is_flatpak_enabled().await;
    let backend_aur_enabled = state_repo.inner().is_aur_enabled().await;
    let backend_chaotic_enabled = state_repo.inner().is_repo_enabled("chaotic-aur").await;
    let include_flatpak = installed_lookup
        || (backend_flatpak_enabled && flatpak_enabled.unwrap_or(true));
    let include_aur = installed_lookup || (backend_aur_enabled && aur_enabled.unwrap_or(true));
    let include_chaotic =
        installed_lookup || (backend_chaotic_enabled && chaotic_enabled.unwrap_or(true));

    let cache_key = format!(
        "q={}::f={}::a={}::c={}::i={}",
        query.trim().to_lowercase(),
        include_flatpak,
        include_aur,
        include_chaotic,
        installed_lookup
    );
    if let Some(cached) = SEARCH_RESULTS_CACHE.get(&cache_key).await {
        log::debug!(
            "[SEARCH] cache-hit query='{}' results={}",
            query,
            cached.len()
        );
        return Ok(cached);
    }

    let query_lower = query.to_lowercase();
    let repo_manager = state_repo.inner();
    let flathub = state_flathub.inner();
    let t0 = Instant::now();

    // Expansion terms (e.g. "heroic" -> "heroic-games-launcher")
    let expansion_terms: Vec<String> = utils::aur_search_expansion_terms(&query)
        .into_iter()
        .map(String::from)
        .collect();
    // Interactive AUR search: single broad query for responsiveness.
    // Expansion terms are retained for repo matching but are too expensive as extra AUR network calls.
    let aur_terms: Vec<String> = vec![query.clone()];
    // Repo/CachyOS search: include expansion terms
    let repo_query = if expansion_terms.is_empty() {
        query.clone()
    } else {
        format!("{} {}", query.trim(), expansion_terms.join(" "))
    };

    let distro = state_distro.inner();
    let advanced_mode = state_repo.inner().is_advanced_mode().await;
    let chaotic_allowed = distro.is_chaotic_compatible() || advanced_mode;

    let remote_augmentation = installed_lookup;
    let (registry_res, legacy_res, aur_res, flatpak_res, chaotic, installed_flatpaks) = tokio::join!(
        async { state_registry.manager.search_packages_sql(&query, 50) },
        repo_manager.get_packages_matching(&repo_query, state_distro.inner()),
        async {
            if remote_augmentation && include_aur && !aur_terms.is_empty() {
                let mut seen = HashSet::new();
                let mut merged: Vec<models::Package> = Vec::new();

                let futures = aur_terms
                    .iter()
                    .map(|term| async move {
                        let res = tokio::time::timeout(
                            std::time::Duration::from_secs(6),
                            crate::aur_api::search_aur(term),
                        )
                        .await
                        .map_err(|_| "AUR search timeout".to_string())
                        .and_then(|inner| inner);
                        (term, res)
                    });

                let results = futures::future::join_all(futures).await;

                for (term, res) in results {
                    match res {
                        Ok(pkgs) => {
                            for p in pkgs {
                                if seen.insert(p.name.clone()) {
                                    merged.push(p);
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("[SEARCH] AUR Search failed for term '{}': {}", term, e);
                        }
                    }
                }
                Ok::<Vec<models::Package>, String>(merged)
            } else {
                Ok::<Vec<models::Package>, String>(Vec::new())
            }
        },
        async {
            if remote_augmentation && include_flatpak {
                flathub.search_flathub(&query).await
            } else {
                None
            }
        },
        async {
            let actually_chaotic_enabled = remote_augmentation && chaotic_allowed && include_chaotic;
            if !actually_chaotic_enabled {
                return Vec::new();
            }
            let fetch = tokio::time::timeout(
                std::time::Duration::from_secs(4),
                state_chaotic.inner().fetch_packages(),
            )
            .await;
            let chaotic_data = match fetch {
                Ok(Ok(arc)) => arc,
                Ok(Err(e)) => {
                    log::warn!("[SEARCH] Chaotic-AUR fetch failed: {}", e);
                    return Vec::new();
                }
                Err(_) => {
                    log::warn!("[SEARCH] Chaotic-AUR fetch timed out for query '{}'", query);
                    return Vec::new();
                }
            };
            let query_parts: Vec<&str> = query_lower.split_whitespace().collect();
            let expansion_lower: Vec<String> =
                expansion_terms.iter().map(|t| t.to_lowercase()).collect();
            chaotic_data
                .iter()
                .filter(|p| {
                    let name_lower = p.pkgname.to_lowercase();
                    let name_ok = name_lower.contains(&query_lower)
                        || query_parts.iter().all(|q| name_lower.contains(q))
                        || expansion_lower.iter().any(|t| name_lower.contains(t));
                    let desc_ok = p
                        .metadata
                        .as_ref()
                        .and_then(|m| m.desc.as_deref())
                        .map(|d| d.to_lowercase().contains(&query_lower))
                        .unwrap_or(false);
                    name_ok || desc_ok
                })
                .map(|p| {
                    let version = p.version.clone().unwrap_or_default();
                    Package {
                        name: p.pkgname.clone(),
                        display_name: Some(utils::to_pretty_name(&p.pkgname)),
                        description: p
                            .metadata
                            .as_ref()
                            .and_then(|m| m.desc.clone())
                            .unwrap_or_default(),
                        version: version.clone(),
                        source: models::PackageSource::chaotic(&p.pkgname),
                        maintainer: Some("Chaotic-AUR Team".to_string()),
                        license: p
                            .metadata
                            .as_ref()
                            .and_then(|m| m.license.clone())
                            .map(|l| vec![l]),
                        url: p.metadata.as_ref().and_then(|m| m.url.clone()),
                        installed: crate::utils::is_package_or_alias_installed(&p.pkgname),
                        ..Default::default()
                    }
                })
                .map(|mut pkg| {
                    if let Ok(guard) = state_metadata.loader.lock() {
                        pkg.app_id = guard.find_app_id(&pkg.name);
                        pkg.icon = guard.find_icon_heuristic(&pkg.name);
                    }
                    pkg
                })
                .collect()
        },
        async {
            if remote_augmentation && include_flatpak {
                crate::flathub_api::get_installed_flatpak_app_ids()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .collect::<HashSet<String>>()
            } else {
                HashSet::new()
            }
        }
    );
    log::debug!(
        "[SEARCH] phase_a_ms={} query='{}' aur_used={}",
        t0.elapsed().as_millis(),
        query,
        remote_augmentation
    );

    let distro_id_str = match &state_distro.id {
        crate::distro_context::DistroId::Manjaro => "manjaro",
        crate::distro_context::DistroId::Garuda => "garuda",
        crate::distro_context::DistroId::CachyOS => "cachyos",
        crate::distro_context::DistroId::EndeavourOS => "endeavouros",
        crate::distro_context::DistroId::Arch => "arch",
        crate::distro_context::DistroId::Unknown(s) => s.as_str(),
    };

    let mut official: Vec<Package> = if let Ok(sql_pkgs) = registry_res {
        if !sql_pkgs.is_empty() {
            sql_pkgs
        } else {
            legacy_res.unwrap_or_default()
        }
    } else {
        legacy_res.unwrap_or_default()
    }
    .into_iter()
    .map(|mut p| {
        if p.source.id == "local" {
            p.source.label = "Installed (Local)".to_string();
        } else {
            p.source.label =
                crate::labels::get_friendly_label(&p.source.id, distro_id_str).to_string();
        }
        p
    })
    .collect();

    let mut aur: Vec<Package> = aur_res.unwrap_or_default();
    let flatpak_raw: Vec<SearchResult> = flatpak_res.unwrap_or_default();

    // Interactive search must stay responsive.
    // Avoid per-query `flatpak remote-ls` (can block for several seconds on some systems).
    // We still include Flatpak hits and their rich metadata; version can be resolved later in details.
    let mut flatpak: Vec<(SearchResult, Option<String>)> =
        flatpak_raw.into_iter().map(|hit| (hit, None)).collect();

    official.extend(chaotic);

    // Iron Core Enrichment
    let mut candidate_keys = HashSet::new();
    for p in &official {
        candidate_keys.insert(utils::canonical_merge_key(&p.name, p.app_id.as_deref()));
    }
    for p in &aur {
        candidate_keys.insert(utils::canonical_merge_key(&p.name, p.app_id.as_deref()));
    }
    for (hit, _) in &flatpak {
        candidate_keys.insert(hit.app_id.to_lowercase());
        candidate_keys.insert(utils::canonical_merge_key(&hit.name, Some(&hit.app_id)));
    }

    let candidate_vec: Vec<String> = candidate_keys.into_iter().collect();
    let registry_pkgs_map = if !candidate_vec.is_empty() {
        if let Ok(pkgs) = state_registry
            .manager
            .get_packages_by_canonical_ids(&candidate_vec)
        {
            pkgs.into_iter()
                .map(|p| (p.canonical_id.clone(), p))
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    if !registry_pkgs_map.is_empty() {
        for p in &mut official {
            let key = utils::canonical_merge_key(&p.name, p.app_id.as_deref());
            if let Some(reg) = registry_pkgs_map.get(&key) {
                if let Some(dn) = &reg.display_name {
                    if !dn.is_empty() {
                        p.display_name = Some(dn.clone());
                    }
                }
                if reg
                    .icon
                    .as_deref()
                    .map(|i| i.starts_with("http") || i.starts_with("data:"))
                    .unwrap_or(false)
                    || p.icon.is_none()
                {
                    p.icon = reg.icon.clone();
                }
                if let Some(id) = &reg.app_id {
                    p.app_id = Some(id.clone());
                }
            }
        }
        for p in &mut aur {
            let key = utils::canonical_merge_key(&p.name, p.app_id.as_deref());
            if let Some(reg) = registry_pkgs_map.get(&key) {
                if let Some(dn) = &reg.display_name {
                    if !dn.is_empty() {
                        p.display_name = Some(dn.clone());
                    }
                }
                if reg
                    .icon
                    .as_deref()
                    .map(|i| i.starts_with("http") || i.starts_with("data:"))
                    .unwrap_or(false)
                    || p.icon.is_none()
                {
                    p.icon = reg.icon.clone();
                }
                if let Some(id) = &reg.app_id {
                    p.app_id = Some(id.clone());
                }
            }
        }
        for (hit, _) in &mut flatpak {
            let id_key = hit.app_id.to_lowercase();
            let name_key = utils::canonical_merge_key(&hit.name, Some(&hit.app_id));
            if let Some(reg) = registry_pkgs_map
                .get(&id_key)
                .or_else(|| registry_pkgs_map.get(&name_key))
            {
                if let Some(dn) = &reg.display_name {
                    if !dn.is_empty() {
                        hit.name = dn.clone();
                    }
                }
                if !reg.description.is_empty() {
                    hit.summary = Some(reg.description.clone());
                }
                if reg
                    .icon
                    .as_deref()
                    .map(|i| i.starts_with("http") || i.starts_with("data:"))
                    .unwrap_or(false)
                    || hit.icon.is_none()
                {
                    hit.icon = reg.icon.clone();
                }
            }
        }
    }

    {
        if let Ok(loader) = state_metadata.loader.lock() {
            use crate::middleware::aggregation::enrich_with_local_metadata;
            enrich_with_local_metadata(&mut official, &loader);
            enrich_with_local_metadata(&mut aur, &loader);
        }
    }

    let mut results = aggregation::build_package_view_models_v2(
        official,
        aur,
        flatpak,
        &state_registry.manager,
        &installed_flatpaks,
    );

    // Do NOT run heavy Flathub per-package enrichment on the interactive search path.
    // Search results already receive registry backfill + source metadata above; deep metadata is resolved in details.
    log::debug!("[SEARCH] phase_b_ms={} query='{}'", t0.elapsed().as_millis(), query);
    // Fetch ODRS ratings for any package that now has a valid RDN app_id (must run AFTER enrich_packages_metadata)
    // REMOVED: Frontend now handles this asynchronously to avoid blocking search results
    // aggregation::enrich_packages_ratings(&mut results).await;
    utils::prepare_package_descriptions_for_ui(&mut results);

    let popular_names: Vec<String> = state_discovery.inner().popular_aur_names().await;
    {
        let metadata_loader = state_metadata
            .loader
            .lock()
            .expect("MetadataState lock poisoned");

        // CPU-tier tiebreaker
        let derive_opt_level = |source_id: &str| -> u8 {
            let id = source_id.to_lowercase();
            if id.contains("-znver4") {
                3
            } else if id.contains("-v4") {
                2
            } else if id.contains("-v3") || id.contains("-core-v3") {
                1
            } else {
                0
            }
        };

        // --- STABLE RANKING (Task 3) ---
        results.sort_by(|a, b| {
            let score_a =
                calculate_relevance(a, &query_lower, &metadata_loader, popular_names.as_slice());
            let score_b =
                calculate_relevance(b, &query_lower, &metadata_loader, popular_names.as_slice());
            score_b
                .cmp(&score_a)
                .then_with(|| {
                    let rank_a = crate::repo_manager::calculate_package_rank(
                        a,
                        derive_opt_level(&a.source.id),
                        distro,
                    );
                    let rank_b = crate::repo_manager::calculate_package_rank(
                        b,
                        derive_opt_level(&b.source.id),
                        distro,
                    );
                    rank_a.cmp(&rank_b)
                })
                .then_with(|| a.name.len().cmp(&b.name.len()))
                .then_with(|| a.name.cmp(&b.name))
        });
    }

    crate::utils::finalize_packages_contract(&mut results);
    SEARCH_RESULTS_CACHE.insert(cache_key, results.clone()).await;
    log::info!(
        "[SEARCH] completed query='{}' total_ms={} results={}",
        query,
        t0.elapsed().as_millis(),
        results.len()
    );

    Ok(results)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
#[specta::specta]
pub async fn search_packages_rich(
    state_repo: State<'_, RepoManager>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_flathub: State<'_, FlathubApiClient>,
    state_metadata: State<'_, metadata::MetadataState>,
    state_registry: State<'_, crate::registry::RegistryState>,
    state_distro: State<'_, crate::distro_context::DistroContext>,
    state_discovery: State<'_, discovery_manager::DiscoveryManager>,
    query: String,
    options: Option<SearchOptions>,
) -> Result<models::SearchResponse, String> {
    let normalized = normalize_search_query(&query);
    let effective_query = if normalized.is_empty() {
        query.trim().to_string()
    } else {
        normalized
    };

    let mut packages = match get_or_build_search_snapshot(state_registry.inner()).await {
        Ok(snapshot) => {
            let fast = search_packages_from_snapshot(&snapshot, &effective_query);
            if !fast.is_empty() {
                log::debug!(
                    "[SEARCH] fast-path query='{}' results={}",
                    effective_query,
                    fast.len()
                );
            }
            fast
        }
        Err(e) => {
            log::warn!("[SEARCH] fast-path snapshot unavailable: {}", e);
            Vec::new()
        }
    };

    let should_augment = packages.len() < 8
        || packages
            .iter()
            .all(|pkg| {
                let q = effective_query.to_lowercase();
                pkg.canonical_id.to_lowercase() != q
                    && pkg.name.to_lowercase() != q
                    && pkg.display_name
                        .as_ref()
                        .map(|value| value.to_lowercase() != q)
                        .unwrap_or(true)
            });

    if should_augment {
        let mut live_packages = search_packages(
            state_repo.clone(),
            state_chaotic.clone(),
            state_flathub.clone(),
            state_metadata.clone(),
            state_registry.clone(),
            state_distro.clone(),
            state_discovery.clone(),
            effective_query.clone(),
            options.clone(),
        )
        .await?;

        if packages.is_empty() {
            packages = live_packages;
        } else {
            let mut seen = std::collections::HashSet::new();
            let mut merged = Vec::new();
            for pkg in packages.into_iter().chain(live_packages.drain(..)) {
                let key = if pkg.canonical_id.is_empty() {
                    crate::utils::canonical_merge_key(&pkg.name, pkg.app_id.as_deref())
                } else {
                    pkg.canonical_id.clone()
                };
                if seen.insert(key) {
                    merged.push(pkg);
                }
            }
            packages = merged;
        }
    }

    let mut interpretation = None;
    if packages.is_empty() || packages.len() <= 1 {
        if let Some((label, alias_query)) = search_aliases(&effective_query) {
            let mut alias_results = search_packages(
                state_repo,
                state_chaotic,
                state_flathub,
                state_metadata,
                state_registry,
                state_distro,
                state_discovery,
                alias_query.to_string(),
                options,
            )
            .await?;
            if !alias_results.is_empty() {
                interpretation = Some(label.to_string());
                if packages.is_empty() {
                    packages = alias_results;
                } else {
                    let mut seen = std::collections::HashSet::new();
                    let mut merged = Vec::new();
                    for pkg in packages.into_iter().chain(alias_results.drain(..)) {
                        if seen.insert(pkg.canonical_id.clone()) {
                            merged.push(pkg);
                        }
                    }
                    packages = merged;
                }
            }
        }
    }

    let suggestions = build_search_suggestions(&effective_query, &packages);

    Ok(models::SearchResponse {
        packages,
        suggestions,
        query_interpretation: interpretation,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn search_aur(query: String) -> Result<Vec<models::Package>, String> {
    crate::aur_api::search_aur(&query).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_chaotic_package_info(
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    name: String,
) -> Result<Option<chaotic_api::ChaoticPackage>, String> {
    Ok(state_chaotic.inner().find_package(&name).await)
}

#[tauri::command]
#[specta::specta]
pub async fn get_chaotic_packages_batch(
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    names: Vec<String>,
) -> Result<HashMap<String, chaotic_api::ChaoticPackage>, String> {
    Ok(state_chaotic.inner().get_packages_batch(names).await)
}

#[tauri::command]
#[specta::specta]
pub async fn get_package_variants(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_flathub: State<'_, crate::flathub_api::FlathubApiClient>,
    state_repo: State<'_, RepoManager>,
    pkg_name: String,
    options: Option<SearchOptions>,
) -> Result<Vec<models::PackageVariant>, String> {
    let flatpak_enabled = options.as_ref().and_then(|o| o.flatpak_enabled);
    let aur_enabled = options.as_ref().and_then(|o| o.aur_enabled);
    let chaotic_enabled = options.as_ref().and_then(|o| o.chaotic_enabled);
    let for_installed_lookup = options.as_ref().and_then(|o| o.for_installed_lookup);

    let installed_lookup = for_installed_lookup == Some(true);
    let backend_flatpak_enabled = state_repo.inner().is_flatpak_enabled().await;
    let backend_aur_enabled = state_repo.inner().is_aur_enabled().await;
    let backend_chaotic_enabled = state_repo.inner().is_repo_enabled("chaotic-aur").await;

    let include_flatpak = installed_lookup || (backend_flatpak_enabled && flatpak_enabled.unwrap_or(true));
    let include_aur = installed_lookup || (backend_aur_enabled && aur_enabled.unwrap_or(true));
    let include_chaotic =
        installed_lookup || (backend_chaotic_enabled && chaotic_enabled.unwrap_or(true));

    let pkg_lower = pkg_name.trim().to_lowercase();
    let base_name = utils::strip_package_suffix(&pkg_lower);
    let canonical_base: String = if pkg_lower.contains('.') {
        pkg_lower
            .split('.')
            .next_back()
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_else(|| base_name.to_string())
    } else {
        base_name.to_string()
    };
    let mapped_id = crate::flathub_api::get_flathub_app_id(&canonical_base);
    let app_id = state_meta
        .loader
        .lock()
        .ok()
        .and_then(|loader| loader.find_app_id(&pkg_name));

    let mut combined_packages = Vec::new();
    let mut search_names: Vec<String> = vec![
        pkg_lower.clone(),
        base_name.to_string(),
        canonical_base.clone(),
    ];
    for repo_name in utils::canonical_to_repo_lookup_names(&canonical_base) {
        search_names.push(repo_name.to_string());
    }
    search_names.sort();
    search_names.dedup();

    let enabled_repo_names = if installed_lookup {
        Vec::new()
    } else {
        state_repo.inner().get_enabled_repo_names().await
    };

    let search_names_clone = search_names.clone();
    let enabled_repo_names_clone = enabled_repo_names.clone();
    let repo_pkgs = tokio::task::spawn_blocking(move || {
        crate::alpm_read::get_packages_batch(&search_names_clone, &enabled_repo_names_clone)
    })
    .await
    .map_err(|e| e.to_string())?;
    combined_packages.extend(repo_pkgs);

    let advanced_mode = state_repo.inner().is_advanced_mode().await;
    let chaotic_allowed = crate::distro_context::DistroContext::new().is_chaotic_compatible()
        || advanced_mode;
    if chaotic_allowed && include_chaotic {
        let chaotic_fetch = tokio::time::timeout(
            std::time::Duration::from_secs(4),
            state_chaotic.inner().fetch_packages(),
        )
        .await;
        if let Ok(Ok(chaotic_arc)) = chaotic_fetch {
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
                    installed: crate::utils::is_package_or_alias_installed(&p.pkgname),
                    ..Default::default()
                })
                .collect();
            combined_packages.extend(matches);
        } else {
            log::warn!(
                "[SEARCH] Skipping Chaotic package variants for '{}' due to timeout or fetch failure",
                pkg_name
            );
        }
    }

    if include_aur {
        if let Ok(aur_results) = aur_api::search_aur(&canonical_base).await {
            for p in aur_results {
                if utils::canonical_merge_key(&p.name, p.app_id.as_deref()) == canonical_base
                    || p.name.to_lowercase() == pkg_lower
                    || utils::strip_package_suffix(&p.name.to_lowercase()) == canonical_base
                {
                    combined_packages.push(p);
                }
            }
        }
        if canonical_base != base_name && base_name != pkg_lower {
            if let Ok(aur_results) = aur_api::search_aur(base_name).await {
                for p in aur_results {
                    if utils::canonical_merge_key(&p.name, p.app_id.as_deref()) == canonical_base
                        && !combined_packages.iter().any(|c| c.name == p.name)
                    {
                        combined_packages.push(p);
                    }
                }
            }
        }
    }

    if include_flatpak {
        if let Some(flatpak_results) = state_flathub.inner().search_flathub(&canonical_base).await {
            for hit in flatpak_results {
                let hit_canonical = utils::canonical_merge_key(&hit.app_id, Some(&hit.app_id));
                if hit_canonical == canonical_base || hit.app_id.eq_ignore_ascii_case(&pkg_name) {
                    combined_packages.push(models::Package {
                        name: hit.app_id.clone(),
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
        }
    }

    let mut final_variants: Vec<models::PackageVariant> = Vec::new();
    let mut seen = HashSet::new();
    for p in combined_packages {
        let p_source = p.source.clone();
        let p_lower = p.name.to_lowercase();
        let p_app_id = state_meta
            .loader
            .lock()
            .ok()
            .and_then(|loader| loader.find_app_id(&p.name));
        let matches_app_id = app_id.is_some() && p_app_id == app_id;
        let is_flatpak_match = p_source.source_type == "flatpak"
            && (p_lower.ends_with(&format!(".{}", canonical_base))
                || p_lower.ends_with(&format!(".{}", base_name))
                || mapped_id
                    .as_deref()
                    .map(|id| id.eq_ignore_ascii_case(&p.name))
                    .unwrap_or(false));
        let p_canonical = utils::canonical_merge_key(&p.name, p.app_id.as_deref());
        let matches_name = p_lower == pkg_lower
            || p_lower == base_name
            || p_lower == canonical_base
            || p_canonical == canonical_base
            || utils::strip_package_suffix(&p_lower) == base_name
            || utils::strip_package_suffix(&p_lower) == canonical_base
            || is_flatpak_match;

        if matches_app_id || matches_name {
            if p_source.source_type == "repo"
                && (p_source.id.is_empty()
                    || p_source.id.eq_ignore_ascii_case("other")
                    || p_source.id.eq_ignore_ascii_case("unknown"))
            {
                continue;
            }
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
                    download_size: p.download_size,
                    installed_size: p.installed_size,
                    maintainer: p.maintainer.clone(),
                    license: p.license.clone(),
                    description: Some(p.description.clone()),
                    screenshots: p.screenshots.clone(),
                    security: None,
                });
                seen.insert(key);
                if p_source.source_type == "flatpak" && p_source.id == "flathub" {
                    let beta_source = models::PackageSource::new(
                        "flatpak",
                        "flathub-beta",
                        &p.version,
                        "Flatpak (Beta)",
                    );
                    let beta_key = format!("{:?}-{}", beta_source, p.name);
                    if !seen.contains(&beta_key) {
                        final_variants.push(models::PackageVariant {
                            source: beta_source,
                            version: p.version.clone(),
                            repo_name: None,
                            pkg_name: Some(p.name.clone()),
                            download_size: p.download_size,
                            installed_size: p.installed_size,
                            maintainer: p.maintainer.clone(),
                            license: p.license.clone(),
                            description: Some(p.description.clone()),
                            screenshots: p.screenshots.clone(),
                            security: None,
                        });
                        seen.insert(beta_key);
                    }
                }
            }
        }
    }
    Ok(final_variants)
}

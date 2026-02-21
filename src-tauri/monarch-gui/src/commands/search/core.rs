use crate::{
    aur_api, chaotic_api, discovery_manager, metadata,
    middleware::aggregation,
    models::{self, Package},
    repo_manager::RepoManager,
    utils,
};
use std::collections::{HashMap, HashSet};
use tauri::State;

use super::ranking::calculate_relevance;
use crate::flathub_api::{FlathubApiClient, SearchResult};

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
    let include_flatpak = installed_lookup || flatpak_enabled.unwrap_or(true);
    let include_aur =
        installed_lookup || aur_enabled.unwrap_or(state_repo.inner().is_aur_enabled().await);
    let include_chaotic = installed_lookup
        || chaotic_enabled.unwrap_or(state_repo.inner().is_repo_enabled("chaotic-aur").await);

    let query_lower = query.to_lowercase();
    let repo_manager = state_repo.inner();
    let flathub = state_flathub.inner();

    // Expansion terms (e.g. "heroic" -> "heroic-games-launcher")
    let expansion_terms: Vec<String> = utils::aur_search_expansion_terms(&query)
        .into_iter()
        .map(String::from)
        .collect();
    let aur_terms: Vec<String> = std::iter::once(query.clone())
        .chain(expansion_terms.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    // Repo/CachyOS search: include expansion terms
    let repo_query = if expansion_terms.is_empty() {
        query.clone()
    } else {
        format!("{} {}", query.trim(), expansion_terms.join(" "))
    };

    let distro = state_distro.inner();
    let chaotic_allowed = distro.is_chaotic_compatible();

    let (registry_res, legacy_res, aur_res, flatpak_res, chaotic, installed_flatpaks) = tokio::join!(
        async { state_registry.manager.search_packages_sql(&query, 50) },
        repo_manager.get_packages_matching(&repo_query, state_distro.inner()),
        async {
            if include_aur && !aur_terms.is_empty() {
                let mut seen = HashSet::new();
                let mut merged: Vec<models::Package> = Vec::new();
                for term in &aur_terms {
                    match crate::aur_api::search_aur(term).await {
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
            if include_flatpak {
                flathub.search_flathub(&query).await
            } else {
                None
            }
        },
        async {
            let actually_chaotic_enabled = chaotic_allowed && include_chaotic;
            if !actually_chaotic_enabled {
                return Vec::new();
            }
            match state_chaotic.inner().fetch_packages().await {
                Ok(arc) => {
                    let query_parts: Vec<&str> = query_lower.split_whitespace().collect();
                    let expansion_lower: Vec<String> =
                        expansion_terms.iter().map(|t| t.to_lowercase()).collect();
                    arc.iter()
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
                            let mut pkg = Package {
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
                                installed: crate::alpm_read::is_package_installed(&p.pkgname),
                                ..Default::default()
                            };
                            if let Ok(guard) = state_metadata.loader.lock() {
                                pkg.app_id = guard.find_app_id(&p.pkgname);
                                pkg.icon = guard.find_icon_heuristic(&p.pkgname);
                            }
                            pkg
                        })
                        .collect()
                }
                Err(e) => {
                    log::warn!("Chaotic-AUR search failed: {}", e);
                    Vec::new()
                }
            }
        },
        async {
            if include_flatpak {
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

    let mut flatpak = Vec::new();
    if !flatpak_raw.is_empty() {
        let app_ids: Vec<String> = flatpak_raw.iter().map(|h| h.app_id.clone()).collect();
        let versions = flathub
            .get_remote_versions_batch(&app_ids)
            .await
            .unwrap_or_default();
        for hit in flatpak_raw {
            let v = versions.get(&hit.app_id).cloned();
            flatpak.push((hit, v));
        }
    }

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

    let mut results = aggregation::merge_search_results(
        official,
        aur,
        flatpak,
        &state_registry.manager,
        &installed_flatpaks,
    );
    results = utils::deduplicate_by_canonical_key(results);
    aggregation::enrich_packages_metadata(&mut results, state_flathub.inner()).await;
    results = aggregation::deduplicate_and_merge_packages(results);
    if let Ok(loader) = state_metadata.loader.lock() {
        aggregation::enrich_with_local_metadata(&mut results, &loader);
    }

    let popular_names: Vec<String> = state_discovery.inner().popular_aur_names().await;
    let metadata_loader = state_metadata
        .loader
        .lock()
        .expect("MetadataState lock poisoned");

    // CPU-tier tiebreaker: derive opt_level from source.id for CachyOS-optimized ranking.
    // This helper mirrors the logic in RepoManager::get_all_packages_with_repos.
    let derive_opt_level = |source_id: &str| -> u8 {
        let id = source_id.to_lowercase();
        if id.contains("-znver4") {
            3
        } else if id.contains("-v4") {
            2
        } else if id.contains("-v3") || id.contains("-core-v3") || id.contains("-extra-v3") {
            1
        } else {
            0
        }
    };

    results.sort_by(|a, b| {
        let score_a = calculate_relevance(a, &query_lower, &metadata_loader, &popular_names);
        let score_b = calculate_relevance(b, &query_lower, &metadata_loader, &popular_names);
        score_b
            .cmp(&score_a)
            .then_with(|| {
                // CPU-tier tiebreaker: lower rank = higher priority
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

    utils::prepare_package_descriptions_for_ui(&mut results);
    results = utils::deduplicate_by_canonical_id_final(results);

    Ok(results)
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
    let include_flatpak = installed_lookup || flatpak_enabled.unwrap_or(true);
    let include_aur =
        installed_lookup || aur_enabled.unwrap_or(state_repo.inner().is_aur_enabled().await);
    let include_chaotic = installed_lookup
        || chaotic_enabled.unwrap_or(state_repo.inner().is_repo_enabled("chaotic-aur").await);

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

    let search_names_clone = search_names.clone();
    let repo_pkgs = tokio::task::spawn_blocking(move || {
        crate::alpm_read::get_packages_batch(&search_names_clone, &[])
    })
    .await
    .map_err(|e| e.to_string())?;
    combined_packages.extend(repo_pkgs);

    if crate::distro_context::DistroContext::new().is_chaotic_compatible() && include_chaotic {
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
                    installed: crate::alpm_read::is_package_installed(&p.pkgname),
                    ..Default::default()
                })
                .collect();
            combined_packages.extend(matches);
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
                        });
                        seen.insert(beta_key);
                    }
                }
            }
        }
    }
    Ok(final_variants)
}

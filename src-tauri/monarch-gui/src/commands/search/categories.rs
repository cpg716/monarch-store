use crate::{
    chaotic_api, discovery_manager, metadata, middleware::aggregation, models,
    repo_manager::RepoManager, utils,
};
use serde::Serialize;
use specta::Type;
use tauri::State;

use super::cache::{try_read_category_disk, write_category_disk, CATEGORY_CACHE};
use super::core::CategoryQuery;
use crate::flathub_api::FlathubApiClient;

const CATEGORY_CACHE_VERSION: &str = "v8";

fn build_registry_category_featured_fallback(
    state_registry: &crate::registry::RegistryState,
    category: &str,
) -> Vec<models::Package> {
    let names = discovery_manager::get_featured_names_for_category(category);
    let mut lookup_ids = Vec::new();
    let mut ordered_candidates: Vec<Vec<String>> = Vec::new();

    for raw_name in names {
        let mut candidates = Vec::new();
        let canonical = utils::canonical_merge_key(&raw_name, None);
        if !canonical.is_empty() {
            candidates.push(canonical);
        }
        let raw_lower = raw_name.to_lowercase();
        if !candidates.iter().any(|c| c == &raw_lower) {
            candidates.push(raw_lower);
        }
        for alias in utils::canonical_to_repo_lookup_names(&raw_name) {
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

    let mut packages = Vec::new();
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
                    packages.push(pkg.clone());
                    break;
                }
            }
        }
        if packages.len() >= 24 {
            break;
        }
    }

    crate::utils::finalize_packages_contract(&mut packages);
    packages
}

fn build_local_category_fallback(
    state_meta: &metadata::MetadataState,
    category: &str,
) -> Vec<models::Package> {
    if let Ok(loader) = state_meta.loader.lock() {
        let mut packages: Vec<models::Package> = loader
            .get_apps_by_category(category)
            .into_iter()
            .take(40)
            .map(|app| models::Package {
                name: app.pkg_name.clone().unwrap_or_else(|| app.app_id.clone()),
                display_name: Some(app.name.clone()),
                display_title: Some(app.name),
                description: app.summary.unwrap_or_default(),
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
                installed: false,
                ..Default::default()
            })
            .collect();
        crate::utils::finalize_packages_contract(&mut packages);
        return packages;
    }
    Vec::new()
}

fn build_static_category_featured_fallback(category: &str) -> Vec<models::Package> {
    let mut packages: Vec<models::Package> = discovery_manager::get_featured_names_for_category(category)
        .into_iter()
        .take(24)
        .map(|name| models::Package {
            name: name.clone(),
            display_name: Some(utils::to_pretty_name(&name)),
            display_title: Some(utils::to_pretty_name(&name)),
            description: format!("Curated {category} application"),
            version: "latest".to_string(),
            source: models::PackageSource::official(&name),
            installed: utils::is_package_or_alias_installed(&name),
            ..Default::default()
        })
        .collect();
    utils::prepare_package_descriptions_for_ui(&mut packages);
    crate::utils::finalize_packages_contract(&mut packages);
    packages
}

fn default_category_cache_key(category: &str) -> String {
    format!(
        "cat:{}:{}:{}:f{}:a{}:c{}:i{}:{}",
        category,
        "",
        "featured",
        true,
        true,
        true,
        false,
        CATEGORY_CACHE_VERSION
    )
}

pub(crate) async fn prewarm_core_category_snapshots(
    state_meta: &metadata::MetadataState,
    state_registry: &crate::registry::RegistryState,
) {
    for category in [
        "Game",
        "System",
        "Graphics",
        "Network",
        "Office",
        "AudioVideo",
        "Development",
        "Utilities",
    ] {
        let cache_key = default_category_cache_key(category);
        if CATEGORY_CACHE.get(&cache_key).await.is_some() {
            continue;
        }

        let mut packages = build_local_category_fallback(state_meta, category);
        if packages.is_empty() {
            packages = build_registry_category_featured_fallback(state_registry, category);
        }
        if packages.is_empty() {
            packages = build_static_category_featured_fallback(category);
        }

        if packages.is_empty() {
            continue;
        }

        CATEGORY_CACHE.insert(cache_key.clone(), packages.clone()).await;
        write_category_disk(&cache_key, &packages);
        log::info!(
            "[CATEGORY] prewarmed snapshot category='{}' packages={}",
            category,
            packages.len()
        );
    }
}

#[derive(Serialize, Clone, Debug, Type)]
pub struct PaginatedResponse {
    pub packages: Vec<models::Package>,
    pub total: u32,
    pub page: u32,
    pub has_more: bool,
}

#[derive(Clone)]
struct CategorySeed {
    canonical_id: String,
    name: String,
    app_id: Option<String>,
}

#[specta::specta]
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn get_category_packages_paginated(
    state_meta: State<'_, metadata::MetadataState>,
    state_chaotic: State<'_, chaotic_api::ChaoticApiClient>,
    state_repo: State<'_, RepoManager>,
    state_flathub: State<'_, FlathubApiClient>,
    state_registry: State<'_, crate::registry::RegistryState>,
    query: CategoryQuery,
) -> Result<PaginatedResponse, String> {
    let options = query.options.as_ref();
    let flatpak_enabled = options.and_then(|o| o.flatpak_enabled);
    let aur_enabled = options.and_then(|o| o.aur_enabled);
    let chaotic_enabled = options.and_then(|o| o.chaotic_enabled);
    let for_installed_lookup = options.and_then(|o| o.for_installed_lookup) == Some(true);

    let backend_flatpak_enabled = state_repo.inner().is_flatpak_enabled().await;
    let backend_aur_enabled = state_repo.inner().is_aur_enabled().await;
    let backend_chaotic_enabled = state_repo.inner().is_repo_enabled("chaotic-aur").await;
    let include_flatpak = for_installed_lookup
        || (backend_flatpak_enabled && flatpak_enabled.unwrap_or(true));
    let include_aur = for_installed_lookup || (backend_aur_enabled && aur_enabled.unwrap_or(true));
    let include_chaotic =
        for_installed_lookup || (backend_chaotic_enabled && chaotic_enabled.unwrap_or(true));

    let cache_key = format!(
        "cat:{}:{}:{}:f{}:a{}:c{}:i{}:{}",
        query.category,
        query
            .repo_filter
            .as_ref()
            .map(|v| v.join(","))
            .unwrap_or_default(),
        query.sort_by.as_deref().unwrap_or("default"),
        include_flatpak,
        include_aur,
        include_chaotic,
        for_installed_lookup,
        CATEGORY_CACHE_VERSION
    );

    state_meta.inner().wait_until_ready().await;
    let packages = if let Some(mut cached) = CATEGORY_CACHE.get(&cache_key).await.filter(|c| !c.is_empty()) {
        crate::utils::finalize_packages_contract(&mut cached);
        log::info!(
            "[CATEGORY] cache-hit category='{}' packages={}",
            query.category,
            cached.len()
        );
        cached
    } else if let Some(disk_cached) = try_read_category_disk(&cache_key) {
        // Moka miss — warm start from disk
        let mut disk_cached = disk_cached;
        crate::utils::finalize_packages_contract(&mut disk_cached);
        CATEGORY_CACHE
            .insert(cache_key.clone(), disk_cached.clone())
            .await;
        log::info!(
            "[CATEGORY] disk-cache category='{}' packages={}",
            query.category,
            disk_cached.len()
        );
        disk_cached
    } else {
        // ONE CARD PER APP: Collect unique (name, app_id) seeds by canonical key, then fetch unified packages once.
        let mut seeds: std::collections::HashMap<String, CategorySeed> =
            std::collections::HashMap::new();

        // 1. Registry (AppStream/SQLite) — one seed per canonical key; prefer friendly name.
        if let Ok(pkgs) = state_registry
            .manager
            .get_packages_by_category(&query.category, 100, 0)
        {
            log::info!(
                "[CATEGORY] Found {} packages in Registry for '{}'",
                pkgs.len(),
                query.category
            );
            for pkg in pkgs {
                let name = pkg.name.clone();
                let app_id = pkg.app_id.clone();
                let key = pkg.canonical_id.clone();
                seeds.insert(
                    key.clone(),
                    CategorySeed {
                        canonical_id: key,
                        name,
                        app_id,
                    },
                );
            }
        } else {
            log::warn!("[CATEGORY] Registry lookup failed for '{}'", query.category);
        }

        // 2. Chaotic — add seeds not already present; same canonical key = one app.
        let chaotic_ok = crate::distro_context::DistroContext::new().is_chaotic_compatible()
            || state_repo.inner().is_advanced_mode().await;
        let c_matches = if include_chaotic && chaotic_ok {
            state_chaotic
                .inner()
                .get_packages_by_category(&query.category)
                .await
        } else {
            Vec::new()
        };
        for p in c_matches {
            let app_id = if let Ok(loader) = state_meta.loader.lock() {
                loader.find_app_id(&p.pkgname)
            } else {
                None
            };
            let key = utils::canonical_merge_key(&p.pkgname, app_id.as_deref());
            if !seeds.contains_key(&key) {
                seeds.insert(
                    key.clone(),
                    CategorySeed {
                        canonical_id: key,
                        name: p.pkgname.clone(),
                        app_id,
                    },
                );
            }
        }

        // If Registry returned nothing for this category, seed from local AppStream category index
        // before featured/chaotic expansion. This gives a deterministic baseline even on sparse hosts.
        if seeds.is_empty() {
            if let Ok(loader) = state_meta.loader.lock() {
                for meta in loader.get_apps_by_category(&query.category).into_iter().take(80) {
                    let pkg_name = meta
                        .pkg_name
                        .clone()
                        .unwrap_or_else(|| meta.name.clone());
                    let key = utils::canonical_merge_key(&pkg_name, Some(&meta.app_id));
                    seeds.entry(key.clone()).or_insert(CategorySeed {
                        canonical_id: key,
                        name: pkg_name,
                        app_id: Some(meta.app_id.clone()),
                    });
                }
            }
        }

        // Always seed featured names first so they are guaranteed to be in the first batch
        let featured_names = discovery_manager::get_featured_names_for_category(&query.category);
        let mut final_seeds: Vec<CategorySeed> = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();

        const CATEGORY_SEEDS_CAP: usize = 56; // Lower cap to reduce network fan-out and timeout pressure

        // 1. Prioritize Featured Seeds
        for name in &featured_names {
            let app_id = if let Ok(loader) = state_meta.loader.lock() {
                loader.find_app_id(name)
            } else {
                None
            };
            let key = utils::canonical_merge_key(name, app_id.as_deref());
            if !seen_keys.contains(&key) {
                final_seeds.push(CategorySeed {
                    canonical_id: key.clone(),
                    name: name.clone(),
                    app_id,
                });
                seen_keys.insert(key);
            }
        }

        // 2. Fill remainder with Alpha-sorted category apps (Deterministic)
        let mut other_seeds: Vec<CategorySeed> = seeds
            .into_iter()
            .filter(|(k, _)| !seen_keys.contains(k))
            .map(|(_, val)| val)
            .collect();
        other_seeds.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        let needed = CATEGORY_SEEDS_CAP.saturating_sub(final_seeds.len());
        final_seeds.extend(other_seeds.into_iter().take(needed));

        let unique_items = final_seeds;
        log::info!(
            "[CATEGORY] seed-build category='{}' seeds={} featured={}",
            query.category,
            unique_items.len(),
            featured_names.len()
        );

        // 3. REGISTRY EXPANSION:
        let seed_ids: Vec<String> = unique_items
            .iter()
            .map(|seed| seed.canonical_id.clone())
            .collect();
        let registry_map = state_registry
            .get_repo_names_for_canonical_ids(&seed_ids)
            .unwrap_or_default();

        let mut expanded_items: Vec<(String, Option<String>)> = unique_items
            .iter()
            .map(|seed| (seed.name.clone(), seed.app_id.clone()))
            .collect();
        for seed in &unique_items {
            if let Some(repo_names) = registry_map.get(&seed.canonical_id) {
                for rn in repo_names {
                    if !expanded_items.iter().any(|(existing, _)| existing == rn) {
                        expanded_items.push((rn.clone(), None));
                    }
                }
            }
        }

        let local_fallback = build_local_category_fallback(state_meta.inner(), &query.category);

        // 4. Fetch unified packages (one card per app, full available_sources: Official, CachyOS, Flatpak, AUR).
        let mut packages = tokio::time::timeout(
            std::time::Duration::from_secs(6),
            aggregation::fetch_and_merge_packages_by_names_impl(
            &state_meta,
            &state_chaotic,
            &state_repo,
            &state_flathub,
            &state_registry.manager,
            expanded_items,
            include_flatpak,
            include_aur,
            include_chaotic,
            false,
        ),
        )
        .await
        .ok()
        .and_then(|result| result.ok())
        .unwrap_or_default();
        if packages.is_empty() && !local_fallback.is_empty() {
            packages = local_fallback;
        }
        if packages.is_empty() {
            let registry_fallback =
                build_registry_category_featured_fallback(state_registry.inner(), &query.category);
            if !registry_fallback.is_empty() {
                log::warn!(
                    "[CATEGORY] using registry featured fallback for category='{}' packages={}",
                    query.category,
                    registry_fallback.len()
                );
                packages = registry_fallback;
            }
        }
        if packages.is_empty() {
            let static_fallback = build_static_category_featured_fallback(&query.category);
            if !static_fallback.is_empty() {
                log::warn!(
                    "[CATEGORY] using static featured fallback for category='{}' packages={}",
                    query.category,
                    static_fallback.len()
                );
                packages = static_fallback;
            }
        }
        log::info!(
            "[CATEGORY] hydrate category='{}' packages={} (post-expand)",
            query.category,
            packages.len()
        );

        // 4. Repo filter: show only packages that have at least one source in the selected filter.
        if let Some(repos) = &query.repo_filter {
            let has_all = repos.iter().any(|r| r.to_lowercase() == "all");
            if !has_all && !repos.is_empty() {
                let initial_count = packages.len();
                let allowed: std::collections::HashSet<String> =
                    repos.iter().map(|s| s.to_lowercase()).collect();

                packages.retain(|p| {
                    let sources = p.available_sources.as_deref().unwrap_or(&[]);
                    let p_has_allowed = sources.iter().any(|s| {
                        let id = s.id.to_lowercase();
                        let st = s.source_type.to_lowercase();
                        let src = match st.as_str() {
                            "repo" => {
                                if id.contains("chaotic") {
                                    "chaotic-aur"
                                } else if id.contains("cachyos") {
                                    "cachyos"
                                } else {
                                    "official"
                                }
                            }
                            "flatpak" => "flatpak",
                            "aur" => "aur",
                            _ => "other",
                        };
                        allowed.contains(src)
                            || (src == "chaotic-aur"
                                && (allowed.contains("chaotic") || allowed.contains("chaotic-aur")))
                            || (src == "official" && allowed.contains("repo"))
                    });
                    p_has_allowed
                });

                log::info!(
                    "[CATEGORY] Filter applied: {:?}. Reduced result from {} to {} apps.",
                    allowed,
                    initial_count,
                    packages.len()
                );
            }
        }

        // 5. Stable Featured-First Sort OR Strict Sort
        let is_strict_sort = query.sort_by.as_deref().unwrap_or("") == "newest"
            || query.sort_by.as_deref().unwrap_or("") == "updated";

        if is_strict_sort {
            packages.sort_by(|a, b| {
                b.last_modified
                    .unwrap_or(0)
                    .cmp(&a.last_modified.unwrap_or(0))
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
        } else if !featured_names.is_empty() {
            let featured_map: std::collections::HashMap<String, usize> = featured_names
                .iter()
                .enumerate()
                .map(|(i, name)| (name.to_lowercase(), i))
                .collect();

            for pkg in packages.iter_mut() {
                if featured_map.contains_key(&pkg.name.to_lowercase()) {
                    pkg.is_featured = Some(true);
                }
            }

            packages.sort_by(|a, b| {
                let a_key = a.name.to_lowercase();
                let b_key = b.name.to_lowercase();

                let a_rank = featured_map.get(&a_key).unwrap_or(&9999);
                let b_rank = featured_map.get(&b_key).unwrap_or(&9999);

                a_rank
                    .cmp(b_rank)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
        } else {
            packages.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }

        utils::prepare_package_descriptions_for_ui(&mut packages);
        crate::utils::finalize_packages_contract(&mut packages);
        if !packages.is_empty() {
            CATEGORY_CACHE
                .insert(cache_key.clone(), packages.clone())
                .await;
            write_category_disk(&cache_key, &packages);
        }
        packages
    };

    let total = packages.len();
    let page_idx = (if query.page > 0 { query.page - 1 } else { 0 }) as usize;
    let limit = query.limit as usize;
    let start: usize = page_idx * limit;
    let end: usize = (start + limit).min(total);
    let has_more = end < total;

    let page_items = if start < total {
        packages[start..end].to_vec()
    } else {
        Vec::new()
    };

    log::info!(
        "[CATEGORY] final category='{}' total={} page={} returned={} has_more={}",
        query.category,
        total,
        query.page,
        page_items.len(),
        has_more
    );

    Ok(PaginatedResponse {
        packages: page_items,
        total: total as u32,
        page: query.page,
        has_more,
    })
}

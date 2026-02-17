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

#[derive(Serialize, Clone, Debug, Type)]
pub struct PaginatedResponse {
    pub packages: Vec<models::Package>,
    pub total: u32,
    pub page: u32,
    pub has_more: bool,
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
    // Cache version: bump to invalidate when pipeline changes (one-card-per-app, variants). Restart app to see fixes.
    const CATEGORY_CACHE_VERSION: &str = "v7"; // Bumping to clear any potentially stale v6 data
    let cache_key = format!(
        "cat:{}:{}:{}:{:?}:{}",
        query.category,
        query
            .repo_filter
            .as_ref()
            .map(|v| v.join(","))
            .unwrap_or_default(),
        query.sort_by.as_deref().unwrap_or("default"),
        query
            .options
            .as_ref()
            .and_then(|o| o.flatpak_enabled)
            .unwrap_or(true),
        CATEGORY_CACHE_VERSION
    );

    state_meta.inner().wait_until_ready().await;
    let packages = if let Some(cached) = CATEGORY_CACHE.get(&cache_key).await {
        cached
    } else if let Some(disk_cached) = try_read_category_disk(&cache_key) {
        // Moka miss — warm start from disk
        CATEGORY_CACHE
            .insert(cache_key.clone(), disk_cached.clone())
            .await;
        disk_cached
    } else {
        // ONE CARD PER APP: Collect unique (name, app_id) seeds by canonical key, then fetch unified packages once.
        let mut seeds: std::collections::HashMap<String, (String, Option<String>)> =
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
                seeds.insert(key, (name, app_id));
            }
        } else {
            log::warn!("[CATEGORY] Registry lookup failed for '{}'", query.category);
        }

        // 2. Chaotic — add seeds not already present; same canonical key = one app.
        let chaotic_ok = crate::distro_context::DistroContext::new().is_chaotic_compatible();
        let c_matches = if chaotic_ok && state_repo.inner().is_repo_enabled("chaotic-aur").await {
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
                seeds.insert(key, (p.pkgname.clone(), app_id));
            }
        }

        // Always seed featured names first so they are guaranteed to be in the first batch
        let featured_names = discovery_manager::get_featured_names_for_category(&query.category);
        let mut final_seeds: Vec<(String, Option<String>)> = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();

        const CATEGORY_SEEDS_CAP: usize = 72; // Increased to ensure enough room for featured + expansion

        // 1. Prioritize Featured Seeds
        for name in &featured_names {
            let app_id = if let Ok(loader) = state_meta.loader.lock() {
                loader.find_app_id(name)
            } else {
                None
            };
            let key = utils::canonical_merge_key(name, app_id.as_deref());
            if !seen_keys.contains(&key) {
                final_seeds.push((name.clone(), app_id));
                seen_keys.insert(key);
            }
        }

        // 2. Fill remainder with Alpha-sorted category apps (Deterministic)
        let mut other_seeds: Vec<(String, Option<String>)> = seeds
            .into_iter()
            .filter(|(k, _)| !seen_keys.contains(k))
            .map(|(_, val)| val)
            .collect();
        other_seeds.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        let needed = CATEGORY_SEEDS_CAP.saturating_sub(final_seeds.len());
        final_seeds.extend(other_seeds.into_iter().take(needed));

        let unique_items = final_seeds;

        // 3. REGISTRY EXPANSION:
        let seed_names: Vec<String> = unique_items.iter().map(|(n, _)| n.clone()).collect();
        let registry_map = state_registry
            .get_repo_names_for_canonical_ids(&seed_names)
            .unwrap_or_default();

        let mut expanded_items = unique_items.clone();
        for (name, _) in &unique_items {
            if let Some(repo_names) = registry_map.get(name) {
                for rn in repo_names {
                    if !expanded_items.iter().any(|(existing, _)| existing == rn) {
                        expanded_items.push((rn.clone(), None));
                    }
                }
            }
        }

        // 4. Fetch unified packages (one card per app, full available_sources: Official, CachyOS, Flatpak, AUR).
        let options = query.options.as_ref();
        let flatpak_enabled = options.and_then(|o| o.flatpak_enabled);
        let aur_enabled = options.and_then(|o| o.aur_enabled);
        let chaotic_enabled = options.and_then(|o| o.chaotic_enabled);

        let include_flatpak = flatpak_enabled.unwrap_or(true);
        let include_aur = aur_enabled.unwrap_or(state_repo.inner().is_aur_enabled().await);
        let include_chaotic =
            chaotic_enabled.unwrap_or(state_repo.inner().is_repo_enabled("chaotic-aur").await);

        let mut packages = aggregation::fetch_and_merge_packages_by_names_impl(
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
        )
        .await
        .unwrap_or_default();

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
        CATEGORY_CACHE
            .insert(cache_key.clone(), packages.clone())
            .await;
        write_category_disk(&cache_key, &packages);
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

    Ok(PaginatedResponse {
        packages: page_items,
        total: total as u32,
        page: query.page,
        has_more,
    })
}

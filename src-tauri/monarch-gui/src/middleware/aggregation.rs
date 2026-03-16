use crate::{
    aur_api, chaotic_api,
    flathub_api::{FlathubApiClient, SearchResult},
    metadata, models,
    models::{Package, PackageSource},
    repo_manager::RepoManager,
    utils,
};
use futures::StreamExt;
use std::collections::HashMap;

/// Modifies packages in-place to upgrade them to "Unified" identity.
/// Capped and parallelized so Essentials/Trending/Categories don't stall on many Flathub calls.
const ENRICH_CAP: usize = 80;
const ENRICH_CHUNK: usize = 8;

#[inline]
fn same_source_slot(a: &PackageSource, b: &PackageSource) -> bool {
    a.id == b.id && a.source_type == b.source_type && a.package_name == b.package_name
}

fn normalize_sources_for_package(pkg: &mut Package) {
    let mut sources = pkg
        .available_sources
        .clone()
        .unwrap_or_else(|| vec![pkg.source.clone()]);

    // Dedup by source slot (id + type + package_name), keep newest version per slot.
    let mut deduped: Vec<PackageSource> = Vec::new();
    for src in sources.drain(..) {
        if let Some(existing) = deduped.iter_mut().find(|s| same_source_slot(s, &src)) {
            if src.version > existing.version {
                *existing = src;
            }
        } else {
            deduped.push(src);
        }
    }

    if !deduped.is_empty() {
        deduped.sort_by(|a, b| {
            source_score(b)
                .cmp(&source_score(a))
                .then_with(|| a.id.cmp(&b.id))
                .then_with(|| a.package_name.cmp(&b.package_name))
        });
        let current_source_present = deduped.iter().any(|src| same_source_slot(src, &pkg.source));
        if !current_source_present {
            if pkg.installed {
                if let Some(installed_source) =
                    utils::installed_source_for_package(&pkg.name, pkg.app_id.as_deref())
                {
                    pkg.source = installed_source;
                } else {
                    pkg.source = best_primary_source(&deduped);
                }
            } else {
                pkg.source = best_primary_source(&deduped);
            }
        }
        pkg.available_sources = Some(deduped);
    }
}

fn merge_available_sources_into(target: &mut Package, extra_sources: &[PackageSource]) {
    let mut merged = target
        .available_sources
        .clone()
        .unwrap_or_else(|| vec![target.source.clone()]);

    for src in extra_sources {
        if let Some(existing) = merged.iter_mut().find(|s| same_source_slot(s, src)) {
            if src.version > existing.version {
                *existing = src.clone();
            }
        } else {
            merged.push(src.clone());
        }
    }

    target.available_sources = Some(merged);
}

fn merge_registry_variants(target: &mut Package, cached: &Package) {
    let mut extra_sources = cached
        .available_sources
        .clone()
        .unwrap_or_else(|| vec![cached.source.clone()]);
    if extra_sources.is_empty() {
        extra_sources.push(cached.source.clone());
    }
    merge_available_sources_into(target, &extra_sources);
}

// Shared registry backfill: upgrades a Package's icon, display_name, app_id, and description
// from a cached Registry entry. Centralises the logic previously duplicated in 5 + places.
// NEW BEHAVIOR: Trust the registry outright instead of performing casing/length heuristics,
// because the registry should already reflect the highest tier source data via the BFF hierarchy.
pub fn apply_registry_backfill(pkg: &mut Package, reg: &Package) {
    merge_registry_variants(pkg, reg);

    // 1. Icon: Prefer rich (HTTP/Data) over local/none
    let reg_is_rich = reg
        .icon
        .as_deref()
        .map(|i| i.starts_with("http") || i.starts_with("data:"))
        .unwrap_or(false);
    let current_is_local = pkg
        .icon
        .as_deref()
        .map(|s| !s.starts_with("http") && !s.starts_with("data:"))
        .unwrap_or(true);

    if (reg_is_rich && (pkg.icon.is_none() || current_is_local))
        || (pkg.icon.is_none() && reg.icon.is_some())
    {
        pkg.icon = reg.icon.clone();
    }

    // 2. Display Name: Trust registry if current is missing or just the raw name
    let current_dn = pkg.display_name.as_deref().unwrap_or("");
    if pkg.display_name.is_none() || current_dn == pkg.name {
        if let Some(reg_dn) = &reg.display_name {
            if !reg_dn.is_empty() {
                pkg.display_name = Some(reg_dn.clone());
            }
        }
    }

    // 3. App ID: Trust registry if current is missing
    if pkg.app_id.is_none() {
        if let Some(id) = &reg.app_id {
            pkg.app_id = Some(id.clone());
        }
    }

    // 4. Description: Fallback to registry if current is empty or significantly shorter.
    // If the registry entry contains a Flatpak source, we treat its description as authoritative "Rich" metadata.
    let reg_has_flatpak = reg.source.source_type == "flatpak"
        || reg
            .available_sources
            .as_ref()
            .map(|s| s.iter().any(|src| src.source_type == "flatpak"))
            .unwrap_or(false);

    if (pkg.description.is_empty()
        || pkg.description == pkg.name
        || reg_has_flatpak
        || (reg.description.len() > pkg.description.len() + 20))
        && !reg.description.is_empty() {
            pkg.description = reg.description.clone();
        }
}

/// Shared pipeline step: Enrich packages using local AppStream metadata (AppInfo, Icons).
/// Used by both search and aggregation to ensure "Iron Core" SSOT.
pub fn enrich_with_local_metadata(packages: &mut [Package], loader: &metadata::AppStreamLoader) {
    for pkg in packages {
        // 1. App ID Resolution
        // 1. App ID Resolution
        // Force resolution if Missing OR if it looks like a "weak" ID (no dots, just a name)
        let is_weak_id = pkg
            .app_id
            .as_ref()
            .map(|id| !id.contains('.'))
            .unwrap_or(true);

        if is_weak_id {
            if let Some(found_id) = loader.find_app_id(&pkg.name) {
                pkg.app_id = Some(found_id);
            } else {
                // Fallback: strip suffixes (e.g. -bin)
                let stripped = utils::strip_package_suffix(&pkg.name);
                if stripped != pkg.name {
                    if let Some(found_id) = loader.find_app_id(stripped) {
                        pkg.app_id = Some(found_id);
                    }
                }
            }
        }

        // 2. Icon Enrichment
        // Prefer remote icons (Flatpak/URL) > AppStream local path > repo default
        let has_remote_icon = pkg
            .icon
            .as_ref()
            .map(|s| s.starts_with("http"))
            .unwrap_or(false);

        if pkg.icon.is_none() || (!has_remote_icon && pkg.source.source_type == "repo") {
            if let Some(rich_icon) = loader.find_icon_heuristic(&pkg.name) {
                pkg.icon = Some(rich_icon);
            } else {
                let stripped = utils::strip_package_suffix(&pkg.name);
                if stripped != pkg.name {
                    if let Some(rich_icon) = loader.find_icon_heuristic(stripped) {
                        pkg.icon = Some(rich_icon);
                    }
                }
            }
        }

        // 3. Metadata Upgrade (Display Name / Description)
        if let Some(meta) = loader.find_package(&pkg.name) {
            // IRON CORE PROTECTION: Only upgrade name if we lack a strong App ID.
            // If we have a strong ID (from Registry/Flathub), we already trust our current name.
            let has_strong_id = pkg
                .app_id
                .as_ref()
                .map(|id| id.contains('.'))
                .unwrap_or(false);

            if !has_strong_id {
                pkg.display_name = Some(meta.name.clone());
            }

            // Upgrade description unconditionally if current is empty or matches name
            if pkg.description.is_empty() || pkg.description == pkg.name {
                if let Some(desc) = meta.summary {
                    pkg.description = desc;
                }
            }
        }

        utils::apply_package_ui_defaults(pkg);
    }
}

pub fn merge_search_results(
    official: Vec<Package>,
    aur: Vec<Package>,
    flatpak_hits: Vec<(SearchResult, Option<String>)>,
    state_registry: &crate::registry::RegistryManager,
    installed_flatpaks: &std::collections::HashSet<String>,
) -> Vec<Package> {
    let cap = official
        .len()
        .saturating_add(flatpak_hits.len())
        .saturating_add(aur.len())
        .max(64);
    let mut package_map: HashMap<String, Package> = HashMap::with_capacity(cap);

    // A. Official (Repo) — key = canonical_merge_key only
    for mut p in official {
        let key = utils::canonical_merge_key(&p.name, p.app_id.as_deref());
        p.available_sources = Some(vec![p.source.clone()]);
        p.canonical_id = key.clone();
        if let Some(existing) = package_map.get_mut(&key) {
            if let Some(sources) = &mut existing.available_sources {
                if let Some(existing_src) =
                    sources.iter_mut().find(|s| same_source_slot(s, &p.source))
                {
                    if p.source.version > existing_src.version {
                        *existing_src = p.source.clone();
                    }
                } else {
                    sources.push(p.source.clone());
                }
            }
        } else {
            // BACKFILL FROM REGISTRY: If this is the first time we see this app, try to pull rich metadata
            // BACKFILL FROM REGISTRY: Effective Golden Data Protection
            if let Ok(Some(cached)) = state_registry.get_package(&key) {
                // Log the merge attempt
                log::debug!(
                    "[AGGREGATION-REPO] Merging {} (Incoming) with Cached: {}",
                    p.name,
                    cached.name
                );
                log::debug!(
                    "[AGGREGATION-REPO] Incoming - Name: {:?}, Display: {:?}, Icon: {:?}",
                    p.name,
                    p.display_name,
                    p.icon
                );
                log::debug!(
                    "[AGGREGATION-REPO] Cached   - Name: {:?}, Display: {:?}, Icon: {:?}",
                    cached.name,
                    cached.display_name,
                    cached.icon
                );

                merge_registry_variants(&mut p, &cached);

                // 1. Calculate Priority (using references)
                // IRON CORE: Prefer cached if it has a strict Remote ID or better Visuals
                let cached_has_rdn = cached
                    .app_id
                    .as_ref()
                    .map(|id| id.contains('.'))
                    .unwrap_or(false);
                let current_has_rdn = p
                    .app_id
                    .as_ref()
                    .map(|id| id.contains('.'))
                    .unwrap_or(false);

                let cached_has_visuals = cached
                    .icon
                    .as_ref()
                    .map(|i| i.starts_with("http"))
                    .unwrap_or(false)
                    || !cached
                        .screenshots
                        .as_ref()
                        .map(|s| s.is_empty())
                        .unwrap_or(true);
                let current_has_visuals = p
                    .icon
                    .as_ref()
                    .map(|i| i.starts_with("http"))
                    .unwrap_or(false)
                    || !p.screenshots.as_ref().map(|s| s.is_empty()).unwrap_or(true);

                // IRON CORE: Prefer cached if it's strictly richer (ID, or visuals)
                // Name casing is no longer a heuristic since we trust the hierarchy.
                let prefer_cached = (cached_has_rdn && !current_has_rdn)
                    || (cached_has_visuals && !current_has_visuals);

                // 2. Icon Upgrade (Special case: prefer rich icon even if name isn't better)
                let cached_is_rich_icon = cached
                    .icon
                    .as_ref()
                    .map(|s| s.starts_with("http") || s.starts_with("data:"))
                    .unwrap_or(false);
                let current_is_local_icon = p
                    .icon
                    .as_ref()
                    .map(|s| !s.starts_with("http") && !s.starts_with("data:"))
                    .unwrap_or(true);

                if (cached_is_rich_icon && (p.icon.is_none() || current_is_local_icon))
                    || (p.icon.is_none() && cached.icon.is_some())
                {
                    p.icon = cached.icon.clone();
                }

                // 3. Final Merge with ownership transfer
                if prefer_cached {
                    log::debug!("[AGGREGATION-REPO] Overwriting Metadata with Cached (Rich)");
                    p.display_name = cached.display_name;
                    p.description = cached.description;
                    p.app_id = cached.app_id;
                    if !cached
                        .screenshots
                        .as_ref()
                        .map(|s| s.is_empty())
                        .unwrap_or(true)
                    {
                        p.screenshots = cached.screenshots;
                    }
                } else {
                    if p.display_name.is_none() {
                        p.display_name = cached.display_name;
                    }
                    if p.description.is_empty() {
                        p.description = cached.description;
                    }
                    if p.app_id.is_none() {
                        p.app_id = cached.app_id;
                    }
                    if (p.screenshots.is_none()
                        || p.screenshots.as_ref().map(|s| s.is_empty()).unwrap_or(true))
                        && cached.screenshots.is_some()
                    {
                        p.screenshots = cached.screenshots;
                    }
                }
            }
            package_map.insert(key, p);
        }
    }

    // B. Flatpak — key by app_id so Discord (com.discordapp.Discord) always merges to same key as repo "discord"
    for (hit, version_opt) in flatpak_hits {
        let display_name = Some(hit.name.clone());
        let version = version_opt.unwrap_or_else(|| "latest".to_string());

        let flatpak_source = PackageSource::new_with_name(
            "flatpak",
            "flathub",
            &version,
            "Flatpak (Sandboxed)",
            &hit.app_id,
        );

        let key = utils::canonical_merge_key(&hit.app_id, Some(&hit.app_id));
        let is_installed = installed_flatpaks.contains(&hit.app_id);

        if let Some(existing) = package_map.get_mut(&key) {
            if let Some(sources) = &mut existing.available_sources {
                if let Some(existing_src) =
                    sources.iter_mut().find(|s| same_source_slot(s, &flatpak_source))
                {
                    if flatpak_source.version > existing_src.version {
                        *existing_src = flatpak_source.clone();
                    }
                } else {
                    sources.push(flatpak_source.clone());
                }
            }
            if existing.app_id.is_none() {
                existing.app_id = Some(hit.app_id.clone());
            }

            if is_installed {
                existing.installed = true;
            }

            // METADATA BACKFILL: If existing (Repo/AUR) has no icon, take Flathub's.
            if existing.icon.is_none() && hit.icon.is_some() {
                existing.icon = hit.icon.clone();
            }
            if existing.description.is_empty() {
                if let Some(summary) = &hit.summary {
                    existing.description = utils::truncate_description_for_ui(summary, 200);
                }
            }

            // Name Upgrade: If existing is "com.discordapp.Discord" (ID-like) and hit is "Discord", take hit.
            // OR if hit is also ID-like, try to pretty-print it (e.g. "Discord" from "com.discordapp.Discord")
            if !hit.name.contains('.') && existing.name.contains('.') {
                existing.display_name = Some(hit.name);
            } else if hit.name.contains('.')
                && existing.name.contains('.')
                && existing.display_name.is_none()
            {
                // Both are IDs? Try to extract friendly name from hit
                existing.display_name = Some(utils::to_pretty_name(&hit.name));
            } else if existing.display_name.is_none() && !hit.name.contains('.') {
                existing.display_name = Some(hit.name);
            }
        } else {
            // New Entry
            let mut display_name = display_name;
            if let Some(dn) = &display_name {
                if dn.contains('.') {
                    display_name = Some(utils::to_pretty_name(dn));
                }
            }
            let desc = hit.summary.as_deref().unwrap_or("");
            let mut p = Package {
                name: hit.app_id.clone(), // Use app_id as primary name for Flatpaks
                display_name: display_name.clone(),
                description: utils::truncate_description_for_ui(desc, 200),
                version: version.clone(),
                source: flatpak_source.clone(), // Default source if only Flatpak found
                icon: hit.icon.clone(),
                app_id: Some(hit.app_id.clone()),
                canonical_id: key.clone(),
                installed: is_installed,
                available_sources: Some(vec![flatpak_source.clone()]),
                ..Default::default()
            };

            // BACKFILL FROM REGISTRY: Even for Flatpaks, the Registry might have better AppStream descriptions
            if let Ok(Some(cached)) = state_registry.get_package(&key) {
                merge_registry_variants(&mut p, &cached);
                if p.description.is_empty() {
                    p.description = cached.description;
                }
                if p.icon.is_none() {
                    p.icon = cached.icon;
                }
            }
            package_map.insert(key, p);
        }
    }

    // C. AUR — key = canonical_merge_key only
    for mut p in aur {
        let key = utils::canonical_merge_key(&p.name, p.app_id.as_deref());
        if let Some(existing) = package_map.get_mut(&key) {
            if let Some(sources) = &mut existing.available_sources {
                if let Some(existing_src) =
                    sources.iter_mut().find(|s| same_source_slot(s, &p.source))
                {
                    if p.source.version > existing_src.version {
                        *existing_src = p.source.clone();
                    }
                } else {
                    sources.push(p.source.clone());
                }
            }
            // CRITICAL FIX: If AUR package is installed, the unified package is installed!
            if p.installed {
                existing.installed = true;
                // If existing was just a Flatpak placeholder, maybe we should adopt AUR package as the base?
                // But Flatpak metadata (icon) is better.
                // Let's keep existing (Flatpak) metadata but set installed=true.
                // AND ensure we have the description if missing.
                if existing.description.is_empty() {
                    existing.description = p.description.clone();
                }
            }

            // If existing has no friendly name (e.g. was just ID), and AUR has one, take it?
            // Actually, Flathub name is usually better than AUR "discord-bin".
            // So only overwrite if existing name looks like an ID.
            if !p.name.contains('.') && existing.name.contains('.') {
                // p.name is "discord", existing.name is "com.discordapp.Discord"
                // But we prefer Flathub Display Name if we have it...
                // Let's trusting existing.display_name from Flatpak loop above.
            }
        } else {
            p.available_sources = Some(vec![p.source.clone()]);
            p.canonical_id = key.clone();

            // BACKFILL FROM REGISTRY: Effective Golden Data Protection
            // BACKFILL FROM REGISTRY: Effective Golden Data Protection
            if let Ok(Some(cached)) = state_registry.get_package(&key) {
                merge_registry_variants(&mut p, &cached);
                // Log the merge attempt
                log::debug!(
                    "[AGGREGATION-AUR] Merging {} (Incoming) with Cached: {}",
                    p.name,
                    cached.name
                );
                log::debug!(
                    "[AGGREGATION-AUR] Incoming - Name: {:?}, Display: {:?}, Icon: {:?}",
                    p.name,
                    p.display_name,
                    p.icon
                );
                log::debug!(
                    "[AGGREGATION-AUR] Cached   - Name: {:?}, Display: {:?}, Icon: {:?}",
                    cached.name,
                    cached.display_name,
                    cached.icon
                );

                // ICON: Prefer HTTP/Data (cached) over Local/None (current)
                let cached_is_rich = cached
                    .icon
                    .as_deref()
                    .map(|s| s.starts_with("http") || s.starts_with("data:"))
                    .unwrap_or(false);
                let current_is_local = p
                    .icon
                    .as_deref()
                    .map(|s| !s.starts_with("http") && !s.starts_with("data:"))
                    .unwrap_or(true);

                if cached_is_rich && (p.icon.is_none() || current_is_local) {
                    log::debug!("[AGGREGATION-AUR] Overwriting Icon with Cached (Rich)");
                    p.icon = cached.icon;
                } else if p.icon.is_none() {
                    p.icon = cached.icon;
                }

                // NAME: Prefer Title Case (cached) over Lowercase (current)
                let cached_dn = cached.display_name.as_deref().unwrap_or("");
                let current_dn = p.display_name.as_deref().unwrap_or("");

                let cached_has_upper = cached_dn.chars().any(|c| c.is_uppercase());
                let current_has_upper = current_dn.chars().any(|c| c.is_uppercase());

                if cached_has_upper && !current_has_upper {
                    log::debug!(
                        "[AGGREGATION-AUR] Overwriting Display Name with Cached (Title Case): {}",
                        cached_dn
                    );
                    p.display_name = cached.display_name;
                } else if p.display_name.is_none() {
                    p.display_name = cached.display_name;
                }

                if p.description.is_empty() {
                    p.description = cached.description;
                }
                if p.app_id.is_none() {
                    p.app_id = cached.app_id;
                }
            } else {
                log::info!("[AGGREGATION-AUR] No cached entry found for key: {}", key);
            }
            package_map.insert(key, p);
        }
    }

    // Set primary source from available_sources (Official/Repo first, then Flatpak, then AUR)
    let len = package_map.len();
    let mut out = Vec::with_capacity(len);
    for mut pkg in package_map.into_values() {
        if let Some(ref sources) = pkg.available_sources {
            if !sources.is_empty() {
                pkg.source = best_primary_source(sources);
            }
        }
        out.push(pkg);
    }
    out
}

/// Canonical output builder used by Search/Trending/Essentials/Categories/Details seeds.
/// One identity engine, one dedup pipeline, one source-priority policy.
pub fn build_package_view_models_v2(
    official: Vec<Package>,
    aur: Vec<Package>,
    flatpak_hits: Vec<(SearchResult, Option<String>)>,
    state_registry: &crate::registry::RegistryManager,
    installed_flatpaks: &std::collections::HashSet<String>,
) -> Vec<Package> {
    let mut packages = merge_search_results(
        official,
        aur,
        flatpak_hits,
        state_registry,
        installed_flatpaks,
    );
    packages = utils::deduplicate_by_canonical_key(packages);
    packages = deduplicate_and_merge_packages(packages);

    for pkg in &mut packages {
        if pkg.canonical_id.is_empty() {
            pkg.canonical_id = utils::canonical_merge_key(&pkg.name, pkg.app_id.as_deref());
        }
    }

    packages
}

fn best_primary_source(sources: &[PackageSource]) -> PackageSource {
    // Rank: Repo > Flatpak > AUR
    // Within Repo: Official > Chaotic
    let mut best = &sources[0];
    let mut best_score = source_score(best);

    for s in sources.iter().skip(1) {
        let score = source_score(s);
        if score > best_score {
            best = s;
            best_score = score;
        }
    }
    best.clone()
}

fn source_score(s: &PackageSource) -> i32 {
    match s.source_type.as_str() {
        "repo" => {
            let id = s.id.to_lowercase();
            if id.contains("cachyos")
                || id.contains("manjaro")
                || id.contains("garuda")
                || id.contains("endeavour")
            {
                100
            } else if matches!(
                id.as_str(),
                "core" | "extra" | "community" | "multilib" | "official"
            ) {
                90
            } else if id.contains("chaotic") {
                70
            } else {
                80
            }
        }
        "aur" => 60,
        "flatpak" => 50,
        _ => 0,
    }
}

fn source_rank_for_default(s: &PackageSource) -> i32 {
    let id = s.id.to_lowercase();
    match s.source_type.as_str() {
        "repo" => {
            if id.contains("cachyos")
                || id.contains("manjaro")
                || id.contains("garuda")
                || id.contains("endeavour")
            {
                60
            } else if matches!(
                id.as_str(),
                "core" | "extra" | "community" | "multilib" | "official"
            ) {
                50
            } else if id.contains("chaotic") {
                35
            } else {
                40
            }
        }
        "aur" => 30,
        "flatpak" => 20,
        "local" => 10,
        _ => 0,
    }
}

fn metadata_quality_score(pkg: &Package) -> i32 {
    let icon_score = match pkg.icon.as_deref() {
        Some(icon) if icon.starts_with("http") || icon.starts_with("data:") => 5,
        Some(icon) if !icon.trim().is_empty() => 3,
        _ => 0,
    };
    let screenshots_score = if pkg.screenshots.as_ref().map(|shots| !shots.is_empty()).unwrap_or(false) {
        4
    } else {
        0
    };
    let maintainer_score = if pkg.maintainer.as_ref().map(|m| !m.trim().is_empty()).unwrap_or(false) {
        3
    } else {
        0
    };
    let long_desc_score = if pkg
        .long_description
        .as_ref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
    {
        4
    } else {
        0
    };
    let title_score = if pkg
        .display_name
        .as_ref()
        .map(|name| !name.trim().is_empty() && !name.contains('.'))
        .unwrap_or(false)
    {
        2
    } else {
        0
    };
    icon_score + screenshots_score + maintainer_score + long_desc_score + title_score
}

fn maintainer_fallback_for_source(source: &PackageSource) -> Option<String> {
    let id = source.id.to_lowercase();
    match source.source_type.as_str() {
        "repo" => {
            if id.contains("cachyos") {
                Some("CachyOS Packaging Team".to_string())
            } else if id.contains("chaotic") {
                Some("Chaotic-AUR Team".to_string())
            } else if id.contains("manjaro") || id.contains("garuda") || id.contains("endeavour") {
                Some("Distribution Packaging Team".to_string())
            } else {
                Some("Arch Linux Packager".to_string())
            }
        }
        "aur" => None,
        "flatpak" => None,
        _ => None,
    }
}

fn best_presentation_variant(pkg: &Package) -> Package {
    let candidates = pkg
        .alternatives
        .clone()
        .unwrap_or_else(|| vec![pkg.clone()]);

    let mut best = candidates[0].clone();
    let mut best_score = (
        metadata_quality_score(&best),
        if best.installed { 1 } else { 0 },
        source_rank_for_default(&best.source),
    );

    for candidate in candidates.into_iter().skip(1) {
        let score = (
            metadata_quality_score(&candidate),
            if candidate.installed { 1 } else { 0 },
            source_rank_for_default(&candidate.source),
        );
        if score > best_score {
            best = candidate;
            best_score = score;
        }
    }

    best
}

fn best_default_variant(pkg: &Package) -> Package {
    let candidates = pkg
        .alternatives
        .clone()
        .unwrap_or_else(|| vec![pkg.clone()]);

    let mut best = candidates[0].clone();
    let mut best_score = (
        if best.installed { 1 } else { 0 },
        source_rank_for_default(&best.source),
        metadata_quality_score(&best),
    );

    for candidate in candidates.into_iter().skip(1) {
        let score = (
            if candidate.installed { 1 } else { 0 },
            source_rank_for_default(&candidate.source),
            metadata_quality_score(&candidate),
        );
        if score > best_score {
            best = candidate;
            best_score = score;
        }
    }

    best
}

fn apply_display_winner(pkg: &mut Package) {
    let presentation = best_presentation_variant(pkg);
    let default_variant = best_default_variant(pkg);
    log::debug!(
        "[CANONICAL] group={} variants={} source_winner={} presentation_from={} quality={}",
        if pkg.canonical_id.is_empty() {
            utils::canonical_merge_key(&pkg.name, pkg.app_id.as_deref())
        } else {
            pkg.canonical_id.clone()
        },
        pkg.alternatives.as_ref().map(|alts| alts.len()).unwrap_or(1),
        default_variant.source.id,
        presentation.source.id,
        metadata_quality_score(&presentation)
    );

    pkg.source = default_variant.source.clone();
    if !default_variant.version.trim().is_empty() {
        pkg.version = default_variant.version.clone();
    }
    if default_variant.maintainer.is_some() {
        pkg.maintainer = default_variant.maintainer.clone();
    } else if pkg.maintainer.is_none() {
        pkg.maintainer = maintainer_fallback_for_source(&default_variant.source);
    }
    if default_variant.license.is_some() {
        pkg.license = default_variant.license.clone();
    }
    if default_variant.url.is_some() {
        pkg.url = default_variant.url.clone();
    }

    if presentation
        .display_name
        .as_ref()
        .map(|name| !name.trim().is_empty())
        .unwrap_or(false)
    {
        pkg.display_name = presentation.display_name.clone();
    }
    if presentation
        .display_title
        .as_ref()
        .map(|name| !name.trim().is_empty())
        .unwrap_or(false)
    {
        pkg.display_title = presentation.display_title.clone();
    }
    if presentation
        .icon
        .as_ref()
        .map(|icon| !icon.trim().is_empty())
        .unwrap_or(false)
    {
        pkg.icon = presentation.icon.clone();
    }
    if presentation
        .screenshots
        .as_ref()
        .map(|shots| !shots.is_empty())
        .unwrap_or(false)
    {
        pkg.screenshots = presentation.screenshots.clone();
    }
    if presentation
        .long_description
        .as_ref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
    {
        pkg.long_description = presentation.long_description.clone();
    }
    if presentation
        .description
        .trim()
        .len()
        > pkg.description.trim().len()
    {
        pkg.description = presentation.description.clone();
    }
    if presentation.app_id.is_some() {
        pkg.app_id = presentation.app_id.clone();
    }
}

#[allow(clippy::too_many_arguments)] // state refs + items + flags
pub async fn fetch_and_merge_packages_by_names_impl(
    state_meta: &metadata::MetadataState,
    state_chaotic: &chaotic_api::ChaoticApiClient,
    state_repo: &RepoManager,
    state_flathub: &FlathubApiClient,
    state_registry: &crate::registry::RegistryManager,
    items: Vec<(String, Option<String>)>,
    include_flatpak: bool,
    include_aur: bool,
    include_chaotic: bool,
    installed_lookup: bool,
) -> Result<Vec<models::Package>, String> {
    state_meta.wait_until_ready().await;
    let mut packages = Vec::new();

    // Legacy support: Reconstruct names vec for parts of function body relying on it
    let names: Vec<String> = items.iter().map(|(n, _)| n.clone()).collect();
    log::debug!("[AGGREGATION] Starting fetch for {} items", names.len());

    // 0. REGISTRY-FIRST RESOLUTION: Ask the DB for known repo names for these IDs.
    // This stops the "guessing game" (e.g. telegram -> telegram-desktop) if the user has ever seen the app before.
    let mut registry_lookup_keys: Vec<String> = items
        .iter()
        .map(|(name, app_id_opt)| utils::canonical_merge_key(name, app_id_opt.as_deref()))
        .collect();
    registry_lookup_keys.sort();
    registry_lookup_keys.dedup();
    let registry_map = state_registry
        .get_repo_names_for_canonical_ids(&registry_lookup_keys)
        .unwrap_or_default();
    log::debug!(
        "[AGGREGATION] Registry resolution complete ({} mapped)",
        registry_map.len()
    );

    // Expand names & Map App IDs to Repo Names (so get_packages_batch finds CachyOS/repo variants e.g. discord_arch_electron)
    let mut expanded_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut canonical_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (name, app_id_opt) in &items {
        let key = utils::canonical_merge_key(name, app_id_opt.as_deref());
        // If Registry knows this ID, use its authoritative mapping first
        if let Some(known_names) = registry_map.get(&key) {
            for kn in known_names {
                expanded_set.insert(kn.clone());
            }
        }

        expanded_set.insert(name.clone());
        expanded_set.insert(name.to_lowercase());
        let search_base = utils::canonical_search_base(name, app_id_opt.as_deref());

        canonical_keys.insert(key.clone());

        if let Some(app_id) = app_id_opt {
            if let Some(mapped) = utils::known_app_id_to_canonical(app_id) {
                expanded_set.insert(mapped.clone());
            }
            expanded_set.insert(key);
            expanded_set.insert(search_base);
        } else {
            expanded_set.insert(key);
            expanded_set.insert(search_base);
            expanded_set.insert(utils::strip_package_suffix(&name.to_lowercase()).to_string());
            if name.contains('.') {
                expanded_set.insert(utils::canonical_search_base(name, Some(name)));
            }
        }

        if let Some(stripped) = name.strip_suffix("-bin") {
            expanded_set.insert(stripped.to_string());
        }
    }
    for canonical in &canonical_keys {
        for repo_name in utils::canonical_to_repo_lookup_names(canonical) {
            expanded_set.insert(repo_name.to_string());
        }
    }
    let expanded_names: Vec<String> = expanded_set.into_iter().collect();
    let for_repo = expanded_names.clone();

    // Pre-fetch installed Flatpak IDs for accurate "Installed" status in list views.
    // We do this concurrently or before the join. It's fast (file read).
    let installed_flatpaks_future = async {
        if include_flatpak {
            crate::flathub_api::get_installed_flatpak_app_ids()
                .await
                .unwrap_or_default()
                .into_iter()
                .collect::<std::collections::HashSet<String>>()
        } else {
            std::collections::HashSet::new()
        }
    };

    // IRON CORE (SSOT): Fetch authoritative metadata from Registry for these keys.
    // This allows us to overwrite "weak" local data (e.g. "libreoffice-fresh") with "strong" Registry data ("LibreOffice").
    let registry_pkgs_map =
        if let Ok(pkgs) = state_registry.get_packages_by_canonical_ids(&expanded_names) {
            pkgs.into_iter()
                .map(|p| (p.canonical_id.clone(), p))
                .collect::<std::collections::HashMap<_, _>>()
        } else {
            std::collections::HashMap::new()
        };
    log::debug!(
        "[AGGREGATION] Iron Core loaded {} authoritative packages",
        registry_pkgs_map.len()
    );

    // Run repo (ALPM), Chaotic, and Flatpak in parallel to cut latency (Essentials/Trending/Categories).
    let chaotic_allowed = crate::distro_context::DistroContext::new().is_chaotic_compatible()
        || state_repo.is_advanced_mode().await;
    let chaotic_enabled = chaotic_allowed && include_chaotic;

    // Discovery/browse should honor source toggles.
    // Installed lookup remains unfiltered so installed apps can still resolve details
    // even when a discovery source is hidden.
    let enabled_repo_names = if installed_lookup {
        Vec::new()
    } else {
        state_repo.get_enabled_repo_names().await
    };

    let for_repo_clone = for_repo.clone();
    let enabled_repo_names_clone = enabled_repo_names.clone();
    let repo_handle = tokio::task::spawn_blocking(move || {
        crate::alpm_read::get_packages_batch(&for_repo_clone, &enabled_repo_names_clone)
    });

    let chaotic_future = async {
        if chaotic_enabled {
            match tokio::time::timeout(
                std::time::Duration::from_secs(4),
                state_chaotic.get_packages_batch(expanded_names.clone()),
            )
            .await
            {
                Ok(map) => map,
                Err(_) => {
                    log::warn!("[AGGREGATION] Chaotic batch fetch timed out. Continuing without Chaotic variants.");
                    std::collections::HashMap::new()
                }
            }
        } else {
            std::collections::HashMap::new()
        }
    };

    let flatpak_future = async {
        if !include_flatpak {
            return Vec::new();
        }
        #[allow(clippy::redundant_iter_cloned)] // .cloned() needed so async move block owns (String, Option<String>)
        let tasks = items.iter().cloned().map(|(name, app_id_opt)| {
            let flathub_client = state_flathub;
            async move {
                let mut hits = Vec::new();
                let target_key = utils::canonical_merge_key(&name, app_id_opt.as_deref());

                // 1. Exact App ID search if provided
                if let Some(app_id) = &app_id_opt {
                    if let Some(res) = flathub_client.search_flathub(app_id).await {
                        for hit in res {
                            if hit.app_id == *app_id {
                                hits.push(hit);
                                break;
                            }
                        }
                    }
                }

                let base = utils::strip_package_suffix(&name.to_lowercase()).to_string();
                if let Some(res) = flathub_client.search_flathub(&base).await {
                    for hit in res {
                        let hit_key = utils::canonical_merge_key(&hit.app_id, Some(&hit.app_id));
                        if hit_key == target_key {
                            hits.push(hit);
                        }
                    }
                }
                if let Some(mapped_id) = crate::flathub_api::get_flathub_app_id(&base) {
                    if Some(&mapped_id) != app_id_opt.as_ref() {
                        if let Some(res) = flathub_client.search_flathub(&mapped_id).await {
                            for hit in res {
                                if hit.app_id == mapped_id {
                                    hits.push(hit);
                                    break;
                                }
                            }
                        }
                    }
                }
                if name.contains('.') {
                    let canonical = utils::canonical_merge_key(&name, Some(&name));
                    if !canonical.is_empty()
                        && canonical != base
                        && Some(&canonical) != app_id_opt.as_ref()
                    {
                        if let Some(res) = flathub_client.search_flathub(&canonical).await {
                            for hit in res {
                                let hit_key =
                                    utils::canonical_merge_key(&hit.app_id, Some(&hit.app_id));
                                if hit_key == target_key {
                                    hits.push(hit);
                                }
                            }
                        }
                    }
                }
                hits
            }
        });
        let mut flatpak_hits_raw = Vec::new();
        let mut stream = futures::stream::iter(tasks).buffer_unordered(12);
        while let Some(res) = stream.next().await {
            flatpak_hits_raw.extend(res);
        }

        // v0.2.41: Enrich Flatpak hits with real versions if possible
        let mut flatpak_hits = Vec::new();
        if !flatpak_hits_raw.is_empty() {
            let versions = if installed_lookup {
                std::collections::HashMap::new()
            } else {
                let app_ids: Vec<String> = flatpak_hits_raw.iter().map(|h| h.app_id.clone()).collect();
                state_flathub
                    .get_remote_versions_batch(&app_ids)
                    .await
                    .unwrap_or_default()
            };

            for mut hit in flatpak_hits_raw {
                // IRON CORE ENFORCEMENT for Flatpak
                // We use the same registry map we loaded earlier.
                // Canonical key might need to be derived from app_id if name is not standard.
                // Flathub hit.name is usually "Discord", hit.app_id is "com.discordapp.Discord".
                // Our registry keys are usually "discord" or "com.discordapp.discord".

                // Try ID-based lookup first (strongest)
                let id_key = hit.app_id.to_lowercase();
                let name_key = utils::canonical_merge_key(&hit.name, Some(&hit.app_id));

                let reg_entry = registry_pkgs_map
                    .get(&id_key)
                    .or_else(|| registry_pkgs_map.get(&name_key));

                if let Some(reg) = reg_entry {
                    if let Some(dn) = &reg.display_name {
                        if !dn.is_empty() {
                            hit.name = dn.clone();
                        }
                    }
                    if !reg.description.is_empty() {
                        hit.summary = Some(reg.description.clone());
                    }
                    let reg_is_rich = reg
                        .icon
                        .as_deref()
                        .map(|i| i.starts_with("http") || i.starts_with("data:"))
                        .unwrap_or(false);
                    if reg_is_rich || hit.icon.is_none() {
                        hit.icon = reg.icon.clone();
                    }
                }

                let v = versions.get(&hit.app_id).cloned();
                flatpak_hits.push((hit, v));
            }
        }
        flatpak_hits
    };

    let (repo_result, chaotic_pkgs, flatpak_hits, installed_flatpaks) = tokio::join!(
        repo_handle,
        chaotic_future,
        flatpak_future,
        installed_flatpaks_future
    );

    let repo_pkgs = repo_result.map_err(|e| e.to_string())?;

    // FALLBACK: If ALPM returned nothing but we asked for names, try the text-file cache.
    // This happens if libalpm is broken or returned empty results for some reason.
    let mut repo_pkgs = repo_pkgs;
    if repo_pkgs.is_empty() && !for_repo.is_empty() {
        log::warn!(
            "[AGGREGATION] ALPM returned 0 results for {} items. Attempting RepoManager Fallback.",
            for_repo.len()
        );
        let distro = crate::distro_context::DistroContext::new();
        for name in &for_repo {
            if let Some(pkg) = state_repo.get_package_exact(name, &distro).await {
                repo_pkgs.push(pkg);
            }
        }
        if !repo_pkgs.is_empty() {
            log::info!(
                "[AGGREGATION] Fallback successful: Recovered {} packages from text cache.",
                repo_pkgs.len()
            );
        }
    }

    // SSOT ENFORCEMENT: Local enrichment before canonical merge.
    if let Ok(loader) = state_meta.loader.lock() {
        enrich_with_local_metadata(&mut repo_pkgs, &loader);
    }

    for mut pkg in repo_pkgs {
        // Final fallback for display name if still missing
        if pkg.display_name.is_none() {
            pkg.display_name = Some(utils::to_pretty_name(&pkg.name));
        }

        // IRON CORE ENFORCEMENT
        let key = utils::canonical_merge_key(&pkg.name, pkg.app_id.as_deref());
        if let Some(reg) = registry_pkgs_map.get(&key) {
            apply_registry_backfill(&mut pkg, reg);
        }

        packages.push(pkg);
    }

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
            source: models::PackageSource::chaotic(&name),
            maintainer: Some("Chaotic-AUR Team".to_string()),
            license: p
                .metadata
                .as_ref()
                .and_then(|m| m.license.clone())
                .map(|l| vec![l]),
            url: p.metadata.as_ref().and_then(|m| m.url.clone()),
            installed: crate::utils::is_package_or_alias_installed(&name),
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: None,
            icon: {
                let mut icon = None;
                if let Ok(loader) = state_meta.loader.lock() {
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

        if let Ok(loader) = state_meta.loader.lock() {
            pkg.app_id = loader.find_app_id(&name);
        }

        // IRON CORE ENFORCEMENT
        let key = utils::canonical_merge_key(&pkg.name, pkg.app_id.as_deref());
        if let Some(reg) = registry_pkgs_map.get(&key) {
            apply_registry_backfill(&mut pkg, reg);
        }

        packages.push(pkg);
    }

    // 3. AUR for ALL names - Parallelized
    if include_aur && !expanded_names.is_empty() {
        // A. Batch Info Fetch (already batched by nature of get_multi_info)
        let names_refs: Vec<&str> = expanded_names.iter().map(|s| s.as_str()).collect();
        let mut aur_exact = aur_api::get_multi_info(&names_refs)
            .await
            .unwrap_or_default();
        let mut exact_names: std::collections::HashSet<String> =
            aur_exact.iter().map(|p| p.name.clone()).collect();

        // B. Canonical Base Search (Parallelized)
        let canonical_bases: Vec<String> = items
            .iter()
            .map(|(n, app_id)| utils::canonical_merge_key(n, app_id.as_deref()))
            .filter(|k| !k.is_empty())
            .collect::<std::collections::HashSet<_>>() // dedup
            .into_iter()
            .collect();

        let tasks = canonical_bases.into_iter().map(|base| async move {
            let mut results = Vec::new();
            if let Ok(search_results) = aur_api::search_aur(&base).await {
                results = search_results;
            }
            (base, results)
        });

        let mut stream = futures::stream::iter(tasks).buffer_unordered(10);
        while let Some((base, search_results)) = stream.next().await {
            for p in search_results {
                let key = utils::canonical_merge_key(&p.name, p.app_id.as_deref());
                if key == base && !exact_names.contains(&p.name) {
                    aur_exact.push(p.clone());
                    exact_names.insert(p.name.clone());
                }
            }
        }

        // IRON CORE ENFORCEMENT for AUR
        let aur_all: Vec<models::Package> = aur_exact
            .into_iter()
            .map(|mut p| {
                let key = utils::canonical_merge_key(&p.name, p.app_id.as_deref());
                if let Some(reg) = registry_pkgs_map.get(&key) {
                    apply_registry_backfill(&mut p, reg);
                }
                p
            })
            .collect();

        packages = build_package_view_models_v2(
            packages,
            aur_all,
            flatpak_hits,
            state_registry,
            &installed_flatpaks,
        );
    } else {
        packages = build_package_view_models_v2(
            packages,
            vec![],
            flatpak_hits,
            state_registry,
            &installed_flatpaks,
        );
    }

    // 4. Global Metadata Enrichment: Fix icons/names via Flathub API Fallback
    // This fixes the "Missing Icon in Category View" for local packages.
    enrich_packages_metadata(&mut packages, state_flathub).await;

    // 5. Re-normalize source slots after metadata enrichment.
    packages = deduplicate_and_merge_packages(packages);

    // 5b. Final Local Enrichment (Ensures Flatpaks get enriched if they match local AppStream IDs)
    if let Ok(loader) = state_meta.loader.lock() {
        enrich_with_local_metadata(&mut packages, &loader);
    }

    // 6. Enrich with ODRS Ratings
    enrich_packages_ratings(&mut packages).await;

    // PERSIST TO REGISTRY: Seed the persistent index with these high-quality enriched packages.
    let _ = state_registry.bulk_upsert_packages(&packages);

    Ok(packages)
}

pub async fn enrich_packages_metadata(
    packages: &mut [models::Package],
    state_flathub: &FlathubApiClient,
) {
    let mut packages_to_enrich = Vec::new();
    let mut seen_enrichment_keys = std::collections::HashSet::new();
    for (i, pkg) in packages.iter().enumerate() {
        if packages_to_enrich.len() >= ENRICH_CAP {
            break;
        }

        // Trigger enrichment if:
        // 1. app_id is missing or weak (no dot)
        // 2. OR icon is missing
        let is_weak_id = pkg
            .app_id
            .as_ref()
            .map(|id| !id.contains('.'))
            .unwrap_or(true);
        let has_no_icon = pkg.icon.is_none();
        let has_weak_description =
            pkg.description.is_empty() || pkg.description.eq_ignore_ascii_case(&pkg.name);
        let enrichment_key = utils::canonical_search_base(&pkg.name, pkg.app_id.as_deref());

        if (has_no_icon || (is_weak_id && has_weak_description))
            && seen_enrichment_keys.insert(enrichment_key)
        {
            packages_to_enrich.push((i, pkg.name.clone()));
        }
    }

    if packages_to_enrich.is_empty() {
        return;
    }

    let mut stream = futures::stream::iter(packages_to_enrich.into_iter().map(
        |(idx, name)| async move {
            // Apply 3s timeout to individual Flathub queries
            let meta_opt = match tokio::time::timeout(
                std::time::Duration::from_secs(3),
                state_flathub.get_metadata_for_package(&name),
            )
            .await
            {
                Ok(res) => res,
                Err(_) => {
                    log::warn!("[AGGREGATION] Flathub enrichment timed out for {}", name);
                    None
                }
            };
            (idx, meta_opt)
        },
    ))
    .buffer_unordered(ENRICH_CHUNK);

    let stream_start = std::time::Instant::now();
    while let Some((idx, meta_opt)) = stream.next().await {
        if stream_start.elapsed().as_secs() > 8 {
            log::warn!("[AGGREGATION] Global enrichment timeout reached. Aborting remaining.");
            break;
        }
        if let Some(fm) = meta_opt {
            let pkg_name = &packages[idx].name;
            // Only use Flathub metadata when the match is trusted: direct mapping (get_flathub_app_id)
            // or app_id last segment matches our package name. Stops wrong icons (e.g. "stress" -> gst, "tree" -> TreeSheets).
            let trusted_id = crate::flathub_api::get_flathub_app_id(pkg_name);
            let flathub_id = fm.id.as_deref().unwrap_or("");
            let from_mapping = trusted_id.as_deref() == Some(flathub_id);
            let segments: Vec<&str> = flathub_id.split('.').collect();
            let last_segment = segments
                .last()
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            let pkg_norm: String = pkg_name
                .to_lowercase()
                .trim_end_matches("-bin")
                .trim_end_matches("-git")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            // Exact last-segment match (e.g. discord/steam/piper), or second-to-last segment
            // starts with pkg_norm only when name is long enough (>=5) to avoid "gala" -> GalaxyBudsClient.
            let segment_exact = !last_segment.is_empty() && last_segment == pkg_norm;
            let segment_prefix = pkg_norm.len() >= 5
                && segments.len() >= 2
                && segments[segments.len() - 2]
                    .to_lowercase()
                    .starts_with(&pkg_norm)
                && segments[segments.len() - 2].len() <= pkg_norm.len() + 12;
            let segment_match = segment_exact || segment_prefix;
            let trusted = from_mapping || segment_match;

            let full_meta = crate::flathub_api::flathub_to_app_metadata(&fm, pkg_name);
            let p = &mut packages[idx];

            let enriched_key = utils::canonical_merge_key(&p.name, Some(&full_meta.app_id));

            // Never rewrite identity from fuzzy enrichment; only upgrade metadata on trusted match.
            if trusted {
                p.app_id = Some(full_meta.app_id.clone());
                if p.canonical_id.is_empty() {
                    p.canonical_id = enriched_key.clone();
                }

                p.display_name = Some(
                    utils::get_preferred_display_name(&enriched_key)
                        .map(String::from)
                        .unwrap_or_else(|| full_meta.name.clone()),
                );
            }

            // Only set icon when match is trusted to avoid wrong logos (generic names matching different Flathub apps).
            if trusted {
                if let Some(ic) = full_meta.icon_url.clone() {
                    p.icon = Some(ic);
                    log::debug!(
                        "[CARD/DETAILS] enrich_packages_metadata set icon for name={} app_id={}",
                        p.name,
                        p.app_id.as_deref().unwrap_or("")
                    );
                }
            }

            if trusted {
                p.description = full_meta
                    .description
                    .as_deref()
                    .map(utils::strip_html)
                    .unwrap_or_else(|| p.description.clone());
                if p.long_description.is_none() || p.long_description.as_deref().unwrap_or("").is_empty() {
                    p.long_description = full_meta.description.clone();
                }
                p.maintainer = full_meta.maintainer.or(p.maintainer.clone());
                p.license = full_meta.license.map(|l| vec![l]).or(p.license.clone());
                if !full_meta.screenshots.is_empty() {
                    p.screenshots = Some(full_meta.screenshots);
                }
            }
        }
    }
}

/// Deduplicates packages based on AppID/Name after enrichment.
/// Merges available_sources and prioritizes installed instances.
pub fn deduplicate_and_merge_packages(packages: Vec<models::Package>) -> Vec<models::Package> {
    let mut unique_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut deduped_pkgs: Vec<models::Package> = Vec::new();

    for p in packages {
        let key = utils::canonical_merge_key(&p.name, p.app_id.as_deref());

        if let Some(&idx) = unique_map.get(&key) {
            let existing = &mut deduped_pkgs[idx];

            // 1. Merge sources into a separate list first to avoid borrow conflicts
            let mut merged_sources = existing
                .available_sources
                .clone()
                .unwrap_or_else(|| vec![existing.source.clone()]);
            let incoming_sources = p
                .available_sources
                .clone()
                .unwrap_or_else(|| vec![p.source.clone()]);

            for src in incoming_sources {
                if let Some(existing_src) = merged_sources.iter_mut().find(|s| same_source_slot(s, &src)) {
                    if src.version > existing_src.version {
                        *existing_src = src;
                    }
                } else {
                    merged_sources.push(src);
                }
            }

            // Merge Installed Sources (Backend-Driven Status)
            let mut inst_sources = existing.installed_sources.clone().unwrap_or_default();
            // If existing was marked installed but list empty (legacy/init), add its source
            if existing.installed && inst_sources.is_empty() {
                inst_sources.push(existing.source.source_type.clone());
            }

            if p.installed {
                let st = p.source.source_type.clone();
                if !inst_sources.contains(&st) {
                    inst_sources.push(st);
                }
                // If p has its own list (pre-merged), merge it
                if let Some(p_list) = &p.installed_sources {
                    for s in p_list {
                        if !inst_sources.contains(s) {
                            inst_sources.push(s.clone());
                        }
                    }
                }
            }
            existing.installed_sources = Some(inst_sources);

            // Collect all original packages as alternatives for rich source-specific metadata
            let mut alternatives = existing.alternatives.clone().unwrap_or_else(|| {
                // If alternatives was empty, the existing package is the first alternative
                vec![existing.clone()]
            });

            // Avoid duplicate alternatives if we already merged this exact variant before
            // (shouldn't happen with the new deduplication in merge_search_results but safe)
            if !alternatives.iter().any(|a| {
                a.source.id == p.source.id
                    && a.source.source_type == p.source.source_type
                    && a.version == p.version
                    && a.name == p.name
            }) {
                alternatives.push(p.clone());
            }
            existing.alternatives = Some(alternatives);

            // Installed state is a boolean summary, not source provenance.
            // Never swap the base package purely because another variant is installed:
            // for same-name packages across repos (official/cachyos/chaotic), that causes
            // source-label flapping and false "installed from chaotic" presentation.
            existing.installed = existing.installed || p.installed;

            // Metadata merge is additive: fill missing/weak fields from the incoming variant.
            if existing.icon.is_none() || existing.icon.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                existing.icon = p.icon.clone();
            }
            if existing.app_id.is_none() && p.app_id.is_some() {
                existing.app_id = p.app_id.clone();
            }
            if (existing.description.is_empty()
                || (existing.description.len() < 20 && p.description.len() > existing.description.len()))
                && !p.description.is_empty() {
                    existing.description = p.description.clone();
                }
            if existing
                .screenshots
                .as_ref()
                .map(|s| s.is_empty())
                .unwrap_or(true)
                && !p.screenshots.as_ref().map(|s| s.is_empty()).unwrap_or(true)
            {
                existing.screenshots = p.screenshots.clone();
            }
            if existing
                .long_description
                .as_ref()
                .map(|s| s.is_empty())
                .unwrap_or(true)
                && !p
                    .long_description
                    .as_ref()
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
            {
                existing.long_description = p.long_description.clone();
            }
            if (existing
                .display_name
                .as_ref()
                .map(|s| s.contains('-') || s.contains('.'))
                .unwrap_or(true))
                && !p
                    .display_name
                    .as_ref()
                    .map(|s| s.contains('-') || s.contains('.'))
                    .unwrap_or(true)
            {
                existing.display_name = p.display_name.clone();
            }

            existing.available_sources = Some(merged_sources);
            if existing.canonical_id.is_empty() {
                existing.canonical_id = key.clone();
            }
        } else {
            let mut p = p;
            p.canonical_id = key.clone();
            // Initialize installed_sources if this is the first entry
            if p.installed {
                p.installed_sources = Some(vec![p.source.source_type.clone()]);
            } else {
                p.installed_sources = Some(vec![]);
            }
            unique_map.insert(key, deduped_pkgs.len());
            // Initialize alternatives with itself so the primary source's metadata is discoverable
            p.alternatives = Some(vec![p.clone()]);
            deduped_pkgs.push(p);
        }
    }
    // Apply preferred display names (e.g. "heroic" -> "Heroic Game Launcher") so Search and Categories show full names
    for pkg in &mut deduped_pkgs {
        apply_display_winner(pkg);
        normalize_sources_for_package(pkg);
        let key = if pkg.canonical_id.is_empty() {
            utils::canonical_merge_key(&pkg.name, pkg.app_id.as_deref())
        } else {
            pkg.canonical_id.clone()
        };
        if let Some(preferred) = utils::get_preferred_display_name(&key) {
            pkg.display_name = Some(preferred.to_string());
        }
        if pkg.maintainer.is_none() {
            pkg.maintainer = maintainer_fallback_for_source(&pkg.source);
        }
    }
    deduped_pkgs
}

/// Fetches ratings for a batch of packages using their App IDs.
pub async fn enrich_packages_ratings(packages: &mut [models::Package]) {
    if packages.is_empty() {
        return;
    }

    let mut app_ids = Vec::new();
    let mut pkg_indices: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, pkg) in packages.iter().enumerate() {
        if let Some(app_id) = &pkg.app_id {
            if !app_id.is_empty() && app_id.contains('.') {
                app_ids.push(app_id.clone());
                pkg_indices.entry(app_id.clone()).or_default().push(i);
            }
        }
    }

    if app_ids.is_empty() {
        return;
    }

    app_ids.sort();
    app_ids.dedup();

    // Chunk requests to avoid huge GET URLs or ODRS limits
    for chunk in app_ids.chunks(25) {
        let chunk_vec = chunk.to_vec();
        match crate::odrs_api::get_app_ratings_batch(chunk_vec).await {
            Ok(ratings) => {
                for (id, rating) in ratings {
                    if let Some(indices) = pkg_indices.get(&id) {
                        for &idx in indices {
                            if let Some(pkg) = packages.get_mut(idx) {
                                pkg.rating = Some(rating.clone());
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("[AGGREGATION] Failed to fetch ODRS ratings: {}", e);
            }
        }
    }
}

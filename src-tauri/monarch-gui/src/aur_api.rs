use crate::models::{Package, PackageSource};
use once_cell::sync::Lazy;
use raur::{Handle, Raur};
use std::sync::Arc;

// Shared Handle - created once, reused
// Shared Handle - created once, reused
static AUR_HANDLE: Lazy<Arc<Handle>> = Lazy::new(|| Arc::new(Handle::new()));

// Cache for AUR search results to prevent 429s (TTL: 10 mins)
static AUR_SEARCH_CACHE: Lazy<moka::future::Cache<String, Vec<raur::Package>>> = Lazy::new(|| {
    moka::future::Cache::builder()
        .max_capacity(500)
        .time_to_live(std::time::Duration::from_secs(600))
        .build()
});

// Convert raur::Package to our internal Package model
fn raur_to_package(p: raur::Package) -> Package {
    let installed = crate::utils::is_package_or_alias_installed(&p.name);
    Package {
        name: p.name,
        display_name: None,
        description: p.description.unwrap_or_default(),
        version: p.version.clone(),
        source: PackageSource::new("aur", "aur", &p.version, "AUR (Community)"),
        maintainer: p.maintainer,
        num_votes: Some(p.num_votes),
        url: p.url,
        license: Some(p.license),
        keywords: Some(p.keywords),
        last_modified: Some(p.last_modified),
        first_submitted: Some(p.first_submitted),
        out_of_date: p.out_of_date,
        icon: None,
        screenshots: None,
        provides: Some(p.provides),
        app_id: None,
        is_optimized: None,
        depends: Some(p.depends),
        make_depends: Some(p.make_depends),
        is_featured: None,
        installed,
        ..Default::default()
    }
}

// Helper for retry logic
async fn retry_aur_call<F, Fut, T>(operation: F) -> Result<T, String>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, raur::Error>>,
{
    let mut attempts = 0;
    let max_attempts = 3;
    let mut delay = std::time::Duration::from_millis(500);

    loop {
        match operation().await {
            Ok(res) => return Ok(res),
            Err(e) => {
                let err_str = e.to_string();
                // If the error is "Too many package results", it's a permanent failure for this query
                if err_str.contains("Too many package results") {
                    return Err(err_str);
                }

                attempts += 1;
                if attempts >= max_attempts {
                    return Err(err_str);
                }
                log::warn!(
                    "[AUR] Request failed (attempt {}/{}): {}. Retrying in {:?}...",
                    attempts,
                    max_attempts,
                    err_str,
                    delay
                );
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
        }
    }
}

pub async fn search_aur(query: &str) -> Result<Vec<Package>, String> {
    if query.len() < 2 {
        return Ok(vec![]);
    }

    // Check cache first
    if let Some(cached_results) = AUR_SEARCH_CACHE.get(query).await {
        let mut packages: Vec<Package> = cached_results.into_iter().map(raur_to_package).collect();
        packages.sort_by(|a, b| b.num_votes.unwrap_or(0).cmp(&a.num_votes.unwrap_or(0)));
        return Ok(packages);
    }

    let results = retry_aur_call(|| AUR_HANDLE.search(query)).await?;

    // Cache the raw results
    AUR_SEARCH_CACHE
        .insert(query.to_string(), results.clone())
        .await;

    // Sort by votes descending
    let mut packages: Vec<Package> = results.into_iter().map(raur_to_package).collect();
    packages.sort_by(|a, b| b.num_votes.unwrap_or(0).cmp(&a.num_votes.unwrap_or(0)));

    Ok(packages)
}

#[allow(dead_code)]
pub async fn search_aur_by_provides(query: &str) -> Result<Vec<Package>, String> {
    if query.len() < 2 {
        return Ok(vec![]);
    }

    let results = retry_aur_call(|| AUR_HANDLE.search_by(query, raur::SearchBy::Provides)).await?;

    let mut packages: Vec<Package> = results.into_iter().map(raur_to_package).collect();
    packages.sort_by(|a, b| b.num_votes.unwrap_or(0).cmp(&a.num_votes.unwrap_or(0)));

    Ok(packages)
}

// Cache for individual AUR package info (TTL: 10 mins)
static AUR_INFO_CACHE: Lazy<moka::future::Cache<String, raur::Package>> = Lazy::new(|| {
    moka::future::Cache::builder()
        .max_capacity(2000)
        .time_to_live(std::time::Duration::from_secs(600))
        .build()
});

pub async fn get_multi_info(names: &[&str]) -> Result<Vec<Package>, String> {
    if names.is_empty() {
        return Ok(vec![]);
    }

    let mut distinct_names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    distinct_names.sort();
    distinct_names.dedup();

    let mut found_packages = Vec::new();
    let mut missing_names = Vec::new();

    // 1. Check cache for each name
    for name in &distinct_names {
        if let Some(pkg) = AUR_INFO_CACHE.get(name).await {
            found_packages.push(pkg);
        } else {
            missing_names.push(name.clone());
        }
    }

    // 2. Fetch missing from AUR
    if !missing_names.is_empty() {
        let missing_refs: Vec<&str> = missing_names.iter().map(|s| s.as_str()).collect();

        // Chunk requests to avoid URL length limits (approx 8k chars max)
        const CHUNK_SIZE: usize = 50;

        for chunk in missing_refs.chunks(CHUNK_SIZE) {
            match AUR_HANDLE.info(chunk).await {
                Ok(results) => {
                    for pkg in results {
                        AUR_INFO_CACHE.insert(pkg.name.clone(), pkg.clone()).await;
                        found_packages.push(pkg);
                    }
                }
                Err(e) => {
                    log::warn!("Failed to fetch AUR chunk: {}", e);
                    // Continue to next chunk but log warning.
                    // If all fail, found_packages might be empty matching original behavior error check.
                }
            }
        }
    }

    Ok(found_packages.into_iter().map(raur_to_package).collect())
}

// --- UPDATE CHECK LOGIC ---

/// Get potential AUR updates by comparing local versions with upstream
pub async fn get_candidate_updates() -> Result<Vec<crate::models::UpdateItem>, String> {
    // 1. Get all foreign packages installed on the system
    let foreign = tokio::task::spawn_blocking(crate::alpm_read::get_foreign_installed_packages)
        .await
        .map_err(|e| format!("Task join error: {}", e))?;

    if foreign.is_empty() {
        return Ok(vec![]);
    }

    let mut installed_map = std::collections::HashMap::new();
    let mut names = Vec::new();
    for (name, version) in &foreign {
        installed_map.insert(name.clone(), version.clone());
        names.push(name.clone());
    }

    // 2. Query AUR for these packages
    let names_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let aur_info = get_multi_info(&names_refs).await?;

    let mut updates = Vec::new();

    // 3. Compare versions (ALPM vercmp: only offer true upgrades, not downgrades)
    for pkg in aur_info {
        if let Some(local_ver) = installed_map.get(&pkg.name) {
            let is_upgrade = tokio::task::spawn_blocking({
                let new_v = pkg.version.clone();
                let old_v = local_ver.clone();
                move || crate::alpm_read::vercmp_greater(&new_v, &old_v)
            })
            .await
            .map_err(|e| format!("Task join error: {}", e))?;
            if is_upgrade {
                updates.push(crate::models::UpdateItem {
                    name: pkg.name.clone(),
                    current_version: local_ver.clone(),
                    new_version: pkg.version.clone(),
                    source: PackageSource::new("aur", "aur", &pkg.version, "AUR (Community)"),
                    size: None, // AUR doesn't give download size easily (source size varies)
                    icon: None,
                    display_name: None,
                });
            }
        }
    }

    Ok(updates)
}

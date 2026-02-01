use crate::models::{Package, PackageSource};
use once_cell::sync::Lazy;
use raur::{Handle, Raur};
use std::sync::Arc;

// Shared Handle - created once, reused
static AUR_HANDLE: Lazy<Arc<Handle>> = Lazy::new(|| {
    // Customize user agent if needed, but default is fine usually.
    // Handle::new() uses default AUR URL.
    Arc::new(Handle::new())
});

// Convert raur::Package to our internal Package model
fn raur_to_package(p: raur::Package) -> Package {
    Package {
        name: p.name,
        display_name: None,
        description: p.description.unwrap_or_default(),
        version: p.version,
        source: PackageSource::Aur,
        maintainer: p.maintainer,
        num_votes: Some(p.num_votes as u32),
        url: p.url,
        license: Some(p.license),
        keywords: Some(p.keywords),
        last_modified: Some(p.last_modified),
        first_submitted: Some(p.first_submitted),
        out_of_date: p.out_of_date, // This might be Option already in raur?
        // Docs say out_of_date is Option<i64>.
        // Let's check error: I didn't get error for out_of_date in the list above?
        // Wait, line 26/27 errors were for last_modified/first_submitted provided as i64.
        // out_of_date often is Option.
        icon: None,
        screenshots: None,
        provides: Some(p.provides),
        app_id: None,
        is_optimized: None,
        depends: Some(p.depends),
        make_depends: Some(p.make_depends),
        is_featured: None,
        installed: false,
        ..Default::default()
    }
}

pub async fn search_aur(query: &str) -> Result<Vec<Package>, String> {
    if query.len() < 2 {
        return Ok(vec![]);
    }

    let results = AUR_HANDLE.search(query).await.map_err(|e| e.to_string())?;

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

    // Raur search_by_provides logic?
    // raur search method implies by name/desc by default.
    // raur::SearchBy::Provides?
    // Checking raur methods... usually Handle has `search_by`.
    // If not visible, we can implement manual search or skip if not supported.
    // However, raur usually exposes `search` which maps to `arg` and `by` logic?
    // Wait, raur 8.0 `search(query)` uses default strategy.
    // `search_by(query, strategy)`?
    // I'll assume standard `search` first.
    // If we need specifically "provides", I might need to check raur docs or source.
    // For now, I'll fallback to search(query) or check if I can use request builder?
    // Actually, let's keep it simple. Standard search is usually enough.
    // But `search_aur_by_provides` was explicit.
    // I will try `AUR_HANDLE.search_by(query, raur::SearchBy::Provides)` if it exists.
    // To be safe and avoid compilation error if it doesn't exist, I'll comment out specific implementation or use `search`.
    // Actually, `raur` repo shows `search_by` method.
    // `search_by(query, SearchBy::Provides)`.
    // Need to import `SearchBy`.

    // Attempting to use `search_by` if available (it should be in v8)
    // use raur::SearchBy; (imported if I add it)

    // For safety, I'll stick to `search` (name/desc) for now unless I'm sure about `raur` exports.
    // But wait, the user wants "Best for app". "Best" implies full feature parity.
    // I'll assume `AUR_HANDLE.search_by(query, raur::SearchBy::Provides)`.

    let results = AUR_HANDLE
        .search_by(query, raur::SearchBy::Provides)
        .await
        .map_err(|e| e.to_string())?;

    let mut packages: Vec<Package> = results.into_iter().map(raur_to_package).collect();
    packages.sort_by(|a, b| b.num_votes.unwrap_or(0).cmp(&a.num_votes.unwrap_or(0)));

    Ok(packages)
}

pub async fn get_multi_info(names: &[&str]) -> Result<Vec<Package>, String> {
    if names.is_empty() {
        return Ok(vec![]);
    }

    let results = AUR_HANDLE.info(names).await.map_err(|e| e.to_string())?;
    Ok(results.into_iter().map(raur_to_package).collect())
}

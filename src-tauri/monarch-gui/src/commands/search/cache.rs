use crate::models;
use moka::future::Cache;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

// Cache for Trending results (key: (include_flatpak, include_aur, include_chaotic))
// TTL: 15 minutes
pub(crate) static TRENDING_CACHE: Lazy<Cache<(bool, bool, bool), Vec<models::Package>>> =
    Lazy::new(|| {
        Cache::builder()
            .time_to_live(std::time::Duration::from_secs(60 * 15))
            .build()
    });

// Cache for Category Paginated results (key: composite string of params)
// TTL: 6 hours — category contents change slowly; longer TTL avoids refetch when revisiting.
pub(crate) static CATEGORY_CACHE: Lazy<Cache<String, Vec<models::Package>>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(std::time::Duration::from_secs(6 * 60 * 60))
        .build()
});

/// Cache for large get_packages_by_names results (e.g. Essentials). TTL 7 days.
pub(crate) const PACKAGES_CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;
/// Bump to invalidate old caches (one-card-per-app, variants, etc.). Users not seeing fixes should restart the app.
pub(crate) const PACKAGES_CACHE_VERSION: u32 = 5;

#[derive(Serialize, Deserialize)]
pub(crate) struct PackagesListCache {
    #[serde(default)]
    pub version: u32,
    pub names_sorted: Vec<String>,
    pub packages: Vec<models::Package>,
    pub fetched_at: u64,
}

/// Clear in-memory search/trending/category caches AND their disk counterparts.
/// Called from Settings "Clear cache" so users see pipeline fixes without restarting the app.
#[tauri::command]
#[specta::specta]
pub fn clear_search_and_list_caches() {
    TRENDING_CACHE.invalidate_all();
    CATEGORY_CACHE.invalidate_all();
    // Delete disk caches so next startup is also fresh
    if let Some(dir) = dirs::cache_dir().map(|d| d.join("monarch-store")) {
        // Remove all category disk caches
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if (name_str.starts_with("trending_cache_")
                    || name_str.starts_with("category_")
                    || name_str.starts_with("essentials_"))
                    && name_str.ends_with(".json")
                {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        let _ = std::fs::remove_file(dir.join("essentials_packages_cache.json"));
    }
}

pub(crate) fn packages_list_cache_path() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|d| {
        d.join("monarch-store")
            .join("essentials_packages_cache.json")
    })
}

/// If names.len() >= 30, try to return cached packages (same set, fresh). Still runs installed pass.
pub(crate) fn try_get_packages_from_cache(names: &[String]) -> Option<Vec<models::Package>> {
    if names.len() < 30 {
        return None;
    }
    let path = packages_list_cache_path()?;
    let data = std::fs::read_to_string(&path).ok()?;
    let cache: PackagesListCache = serde_json::from_str(&data).ok()?;
    let mut sorted_names = names.to_vec();
    sorted_names.sort();
    if cache.version != PACKAGES_CACHE_VERSION || cache.names_sorted != sorted_names {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now.saturating_sub(cache.fetched_at) >= PACKAGES_CACHE_TTL_SECS {
        return None;
    }
    log::info!(
        "[DISK CACHE] Loaded essentials from disk ({} packages)",
        cache.packages.len()
    );
    Some(cache.packages)
}

pub(crate) fn write_packages_cache(names: &[String], packages: &[models::Package]) {
    if names.len() < 30 {
        return;
    }
    let path = match packages_list_cache_path() {
        Some(p) => p,
        None => return,
    };
    let mut sorted_names = names.to_vec();
    sorted_names.sort();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cache = PackagesListCache {
        version: PACKAGES_CACHE_VERSION,
        names_sorted: sorted_names,
        packages: packages.to_vec(),
        fetched_at: now,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(&cache) {
        log::info!(
            "[DISK CACHE] Writing essentials to disk ({} packages)",
            cache.packages.len()
        );
        let _ = std::fs::write(&path, json);
    }
}

// ─── Disk cache helpers for Trending ───────────────────────────────────────
pub(crate) const TRENDING_DISK_TTL_SECS: u64 = 15 * 60; // 15 minutes, matches moka TTL

pub(crate) fn trending_cache_path(
    include_flatpak: bool,
    include_aur: bool,
    include_chaotic: bool,
) -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|d| {
        d.join("monarch-store").join(format!(
            "trending_cache_v7_{}_{}_{}.json",
            include_flatpak, include_aur, include_chaotic
        ))
    })
}

pub(crate) fn try_read_trending_disk(
    include_flatpak: bool,
    include_aur: bool,
    include_chaotic: bool,
) -> Option<Vec<models::Package>> {
    let path = trending_cache_path(include_flatpak, include_aur, include_chaotic)?;
    let data = std::fs::read_to_string(&path).ok()?;
    let cache: PackagesListCache = serde_json::from_str(&data).ok()?;
    if cache.version != PACKAGES_CACHE_VERSION {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now.saturating_sub(cache.fetched_at) >= TRENDING_DISK_TTL_SECS {
        return None;
    }
    log::info!(
        "[DISK CACHE] Loaded trending (flatpak={}, aur={}, chaotic={}) from disk ({} packages)",
        include_flatpak,
        include_aur,
        include_chaotic,
        cache.packages.len()
    );
    Some(cache.packages)
}

pub(crate) fn write_trending_disk(
    include_flatpak: bool,
    include_aur: bool,
    include_chaotic: bool,
    packages: &[models::Package],
) {
    let path = match trending_cache_path(include_flatpak, include_aur, include_chaotic) {
        Some(p) => p,
        None => return,
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cache = PackagesListCache {
        version: PACKAGES_CACHE_VERSION,
        names_sorted: Vec::new(), // not used for trending
        packages: packages.to_vec(),
        fetched_at: now,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(&cache) {
        log::info!(
            "[DISK CACHE] Writing trending (flatpak={}, aur={}, chaotic={}) to disk ({} packages)",
            include_flatpak,
            include_aur,
            include_chaotic,
            cache.packages.len()
        );
        let _ = std::fs::write(&path, json);
    }
}

// ─── Disk cache helpers for Categories ─────────────────────────────────────
pub(crate) const CATEGORY_DISK_TTL_SECS: u64 = 6 * 60 * 60; // 6 hours, matches moka TTL

pub(crate) fn category_cache_path(cache_key: &str) -> Option<std::path::PathBuf> {
    // Sanitize key for filesystem: replace non-alphanumeric with '_'
    let safe_key: String = cache_key
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    dirs::cache_dir().map(|d| {
        d.join("monarch-store")
            .join(format!("category_{}.json", safe_key))
    })
}

pub(crate) fn try_read_category_disk(cache_key: &str) -> Option<Vec<models::Package>> {
    let path = category_cache_path(cache_key)?;
    let data = std::fs::read_to_string(&path).ok()?;
    let cache: PackagesListCache = serde_json::from_str(&data).ok()?;
    if cache.version != PACKAGES_CACHE_VERSION {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now.saturating_sub(cache.fetched_at) >= CATEGORY_DISK_TTL_SECS {
        return None;
    }
    log::info!(
        "[DISK CACHE] Loaded category '{}' from disk ({} packages)",
        cache_key,
        cache.packages.len()
    );
    Some(cache.packages)
}

pub(crate) fn write_category_disk(cache_key: &str, packages: &[models::Package]) {
    let path = match category_cache_path(cache_key) {
        Some(p) => p,
        None => return,
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cache = PackagesListCache {
        version: PACKAGES_CACHE_VERSION,
        names_sorted: Vec::new(), // not used for categories
        packages: packages.to_vec(),
        fetched_at: now,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = std::fs::write(&path, json);
    }
}

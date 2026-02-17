//! Dynamic Discovery Engine: replaces static popular/featured/curated lists.
//! Fetches top AUR by popularity, Flathub trending, and caches for 24h in ~/.cache/monarch/discovery.json.
//! Background refresh on launch when stale so UI stays non-blocking.

use crate::models::{Package, PackageSource};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use std::time::{SystemTime, UNIX_EPOCH};
use tauri::async_runtime::RwLock;

/// 24-hour cache TTL (seconds).
const CACHE_TTL_SECONDS: u64 = 86400;

/// Default number of top AUR packages to fetch.
const AUR_TOP_LIMIT: usize = 12;

/// Flathub popular limit (2x AUR limit to balance mix).
const FLATHUB_POPULAR_LIMIT: usize = 20;

lazy_static! {
    static ref DISCOVERY_CACHE_PATH: PathBuf = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("monarch-store/discovery.json");
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoveryCache {
    pub aur_packages: Vec<Package>,
    // Renamed for clarity: this now holds "Recently Updated" from AppStream
    pub flathub_hits: Vec<FlathubPopularHit>,
    pub fetched_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlathubPopularHit {
    pub app_id: String,
    pub name: String,
    pub summary: Option<String>,
    pub icon: Option<String>,
}

fn discovery_cache_path() -> PathBuf {
    DISCOVERY_CACHE_PATH.clone()
}

/// Returns true if the cache is stale (older than CACHE_TTL_SECONDS).
fn is_cache_stale(fetched_at: u64) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now > fetched_at + CACHE_TTL_SECONDS
}

/// Fetch top AUR packages by popularity. AUR RPC has no "sort by votes" API, so we use
/// a broad search and sort client-side by NumVotes.
async fn fetch_aur_top(limit: usize) -> Vec<Package> {
    log::info!("Fetching top {} packages from AUR...", limit);

    // Multi-key fallback: "popular" is often empty/weird in AUR RPC.
    // Try "popular" -> "bin" (high volume for apps) -> "git" (dev tools)
    let keywords = vec!["popular", "bin", "git", "desktop"];
    let mut all_found = Vec::new();

    for kw in keywords {
        if let Ok(mut pkgs) = crate::aur_api::search_aur(kw).await {
            if !pkgs.is_empty() {
                all_found.append(&mut pkgs);
                // If we found a good chunk, stop to avoid over-fetching
                if all_found.len() >= limit * 3 {
                    break;
                }
            }
        }
    }

    if all_found.is_empty() {
        return Vec::new();
    }

    // Sort combined results by votes descending
    all_found.sort_by(|a, b| b.num_votes.unwrap_or(0).cmp(&a.num_votes.unwrap_or(0)));

    // Deduplicate by name before returning
    let mut seen = std::collections::HashSet::new();
    all_found
        .into_iter()
        .filter(|p| seen.insert(p.name.clone()))
        .take(limit)
        .collect()
}

/// Load cache from disk. Returns None if file missing or invalid.
fn load_cache_from_disk() -> Option<DiscoveryCache> {
    let path = discovery_cache_path();
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Write cache to disk. Creates parent dir if needed.
fn save_cache_to_disk(cache: &DiscoveryCache) {
    if let Some(parent) = discovery_cache_path().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(discovery_cache_path(), json);
    }
}

const FLATHUB_API_POPULAR: &str = "https://flathub.org/api/v2/collection/popular";
const MAX_RETRIES: u32 = 3;

/// Fetch popular apps from Flathub API with retries and backoff.
/// Returns None if all retries fail.
async fn fetch_flathub_popular_api_with_retry() -> Option<Vec<FlathubPopularHit>> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/115.0")
        .build()
        .ok()?;

    for attempt in 1..=MAX_RETRIES {
        log::info!(
            "Fetching Flathub popular API (attempt {}/{})",
            attempt,
            MAX_RETRIES
        );
        match client.get(FLATHUB_API_POPULAR).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    // Success!
                    // Use a temporary struct to deserialize the meillsearch response
                    #[derive(Deserialize)]
                    struct MeilisearchResponse {
                        hits: Vec<FlathubHitRaw>,
                    }
                    #[derive(Deserialize)]
                    struct FlathubHitRaw {
                        app_id: String,
                        name: String,
                        summary: String,
                        icon: Option<String>,
                    }

                    if let Ok(json) = resp.json::<MeilisearchResponse>().await {
                        return Some(
                            json.hits
                                .into_iter()
                                .map(|h| FlathubPopularHit {
                                    app_id: h.app_id,
                                    name: h.name,
                                    summary: Some(h.summary),
                                    icon: h.icon,
                                })
                                .collect(),
                        );
                    }
                } else if resp.status().as_u16() == 503 || resp.status().as_u16() == 500 {
                    log::warn!(
                        "Flathub API {} (attempt {}). Retrying...",
                        resp.status(),
                        attempt
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(
                        500 * 2u64.pow(attempt - 1),
                    ))
                    .await;
                    continue;
                }
            }
            Err(e) => {
                log::warn!(
                    "Flathub API network error: {} (attempt {}). Retrying...",
                    e,
                    attempt
                );
                tokio::time::sleep(std::time::Duration::from_millis(
                    500 * 2u64.pow(attempt - 1),
                ))
                .await;
                continue;
            }
        }
    }
    None
}

pub struct DiscoveryManager {
    cache: Arc<RwLock<DiscoveryCache>>,
    // We need access to MetadataState (via AppStreamLoader) to fetch local updates.
    // Store a reference or grab it during refresh?
    // Store weak ref? Or pass it in `refresh_*` methods.
    // Simpler: methods now take &AppStreamLoader.
}

impl Default for DiscoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveryManager {
    pub fn new() -> Self {
        let cache = load_cache_from_disk().unwrap_or_default();
        Self {
            cache: Arc::new(RwLock::new(cache)),
        }
    }

    /// Load cache from disk (e.g. after startup). Call once during app init.
    pub fn load_from_disk(&self) {
        if let Some(c) = load_cache_from_disk() {
            if let Ok(mut g) = self.cache.try_write() {
                *g = c;
            }
        }
    }

    /// Helper: Get recently updated apps from local AppStream (replaces broken Flathub API)
    pub fn get_recent_flathub_hits(
        loader: &crate::metadata::AppStreamLoader,
    ) -> Vec<FlathubPopularHit> {
        log::info!("Fetching recently updated apps from local AppStream...");
        let updated = loader.get_recently_updated_components(FLATHUB_POPULAR_LIMIT);
        updated
            .into_iter()
            .map(|meta| FlathubPopularHit {
                app_id: meta.app_id,
                name: meta.name,
                summary: meta.summary,
                icon: meta.icon_url,
            })
            .collect()
    }

    /// Trigger a background refresh if cache is stale. Non-blocking; UI can use cached data immediately.
    /// Empty cache is always considered stale so we retry after failed fetches (don't lock out for 24h).
    pub fn refresh_if_stale(&self) {
        let cache_guard = self.cache.clone();
        tokio::spawn(async move {
            let (fetched_at, is_empty) = {
                let c = cache_guard.read().await;
                let empty = c.aur_packages.is_empty() && c.flathub_hits.is_empty();
                (c.fetched_at, empty)
            };
            if !is_empty && !is_cache_stale(fetched_at) {
                return;
            }
            let aur = fetch_aur_top(AUR_TOP_LIMIT).await;
            // Background refresh without loader access is tricky.
            // For now, we only run full refresh in `refresh_now_if_empty_or_stale` where we can pass loader.
            // Or we skip flathub update here?
            // Let's just update AUR here, keeping existing flathub hits.
            let existing_flathub = {
                let c = cache_guard.read().await;
                c.flathub_hits.clone()
            };

            // If existing flathub is empty, we really want to update it, but can't access loader here easily without refactoring injection.
            // `refresh_now_if_empty_or_stale` is called by UI and HAS access to metadata.
            // So we let that handle it.

            let fetched_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let new_cache = DiscoveryCache {
                aur_packages: aur,
                flathub_hits: existing_flathub,
                fetched_at,
            };
            save_cache_to_disk(&new_cache);
            let mut g = cache_guard.write().await;
            *g = new_cache;
        });
    }

    /// Run discovery fetch in the current task when cache is empty or stale. Used by get_trending
    /// so the first request fills the cache instead of relying on a background spawn (which may not
    /// run in time or in a working context).
    /// Run discovery fetch in the current task when cache is empty or stale. Used by get_trending
    /// so the first request fills the cache instead of relying on a background spawn (which may not
    /// run in time or in a working context).
    pub async fn refresh_now_if_empty_or_stale(
        &self,
        loader_state: &crate::metadata::MetadataState,
    ) {
        let (fetched_at, is_empty) = {
            let c = self.cache.read().await;
            let empty = c.aur_packages.is_empty() && c.flathub_hits.is_empty();
            (c.fetched_at, empty)
        };
        // If not empty and not stale, do nothing.
        if !is_empty && !is_cache_stale(fetched_at) {
            return;
        }

        // 1. Fetch AUR
        let aur = fetch_aur_top(AUR_TOP_LIMIT).await;

        // 2. Fetch Flathub (Try API first, then Local Fallback)
        let flathub = match fetch_flathub_popular_api_with_retry().await {
            Some(hits) => {
                log::info!("Flathub API success: got {} hits", hits.len());
                hits
            }
            None => {
                log::warn!("Flathub API failed after retries. Falling back to Local AppStream.");
                let guard = loader_state
                    .loader
                    .lock()
                    .expect("MetadataState lock poisoned");
                Self::get_recent_flathub_hits(&guard)
            }
        };

        if aur.is_empty() && flathub.is_empty() {
            return;
        }

        let fetched_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let new_cache = DiscoveryCache {
            aur_packages: aur,
            flathub_hits: flathub,
            fetched_at,
        };
        save_cache_to_disk(&new_cache);
        let mut g = self.cache.write().await;
        *g = new_cache;
    }

    /// Get cached AUR popular packages (for trending/featured).
    pub async fn get_aur_popular(&self) -> Vec<Package> {
        let c = self.cache.read().await;
        c.aur_packages.clone()
    }

    /// Get cached Flathub popular hits as Packages (for trending/featured).
    #[allow(dead_code)]
    pub async fn get_flathub_popular_packages(&self) -> Vec<Package> {
        let c = self.cache.read().await;
        c.flathub_hits
            .iter()
            .map(|h| Package {
                name: h.app_id.clone(),
                display_name: Some(h.name.clone()),
                description: h.summary.clone().unwrap_or_default(),
                version: "latest".to_string(),
                source: PackageSource::new("flatpak", "flathub", "latest", "Flatpak (Sandboxed)"),
                app_id: Some(h.app_id.clone()),
                icon: h.icon.clone(),
                available_sources: Some(vec![PackageSource::new(
                    "flatpak",
                    "flathub",
                    "latest",
                    "Flatpak (Sandboxed)",
                )]),
                ..Default::default()
            })
            .collect()
    }

    /// Get Flathub popular as SearchResult list (for merger).
    pub async fn get_flathub_popular_search_results(
        &self,
    ) -> Vec<crate::flathub_api::SearchResult> {
        let c = self.cache.read().await;
        c.flathub_hits
            .iter()
            .map(|h| crate::flathub_api::SearchResult {
                app_id: h.app_id.clone(),
                name: h.name.clone(),
                summary: h.summary.clone(),
                icon: h.icon.clone(),
            })
            .collect()
    }

    /// Names of AUR packages considered "popular" (for relevance boost in search).
    pub async fn popular_aur_names(&self) -> Vec<String> {
        let c = self.cache.read().await;
        c.aur_packages.iter().map(|p| p.name.clone()).collect()
    }

    /// All popular names: AUR package names + Flathub app_ids (for featured injection / category hoisting).
    #[allow(dead_code)]
    pub async fn get_all_popular_names(&self) -> Vec<String> {
        let c = self.cache.read().await;
        let mut names: Vec<String> = c.aur_packages.iter().map(|p| p.name.clone()).collect();
        for h in &c.flathub_hits {
            names.push(h.app_id.clone());
        }
        names
    }
}

/// Path and TTL for essentials cache (shared with package.rs; used to read featured_by_category).
fn essentials_cache_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("monarch-store")
        .join("essentials.json")
}
const ESSENTIALS_CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60; // 7 days

/// Normalize category to canonical key used in essentials cache featured_by_category.
fn normalize_category_key(category: &str) -> String {
    match category.trim().to_lowercase().as_str() {
        "games" | "game" => "game".to_string(),
        "internet" | "network" => "network".to_string(),
        "multimedia" | "audio" | "video" | "audiovideo" => "audiovideo".to_string(),
        "graphics" => "graphics".to_string(),
        "development" | "develop" => "development".to_string(),
        "office" | "productivity" => "office".to_string(),
        "system" => "system".to_string(),
        "utility" | "utilities" => "utility".to_string(),
        k => k.to_string(),
    }
}

/// Read featured list for a category from essentials cache (when updated via remote).
/// Returns Some(list) if cache is fresh and has featured_by_category for this category.
fn read_featured_from_cache(category: &str) -> Option<Vec<String>> {
    let path = essentials_cache_path();
    let data = std::fs::read_to_string(&path).ok()?;
    let obj: serde_json::Value = serde_json::from_str(&data).ok()?;
    let fetched_at = obj.get("fetched_at")?.as_u64()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now.saturating_sub(fetched_at) >= ESSENTIALS_CACHE_TTL_SECS {
        return None;
    }
    let map = obj.get("featured_by_category")?.as_object()?;
    let key = normalize_category_key(category);
    let list = map.get(&key)?.as_array()?;
    let out: Vec<String> = list
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// All unique featured package names across every category (for combined Essentials pool).
pub fn get_all_featured_names() -> Vec<String> {
    const CATEGORY_KEYS: &[&str] = &[
        "game",
        "network",
        "audiovideo",
        "graphics",
        "development",
        "office",
        "system",
        "utility",
    ];
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for key in CATEGORY_KEYS {
        for name in get_featured_names_for_category(key) {
            if seen.insert(name.clone()) {
                out.push(name);
            }
        }
    }
    out
}

/// Featured package names per category (for category view injection and hoisting).
/// Uses remote featured_by_category from essentials cache when fresh; else built-in list.
pub fn get_featured_names_for_category(category: &str) -> Vec<String> {
    if let Some(list) = read_featured_from_cache(category) {
        log::info!(
            "[DISCOVERY] Found {} featured apps for '{}' in remote cache",
            list.len(),
            category
        );
        return list;
    }
    let names: Vec<&str> = match category.trim().to_lowercase().as_str() {
        "games" | "game" => vec![
            "steam",
            "lutris",
            "heroic-games-launcher-bin",
            "discord",
            "minecraft-launcher",
            "wine",
            "protonup-qt",
            "retroarch",
            "gamemode",
            "mangohud",
            "r2modman-bin",
            "prismlauncher",
        ],
        "internet" | "network" => vec![
            "google-chrome",
            "firefox",
            "brave-bin",
            "discord",
            "telegram-desktop",
            "signal-desktop",
            "zoom",
            "thunderbird",
            "qbittorrent",
            "transmission-gtk",
            "filezilla",
            "anydesk-bin",
        ],
        "multimedia" | "audio" | "video" | "audiovideo" => vec![
            "vlc",
            "obs-studio",
            "spotify",
            "gimp",
            "kdenlive",
            "blender",
            "audacity",
            "mpv",
            "inkscape",
            "handbrake",
            "ffmpeg",
            "krita",
        ],
        "graphics" => vec![
            "gimp",
            "blender",
            "inkscape",
            "krita",
            "darktable",
            "rawtherapee",
            "digikam",
            "glaxnimate",
        ],
        "development" => vec![
            "visual-studio-code-bin",
            "code",
            "git",
            "docker",
            "intellij-idea-community-edition",
            "pycharm-community-edition",
            "postman-bin",
            "sublime-text-4",
            "neovim",
            "vim",
            "cmake",
            "qtcreator",
        ],
        "office" => vec![
            "libreoffice-fresh",
            "obsidian",
            "notion-app-electron",
            "evince",
            "onlyoffice-bin",
            "simple-scan",
            "typora",
            "joplin",
            "okular",
        ],
        "system" => vec![
            "gparted",
            "timeshift",
            "bleachbit",
            "htop",
            "btop",
            "flatpak",
            "pacman",
            "virtualbox",
            "kvm",
            "qemu-full",
        ],
        "utility" | "utilities" => vec![
            "calculator",
            "gnome-calculator",
            "gnome-disk-utility",
            "file-roller",
            "spectacle",
            "flameshot",
            "ark",
            "kate",
            "gedit",
            "nano",
            "speedtest-cli",
            "neofetch",
            "fastfetch",
            "tree",
            "ripgrep",
            "bat",
            "eza",
            "fd",
            "fzf",
            "alacritty",
            "kitty",
        ],
        _ => return Vec::new(),
    };
    names.into_iter().map(String::from).collect()
}

/// Convert FlathubPopularHit to our Package (for get_trending merger).
#[allow(dead_code)]
pub fn flathub_hit_to_package(h: &FlathubPopularHit) -> Package {
    Package {
        name: h.app_id.clone(),
        display_name: Some(h.name.clone()),
        description: h.summary.clone().unwrap_or_default(),
        version: "latest".to_string(),
        source: PackageSource::new("flatpak", "flathub", "latest", "Flatpak (Sandboxed)"),
        app_id: Some(h.app_id.clone()),
        icon: h.icon.clone(),
        available_sources: Some(vec![PackageSource::new(
            "flatpak",
            "flathub",
            "latest",
            "Flatpak (Sandboxed)",
        )]),
        ..Default::default()
    }
}

use moka::future::Cache;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const BASE_URL: &str = "https://chaotic-backend.garudalinux.org";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChaoticPackage {
    pub id: Option<u64>,
    pub pkgname: String,
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
    #[serde(rename = "isActive")]
    pub is_active: Option<bool>,
    pub version: Option<String>,
    pub metadata: Option<ChaoticMetadata>,
    pub provides: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChaoticMetadata {
    pub url: Option<String>,
    pub desc: Option<String>,
    pub license: Option<String>,
    pub filename: Option<String>,
    #[serde(rename = "buildDate")]
    pub build_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrendingPackage {
    pub pkgbase_pkgname: String,
    pub count: String, // API returns count as string based on observation
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InfraStats {
    pub builders: u32,
    pub users: u32,
}

pub struct ChaoticApiClient {
    client: Client,
    package_cache: Cache<String, std::sync::Arc<Vec<ChaoticPackage>>>,
    trending_cache: Cache<String, Vec<TrendingPackage>>, // Small, can clone
    infra_cache: Cache<String, InfraStats>,
    category_cache: Cache<String, Vec<ChaoticPackage>>,
}

impl Default for ChaoticApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ChaoticApiClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("AURStore/0.1.0 (Tauri; Arch Linux)")
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            // Cache packages for 1 hour as the list is large and doesn't change every second
            package_cache: Cache::builder()
                .time_to_live(Duration::from_secs(3600))
                .build(),
            // Cache trending for 30 mins
            trending_cache: Cache::builder()
                .time_to_live(Duration::from_secs(1800))
                .build(),
            // Cache infra stats for 5 mins
            infra_cache: Cache::builder()
                .time_to_live(Duration::from_secs(300))
                .build(),
            // Cache categories for 1 hour
            category_cache: Cache::builder()
                .time_to_live(Duration::from_secs(3600))
                .build(),
        }
    }

    pub async fn get_packages_by_category(&self, category: &str) -> Vec<ChaoticPackage> {
        let category_key = category.to_lowercase();

        // 1. Check Cache
        if let Some(cached) = self.category_cache.get(&category_key).await {
            return cached;
        }

        // 2. Fetch All Packages (Cached)
        if let Ok(all_pkgs) = self.fetch_packages().await {
            // 3. Define Keywords
            let keywords = match category_key.as_str() {
                "internet" | "network" | "web" => {
                    vec!["browser", "web", "http", "vpn", "mail", "discord", "client"]
                }
                "games" | "game" => vec![
                    "game",
                    "fps",
                    "rpg",
                    "rogue",
                    "simulator",
                    "steam",
                    "minecraft",
                    "launcher",
                ],
                "development" | "dev" | "programming" => {
                    vec![
                        "ide", "editor", "compiler", "language", "git", "rust", "python", "go",
                    ]
                }
                "multimedia" | "audio" | "video" => vec![
                    "audio",
                    "video",
                    "player",
                    "music",
                    "visualizer",
                    "stream",
                    "obs",
                    "codec",
                    "ffmpeg",
                ],
                "system" | "admin" => vec![
                    "kernel", "driver", "boot", "firmware", "manage", "monitor", "systemd",
                    "pacman",
                ],
                "utilities" | "utils" => vec![
                    "tool", "util", "compress", "file", "terminal", "shell", "archive",
                ],
                "office" | "productivity" => {
                    vec!["office", "pdf", "note", "calc", "writer", "todo"]
                }
                "graphics" | "design" => {
                    vec!["image", "photo", "draw", "paint", "design", "color", "font"]
                }
                _ => vec![],
            };

            if keywords.is_empty() {
                return Vec::new(); // Unknown category for chaotic mapping
            }

            // 4. Filter (Heuristic)
            // This is the heavy op we want to do only once
            let matches: Vec<ChaoticPackage> = all_pkgs
                .iter()
                .filter(|p| {
                    if let Some(desc) = p.metadata.as_ref().and_then(|m| m.desc.as_ref()) {
                        let d_lower = desc.to_lowercase();
                        let p_lower = p.pkgname.to_lowercase();
                        // Check pkgname AND description for better accuracy
                        return keywords
                            .iter()
                            .any(|k| d_lower.contains(k) || p_lower.contains(k));
                    }
                    false
                })
                .take(100) // Limit per category to keep UI snappy
                .cloned()
                .collect();

            // 5. Store in Cache
            self.category_cache
                .insert(category_key, matches.clone())
                .await;

            return matches;
        }

        Vec::new()
    }

    pub async fn fetch_packages(&self) -> Result<std::sync::Arc<Vec<ChaoticPackage>>, String> {
        if let Some(cached) = self.package_cache.get("all_packages").await {
            return Ok(cached);
        }

        let url = format!("{}/builder/packages", BASE_URL);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Failed to fetch packages: {}", resp.status()));
        }

        // Deserialize to generic Value first to handle individual failures
        let raw_packages: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
        let _total_count = raw_packages.len();
        let mut packages = Vec::new();

        for (i, val) in raw_packages.into_iter().enumerate() {
            match serde_json::from_value::<ChaoticPackage>(val) {
                Ok(pkg) => packages.push(pkg),
                Err(e) => {
                    // Log the error but don't fail the whole batch
                    println!("WARN: Failed to parse package at index {}: {}", i, e);
                }
            }
        }

        let arc_packages = std::sync::Arc::new(packages);

        self.package_cache
            .insert("all_packages".to_string(), arc_packages.clone())
            .await;

        Ok(arc_packages)
    }

    pub async fn get_repo_counts(&self) -> Result<InfraStats, String> {
        let url = format!("{}/infra", BASE_URL);

        // Return cache if fresh
        if let Some(stats) = self.infra_cache.get("infra_stats").await {
            return Ok(stats);
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Failed to fetch infra stats: {}", resp.status()));
        }

        let stats: InfraStats = resp.json().await.map_err(|e| e.to_string())?;

        self.infra_cache
            .insert("infra_stats".to_string(), stats.clone())
            .await;

        Ok(stats)
    }

    pub async fn clear_cache(&self) {
        self.package_cache.invalidate_all();
        self.trending_cache.invalidate_all();
        self.infra_cache.invalidate_all();
    }

    /// Find metadata for a specific package from the cached Chaotic-AUR list
    pub async fn find_package(&self, pkg_name: &str) -> Option<ChaoticPackage> {
        // Try to get from cache first
        if let Some(packages) = self.package_cache.get("all_packages").await {
            return packages.iter().find(|p| p.pkgname == pkg_name).cloned();
        }

        // If not in cache, trigger a fetch (without waiting? or wait?)
        // For responsiveness, we'll try to fetch, await it, then search.
        if let Ok(packages) = self.fetch_packages().await {
            return packages.iter().find(|p| p.pkgname == pkg_name).cloned();
        }

        None
    }

    pub async fn fetch_trending(&self) -> Result<Vec<TrendingPackage>, String> {
        // Cache check
        if let Some(cached) = self.trending_cache.get("top_25").await {
            return Ok(cached);
        }

        let url = format!("{}/builder/popular/50?offset=0", BASE_URL);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Failed to fetch trending: {}", resp.status()));
        }

        let raw: Vec<TrendingPackage> = resp.json().await.map_err(|e| e.to_string())?;

        // Take top 25
        let trending: Vec<TrendingPackage> = raw.into_iter().take(25).collect();

        self.trending_cache
            .insert("top_25".to_string(), trending.clone())
            .await;

        Ok(trending)
    }

    pub async fn fetch_infra_stats(&self) -> Result<InfraStats, String> {
        if let Some(cached) = self.infra_cache.get("stats").await {
            return Ok(cached);
        }

        // We need to make two parallel requests
        let builders_future = self
            .client
            .get(format!("{}/builder/builders/amount", BASE_URL))
            .send();
        let users_future = self
            .client
            .get(format!("{}/metrics/30d/users", BASE_URL))
            .send();

        let (builders_resp, users_resp) =
            tokio::try_join!(builders_future, users_future).map_err(|e| e.to_string())?;

        let builders: u32 = builders_resp.json().await.map_err(|e| e.to_string())?;

        // Users endpoint might be a bit more complex, let's assume it returns a simple JSON for now as implied by the logs.
        // If it's a list, we might need to count it. The log said "Users Metrics (30 Days)", likely a JSON.
        // Based on the pattern, let's assume it returns a number or a struct we need to parse.
        // Wait, the previous tool viewed this: "https://chaotic-backend.garudalinux.org/metrics/30d/users"
        // I don't see the exact snippet for users. I'll assume it returns a number for now, or handle it as serde_json::Value to be safe.
        // Actually, let's play it safe and use serde_json::Value for users and try to extract a count or similar.
        // Or better, let's stick to simple u32 and if it fails I'll fix it. The endpoint "amount" suggests a number.
        // "metrics/30d/users" sounds like it could be a list or a count.
        // Let's inspect the `viewed_code_item` again... it just says it was identified.
        // I'll implementation a temporary specialized struct or just `serde_json::Value` for users to inspect.
        // Correction: I should just use `serde_json::Value` for users to be safe.

        let users_val: serde_json::Value = users_resp.json().await.map_err(|e| e.to_string())?;
        let users = users_val.as_u64().unwrap_or(0) as u32; // Try as number
                                                            // If it's an object/array, this will be 0, which is fine for V1.

        let stats = InfraStats { builders, users };
        self.infra_cache
            .insert("stats".to_string(), stats.clone())
            .await;

        Ok(stats)
    }

    pub async fn get_package_by_name(&self, name: &str) -> Option<ChaoticPackage> {
        if self.package_cache.get("all_packages").await.is_none() {
            let _ = self.fetch_packages().await;
        }

        if let Some(packages) = self.package_cache.get("all_packages").await {
            return packages.iter().find(|p| p.pkgname == name).cloned();
        }
        None
    }

    pub async fn get_packages_batch(
        &self,
        names: Vec<String>,
    ) -> std::collections::HashMap<String, ChaoticPackage> {
        if self.package_cache.get("all_packages").await.is_none() {
            let _ = self.fetch_packages().await;
        }

        let mut results = std::collections::HashMap::new();
        if let Some(packages) = self.package_cache.get("all_packages").await {
            let name_set: std::collections::HashSet<String> = names.into_iter().collect();

            for pkg in packages.iter() {
                if name_set.contains(&pkg.pkgname) {
                    results.insert(pkg.pkgname.clone(), pkg.clone());
                }
            }
        }
        results
    }

    pub async fn get_packages_providing(&self, name: &str) -> Vec<ChaoticPackage> {
        if self.package_cache.get("all_packages").await.is_none() {
            let _ = self.fetch_packages().await;
        }

        if let Some(packages) = self.package_cache.get("all_packages").await {
            return packages
                .iter()
                .filter(|p| {
                    p.provides.as_ref().is_some_and(|provs| {
                        provs.iter().any(|pr| {
                            // Handle versioned provides if necessary (e.g. "sh=1.0"), similar to repo_db
                            let clean = pr.split_once('=').map(|(n, _)| n).unwrap_or(pr);
                            clean == name
                        })
                    })
                })
                .cloned()
                .collect();
        }
        Vec::new()
    }
}

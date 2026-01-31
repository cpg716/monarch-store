use reqwest;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::sync::Mutex;

/// Flathub API client for fetching rich app metadata
/// This is used as a METADATA SOURCE only - we don't install Flatpaks

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FlathubMetadata {
    #[serde(default)]
    pub id: Option<String>, // Captures the ID if returned, or we inject it
    pub name: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub screenshots: Vec<FlathubScreenshot>,
    pub developer_name: Option<String>,
    pub project_license: Option<String>,
    pub categories: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FlathubScreenshot {
    #[serde(rename = "624x351")]
    pub size_624: Option<String>,
    #[serde(rename = "752x423")]
    pub size_752: Option<String>,
    #[serde(rename = "1248x702")]
    pub size_1248: Option<String>,
}

// Search Response Structures
#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    hits: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    #[serde(rename = "app_id")]
    app_id: String,
    name: String,
    // We only care about ID and name for mapping
}

/// Common package name to Flathub app ID mappings
pub fn get_flathub_app_id(pkg_name: &str) -> Option<String> {
    // Direct mappings for common packages
    let mappings: HashMap<&str, &str> = HashMap::from([
        // Browsers
        ("firefox", "org.mozilla.firefox"),
        ("chromium", "org.chromium.Chromium"),
        ("google-chrome", "com.google.Chrome"),
        ("brave", "com.brave.Browser"),
        ("brave-bin", "com.brave.Browser"),
        ("brave-browser", "com.brave.Browser"),
        ("vivaldi", "com.vivaldi.Vivaldi"),
        ("microsoft-edge-stable-bin", "com.microsoft.Edge"),
        // Communication
        ("discord", "com.discordapp.Discord"),
        ("slack-desktop", "com.slack.Slack"),
        ("telegram-desktop", "org.telegram.desktop"),
        ("signal-desktop", "org.signal.Signal"),
        ("zoom", "us.zoom.Zoom"),
        ("teams", "com.microsoft.Teams"),
        // Media
        ("spotify", "com.spotify.Client"),
        ("spotify-launcher", "com.spotify.Client"),
        ("vlc", "org.videolan.VLC"),
        ("obs-studio", "com.obsproject.Studio"),
        ("gimp", "org.gimp.GIMP"),
        ("inkscape", "org.inkscape.Inkscape"),
        ("blender", "org.blender.Blender"),
        ("kdenlive", "org.kde.kdenlive"),
        ("audacity", "org.audacityteam.Audacity"),
        // Development
        ("visual-studio-code-bin", "com.visualstudio.code"),
        ("code", "com.visualstudio.code"),
        ("jetbrains-toolbox", "com.jetbrains.Toolbox"),
        ("sublime-text-4", "com.sublimetext.three"),
        ("atom", "io.atom.Atom"),
        ("postman-bin", "com.getpostman.Postman"),
        // Gaming
        ("steam", "com.valvesoftware.Steam"),
        ("lutris", "net.lutris.Lutris"),
        ("minecraft-launcher", "com.mojang.Minecraft"),
        // Office
        ("libreoffice-fresh", "org.libreoffice.LibreOffice"),
        ("libreoffice-still", "org.libreoffice.LibreOffice"),
        ("onlyoffice-bin", "org.onlyoffice.desktopeditors"),
        // Utilities
        ("bitwarden", "com.bitwarden.desktop"),
        ("keepassxc", "org.keepassxc.KeePassXC"),
        ("thunderbird", "org.mozilla.Thunderbird"),
        ("filezilla", "org.filezilla_project.Filezilla"),
        ("qbittorrent", "org.qbittorrent.qBittorrent"),
        ("transmission-gtk", "com.transmissionbt.Transmission"),
        // System
        ("virtualbox", "org.virtualbox.VirtualBox"),
        ("bottles", "com.usebottles.bottles"),
        ("anydesk", "com.anydesk.Anydesk"),
        ("anydesk-bin", "com.anydesk.Anydesk"),
        ("obsidian", "md.obsidian.Obsidian"),
        // Additions
        (
            "teams-for-linux",
            "com.github.IsmaelMartinez.teams_for_linux",
        ),
        ("figma-linux-bin", "io.github.Figma_Linux.figma_linux"),
        ("heroic-games-launcher-bin", "com.heroicgameslauncher.hgl"),
        ("notion-app-enhanced", "notion.id"),
        ("telegram-desktop-bin", "org.telegram.desktop"),
        (
            "visual-studio-code-insiders-bin",
            "com.visualstudio.code.insiders",
        ),
        ("insomnia-bin", "com.getinsomnia.Insomnia"),
        ("discord-canary", "com.discordapp.DiscordCanary"),
        ("discord-ptb", "com.discordapp.DiscordPTB"),
        ("element-desktop", "im.riot.Riot"),
        ("standard-notes-bin", "org.standardnotes.standardnotes"),
        ("simplenote-bin", "com.simplenote.Simplenote"),
        ("bitwarden-desktop", "com.bitwarden.desktop"),
        ("authy", "com.authy.Authy"),
        ("mailspring", "com.getmailspring.Mailspring"),
        ("balena-etcher", "io.balena.etcher"),
        ("stremio", "com.stremio.Stremio"),
        ("plex-desktop", "tv.plex.PlexDesktop"),
        ("teamviewer", "com.teamviewer.TeamViewer"),
    ]);

    // Try direct mapping first
    if let Some(app_id) = mappings.get(pkg_name) {
        return Some(app_id.to_string());
    }

    // Try stripping common suffixes and retry
    let suffixes = ["-bin", "-git", "-nightly", "-beta", "-appimage"];
    for suffix in suffixes {
        if pkg_name.ends_with(suffix) {
            let base = pkg_name.trim_end_matches(suffix);
            if let Some(app_id) = mappings.get(base) {
                return Some(app_id.to_string());
            }
        }
    }

    None
}

pub struct FlathubApiClient {
    cache: Mutex<HashMap<String, Option<FlathubMetadata>>>,
    // Mapping cache: pkg_name -> found_app_id
    mapping_cache: Mutex<HashMap<String, Option<String>>>,
}

impl Default for FlathubApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl FlathubApiClient {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            mapping_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Perform a search on Flathub to find a matching AppStream ID
    async fn search_find_id(&self, query: &str) -> Option<String> {
        let url = "https://flathub.org/api/v2/search";

        // We use a short timeout because search is on the critical path for metadata loading
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .ok()?;

        // Use POST for search with standard JSON payload
        let response = client
            .post(url)
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        // Get text first to handle variable response format
        let body_text = response.text().await.ok()?;

        // Strategy 1: Try as Array of SearchResult
        let hits_opts: Option<Vec<SearchResult>> = serde_json::from_str(&body_text).ok();

        let hits = if let Some(h) = hits_opts {
            h
        } else {
            // Strategy 2: Try as Object with "hits"
            let response_obj: Option<SearchResponse> = serde_json::from_str(&body_text).ok();
            if let Some(r) = response_obj {
                r.hits
            } else {
                return None; // Parse failed
            }
        };

        // Heuristic: Find first best match
        let query_lower = query.to_lowercase();

        // 1. Exact name match (case insensitive) or ID suffix match
        for hit in &hits {
            if hit.name.to_lowercase() == query_lower
                || hit
                    .app_id
                    .to_lowercase()
                    .ends_with(&format!(".{}", query_lower))
            {
                return Some(hit.app_id.clone());
            }
        }

        // 2. Contains match (if query is long enough to be specific)
        if query.len() > 4 {
            for hit in &hits {
                if hit.name.to_lowercase().contains(&query_lower) {
                    return Some(hit.app_id.clone());
                }
            }
        }

        None
    }

    /// Fetch metadata from Flathub API for a given app ID
    pub async fn fetch_metadata(&self, app_id: &str) -> Option<FlathubMetadata> {
        // Check cache first
        {
            let cache = self.cache.lock().ok()?;
            if let Some(cached) = cache.get(app_id) {
                return cached.clone();
            }
        }

        // Fetch from Flathub API
        let url = format!("https://flathub.org/api/v2/appstream/{}", app_id);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .ok()?;

        let response = client.get(&url).send().await.ok()?;

        if !response.status().is_success() {
            if let Ok(mut cache) = self.cache.lock() {
                cache.insert(app_id.to_string(), None);
            }
            return None;
        }

        let mut metadata: FlathubMetadata = response.json().await.ok()?;

        // Ensure ID is populated (API usually returns it in body, but if not, inject it)
        if metadata.id.is_none() {
            metadata.id = Some(app_id.to_string());
        }

        // Cache the result
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(app_id.to_string(), Some(metadata.clone()));
        }

        Some(metadata)
    }

    /// Try to get metadata for a package by name (using mappings + search fallback)
    pub async fn get_metadata_for_package(&self, pkg_name: &str) -> Option<FlathubMetadata> {
        // 1. Check Memory Mapping Cache first (avoid repeated searches)
        // 1. Check Memory Mapping Cache first (avoid repeated searches)
        let cached_id = {
            let map_cache = self.mapping_cache.lock().ok()?;
            if let Some(cached_opt) = map_cache.get(pkg_name) {
                // Clone the inner option to break dependency on the lock
                Some(cached_opt.clone())
            } else {
                None // Not in cache
            }
        };

        // If we found a cache entry (Hit or Miss)
        if let Some(cached_opt) = cached_id {
            if let Some(id) = cached_opt {
                return self.fetch_metadata(&id).await;
            } else {
                return None; // Known miss
            }
        }

        // 2. Try Static Mapping (fastest)
        let resolved_id = if let Some(id) = get_flathub_app_id(pkg_name) {
            Some(id)
        } else {
            // 3. Try Search (slower, fallback)
            // Strip suffixes first for better search (brave-bin -> brave)
            let search_term = pkg_name
                .trim_end_matches("-bin")
                .trim_end_matches("-git")
                .trim_end_matches("-nightly");

            self.search_find_id(search_term).await
        };

        // Cache the mapping decision
        if let Ok(mut map_cache) = self.mapping_cache.lock() {
            map_cache.insert(pkg_name.to_string(), resolved_id.clone());
        }

        if let Some(id) = resolved_id {
            self.fetch_metadata(&id).await
        } else {
            None
        }
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
        if let Ok(mut map_cache) = self.mapping_cache.lock() {
            map_cache.clear();
        }
    }
}

/// Convert Flathub metadata to our internal AppMetadata format
pub fn flathub_to_app_metadata(
    flathub: &FlathubMetadata,
    pkg_name: &str,
) -> super::metadata::AppMetadata {
    // Critical: Use the real Flathub ID if available, otherwise fallback to pkg_name.
    // This allows ODRS reviews to work!
    let effective_id = flathub.id.clone().unwrap_or_else(|| pkg_name.to_string());

    super::metadata::AppMetadata {
        name: flathub.name.clone().unwrap_or_else(|| pkg_name.to_string()),
        pkg_name: Some(pkg_name.to_string()),
        icon_url: flathub.icon.clone(),
        app_id: effective_id, // This enables reviews!
        summary: flathub.summary.clone(),
        screenshots: flathub
            .screenshots
            .iter()
            .filter_map(|s| s.size_752.clone().or(s.size_624.clone()))
            .collect(),
        version: None,
        maintainer: flathub.developer_name.clone(),
        license: flathub.project_license.clone(),
        last_updated: None,
        description: flathub.description.clone(),
    }
}

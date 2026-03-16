use crate::models::{PackageSource, UpdateItem};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;

pub static ACTIVE_FLATPAK_CHILD: Lazy<Arc<TokioMutex<Option<tokio::process::Child>>>> =
    Lazy::new(|| Arc::new(TokioMutex::new(None)));

fn parse_human_size_to_bytes(input: &str) -> Option<u64> {
    let cleaned = input
        .replace('\u{a0}', " ")
        .replace(',', "")
        .trim()
        .to_string();
    let mut parts = cleaned.split_whitespace();
    let value = parts.next()?.parse::<f64>().ok()?;
    let unit = parts.next().unwrap_or("B").to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "b" | "bytes" => 1_f64,
        "kb" => 1000_f64,
        "mb" => 1000_f64.powi(2),
        "gb" => 1000_f64.powi(3),
        "tb" => 1000_f64.powi(4),
        "kib" => 1024_f64,
        "mib" => 1024_f64.powi(2),
        "gib" => 1024_f64.powi(3),
        "tib" => 1024_f64.powi(4),
        _ => return None,
    };
    Some((value * multiplier).round() as u64)
}

pub async fn get_remote_app_sizes(
    app_id: &str,
    remote: &str,
) -> Result<(Option<u64>, Option<u64>), String> {
    let flatpak = flatpak_binary()?;
    let output = tokio::process::Command::new(&flatpak)
        .args(["remote-info", "--show-size", remote, app_id])
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FLATPAK_NOT_INSTALLED_MSG.to_string()
            } else {
                format!("Failed to run flatpak remote-info: {}", e)
            }
        })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut download_size = None;
    let mut installed_size = None;

    for line in stdout.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some((_, value)) = line.split_once(':') {
            if lower.contains("download") {
                download_size = parse_human_size_to_bytes(value.trim()).or(download_size);
            } else if lower.contains("installed") {
                installed_size = parse_human_size_to_bytes(value.trim()).or(installed_size);
            }
        }
    }

    Ok((download_size, installed_size))
}

/// Abort the active Flatpak command if one is running.
pub async fn abort_flatpak() -> Result<(), String> {
    let mut guard = ACTIVE_FLATPAK_CHILD.lock().await;
    if let Some(mut child) = guard.take() {
        match child.kill().await {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::InvalidInput {
                    // Already exited
                    Ok(())
                } else {
                    Err(format!("Failed to kill flatpak process: {}", e))
                }
            }
        }
    } else {
        Ok(())
    }
}

/// Fetch available Flatpak updates by parsing `flatpak remote-ls --updates`
pub async fn get_updates() -> Result<Vec<UpdateItem>, String> {
    let flatpak = flatpak_binary()?;
    let output = tokio::process::Command::new(&flatpak)
        .args([
            "remote-ls",
            "--updates",
            "--app",
            "--columns=application,version,installed-size,name",
        ])
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FLATPAK_NOT_INSTALLED_MSG.to_string()
            } else {
                format!("Failed to run flatpak: {}", e)
            }
        })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut updates = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        // Expected columns: application (ID), version, installed-size, name
        // Sometimes version is missing or size is different format, but standard columns help.
        // flatpak remote-ls output is tab-separated with --columns.

        if parts.len() >= 3 {
            let app_id = parts[0].trim().to_string();
            let new_version = parts[1].trim().to_string();
            let size_str = parts[2].trim();
            // Convention: We use the App ID as the 'name' for the UpdateItem so the execution engine
            // knows exactly what to update. Display names can be fetched from metadata if needed.
            // parts[0] is the Application ID.
            let name = app_id.clone();

            let size = size_str.parse::<u64>().ok();

            updates.push(UpdateItem {
                name,
                current_version: "Unknown".to_string(), // Filled below
                new_version: new_version.clone(),
                source: PackageSource::new(
                    "flatpak",
                    "flathub",
                    &new_version,
                    "Flatpak (Sandboxed)",
                ),
                size,
                icon: None,
                display_name: None,
            });
        }
    }

    // Optimization: Fetch current versions to fill in the gaps
    if !updates.is_empty() {
        let installed = get_installed_versions().await.unwrap_or_default();
        for update in &mut updates {
            if let Some(ver) = installed.get(&update.name) {
                update.current_version = ver.clone();
            }
        }
    }

    Ok(updates)
}

/// Helper to get installed versions for comparison
async fn get_installed_versions() -> Result<HashMap<String, String>, String> {
    let flatpak = flatpak_binary()?;
    let output = tokio::process::Command::new(&flatpak)
        .args(["list", "--app", "--columns=application,version"])
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FLATPAK_NOT_INSTALLED_MSG.to_string()
            } else {
                e.to_string()
            }
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut map = HashMap::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            map.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
        }
    }
    Ok(map)
}

/// Returns the list of installed Flatpak application IDs (for install-status and launch resolution).
pub async fn get_installed_flatpak_app_ids() -> Result<Vec<String>, String> {
    let ids = get_installed_flatpaks_detailed()
        .await?
        .into_iter()
        .map(|p| p.app_id)
        .collect();
    Ok(ids)
}

#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct InstalledFlatpak {
    pub app_id: String,
    pub name: String,
    pub version: String,
    pub summary: String,
    pub origin: String,
}

pub async fn get_installed_flatpaks_detailed() -> Result<Vec<InstalledFlatpak>, String> {
    let flatpak = flatpak_binary()?;
    let output = tokio::process::Command::new(&flatpak)
        .args([
            "list",
            "--app",
            "--columns=application,name,version,description,origin",
        ])
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FLATPAK_NOT_INSTALLED_MSG.to_string()
            } else {
                e.to_string()
            }
        })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 5 {
            results.push(InstalledFlatpak {
                app_id: parts[0].trim().to_string(),
                name: parts[1].trim().to_string(),
                version: parts[2].trim().to_string(),
                summary: parts[3].trim().to_string(),
                origin: parts[4].trim().to_string(),
            });
        }
    }
    Ok(results)
}

/// Flathub API client for fetching rich app metadata
/// This is used as a METADATA SOURCE only - we don't install Flatpaks

#[derive(Debug, Serialize, Deserialize, Clone, Type)]
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
    #[serde(default)]
    pub releases: Option<Vec<FlathubRelease>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct FlathubRelease {
    pub version: Option<String>,
    pub timestamp: Option<String>,
}

/// Legacy format: flat keys "624x351", "752x423", "1248x702" (some older API responses).
#[derive(Debug, Serialize, Deserialize, Clone, Default, Type)]
pub struct FlathubScreenshot {
    #[serde(rename = "624x351", default)]
    pub size_624: Option<String>,
    #[serde(rename = "752x423", default)]
    pub size_752: Option<String>,
    #[serde(rename = "1248x702", default)]
    pub size_1248: Option<String>,
    /// Flathub API v2 appstream format: array of { width, height, scale?, src }.
    #[serde(default)]
    pub sizes: Option<Vec<FlathubScreenshotSize>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct FlathubScreenshotSize {
    pub width: String,
    pub height: String,
    #[serde(default)]
    pub scale: Option<String>,
    pub src: String,
}

/// Prefer 1248, then 752, then 624, then largest available. One URL per screenshot entry.
pub(crate) fn screenshot_urls_from_flathub(screenshots: &[FlathubScreenshot]) -> Vec<String> {
    let preferred_widths = [1248, 752, 624];
    screenshots
        .iter()
        .filter_map(|s| {
            if let Some(ref sizes) = s.sizes {
                let mut by_width: Vec<(u32, &str)> = sizes
                    .iter()
                    .filter_map(|sz| sz.width.parse::<u32>().ok().map(|w| (w, sz.src.as_str())))
                    .collect();
                by_width.sort_by_key(|(w, _)| std::cmp::Reverse(*w));
                let url = preferred_widths
                    .iter()
                    .find_map(|&pw| {
                        by_width
                            .iter()
                            .find(|(w, _)| *w == pw)
                            .map(|(_, u)| u.to_string())
                    })
                    .or_else(|| by_width.first().map(|(_, u)| u.to_string()));
                url
            } else {
                s.size_1248
                    .clone()
                    .or_else(|| s.size_752.clone())
                    .or_else(|| s.size_624.clone())
            }
        })
        .collect()
}

// Search Response Structures
#[derive(Debug, Deserialize, Clone, Type)]
pub struct SearchResponse {
    #[serde(default)]
    pub hits: Vec<SearchResult>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Type)]
pub struct SearchResult {
    #[serde(rename = "app_id")]
    pub app_id: String,
    pub name: String,
    pub summary: Option<String>,
    pub icon: Option<String>,
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
        ("lutris-ge", "net.lutris.Lutris"),
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
        ("heroic", "com.heroicgameslauncher.hgl"),
        ("figma-linux-bin", "io.github.Figma_Linux.figma_linux"),
        ("heroic-games-launcher", "com.heroicgameslauncher.hgl"),
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

/// Reverse lookup: app_id -> package name. Used for resolve_package_name and ODRS identity.
/// Iterates the same mappings as get_flathub_app_id (pkg -> app_id) in reverse.
pub fn get_package_name_from_app_id(app_id: &str) -> Option<String> {
    let id_lower = app_id.trim().to_lowercase();
    if id_lower.is_empty() {
        return None;
    }
    // Same (pkg, app_id) pairs as get_flathub_app_id - iterate and match
    let pairs: &[(&str, &str)] = &[
        ("firefox", "org.mozilla.firefox"),
        ("chromium", "org.chromium.Chromium"),
        ("google-chrome", "com.google.Chrome"),
        ("brave", "com.brave.Browser"),
        ("vivaldi", "com.vivaldi.Vivaldi"),
        ("microsoft-edge-stable-bin", "com.microsoft.Edge"),
        ("discord", "com.discordapp.Discord"),
        ("slack-desktop", "com.slack.Slack"),
        ("telegram-desktop", "org.telegram.desktop"),
        ("signal-desktop", "org.signal.Signal"),
        ("zoom", "us.zoom.Zoom"),
        ("teams", "com.microsoft.Teams"),
        ("spotify", "com.spotify.Client"),
        ("vlc", "org.videolan.VLC"),
        ("obs-studio", "com.obsproject.Studio"),
        ("gimp", "org.gimp.GIMP"),
        ("inkscape", "org.inkscape.Inkscape"),
        ("blender", "org.blender.Blender"),
        ("kdenlive", "org.kde.kdenlive"),
        ("audacity", "org.audacityteam.Audacity"),
        ("visual-studio-code-bin", "com.visualstudio.code"),
        ("code", "com.visualstudio.code"),
        ("steam", "com.valvesoftware.Steam"),
        ("lutris", "net.lutris.Lutris"),
        ("minecraft-launcher", "com.mojang.Minecraft"),
        ("libreoffice-fresh", "org.libreoffice.LibreOffice"),
        ("libreoffice-still", "org.libreoffice.LibreOffice"),
        ("onlyoffice-bin", "org.onlyoffice.desktopeditors"),
        ("bitwarden", "com.bitwarden.desktop"),
        ("keepassxc", "org.keepassxc.KeePassXC"),
        ("thunderbird", "org.mozilla.Thunderbird"),
        ("filezilla", "org.filezilla_project.Filezilla"),
        ("qbittorrent", "org.qbittorrent.qBittorrent"),
        ("transmission-gtk", "com.transmissionbt.Transmission"),
        ("virtualbox", "org.virtualbox.VirtualBox"),
        ("heroic-games-launcher-bin", "com.heroicgameslauncher.hgl"),
        ("element-desktop", "im.riot.Riot"),
        (
            "teams-for-linux",
            "com.github.IsmaelMartinez.teams_for_linux",
        ),
    ];
    for (pkg, id) in pairs {
        let id_l = id.to_lowercase();
        if id_lower == id_l || id_lower == format!("{}.desktop", id_l) {
            return Some((*pkg).to_string());
        }
    }
    None
}

pub struct FlathubApiClient {
    cache: Mutex<HashMap<String, Option<FlathubMetadata>>>,
    // Mapping cache: pkg_name -> found_app_id
    mapping_cache: Mutex<HashMap<String, Option<String>>>,
    // Search cache: query -> search results (avoids repeated HTTP calls for same query)
    search_cache: Mutex<HashMap<String, Option<Vec<SearchResult>>>>,
}

impl Default for FlathubApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl FlathubApiClient {
    pub fn new() -> Self {
        let mut client = Self {
            cache: Mutex::new(HashMap::new()),
            mapping_cache: Mutex::new(HashMap::new()),
            search_cache: Mutex::new(HashMap::new()),
        };
        client.load_search_cache();
        client
    }

    fn load_search_cache(&mut self) {
        if let Some(cache_dir) = dirs::cache_dir() {
            let path = cache_dir
                .join("monarch-store")
                .join("flathub_search_cache.json");
            if path.exists() {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(json) =
                        serde_json::from_str::<HashMap<String, Option<Vec<SearchResult>>>>(&data)
                    {
                        log::info!(
                            "[DISK CACHE] Loaded Flathub search cache ({} entries)",
                            json.len()
                        );
                        *self.search_cache.get_mut().unwrap() = json;
                    }
                }
            }
        }
    }

    fn save_search_cache(&self) {
        if let Some(cache_dir) = dirs::cache_dir() {
            let path = cache_dir
                .join("monarch-store")
                .join("flathub_search_cache.json");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(cache) = self.search_cache.lock() {
                if let Ok(json) = serde_json::to_string(&*cache) {
                    let _ = std::fs::write(&path, json);
                }
            }
        }
    }

    /// Perform a search on Flathub to find a matching AppStream ID
    async fn search_find_id(&self, query: &str) -> Option<String> {
        let hits = self.search_flathub(query).await?;

        // Heuristic: Find first best match
        let query_lower = query.to_lowercase();
        // Normalized: remove all non-alphanumeric (spaces, hyphens, underscores) to compare "heroicgameslauncher" vs "heroicgameslauncher"
        let query_norm = query_lower.replace(&['-', '_', ' '][..], "");

        // 1. Exact name match (case insensitive) or ID suffix match
        for hit in &hits {
            let hit_name_lower = hit.name.trim().to_lowercase();
            if hit_name_lower == query_lower
                || hit
                    .app_id
                    .to_lowercase()
                    .ends_with(&format!(".{}", query_lower))
            {
                return Some(hit.app_id.clone());
            }
        }

        // 2. Loose Normalized Match (handles hyphen vs space differences)
        if query_norm.len() > 2 {
            for hit in &hits {
                let hit_norm = hit.name.to_lowercase().replace(&['-', '_', ' '][..], "");
                let id_norm = hit.app_id.to_lowercase().replace(&['-', '_', ' '][..], "");

                // Proportionality guard: skip if the hit name is >3x longer than the query.
                // This prevents 'steam' (5 chars) from matching 'Steam Metadata Editor' (19 chars).
                // It still allows 'heroic' (6 chars) to match 'Heroic Games Launcher' (19 chars)
                // because 'heroic' is the full product name, not just a prefix.
                let len_ratio = hit_norm.len() as f32 / (query_norm.len() as f32).max(1.0);
                if len_ratio > 3.0 {
                    continue;
                }

                if hit_norm.contains(&query_norm)
                    || query_norm.contains(&hit_norm)
                    || id_norm.contains(&query_norm)
                {
                    return Some(hit.app_id.clone());
                }
            }
        }

        // 3. Multi-word containment (e.g. "heroic games launcher" matches "heroic-launcher")
        let query_words: Vec<&str> = query_lower
            .split(&['-', '_', ' '][..])
            .filter(|s| !s.is_empty())
            .collect();
        if query_words.len() > 1 {
            for hit in &hits {
                let hit_lower = hit.name.to_lowercase();
                // If hit contains ALL query words, or query contains ALL hit words
                if query_words.iter().all(|w| hit_lower.contains(w))
                    || query_words
                        .iter()
                        .all(|w| hit.app_id.to_lowercase().contains(w))
                {
                    return Some(hit.app_id.clone());
                }
            }
        }

        // 3. Contains match (legacy fallback)
        if query.len() > 4 {
            for hit in &hits {
                if hit.name.to_lowercase().contains(&query_lower) {
                    return Some(hit.app_id.clone());
                }
            }
        }

        None
    }

    /// Public search function returning a list of results (with in-memory cache)
    pub async fn search_flathub(&self, query: &str) -> Option<Vec<SearchResult>> {
        let query_lower = query.to_lowercase();

        // Check search cache first
        if let Ok(cache) = self.search_cache.lock() {
            if let Some(cached) = cache.get(&query_lower) {
                return cached.clone();
            }
        }

        let url = "https://flathub.org/api/v2/search";

        // We use a short timeout because search is on the critical path for metadata loading
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
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
            // Cache the miss so we don't retry
            if let Ok(mut cache) = self.search_cache.lock() {
                cache.insert(query_lower, None);
            }
            self.save_search_cache(); // Persist the miss
            return None;
        }

        // Try to parse the response (v2 API returns SearchResponse with hits)
        if let Ok(json) = response.json::<SearchResponse>().await {
            let results = if json.hits.is_empty() {
                None
            } else {
                Some(json.hits)
            };
            if let Ok(mut cache) = self.search_cache.lock() {
                cache.insert(query_lower, results.clone());
            }
            // Persist after finding new results to survive timeouts
            self.save_search_cache();
            return results;
        }
        // If parsing as SearchResponse failed, try the old way (array of SearchResult)
        // Get text first to handle variable response format
        // Note: response.json() consumes the response, so if the above failed, we can't call .text() on the same response.
        // We need to re-fetch or handle the response body differently if we want to try multiple parsing strategies.
        // For now, we'll assume if json::<SearchResponse> fails, it's not the v2 format.
        // The instruction implies a change to avoid consuming, but the current reqwest API design
        // means `json()` or `text()` consume the body.
        // Given the instruction's provided code, it seems the intent was to remove the fallback to `text()`
        // and `serde_json::from_str` if `response.json::<SearchResponse>()` fails.
        // So, if the v2 parsing fails, we just return None.

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
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok()?;

        let response = client.get(&url).send().await.ok()?;

        if !response.status().is_success() {
            log::warn!(
                "[FLATHUB-API] Failed to fetch metadata for {}: HTTP {}",
                app_id,
                response.status()
            );
            if let Ok(mut cache) = self.cache.lock() {
                cache.insert(app_id.to_string(), None);
            }
            return None;
        }

        let mut metadata: FlathubMetadata = response.json().await.ok()?;
        log::info!(
            "[FLATHUB-API] Successfully fetched metadata for app_id: {}",
            app_id
        );

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
            map_cache.get(pkg_name).cloned()
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
            // Strip PACKAGING markers for better Flathub search (brave-bin → brave,
            // chromium-stable → chromium, linux-lts → linux).
            // Do NOT strip channel markers (-canary, -beta, -ptb) — those are separate products.
            let search_term = pkg_name
                .trim_end_matches("-bin")
                .trim_end_matches("-git")
                .trim_end_matches("-nightly")
                .trim_end_matches("-stable")
                .trim_end_matches("-lts")
                .trim_end_matches("-appimage");

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

    // ... (existing content)

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
        if let Ok(mut map_cache) = self.mapping_cache.lock() {
            map_cache.clear();
        }
        if let Ok(mut search_cache) = self.search_cache.lock() {
            search_cache.clear();
        }
    }

    /// v0.2.41: Batch version lookup for search results enrichment.
    pub async fn get_remote_versions_batch(
        &self,
        app_ids: &[String],
    ) -> Result<HashMap<String, String>, String> {
        if app_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let flatpak = flatpak_binary()?;
        let output = tokio::process::Command::new(&flatpak)
            .args(["remote-ls", "--app", "--columns=application,version"])
            .output()
            .await
            .map_err(|e| e.to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut map = HashMap::new();
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let id = parts[0].trim().to_string();
                if app_ids.iter().any(|uid| uid == &id) {
                    map.insert(id, parts[1].trim().to_string());
                }
            }
        }
        Ok(map)
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
        screenshots: screenshot_urls_from_flathub(&flathub.screenshots),
        version: flathub
            .releases
            .as_ref()
            .and_then(|r| r.first())
            .and_then(|rel| rel.version.clone()),
        maintainer: flathub.developer_name.clone(),
        license: flathub.project_license.clone(),
        last_updated: flathub
            .releases
            .as_ref()
            .and_then(|r| r.first())
            .and_then(|rel| rel.timestamp.as_ref())
            .and_then(|ts| ts.parse::<i64>().ok())
            .map(|ts| ts as u64),
        description: flathub
            .description
            .as_ref()
            .map(|s| crate::utils::strip_html(s)),
        is_local: false,
        available_sources: None,
        installed: None,
    }
}

// --- FLATPAK MANAGEMENT COMMANDS ---

use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;

pub(crate) const FLATPAK_NOT_INSTALLED_MSG: &str =
    "Flatpak is not installed or not in PATH. Install it with: sudo pacman -S flatpak. \
     Then add your user to the flatpak group (sudo usermod -aG flatpak $USER) and log out and back in.";

/// Resolve path to flatpak binary (PATH or /usr/bin/flatpak).
pub(crate) fn flatpak_binary() -> Result<std::path::PathBuf, String> {
    which::which("flatpak")
        .ok()
        .or_else(|| {
            let p = std::path::Path::new("/usr/bin/flatpak");
            if p.exists() {
                Some(p.to_path_buf())
            } else {
                None
            }
        })
        .ok_or_else(|| FLATPAK_NOT_INSTALLED_MSG.to_string())
}

pub async fn get_flatpak_permissions(app_id: &str) -> Result<Vec<String>, String> {
    let flatpak = flatpak_binary()?;
    let output = tokio::process::Command::new(&flatpak)
        .args(["info", "--show-permissions", app_id])
        .output()
        .await
        .map_err(|e| format!("Failed to run flatpak info: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut permissions = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if !line.is_empty() {
            permissions.push(line.to_string());
        }
    }
    Ok(permissions)
}

async fn run_flatpak_command(
    app: AppHandle,
    args: Vec<&str>,
    log_prefix: &str,
) -> Result<(), String> {
    let flatpak = flatpak_binary()?;
    let mut command = tokio::process::Command::new(&flatpak);
    command.args(&args);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    // Clean env to avoid localization issues in parsing if we parse, but for logging it's fine.
    command.env("LC_ALL", "C");

    let run_msg = format!("{} Running: flatpak {}", log_prefix, args.join(" "));
    let _ = app.emit("build://log", run_msg.clone());
    let _ = app.emit("install-output", run_msg);

    let child = command.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            FLATPAK_NOT_INSTALLED_MSG.to_string()
        } else {
            format!("Failed to start flatpak: {}", e)
        }
    })?;

    // ✅ Store child in global static for abortion tracking
    {
        let mut guard = ACTIVE_FLATPAK_CHILD.lock().await;
        *guard = Some(child);
    }

    // Wait and clear child handle
    let (status, h1_res, h2_res, stderr_lines) = match ACTIVE_FLATPAK_CHILD.lock().await.take() {
        Some(mut child) => {
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let app_c1 = app.clone();
            let app_c2 = app.clone();
            let prefix_c1 = log_prefix.to_string();
            let prefix_c2 = log_prefix.to_string();

            // Regex for extracting progress percentage from Flatpak output
            let progress_re = regex::Regex::new(r"(\d{1,3})%").ok();

            let mut reader_out = tokio::io::BufReader::new(stdout);
            let mut reader_err = tokio::io::BufReader::new(stderr);

            let progress_re_c1 = progress_re.clone();
            let h1 = tokio::spawn(async move {
                let mut line = String::new();
                while let Ok(n) = reader_out.read_line(&mut line).await {
                    if n == 0 {
                        break;
                    }
                    let trimmed = line.trim().to_string();
                    let msg = format!("{} {}", prefix_c1, trimmed);
                    let _ = app_c1.emit("build://log", msg.clone());
                    let _ = app_c1.emit("install-output", msg);

                    if let Some(ref re) = progress_re_c1 {
                        if let Some(caps) = re.captures(&trimmed) {
                            if let Ok(pct) = caps[1].parse::<u32>() {
                                if pct <= 100 {
                                    let _ = app_c1.emit("flatpak-progress", pct);
                                }
                            }
                        }
                    }
                    line.clear();
                }
            });

            let stderr_lines = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
            let stderr_lines_c = stderr_lines.clone();
            let progress_re_c2 = progress_re;

            let h2 = tokio::spawn(async move {
                let mut line = String::new();
                while let Ok(n) = reader_err.read_line(&mut line).await {
                    if n == 0 {
                        break;
                    }
                    let trimmed = line.trim().to_string();
                    let msg = format!("{} ERR: {}", prefix_c2, trimmed);
                    let _ = app_c2.emit("build://log", msg.clone());
                    let _ = app_c2.emit("install-output", msg);

                    if let Some(ref re) = progress_re_c2 {
                        if let Some(caps) = re.captures(&trimmed) {
                            if let Ok(pct) = caps[1].parse::<u32>() {
                                if pct <= 100 {
                                    let _ = app_c2.emit("flatpak-progress", pct);
                                }
                            }
                        }
                    }

                    stderr_lines_c.lock().await.push(trimmed);
                    line.clear();
                }
            });

            let status = child.wait().await;
            let res = tokio::join!(h1, h2);
            (status, res.0, res.1, stderr_lines)
        }
        None => return Err("Flatpak process lost during initialization".to_string()),
    };

    let status = status.map_err(|e| e.to_string())?;
    let _ = h1_res.map_err(|e| e.to_string());
    let _ = h2_res.map_err(|e| e.to_string());

    if status.success() {
        let _ = app.emit("flatpak-progress", 100u32);
        let msg = format!("{} Success.", log_prefix);
        let _ = app.emit("build://log", msg.clone());
        let _ = app.emit("install-output", msg);
        Ok(())
    } else {
        // Classify Flatpak errors from collected stderr
        let combined_stderr = stderr_lines.lock().await.join("\n");
        if let Some(classified) =
            crate::error_classifier::ClassifiedError::from_output(&combined_stderr)
        {
            let _ = app.emit("install-error-classified", &classified);
        }

        let msg = format!("{} Failed with code: {:?}", log_prefix, status.code());
        let _ = app.emit("build://log", msg.clone());
        let _ = app.emit("install-output", msg.clone());
        Err(msg)
    }
}

const FLATHUB_REPO_URL: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";
const FLATHUB_BETA_REPO_URL: &str = "https://flathub.org/beta-repo/flathub-beta.flatpakrepo";

/// Ensure the Flathub (stable) remote exists. Call when user enables Flatpak in onboarding or Settings,
/// and before first install, so install/uninstall work properly.
pub async fn ensure_flathub_remote(app: AppHandle) -> Result<(), String> {
    run_flatpak_command(
        app.clone(),
        vec!["remote-add", "--if-not-exists", "flathub", FLATHUB_REPO_URL],
        "[Flatpak Add Flathub Remote]",
    )
    .await
}

/// Ensure the Flathub Beta remote exists so we can install from it.
async fn ensure_flathub_beta_remote(app: AppHandle) -> Result<(), String> {
    run_flatpak_command(
        app.clone(),
        vec![
            "remote-add",
            "--if-not-exists",
            "flathub-beta",
            FLATHUB_BETA_REPO_URL,
        ],
        "[Flatpak Add Beta Remote]",
    )
    .await
}

/// Install a Flatpak app from the given remote. `remote` should be "flathub" (stable) or "flathub-beta" (beta).
/// Ensures the remote exists before installing (so first install works when Flatpak is turned on).
pub async fn install_flatpak(
    app: AppHandle,
    app_id: String,
    remote: Option<&str>,
) -> Result<(), String> {
    let remote = remote.unwrap_or("flathub");
    if remote == "flathub-beta" {
        ensure_flathub_beta_remote(app.clone()).await?;
    } else if remote == "flathub" {
        ensure_flathub_remote(app.clone()).await?;
    }
    let log_label = if remote == "flathub-beta" {
        "[Flatpak Install Beta]"
    } else {
        "[Flatpak Install]"
    };
    run_flatpak_command(app, vec!["install", remote, &app_id, "-y"], log_label).await
}

pub async fn remove_flatpak(app: AppHandle, app_id: String) -> Result<(), String> {
    run_flatpak_command(app, vec!["uninstall", &app_id, "-y"], "[Flatpak Remove]").await
}

pub async fn update_flatpak(app: AppHandle, app_id: String) -> Result<(), String> {
    run_flatpak_command(app, vec!["update", &app_id, "-y"], "[Flatpak Update]").await
}

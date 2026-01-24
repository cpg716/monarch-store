use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// Flathub API client for fetching rich app metadata
/// This is used as a METADATA SOURCE only - we don't install Flatpaks

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FlathubMetadata {
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

/// Common package name to Flathub app ID mappings
/// Many popular apps have consistent naming patterns
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
        ("microsoft-edge-stable-bin", "com.microsoft.Edge"),
        ("microsoft-edge-dev-bin", "com.microsoft.Edge"),
        (
            "teams-for-linux",
            "com.github.IsmaelMartinez.teams_for_linux",
        ),
        ("figma-linux-bin", "io.github.Figma_Linux.figma_linux"),
        ("heroic-games-launcher-bin", "com.heroicgameslauncher.hgl"),
        ("notion-app", "notion.id"), // Unofficial snap/flatpak usually used or web wrapper
        ("notion-app-enhanced", "notion.id"),
        ("telegram-desktop-bin", "org.telegram.desktop"), // Explicit mapping to save heuristic check
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
        }
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

        // Fetch from Flathub API with timeout
        let url = format!("https://flathub.org/api/v2/appstream/{}", app_id);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .ok()?;

        let response = client.get(&url).send().await.ok()?;

        if !response.status().is_success() {
            // Cache the miss to avoid repeated requests
            if let Ok(mut cache) = self.cache.lock() {
                cache.insert(app_id.to_string(), None);
            }
            return None;
        }

        let metadata: FlathubMetadata = response.json().await.ok()?;

        // Cache the result
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(app_id.to_string(), Some(metadata.clone()));
        }

        Some(metadata)
    }

    /// Try to get metadata for a package by name (using mappings)
    pub async fn get_metadata_for_package(&self, pkg_name: &str) -> Option<FlathubMetadata> {
        let app_id = get_flathub_app_id(pkg_name)?;
        self.fetch_metadata(&app_id).await
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }
}

/// Convert Flathub metadata to our internal AppMetadata format
pub fn flathub_to_app_metadata(
    flathub: &FlathubMetadata,
    pkg_name: &str,
) -> super::metadata::AppMetadata {
    super::metadata::AppMetadata {
        name: flathub.name.clone().unwrap_or_else(|| pkg_name.to_string()),
        pkg_name: Some(pkg_name.to_string()),
        icon_url: flathub.icon.clone(),
        app_id: pkg_name.to_string(),
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

use appstream::{enums::Icon, Collection, Component};

use lazy_static::lazy_static;
// use regex::Regex;
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

/*
lazy_static! {
    static ref RE_URL: Regex = Regex::new(r#"(?s)<url\b([^>]*)>(.*?)</url>"#).expect("valid regex RE_URL");
    static ref RE_IMG: Regex = Regex::new(r#"(?s)<image\b([^>]*)>(.*?)</image>"#).expect("valid regex RE_IMG");
    static ref RE_ICON: Regex = Regex::new(r#"(?s)<icon\b([^>]*)>(.*?)</icon>"#).expect("valid regex RE_ICON");
}
*/

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppMetadata {
    pub name: String,
    pub pkg_name: Option<String>,
    pub icon_url: Option<String>,
    pub app_id: String,
    pub summary: Option<String>,
    pub screenshots: Vec<String>,
    pub version: Option<String>,
    pub maintainer: Option<String>,
    pub license: Option<String>,
    pub last_updated: Option<u64>,
    pub description: Option<String>,
}

pub struct AppStreamLoader {
    collection: Option<Collection>,
    // Indices for O(1) lookup
    category_index: HashMap<String, Vec<AppMetadata>>,
    icon_index: HashMap<String, String>,
    pkg_index: HashMap<String, AppMetadata>,
    // Optimizing "The Storm": Cache local filesystem icons to avoid 1500+ disk scans
    local_icon_index: HashMap<String, String>,
}

impl Default for AppStreamLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AppStreamLoader {
    pub fn new() -> Self {
        let mut loader = Self {
            collection: None,
            category_index: HashMap::new(),
            icon_index: HashMap::new(),
            pkg_index: HashMap::new(),
            local_icon_index: HashMap::new(),
        };

        // Pre-scan local icons (O(N) once, instead of O(N) * Requests)
        loader.refresh_local_icon_index();

        // Initial load try local (linux)
        let collection =
            Collection::from_path(PathBuf::from("/usr/share/app-info/xmls/community.xml.gz"))
                .or_else(|_| {
                    Collection::from_path(PathBuf::from("/usr/share/app-info/xmls/extra.xml.gz"))
                })
                .or_else(|_| Collection::from_path(PathBuf::from("extra_v5.xml"))) // Cached/Dev
                .ok();

        if let Some(col) = collection {
            loader.set_collection(col);
        }

        loader
    }

    pub fn set_collection(&mut self, col: Collection) {
        self.collection = Some(col.clone());
        self.rebuild_indices(&col);
    }

    pub fn refresh_local_icon_index(&mut self) {
        let icons_dir = get_icons_dir();
        let mut index = HashMap::new();
        log::info!("Building Local Icon Index from {:?}", icons_dir);

        if let Ok(entries) = std::fs::read_dir(&icons_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name_os) = path.file_name() {
                    let name = name_os.to_string_lossy().to_string();
                    if name.ends_with(".png") || name.ends_with(".svg") {
                        // Store full filename as key? No, we need to match by package name efficiently.
                        // We store the filename, but keyed by... what?
                        // `find_icon_heuristic` does prefix matching.
                        // We scan for:
                        // 1. exact pkg_name.png
                        // 2. pkg_name.svg
                        // 3. pkg_name_*.png

                        // Simplest: just store the valid icon filenames in a HashMap<String, PathBuf>
                        // Key = Filename
                        index.insert(name, path.to_string_lossy().to_string());
                    }
                }
            }
        }
        self.local_icon_index = index;
    }

    fn rebuild_indices(&mut self, col: &Collection) {
        let mut cat_idx = HashMap::new();
        let mut icon_idx = HashMap::new();
        let mut pkg_idx = HashMap::new();

        for component in col.components.iter() {
            let meta = self.component_to_metadata(component);

            // 1. Package Index
            if let Some(pkg_name) = &meta.pkg_name {
                pkg_idx.insert(pkg_name.to_lowercase(), meta.clone());

                // 2. Icon Index (Exact Match)
                if let Some(icon) = &meta.icon_url {
                    icon_idx.insert(pkg_name.to_lowercase(), icon.clone());
                }
            }
            // Also index by ID if different
            let app_id_lower = meta.app_id.to_lowercase();
            if !pkg_idx.contains_key(&app_id_lower) {
                pkg_idx.insert(app_id_lower, meta.clone());
            }

            // 3. Category Index
            for category in &component.categories {
                let cat_key = format!("{:?}", category).to_lowercase();
                cat_idx
                    .entry(cat_key)
                    .or_insert_with(Vec::new)
                    .push(meta.clone());
            }
        }

        self.category_index = cat_idx;
        self.icon_index = icon_idx;
        self.pkg_index = pkg_idx;
    }

    pub fn find_package(&self, pkg_name: &str) -> Option<AppMetadata> {
        self.pkg_index.get(pkg_name).cloned()
    }

    pub fn find_app_id(&self, pkg_name: &str) -> Option<String> {
        let pkg_lower = pkg_name.to_lowercase();

        // 1. Exact match
        if let Some(meta) = self.pkg_index.get(&pkg_lower) {
            return Some(meta.app_id.clone());
        }

        // 2. Try Suffix Stripping (e.g. brave-bin -> brave)
        let base_name = crate::utils::strip_package_suffix(&pkg_lower);
        if base_name != pkg_lower {
            if let Some(meta) = self.pkg_index.get(base_name) {
                return Some(meta.app_id.clone());
            }
        }

        // 3. Manual Overrides (High-profile Bidirectional Mapping)
        match pkg_lower.as_str() {
            "steam" => Some("com.valvesoftware.steam".to_string()),
            "gimp" | "gimp-git" => Some("org.gimp.gimp".to_string()),
            "teams-for-linux" => Some("com.github.ismaelmartinez.teams_for_linux".to_string()),
            "teams" | "teams-insiders" => Some("com.microsoft.teams".to_string()),
            "spotify" | "spotify-launcher" => Some("com.spotify.client".to_string()),
            "discord" | "discord-canary" | "discord-ptb" => {
                Some("com.discordapp.discord".to_string())
            }
            "visual-studio-code-bin" | "code" | "vscode" => {
                Some("com.visualstudio.code".to_string())
            }
            "vlc" | "vlc-git" => Some("org.videolan.vlc".to_string()),
            "google-chrome" => Some("com.google.chrome".to_string()),
            "firefox" | "firefox-developer-edition" => Some("org.mozilla.firefox".to_string()),
            "telegram-desktop" | "telegram-desktop-bin" => Some("org.telegram.desktop".to_string()),
            "obs-studio" | "obs-studio-git" => Some("com.obsproject.studio".to_string()),
            "inkscape" => Some("org.inkscape.inkscape".to_string()),
            "blender" => Some("org.blender.blender".to_string()),
            "kdenlive" => Some("org.kde.kdenlive".to_string()),
            "element-desktop" => Some("im.riot.riot".to_string()),
            "pamac-manager" | "pamac" => Some("org.manjaro.pamac.manager".to_string()),
            "endeavouros-welcome" => Some("com.endeavouros.welcome".to_string()),
            "garuda-welcome" => Some("org.garudalinux.welcome".to_string()),
            "brave" | "brave-bin" | "brave-browser" => Some("com.brave.Browser".to_string()),
            _ => None,
        }
    }

    pub fn resolve_package_name(&self, input: &str) -> String {
        let input_lower = input.to_lowercase();

        // 1. If it doesn't look like an App ID (no dots), it's probably already a package name
        if !input_lower.contains('.') {
            return input_lower;
        }

        // 2. Manual Inverse Overrides (High-profile mismatches)
        match input_lower.as_str() {
            "com.valvesoftware.steam" | "com.valvesoftware.steam.desktop" | "steam" => {
                return "steam".to_string()
            }
            "org.gimp.gimp" | "org.gimp.gimp.desktop" | "gimp" => return "gimp".to_string(),
            "com.github.ismaelmartinez.teams_for_linux" | "teams-for-linux" => {
                return "teams-for-linux".to_string()
            }
            "com.microsoft.teams" | "com.microsoft.teams.desktop" => return "teams".to_string(),
            "com.spotify.client" => return "spotify".to_string(),
            "com.discordapp.discord" => return "discord".to_string(),
            "com.visualstudio.code" => return "visual-studio-code-bin".to_string(),
            "org.videolan.vlc" => return "vlc".to_string(),
            "com.google.chrome" => return "google-chrome".to_string(),
            "org.mozilla.firefox" => return "firefox".to_string(),
            "org.telegram.desktop" => return "telegram-desktop".to_string(),
            "com.obsproject.studio" => return "obs-studio".to_string(),
            "org.inkscape.inkscape" => return "inkscape".to_string(),
            "org.blender.blender" => return "blender".to_string(),
            "org.kde.kdenlive" => return "kdenlive".to_string(),
            "im.riot.riot" => return "element-desktop".to_string(),
            _ => {}
        }

        // 3. Metadata Lookup
        if let Some(meta) = self.pkg_index.get(&input_lower) {
            if let Some(pkg) = &meta.pkg_name {
                return pkg.to_lowercase();
            }
        }

        // 4. SMART FALLBACK (User's Strategy: "Check the installed section")
        // ALPM read-only: scan installed packages to see if any claim this App ID.
        for pkg in crate::alpm_read::get_installed_packages_native() {
            let pkg_name = &pkg.name;
            if let Some(found_id) = self.find_app_id(pkg_name) {
                if found_id.to_lowercase() == input_lower {
                    return pkg_name.to_string();
                }
            }
        }

        // 5. Heuristic: Reverse DNS last part
        if let Some(last) = input_lower.split('.').last() {
            let last_lower = last.to_lowercase().replace('_', "-");
            return last_lower;
        }

        input_lower
    }

    /// Returns a human-readable name for a given package name if available.
    /// e.g. "google-chrome" -> "Google Chrome", "visual-studio-code-bin" -> "VS Code"
    /// Order: AppStream (Linux standard) first, then static map, then None.
    pub fn get_friendly_name(&self, pkg_name: &str) -> Option<String> {
        let pkg_lower = pkg_name.to_lowercase();

        // 1. AppStream (Linux standard for nice names)
        if let Some(meta) = self.pkg_index.get(&pkg_lower) {
            return Some(meta.name.clone());
        }
        let base = crate::utils::strip_package_suffix(&pkg_lower);
        if base != pkg_lower {
            if let Some(meta) = self.pkg_index.get(base) {
                return Some(meta.name.clone());
            }
        }

        // 2. Static Map (AUR / known mappings fallback)
        match pkg_lower.as_str() {
            "google-chrome" | "google-chrome-stable" => Some("Google Chrome".to_string()),
            "firefox" | "firefox-developer-edition" | "firefox-nightly" => {
                Some("Mozilla Firefox".to_string())
            }
            "steam" | "steam-native-runtime" => Some("Steam".to_string()),
            "vlc" | "vlc-git" => Some("VLC Media Player".to_string()),
            "visual-studio-code-bin" | "code" | "vscode" => Some("VS Code".to_string()),
            "discord" | "discord-canary" | "discord-ptb" => Some("Discord".to_string()),
            "spotify" | "spotify-launcher" => Some("Spotify".to_string()),
            "obs-studio" | "obs-studio-git" => Some("OBS Studio".to_string()),
            "gimp" | "gimp-git" => Some("GIMP".to_string()),
            "inkscape" | "inkscape-git" => Some("Inkscape".to_string()),
            "blender" | "blender-git" => Some("Blender".to_string()),
            "kdenlive" | "kdenlive-git" => Some("Kdenlive".to_string()),
            "telegram-desktop" | "telegram-desktop-bin" => Some("Telegram Desktop".to_string()),
            "signal-desktop" | "signal-desktop-beta-bin" => Some("Signal".to_string()),
            "slack-desktop" => Some("Slack".to_string()),
            "zoom" => Some("Zoom".to_string()),
            "teams" | "teams-for-linux" => Some("Microsoft Teams".to_string()),
            "notion-app-electron" | "notion-app" => Some("Notion".to_string()),
            "postman-bin" => Some("Postman".to_string()),
            "alacritty" | "alacritty-git" => Some("Alacritty".to_string()),
            "kitty" | "kitty-git" => Some("Kitty Terminal".to_string()),
            "neovim" | "neovim-git" => Some("Neovim".to_string()),
            "brave-bin" | "brave-browser" => Some("Brave Browser".to_string()),
            "libreoffice-fresh" | "libreoffice-still" => Some("LibreOffice".to_string()),
            "onlyoffice-bin" => Some("OnlyOffice".to_string()),
            "thunderbird" | "thunderbird-beta-bin" => Some("Mozilla Thunderbird".to_string()),
            "audacity" | "audacity-git" => Some("Audacity".to_string()),
            "lutris" | "lutris-git" => Some("Lutris".to_string()),
            _ => None,
        }
    }

    pub fn find_icon_heuristic(&self, pkg_name: &str) -> Option<String> {
        // 1. O(1) Exact lookup in index
        if let Some(icon) = self.icon_index.get(pkg_name) {
            return Some(icon.clone());
        }

        // 2. Try Suffix Stripping (e.g. brave-bin -> brave)
        let base_name = crate::utils::strip_package_suffix(pkg_name);
        if base_name != pkg_name {
            if let Some(icon) = self.icon_index.get(base_name) {
                return Some(icon.clone());
            }
        }

        // 3. Fallback: Check for dash replacement (e.g. "gnome 2048" might match "org.gnome.TwentyFortyEight")
        // This is hard without the reverse map, but we can check if any key in icon_index ENDS with the package name
        // Iterate only if we must (slow-ish but cached)
        // Optimization: Only do this for short names or numbers like "2048"
        if pkg_name.chars().all(char::is_numeric) || pkg_name == "angband" {
            for (key, icon) in &self.icon_index {
                if key.contains(pkg_name) {
                    return Some(icon.clone());
                }
            }
        }

        // 3. Check Cache (Now O(1) Memory Lookup instead of Disk Scan)
        let exact_png = format!("{}.png", pkg_name);
        let exact_svg = format!("{}.svg", pkg_name);

        // Check if exact matches exist in our index
        let found_path = if let Some(p) = self.local_icon_index.get(&exact_png) {
            Some(p.clone())
        } else if let Some(p) = self.local_icon_index.get(&exact_svg) {
            Some(p.clone())
        } else {
            // Heuristic prefix scan (slower, but memory-only now)
            // Optimization: Only scan if we have to.
            self.local_icon_index
                .iter()
                .find(|(k, _)| k.starts_with(&format!("{}_", pkg_name)))
                .map(|(_, v)| v.clone())
        };

        if let Some(path_str) = found_path {
            let path = std::path::PathBuf::from(path_str);
            if let Ok(bytes) = std::fs::read(&path) {
                let mime = if path.extension().is_some_and(|e| e == "svg") {
                    "image/svg+xml"
                } else {
                    "image/png"
                };
                let encoded = BASE64_STANDARD.encode(&bytes);
                return Some(format!("data:{};base64,{}", mime, encoded));
            }
        }

        // 3b. Check System Search Paths (Linux)
        // Optimize: Define helper for system path scanning (fallback only)
        let check_dir = |dir: &PathBuf| -> Option<String> {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name_os) = path.file_name() {
                        let name = name_os.to_string_lossy();
                        if (name.starts_with(pkg_name)
                            && (name.ends_with(".png") || name.ends_with(".svg")))
                            && (name == format!("{}.png", pkg_name)
                                || name == format!("{}.svg", pkg_name)
                                || name.starts_with(&format!("{}_", pkg_name)))
                        {
                            if let Ok(bytes) = std::fs::read(&path) {
                                let mime = if path.extension().is_some_and(|e| e == "svg") {
                                    "image/svg+xml"
                                } else {
                                    "image/png"
                                };
                                let encoded = BASE64_STANDARD.encode(&bytes);
                                return Some(format!("data:{};base64,{}", mime, encoded));
                            }
                        }
                    }
                }
            }
            None
        };

        let system_paths = [
            PathBuf::from("/usr/share/pixmaps"),
            PathBuf::from("/usr/share/icons/hicolor/128x128/apps"),
            PathBuf::from("/usr/share/icons/hicolor/scalable/apps"),
            PathBuf::from("/usr/share/icons/hicolor/48x48/apps"),
            PathBuf::from("/usr/share/icons/hicolor/256x256/apps"),
            PathBuf::from("/usr/share/icons/hicolor/512x512/apps"),
        ];

        for path in system_paths {
            if path.exists() {
                if let Some(res) = check_dir(&path) {
                    return Some(res);
                }
            }
        }

        None
    }

    pub fn get_apps_by_category(&self, category: &str) -> Vec<AppMetadata> {
        let cat_lower = category.to_lowercase();
        let query_key = match cat_lower.as_str() {
            "utilities" => "utility",
            "games" => "game",
            "multimedia" => "audiovideo", // AudioVideo is XDG standard
            "graphics" => "graphics",
            "network" | "internet" => "network",
            "office" | "productivity" => "office",
            "development" | "develop" => "development",
            "system" => "system",
            k => k,
        };

        if let Some(res) = self.category_index.get(query_key) {
            return res.clone();
        }

        // Fallback: Try generic lookup if alias failed or exact match wanted
        self.category_index
            .get(&cat_lower)
            .cloned()
            .unwrap_or_default()
    }

    fn component_to_metadata(&self, component: &Component) -> AppMetadata {
        #[allow(unused_assignments)]
        // Sort icons by size (descending) to prefer higher resolution
        let mut sorted_icons = component.icons.clone();
        sorted_icons.sort_by(|a, b| {
            let get_size = |i: &Icon| match i {
                Icon::Cached { width, .. } => width.unwrap_or(0),
                Icon::Local { width, .. } => width.unwrap_or(0),
                _ => 0,
            };
            get_size(b).cmp(&get_size(a))
        });

        #[allow(unused_assignments)]
        let icon_url = sorted_icons.iter().find_map(|icon| match icon {
            Icon::Cached { path, .. } => {
                // Check extracted 'icons/' dir first
                let filename = path.file_name()?;
                let local_path = get_icons_dir().join(filename);

                if local_path.exists() {
                    if let Ok(bytes) = std::fs::read(&local_path) {
                        let mime = if local_path.extension().is_some_and(|e| e == "svg") {
                            "image/svg+xml"
                        } else {
                            "image/png"
                        };
                        let encoded = BASE64_STANDARD.encode(&bytes);
                        Some(format!("data:{};base64,{}", mime, encoded))
                    } else {
                        None
                    }
                } else if path.is_absolute() && path.exists() {
                    // Fallback: Check if the original path provided by AppStream is absolute and exists on filesystem (Linux system icons)
                    if let Ok(bytes) = std::fs::read(path) {
                        let mime = if path.extension().is_some_and(|e| e == "svg") {
                            "image/svg+xml"
                        } else {
                            "image/png"
                        };
                        let encoded = BASE64_STANDARD.encode(&bytes);
                        Some(format!("data:{};base64,{}", mime, encoded))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }

            Icon::Remote { url, .. } => Some(url.to_string()),
            _ => None,
        });

        let screenshots = component
            .screenshots
            .iter()
            .filter_map(|s| {
                s.images
                    .iter()
                    .find(|i| i.kind == appstream::enums::ImageKind::Source) // prioritize source or default
                    .or_else(|| s.images.first())
                    .map(|i| i.url.to_string())
            })
            .collect::<Vec<String>>();

        let version = component.releases.first().map(|r| r.version.clone());
        let last_updated = component
            .releases
            .first()
            .and_then(|r| r.date)
            .map(|d| d.timestamp() as u64);

        let maintainer = component
            .developer_name
            .as_ref()
            .and_then(|d| d.0.values().next().cloned());
        let license = component.project_license.as_ref().map(|l| l.to_string());
        let description = component
            .description
            .as_ref()
            .and_then(|d| d.0.values().next().cloned());

        let meta = AppMetadata {
            name: component
                .name
                .0
                .values()
                .next()
                .cloned()
                .unwrap_or_default(),
            pkg_name: component.pkgname.clone(),
            icon_url,
            app_id: component.id.to_string(),
            summary: component
                .summary
                .as_ref()
                .and_then(|s| s.0.values().next().cloned()),
            screenshots: screenshots.clone(), // Clone here if needed or just move
            version,
            maintainer,
            license,
            last_updated,
            description,
        };

        if component
            .pkgname
            .clone()
            .unwrap_or_default()
            .contains("gimp")
        {}

        meta
    }
}

lazy_static! {
    static ref RE_URL: regex::Regex =
        regex::Regex::new(r#"(?s)<url\b([^>]*)>(.*?)</url>"#).expect("valid regex RE_URL");
    static ref RE_IMG: regex::Regex =
        regex::Regex::new(r#"(?s)<image\b([^>]*)>(.*?)</image>"#).expect("valid regex RE_IMG");
    static ref RE_ICON: regex::Regex =
        regex::Regex::new(r#"(?s)<icon\b([^>]*)>(.*?)</icon>"#).expect("valid regex RE_ICON");
}

pub fn sanitize_xml(content: &str) -> String {
    // Strip null bytes immediately
    let mut content = content.replace('\0', "");

    content = content
        .replace("type=\"service\"", "type=\"console-application\"")
        .replace("type=\"web-application\"", "type=\"console-application\"");

    // Helper closure to sanitize URL content inside tags
    let sanitize_tag = |caps: &regex::Captures, is_url_tag: bool| -> String {
        let attrs = &caps[1];
        let raw_content = &caps[2];
        let url_content = raw_content.trim();

        if url_content.is_empty() {
            return String::new();
        }

        if url_content.contains('<') || url_content.contains('>') {
            return String::new();
        }

        if !url_content.contains("://") && !url_content.starts_with("mailto:") {
            // For <icon> tags, we only check if it is remote type or looks relative
            if !is_url_tag {
                if attrs.contains(r#"type="remote""#) || !url_content.contains("://") {
                    return format!("<icon{}>https://{}</icon>", attrs, url_content);
                }
                return caps[0].to_string();
            }

            // For <url> and <image> tags, always ensure protocol
            let tag_name = if is_url_tag { "url" } else { "image" };
            format!(
                "<{}{}>https://{}</{}>",
                tag_name, attrs, url_content, tag_name
            )
        } else {
            caps[0].to_string()
        }
    };

    content = RE_URL
        .replace_all(&content, |caps: &regex::Captures| sanitize_tag(caps, true))
        .to_string();
    content = RE_IMG
        .replace_all(&content, |caps: &regex::Captures| sanitize_tag(caps, false))
        .to_string(); // treat image like url for proto check

    // Icon has special logic in original code, but effectively it was just ensuring https:// for remote/relative icons
    content = RE_ICON
        .replace_all(&content, |caps: &regex::Captures| {
            let attrs = &caps[1];
            let url_content = &caps[2];
            let url_trimmed = url_content.trim();

            if attrs.contains(r#"type="remote""#) {
                if !url_trimmed.contains("://") {
                    format!("<icon{}>https://{}</icon>", attrs, url_trimmed)
                } else {
                    caps[0].to_string()
                }
            } else {
                caps[0].to_string()
            }
        })
        .to_string();

    content
}

// Download logic
pub async fn download_and_cache_appstream(
    interval_hours: u64,
    base_dir: &PathBuf,
) -> Result<PathBuf, String> {
    let target_path = base_dir.join("extra_v5.xml");

    // Check if cache is fresh
    if target_path.exists() {
        let is_fresh = if let Ok(metadata) = std::fs::metadata(&target_path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    let max_age = interval_hours * 3600;
                    elapsed.as_secs() < max_age
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if is_fresh {
            let content = std::fs::read_to_string(&target_path).map_err(|e| e.to_string())?;

            // Basic check if already sanitized or needs it
            if content.contains("type=\"service\"") || content.contains("type=\"web-application\"")
            {
                log::info!("Sanitizing existing AppStream XML...");
                let sanitized = sanitize_xml(&content);
                std::fs::write(&target_path, sanitized).map_err(|e| e.to_string())?;
            }

            return Ok(target_path);
        } else {
            log::info!(
                "AppStream cache expired ({}h interval), re-downloading",
                interval_hours
            );
        }
    }

    log::info!("Downloading Arch AppStream data...");
    let url = "https://archlinux.org/packages/extra/any/archlinux-appstream-data/download/";
    let resp = reqwest::get(url).await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("Failed to download AppStream: {}", resp.status()));
    }

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;

    let cursor = Cursor::new(bytes);
    let decoder = zstd::stream::read::Decoder::new(cursor).map_err(|e| e.to_string())?;
    let mut archive = tar::Archive::new(decoder);
    let mut found_xml = false;

    // Create icons directory early
    let icons_dir = base_dir.join("icons");
    let _ = std::fs::create_dir_all(&icons_dir);

    let mut extracted_count = 0;
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?;
        let path_str = path.to_string_lossy();

        if path_str.ends_with("extra.xml.gz") {
            // Decompress on the fly
            let mut out_file = std::fs::File::create(&target_path).map_err(|e| e.to_string())?;
            let mut gz = flate2::read::GzDecoder::new(entry);
            std::io::copy(&mut gz, &mut out_file).map_err(|e| e.to_string())?;
            found_xml = true;
        } else if path_str.contains("icons/")
            && (path_str.ends_with(".png") || path_str.ends_with(".svg"))
        {
            // Extract icons - match "icons/" anywhere in path
            if let Some(file_name) = path.file_name() {
                let icon_target = icons_dir.join(file_name);
                if let Ok(mut out_file) = std::fs::File::create(&icon_target) {
                    let _ = std::io::copy(&mut entry, &mut out_file);
                    extracted_count += 1;
                }
            }
        }
    }
    log::info!("Extracted {} icons to {:?}", extracted_count, icons_dir);

    if found_xml {
        log::info!("Sanitizing AppStream XML (Scorched Earth Mode)...");
        let content = std::fs::read_to_string(&target_path).map_err(|e| e.to_string())?;
        let sanitized = sanitize_xml(&content);
        std::fs::write(&target_path, sanitized).map_err(|e| e.to_string())?;

        log::info!(
            "Extracted, Decompressed and Sanitized AppStream data to {:?}",
            target_path
        );
        return Ok(target_path);
    }

    Err("Could not find extra.xml.gz in package".to_string())
}

pub fn get_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("monarch-store")
}

pub fn get_icons_dir() -> PathBuf {
    get_cache_dir().join("icons")
}

pub struct MetadataState(pub Mutex<AppStreamLoader>);

impl MetadataState {
    pub async fn init(&self, interval_hours: u64) {
        // Run on all platforms (Linux/macOS) to ensure consistent cache
        let cache_dir = get_cache_dir();
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            log::error!("Failed to create cache dir: {}", e);
            return;
        }

        match download_and_cache_appstream(interval_hours, &cache_dir).await {
            Ok(path) => match Collection::from_path(path.clone()) {
                Ok(col) => {
                    log::info!("Loaded AppStream data from {:?}", path);
                    let mut loader = self.0.lock().expect("MetadataState lock poisoned");
                    loader.set_collection(col);
                }
                Err(e) => {
                    log::warn!(
                        "Failed to parse AppStream data: {}. Marking cache as invalid.",
                        e
                    );

                    // Instead of deleting and retrying immediately (which causes loops),
                    // we flag it for next time or just wait.
                    // If the user manually clears cache, it will retry.
                    // This prevents the infinite "download-fail-retry" loop.
                    let _ = std::fs::remove_file(&path);
                }
            },
            Err(e) => {
                log::error!("Failed to download AppStream: {}", e);
            }
        }
    }
}

pub fn get_favicon_url(domain_url: &str) -> String {
    format!(
        "https://www.google.com/s2/favicons?sz=64&domain_url={}",
        domain_url
    )
}

/// Fetch OpenGraph image from a webpage (async, used as fallback)
pub async fn fetch_og_image(url: &str) -> Option<String> {
    // Only try if it looks like a valid URL
    if !url.starts_with("http") {
        return None;
    }

    // Use a short timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .ok()?;

    // Fetch just enough of the page to get meta tags (usually in first 16kb)
    let resp = client
        .get(url)
        .header("User-Agent", "MonARCH-Store/1.0")
        .header("Range", "bytes=0-16383")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() && resp.status().as_u16() != 206 {
        return None;
    }

    let body = resp.text().await.ok()?;

    // Simple regex for og:image
    let re =
        regex::Regex::new(r#"<meta[^>]+property=["']og:image["'][^>]+content=["']([^"']+)["']"#)
            .ok()?;

    if let Some(cap) = re.captures(&body) {
        if let Some(img) = cap.get(1) {
            return Some(img.as_str().to_string());
        }
    }

    // Try alternate order (content before property)
    let re2 =
        regex::Regex::new(r#"<meta[^>]+content=["']([^"']+)["'][^>]+property=["']og:image["']"#)
            .ok()?;

    if let Some(cap) = re2.captures(&body) {
        if let Some(img) = cap.get(1) {
            return Some(img.as_str().to_string());
        }
    }

    None
}

#[tauri::command]
pub async fn get_metadata_batch(
    state: State<'_, MetadataState>,
    scm_state: State<'_, crate::ScmState>,
    chaotic_state: State<'_, crate::chaotic_api::ChaoticApiClient>,
    flathub_state: State<'_, crate::flathub_api::FlathubApiClient>,
    pkg_names: Vec<String>,
) -> Result<HashMap<String, AppMetadata>, ()> {
    let mut results = HashMap::new();

    // Process in parallel using join_all
    let futures = pkg_names.into_iter().map(|pkg_name| {
        let state = state.inner();
        let scm_state = scm_state.clone();
        let chaotic_state = chaotic_state.clone();
        let flathub_state = flathub_state.clone();

        async move {
            let meta = get_metadata_core(
                &state,
                &scm_state,
                &chaotic_state,
                &flathub_state,
                pkg_name.clone(),
                None,
            )
            .await;
            (pkg_name, meta)
        }
    });

    let results_vec = futures::future::join_all(futures).await;

    for (pkg_name, result) in results_vec {
        if let Ok(meta) = result {
            results.insert(pkg_name, meta);
        }
    }

    Ok(results)
}

#[tauri::command]
pub async fn get_metadata(
    state: State<'_, MetadataState>,
    scm_state: State<'_, crate::ScmState>,
    chaotic_state: State<'_, crate::chaotic_api::ChaoticApiClient>,
    flathub_state: State<'_, crate::flathub_api::FlathubApiClient>,
    pkg_name: String,
    upstream_url: Option<String>,
) -> Result<AppMetadata, ()> {
    get_metadata_core(
        state.inner(),
        scm_state.inner(),
        chaotic_state.inner(),
        flathub_state.inner(),
        pkg_name,
        upstream_url,
    )
    .await
}

pub async fn get_metadata_core(
    state: &MetadataState,
    scm_state: &crate::ScmState,
    _chaotic_state: &crate::chaotic_api::ChaoticApiClient,
    flathub_state: &crate::flathub_api::FlathubApiClient,
    pkg_name: String,
    upstream_url: Option<String>,
) -> Result<AppMetadata, ()> {
    // 1. Try AppStream Match
    let app_meta = {
        let loader = state.0.lock().expect("MetadataState lock poisoned");
        loader.find_package(&pkg_name).or_else(|| {
            // Heuristic stripper match
            let base_name = crate::utils::strip_package_suffix(&pkg_name);
            if base_name != pkg_name {
                loader.find_package(base_name)
            } else {
                None
            }
        })
    };

    // 2. Try Flathub (If AppStream failed OR if AppStream found package but missing critical rich media)
    let flathub_meta = if app_meta.is_none()
        || app_meta
            .as_ref()
            .map(|m| m.icon_url.is_none() || m.screenshots.is_empty() || m.description.is_none())
            .unwrap_or(false)
    {
        flathub_state.get_metadata_for_package(&pkg_name).await
    } else {
        None
    };

    // Initialize our results with best available "Base" metadata
    let mut final_meta = if let Some(meta) = app_meta {
        let mut base = meta;
        // Merge Flathub enhancements into AppStream base
        if let Some(f_meta) = flathub_meta {
            let enriched = crate::flathub_api::flathub_to_app_metadata(&f_meta, &pkg_name);

            // CRITICAL: Upgrade the App ID if Flathub provides a canonical one (e.g. org.foo.Bar)
            // This ensures ODRS reviews work even if local AppStream only had "foo" as ID.
            if enriched.app_id.contains('.') && !base.app_id.contains('.') {
                base.app_id = enriched.app_id.clone();
            }

            if base.icon_url.is_none() {
                base.icon_url = enriched.icon_url;
            }
            if base.screenshots.is_empty() {
                base.screenshots = enriched.screenshots;
            }
            // Use Flathub description if AppStream is missing or very short (heuristic < 50 chars)
            if base.description.is_none()
                || base
                    .description
                    .as_ref()
                    .map(|d| d.len() < 50)
                    .unwrap_or(false)
            {
                if enriched.description.is_some() {
                    base.description = enriched.description;
                }
            }
        }
        base
    } else if let Some(meta) = flathub_meta {
        crate::flathub_api::flathub_to_app_metadata(&meta, &pkg_name)
    } else {
        AppMetadata {
            name: pkg_name.clone(),
            pkg_name: Some(pkg_name.clone()),
            icon_url: None,
            app_id: pkg_name.clone(),
            summary: None,
            screenshots: vec![],
            version: None,
            maintainer: None,
            license: None,
            last_updated: None,
            description: None,
        }
    };

    // 3. Icon Fallback Chain
    if final_meta.icon_url.is_none() {
        // A. Try Local Heuristics (Icons folder)
        let icon_heuristic = {
            let loader = state.0.lock().expect("MetadataState lock poisoned");
            loader.find_icon_heuristic(&pkg_name)
        };

        if let Some(icon) = icon_heuristic {
            final_meta.icon_url = Some(icon);
        } else {
            // B. Try SCM (GitHub/GitLab)
            if let Some(url) = &upstream_url {
                if let Some(scm) = scm_state.0.fetch_metadata(url).await {
                    if let Some(icon) = scm.icon_url {
                        final_meta.icon_url = Some(icon);
                    }
                    if final_meta.summary.is_none() {
                        final_meta.summary = scm.description.clone();
                    }
                }
            }

            // C. Try OG image/Favicon
            if final_meta.icon_url.is_none() {
                if let Some(url) = &upstream_url {
                    if let Some(og) = fetch_og_image(url).await {
                        final_meta.icon_url = Some(og);
                    } else {
                        final_meta.icon_url = Some(get_favicon_url(url));
                    }
                }
            }
        }
    }

    Ok(final_meta)
}

// Health logic successfully moved to repair.rs

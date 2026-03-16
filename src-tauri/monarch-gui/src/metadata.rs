use appstream::{enums::Icon, Collection, Component};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub struct MetadataState {
    pub loader: Mutex<AppStreamLoader>,
    pub initialized: AtomicBool,
}

impl MetadataState {
    pub fn new() -> Self {
        Self {
            loader: Mutex::new(AppStreamLoader::new()),
            initialized: AtomicBool::new(false),
        }
    }

    pub async fn init(&self, _interval_hours: u64) {
        // Initialization logic: the loader already scans on new(),
        // but we mark as initialized here.
        self.initialized.store(true, Ordering::SeqCst);
    }

    pub async fn wait_until_ready(&self) {
        let mut count = 0;
        while !self.initialized.load(Ordering::SeqCst) && count < 200 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            count += 1;
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Type, Default)]
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
    pub is_local: bool,
    pub available_sources: Option<Vec<crate::models::PackageSource>>,
    pub installed: Option<bool>,
}

pub struct AppStreamLoader {
    collection: Option<Collection>,
    // Indices for O(1) lookup
    category_index: HashMap<String, Vec<AppMetadata>>,
    icon_index: HashMap<String, String>,
    pub(crate) pkg_index: HashMap<String, AppMetadata>,
    package_category_index: HashMap<String, Vec<String>>,
    // Optimizing "The Storm": Cache local filesystem icons to avoid 1500+ disk scans
    local_icon_index: HashMap<String, String>,
}

fn clean_category(cat: &appstream::enums::Category) -> String {
    let raw = format!("{:?}", cat).to_lowercase();
    if raw.starts_with("unknown(\"") || raw.starts_with("other(\"") {
        if let Some(start) = raw.find('"') {
            if let Some(end) = raw.rfind('"') {
                if end > start {
                    return raw[start + 1..end].to_string();
                }
            }
        }
    }
    raw
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
            package_category_index: HashMap::new(),
            local_icon_index: HashMap::new(),
        };

        // Pre-scan local icons
        loader.refresh_local_icon_index();

        // Initial load: Scan all standard system paths for AppStream XMLs
        let mut paths = vec![
            PathBuf::from("/usr/share/app-info/xmls/community.xml.gz"),
            PathBuf::from("/usr/share/app-info/xmls/extra.xml.gz"),
        ];

        // Flatpak system-wide and user-specific appstream paths
        let mut flatpak_bases = vec![PathBuf::from("/var/lib/flatpak/appstream")];
        if let Some(home) = dirs::home_dir() {
            flatpak_bases.push(home.join(".local/share/flatpak/appstream"));
        }

        for base in flatpak_bases {
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let remote_dir = entry.path();
                    // Flatpak stores appstream in: <remote>/<arch>/active/appstream.xml.gz
                    // We check x86_64 specifically as it's the primary target.
                    let target = remote_dir.join("x86_64/active/appstream.xml.gz");
                    if target.exists() {
                        paths.push(target);
                    } else {
                        // More flexible scan: find any appstream.xml.gz in the remote's hierarchy
                        if let Ok(sub) = std::fs::read_dir(&remote_dir) {
                            for e in sub.flatten() {
                                let p = e.path();
                                if p.is_dir() {
                                    // Deep check for the .gz
                                    let deep = p.join("active/appstream.xml.gz");
                                    if deep.exists() {
                                        paths.push(deep);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Cached/Dev fallback
        paths.push(PathBuf::from("extra_v5.xml"));

        for path in paths {
            if path.exists() {
                if let Ok(col) = Collection::from_path(path.clone()) {
                    log::info!("Found local AppStream source: {:?}", path);
                    loader.add_collection(col);
                }
            }
        }

        loader
    }

    pub fn add_collection(&mut self, col: Collection) {
        self.collection = Some(col.clone()); // We store the latest one as the "primary" if needed
        self.rebuild_indices_extended(&col);
    }

    fn rebuild_indices_extended(&mut self, col: &Collection) {
        for component in col.components.iter() {
            let meta = self.component_to_metadata(component);
            let categories: Vec<String> = component.categories.iter().map(clean_category).collect();

            // 1. Package Index
            if let Some(pkg_name) = &meta.pkg_name {
                let pkg_lower = pkg_name.to_lowercase();
                // If we already have this package from a "better" source (e.g. repo vs flatpak),
                // we might want to prioritize. For now, repo (first in paths) wins.
                self.pkg_index
                    .entry(pkg_lower.clone())
                    .or_insert_with(|| meta.clone());

                // 2. Icon Index
                if let Some(icon) = &meta.icon_url {
                    self.icon_index
                        .entry(pkg_lower)
                        .or_insert_with(|| icon.clone());
                }

                self.package_category_index
                    .entry(pkg_name.to_lowercase())
                    .or_insert_with(|| categories.clone());
            }

            // 3. App ID Index
            let app_id_lower = meta.app_id.to_lowercase();
            self.pkg_index
                .entry(app_id_lower)
                .or_insert_with(|| meta.clone());
            self.package_category_index
                .entry(meta.app_id.to_lowercase())
                .or_insert_with(|| categories.clone());
            self.package_category_index
                .entry(meta.name.to_lowercase())
                .or_insert_with(|| categories.clone());

            // 4. Category Index
            for category in &component.categories {
                let cat_key = clean_category(category);
                self.category_index
                    .entry(cat_key)
                    .or_default()
                    .push(meta.clone());
            }
        }
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

    pub fn find_package(&self, pkg_name: &str) -> Option<AppMetadata> {
        let pkg_lower = pkg_name.to_lowercase();
        log::debug!("[METADATA_DEBUG] find_package: {}", pkg_lower);

        // 1. Exact Package Name match
        if let Some(meta) = self.pkg_index.get(&pkg_lower) {
            return Some(meta.clone());
        }

        // 2. Try Suffix Stripping (e.g. brave-bin -> brave)
        let base_name = crate::utils::strip_package_suffix(&pkg_lower);
        if base_name != pkg_lower {
            if let Some(meta) = self.pkg_index.get(base_name) {
                return Some(meta.clone());
            }
        }

        // 3. Try App ID Lookup (The Missing Link for Steam/Lutris)
        // If we know "steam" maps to "com.valvesoftware.Steam", let's check that ID in the index!
        if let Some(app_id) = self.find_app_id(&pkg_lower) {
            let app_id_lower = app_id.to_lowercase();
            if let Some(meta) = self.pkg_index.get(&app_id_lower) {
                return Some(meta.clone());
            }
        }

        None
    }

    pub fn get_all_entries_with_categories(&self) -> Vec<(crate::models::Package, Vec<String>)> {
        let mut entries = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for meta in self.pkg_index.values() {
            let dedupe_key = format!("{}|{}", meta.app_id, meta.pkg_name.clone().unwrap_or_default());
            if !seen.insert(dedupe_key) {
                continue;
            }
            let pkg = self.app_metadata_to_package(meta);
            let categories = self.resolve_categories_for_package(
                meta.pkg_name.as_deref().unwrap_or(&meta.name),
                Some(&meta.app_id),
            );
            entries.push((pkg, categories));
        }
        entries
    }

    fn app_metadata_to_package(&self, meta: &AppMetadata) -> crate::models::Package {
        crate::models::Package {
            name: meta.pkg_name.clone().unwrap_or_else(|| meta.name.clone()),
            display_name: Some(meta.name.clone()),
            description: meta.summary.clone().unwrap_or_default(),
            version: meta.version.clone().unwrap_or_default(),
            icon: meta.icon_url.clone(),
            app_id: Some(meta.app_id.clone()),
            screenshots: Some(meta.screenshots.clone()),
            maintainer: meta.maintainer.clone(),
            license: meta
                .license
                .as_ref()
                .map(|l| l.split(',').map(|s| s.trim().to_string()).collect()),
            long_description: meta.description.clone(),
            ..Default::default()
        }
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

        // 3. Flathub mapping (single source of truth for ODRS-compatible app IDs)
        // Covers 50+ common apps; no per-app manual overrides needed.
        if let Some(id) = crate::flathub_api::get_flathub_app_id(&pkg_lower) {
            return Some(id);
        }
        if base_name != pkg_lower {
            if let Some(id) = crate::flathub_api::get_flathub_app_id(base_name) {
                return Some(id);
            }
        }

        // 4. Fallback for known legacy apps if needed
        match pkg_lower.as_str() {
            "pamac-manager" | "pamac" => Some("org.manjaro.pamac.manager".to_string()),
            "endeavouros-welcome" => Some("com.endeavouros.welcome".to_string()),
            "garuda-welcome" => Some("org.garudalinux.welcome".to_string()),
            _ => None,
        }
    }

    pub fn resolve_package_name(&self, input: &str) -> String {
        let input_lower = input.to_lowercase();

        // 1. If it doesn't look like an App ID (no dots), it's probably already a package name
        if !input_lower.contains('.') {
            return input_lower;
        }

        // 2. Flathub reverse lookup (app_id -> pkg_name)
        if let Some(pkg) = crate::flathub_api::get_package_name_from_app_id(&input_lower) {
            return pkg;
        }

        // 3. AppStream-only packages (not on Flathub)
        match input_lower.as_str() {
            "org.gimp.gimp" | "org.gimp.gimp.desktop" | "gimp" => return "gimp".to_string(),
            "com.github.ismaelmartinez.teams_for_linux" | "teams-for-linux" => {
                return "teams-for-linux".to_string()
            }
            "org.manjaro.pamac.manager" => return "pamac".to_string(),
            "com.endeavouros.welcome" => return "endeavouros-welcome".to_string(),
            "org.garudalinux.welcome" => return "garuda-welcome".to_string(),
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
        if let Some(last) = input_lower.split('.').next_back() {
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
        let exact_symbolic_svg = format!("{}-symbolic.svg", pkg_name);

        // Check if exact matches exist in our index
        let found_path = if let Some(p) = self.local_icon_index.get(&exact_png) {
            Some(p.clone())
        } else if let Some(p) = self.local_icon_index.get(&exact_svg) {
            Some(p.clone())
        } else if let Some(p) = self.local_icon_index.get(&exact_symbolic_svg) {
            Some(p.clone())
        } else {
            // Heuristic prefix scan (slower, but memory-only now)
            // Optimization: Only scan if we have to.
            self.local_icon_index
                .iter()
                .find(|(k, _)| {
                    k.starts_with(&format!("{}_", pkg_name))
                        || k.starts_with(&format!("{}-", pkg_name))
                })
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
                                || name == format!("{}-symbolic.svg", pkg_name)
                                || name.starts_with(&format!("{}_", pkg_name))
                                || name.starts_with(&format!("{}-", pkg_name)))
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
            PathBuf::from("/usr/share/icons/breeze/apps/24"),
            PathBuf::from("/usr/share/icons/breeze-dark/apps/24"),
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

    pub fn get_recently_updated_components(&self, limit: usize) -> Vec<AppMetadata> {
        let mut all_meta: Vec<AppMetadata> = self.pkg_index.values().cloned().collect();

        // Sort by last_updated (descending)
        all_meta.sort_by(|a, b| {
            b.last_updated
                .unwrap_or(0)
                .cmp(&a.last_updated.unwrap_or(0))
        });

        // Take top N
        all_meta.into_iter().take(limit).collect()
    }

    pub fn get_apps_by_category(&self, category: &str) -> Vec<AppMetadata> {
        let cat_lower = category.to_lowercase();
        let query_keys: &[&str] = match cat_lower.as_str() {
            "game" | "games" => &["game", "games"],
            "utilities" | "utility" => &["utility", "utilities"],
            "multimedia" | "audiovideo" | "audio" | "video" => {
                &["audiovideo", "multimedia", "audio", "video"]
            }
            "graphics" => &["graphics"],
            "network" | "internet" => &["network", "internet"],
            "office" | "productivity" => &["office", "productivity"],
            "development" | "develop" => &["development", "develop"],
            "system" => &["system"],
            _ => &[],
        };

        let mut combined = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for key in query_keys {
            if let Some(res) = self.category_index.get(*key) {
                for meta in res {
                    let dedupe_key = format!(
                        "{}|{}",
                        meta.app_id,
                        meta.pkg_name.clone().unwrap_or_default()
                    );
                    if seen.insert(dedupe_key) {
                        combined.push(meta.clone());
                    }
                }
            }
        }

        if !combined.is_empty() {
            return combined;
        }

        // Fallback: Try generic lookup if alias failed or exact match wanted
        self.category_index
            .get(&cat_lower)
            .cloned()
            .unwrap_or_default()
    }

    pub fn resolve_categories_for_package(
        &self,
        pkg_name: &str,
        app_id: Option<&str>,
    ) -> Vec<String> {
        let pkg_lower = pkg_name.to_lowercase();
        let stripped_lower = crate::utils::strip_package_suffix(pkg_name).to_lowercase();
        let app_id_lower = app_id.map(|value| value.to_lowercase());

        if let Some(app_id_key) = app_id_lower.as_ref() {
            if let Some(categories) = self.package_category_index.get(app_id_key) {
                return categories.clone();
            }
        }

        if let Some(categories) = self.package_category_index.get(&pkg_lower) {
            return categories.clone();
        }

        if let Some(categories) = self.package_category_index.get(&stripped_lower) {
            return categories.clone();
        }

        Vec::new()
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
            is_local: true,
            available_sources: None,
            installed: None,
        };

        meta
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_metadata(
    pkg_name: String,
    state: tauri::State<'_, MetadataState>,
) -> Result<Option<AppMetadata>, String> {
    let loader = state.loader.lock().map_err(|e| e.to_string())?;
    Ok(loader.find_package(&pkg_name))
}

#[tauri::command]
#[specta::specta]
pub async fn get_metadata_batch(
    pkg_names: Vec<String>,
    state: tauri::State<'_, MetadataState>,
) -> Result<HashMap<String, AppMetadata>, String> {
    let loader = state.loader.lock().map_err(|e| e.to_string())?;
    let mut results = HashMap::new();
    for name in pkg_names {
        if let Some(meta) = loader.find_package(&name) {
            results.insert(name, meta);
        }
    }
    Ok(results)
}

pub fn get_icons_dir() -> PathBuf {
    let mut path = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    path.push("monarch-store");
    path.push("icons");
    let _ = std::fs::create_dir_all(&path);
    path
}

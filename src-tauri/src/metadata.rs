use appstream::{enums::Icon, Collection, Component};

// use lazy_static::lazy_static;
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
    static ref RE_URL: Regex = Regex::new(r#"(?s)<url\b([^>]*)>(.*?)</url>"#).unwrap();
    static ref RE_IMG: Regex = Regex::new(r#"(?s)<image\b([^>]*)>(.*?)</image>"#).unwrap();
    static ref RE_ICON: Regex = Regex::new(r#"(?s)<icon\b([^>]*)>(.*?)</icon>"#).unwrap();
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
        };

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

    fn rebuild_indices(&mut self, col: &Collection) {
        let mut cat_idx = HashMap::new();
        let mut icon_idx = HashMap::new();
        let mut pkg_idx = HashMap::new();

        for component in col.components.iter() {
            let meta = self.component_to_metadata(component);

            // 1. Package Index
            if let Some(pkg_name) = &meta.pkg_name {
                pkg_idx.insert(pkg_name.clone(), meta.clone());

                // 2. Icon Index (Exact Match)
                if let Some(icon) = &meta.icon_url {
                    icon_idx.insert(pkg_name.clone(), icon.clone());
                }
            }
            // Also index by ID if different
            if !pkg_idx.contains_key(&meta.app_id) {
                pkg_idx.insert(meta.app_id.clone(), meta.clone());
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
        // 1. Exact match
        if let Some(meta) = self.pkg_index.get(pkg_name) {
            return Some(meta.app_id.clone());
        }

        // 2. Try Suffix Stripping (e.g. brave-bin -> brave)
        let base_name = crate::utils::strip_package_suffix(pkg_name);
        if base_name != pkg_name {
            if let Some(meta) = self.pkg_index.get(base_name) {
                return Some(meta.app_id.clone());
            }
        }

        None
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

        // println!("DEBUG: Looking for icon for pkg: '{}'", pkg_name);
        // 3. Search the icons directory for pattern match
        let icons_dir = get_icons_dir();
        // println!("DEBUG: Checking icons dir: {:?}", icons_dir);

        // Helper to check a dir for the icon
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
                                // println!(
                                //     "DEBUG: Found icon for '{}' at {:?} ({} bytes)",
                                //     pkg_name,
                                //     path,
                                //     bytes.len()
                                // );
                                return Some(format!("data:{};base64,{}", mime, encoded));
                            } else {
                                // println!(
                                //     "DEBUG: Failed to read file for '{}' at {:?}",
                                //     pkg_name, path
                                // );
                            }
                        }
                    }
                }
            } else {
                // println!("DEBUG: Failed to read_dir {:?}", dir);
            }
            None
        };

        // 3a. Check Cache
        if let Some(res) = check_dir(&icons_dir) {
            return Some(res);
        }

        // 3b. Check System Search Paths (Linux)
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

    pub fn search_apps(&self, query: &str) -> Vec<AppMetadata> {
        let mut apps = Vec::new();
        let query_lower = query.to_lowercase();

        if let Some(col) = &self.collection {
            for component in col.components.iter() {
                let mut matched = false;

                // Match ID or Package Name
                if component.id.0.to_lowercase().contains(&query_lower) {
                    matched = true;
                }
                if let Some(pkgname) = &component.pkgname {
                    if pkgname.to_lowercase().contains(&query_lower) {
                        matched = true;
                    }
                }

                // Match Human Name
                if let Some(name) = component.name.0.values().next() {
                    if name.to_lowercase().contains(&query_lower) {
                        matched = true;
                    }
                }

                // Match Keywords
                if !matched {
                    if let Some(keywords) = &component.keywords {
                        if keywords.0.values().any(|vals| {
                            vals.iter().any(|v| v.to_lowercase().contains(&query_lower))
                        }) {
                            matched = true;
                        }
                    }
                }

                if matched {
                    apps.push(self.component_to_metadata(component));
                }
            }
        }
        apps
    }

    pub fn get_apps_by_category(&self, category: &str) -> Vec<AppMetadata> {
        self.category_index
            .get(&category.to_lowercase())
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

        if let Some(pkg) = &component.pkgname {
            if pkg.contains("brave") || pkg.contains("spotify") {
                // println!(
                //     "DEBUG: component_to_metadata '{}' -> Icon: {:?}",
                //     pkg, icon_url
                // );
            }
        }

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
            .collect();

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

        AppMetadata {
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
            screenshots,
            version,
            maintainer,
            license,
            last_updated,
            description,
        }
    }
}

pub fn sanitize_xml(content: &str) -> String {
    // 1. Strip null bytes (Essential)
    let mut content = content.replace('\0', "");

    // 2. Fix known Enum variant incompatibilities
    // "service" and "web-application" are not supported by the old appstream crate we might be using,
    // or cause issues. Mapping them to "console-application" (generic) is safe.
    content = content
        .replace("type=\"service\"", "type=\"console-application\"")
        .replace("type=\"web-application\"", "type=\"console-application\"");

    // 3. Fix &amp; entities double encoding if present (common issue)
    // content = content.replace("&amp;amp;", "&amp;");

    // NOTE: We disabled the aggressive Regex sanitization for URLs/Icons because
    // it was causing "Unexpected end of stream" errors on the 25MB+ XML file,
    // likely due to regex buffer limits or accidental tag stripping.
    // The AppStream parser is reasonably robust, so we accept a few malformed URLs
    // in exchange for successfully loading the cache.

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
                println!("Sanitizing existing AppStream XML...");
                let sanitized = sanitize_xml(&content);
                std::fs::write(&target_path, sanitized).map_err(|e| e.to_string())?;
            }

            return Ok(target_path);
        } else {
            println!(
                "AppStream cache expired ({}h interval), re-downloading...",
                interval_hours
            );
        }
    }

    println!("Downloading Arch AppStream data...");
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
    println!("Extracted {} icons to {:?}", extracted_count, icons_dir);

    if found_xml {
        println!("Sanitizing AppStream XML (Scorched Earth Mode)...");
        let content = std::fs::read_to_string(&target_path).map_err(|e| e.to_string())?;
        let sanitized = sanitize_xml(&content);
        std::fs::write(&target_path, sanitized).map_err(|e| e.to_string())?;

        println!(
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
        std::fs::create_dir_all(&cache_dir).ok();

        match download_and_cache_appstream(interval_hours, &cache_dir).await {
            Ok(path) => match Collection::from_path(path.clone()) {
                Ok(col) => {
                    println!("Loaded AppStream data from {:?}", path);
                    let mut loader = self.0.lock().unwrap();
                    loader.set_collection(col);
                }
                Err(e) => {
                    println!(
                        "Failed to parse AppStream data: {}. Deleting corrupted cache...",
                        e
                    );
                    let _ = std::fs::remove_file(&path);

                    // Optional: Retry immediately once
                    println!("Retrying download...");
                    if let Ok(new_path) = download_and_cache_appstream(0, &cache_dir).await {
                        // 0 interval forces check/download
                        if let Ok(col) = Collection::from_path(new_path) {
                            println!("Retry successful!");
                            let mut loader = self.0.lock().unwrap();
                            loader.set_collection(col);
                        }
                    }
                }
            },
            Err(e) => {
                println!("Failed to download AppStream: {}", e);
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
        let state = state.clone();
        let scm_state = scm_state.clone();
        let chaotic_state = chaotic_state.clone();
        let flathub_state = flathub_state.clone();

        async move {
            let meta = get_metadata(
                state,
                scm_state,
                chaotic_state,
                flathub_state,
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

    // println!(
    //     "DEBUG: Backend Batch returning {} items. Keys: {:?}",
    //     results.len(),
    //     results.keys()
    // );

    Ok(results)
}

#[tauri::command]
pub async fn get_metadata(
    state: State<'_, MetadataState>,
    scm_state: State<'_, crate::ScmState>,
    _chaotic_state: State<'_, crate::chaotic_api::ChaoticApiClient>,
    flathub_state: State<'_, crate::flathub_api::FlathubApiClient>,
    pkg_name: String,
    upstream_url: Option<String>,
) -> Result<AppMetadata, ()> {
    // 1. Try AppStream Match
    let app_meta = {
        let loader = state.0.lock().unwrap();
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

    // 2. Try Flathub (If AppStream failed)
    // 2. Try Flathub (If AppStream failed OR if AppStream found package but no icon)
    let flathub_meta = if app_meta.is_none()
        || app_meta
            .as_ref()
            .map(|m| m.icon_url.is_none())
            .unwrap_or(false)
    {
        flathub_state.get_metadata_for_package(&pkg_name).await
    } else {
        None
    };

    // Initialize our results with best available "Base" metadata
    let mut final_meta = if let Some(meta) = app_meta {
        meta
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
            let loader = state.0.lock().unwrap();
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
    // Final Check: Log if icon is still missing for debugging
    if final_meta.icon_url.is_none() {
        println!(
            "WARN: No icon found for package '{}' after all fallbacks.",
            pkg_name
        );
    }

    Ok(final_meta)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthIssue {
    pub category: String,
    pub severity: String,
    pub message: String,
    pub action_label: String,
    pub action_command: Option<String>,
}

#[tauri::command]
pub async fn check_system_health() -> Result<Vec<HealthIssue>, String> {
    let mut issues = Vec::new();

    // 1. Check dependencies
    let deps = ["base-devel", "git"];
    for dep in deps {
        let has_dep = std::process::Command::new("pacman")
            .args(["-Qq", dep])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !has_dep {
            issues.push(HealthIssue {
                category: "Dependency".to_string(),
                severity: "Critical".to_string(),
                message: format!("Missing essential build dependency: {}", dep),
                action_label: format!("Install {}", dep),
                action_command: Some(format!("pkexec pacman -S --needed --noconfirm {}", dep)),
            });
        }
    }

    // 2. Check for sync failures (Check if monarch-store/dbs exists and has files)
    let dbs_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("monarch-store")
        .join("dbs");

    if !dbs_dir.exists()
        || std::fs::read_dir(&dbs_dir)
            .map(|d| d.count() == 0)
            .unwrap_or(true)
    {
        issues.push(HealthIssue {
            category: "Repository".to_string(),
            severity: "Warning".to_string(),
            message: "No package databases found. Browse might be empty.".to_string(),
            action_label: "Refresh Repositories".to_string(),
            action_command: None, // Frontend will handle triggering sync
        });
    }

    // 3. Hardware Optimization Status
    let opt_level = if crate::utils::is_cpu_znver4_compatible() {
        "Zen 4/5 (Extreme)"
    } else if crate::utils::is_cpu_v4_compatible() {
        "v4 (AVX-512)"
    } else if crate::utils::is_cpu_v3_compatible() {
        "v3 (AVX2)"
    } else {
        "v1 (Standard x86-64)"
    };

    issues.push(HealthIssue {
        category: "Hardware".to_string(),
        severity: "Info".to_string(),
        message: format!("Hardware Optimization Level: {}", opt_level),
        action_label: "View Optimization Guide".to_string(),
        action_command: None,
    });

    Ok(issues)
}

#[cfg(test)]
mod tests {
    use crate::flathub_api::FlathubApiClient;

    #[tokio::test]
    async fn test_brave_lookup_debug() {
        let flathub = FlathubApiClient::new();

        let meta = flathub.get_metadata_for_package("brave").await;
        if let Some(m) = &meta {
            println!("Brave Meta Icon: {:?}", m.icon);
        } else {
            println!("Brave Meta: None");
        }

        let meta_bin = flathub.get_metadata_for_package("brave-bin").await;
        if let Some(m) = &meta_bin {
            println!("Brave-bin Meta Icon: {:?}", m.icon);
        } else {
            println!("Brave-bin Meta: None");
        }
    }
}

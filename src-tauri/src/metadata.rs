use appstream::{enums::Icon, Collection, Component};
use base64::prelude::*;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

lazy_static! {
    static ref RE_URL: Regex = Regex::new(r#"(?s)<url\b([^>]*)>(.*?)</url>"#).unwrap();
    static ref RE_IMG: Regex = Regex::new(r#"(?s)<image\b([^>]*)>(.*?)</image>"#).unwrap();
    static ref RE_ICON: Regex = Regex::new(r#"(?s)<icon\b([^>]*)>(.*?)</icon>"#).unwrap();
}

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
        // 1. O(1) Exact lookup
        if let Some(icon) = self.icon_index.get(pkg_name) {
            return Some(icon.clone());
        }

        // 2. Try Suffix Stripping (Centralized)
        let base_name = crate::utils::strip_package_suffix(pkg_name);
        if base_name != pkg_name {
            if let Some(icon) = self.icon_index.get(base_name) {
                return Some(icon.clone());
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
                let local_path = std::path::Path::new("icons").join(filename);

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
pub async fn download_and_cache_appstream(interval_hours: u64) -> Result<PathBuf, String> {
    let target_path = PathBuf::from("extra_v5.xml");

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
    let _ = std::fs::create_dir_all("icons");

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
            let file_name = path.file_name().unwrap();
            let icon_target = std::path::Path::new("icons").join(file_name);
            if let Ok(mut out_file) = std::fs::File::create(&icon_target) {
                let _ = std::io::copy(&mut entry, &mut out_file);
            }
        }
    }

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

pub struct MetadataState(pub Mutex<AppStreamLoader>);

impl MetadataState {
    pub async fn init(&self, interval_hours: u64) {
        if cfg!(target_os = "macos") {
            match download_and_cache_appstream(interval_hours).await {
                Ok(path) => match Collection::from_path(path.clone()) {
                    Ok(col) => {
                        println!("Loaded AppStream data from {:?}", path);
                        let mut loader = self.0.lock().unwrap();
                        loader.set_collection(col);
                    }
                    Err(e) => {
                        println!("Failed to parse downloaded AppStream data: {}", e);
                    }
                },
                Err(e) => {
                    println!("Failed to download AppStream: {}", e);
                }
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
pub async fn get_metadata(
    state: State<'_, MetadataState>,
    scm_state: State<'_, crate::ScmState>,
    chaotic_state: State<'_, crate::chaotic_api::ChaoticApiClient>,
    flathub_state: State<'_, crate::flathub_api::FlathubApiClient>,
    pkg_name: String,
    upstream_url: Option<String>,
) -> Result<AppMetadata, ()> {
    // Scope the lock so it is dropped before any await points
    let appstream_result = {
        let loader = state.0.lock().unwrap();

        // 1. Try exact AppStream match
        if let Some(meta) = loader.find_package(&pkg_name) {
            Some(meta)
        } else {
            // 2. Try heuristic matching
            let mut found = None;
            let base_name = crate::utils::strip_package_suffix(&pkg_name);

            if base_name != pkg_name {
                if let Some(mut meta) = loader.find_package(base_name) {
                    meta.pkg_name = Some(pkg_name.clone());
                    found = Some(meta);
                }
            }

            if found.is_none() {
                // 3. Try prefix matching
                if let Some(col) = &loader.collection {
                    for component in col.components.iter() {
                        if let Some(cpkg) = &component.pkgname {
                            if pkg_name.starts_with(cpkg.as_str()) && pkg_name.len() > cpkg.len() {
                                let mut meta = loader.component_to_metadata(component);
                                meta.pkg_name = Some(pkg_name.clone());
                                found = Some(meta);
                                break;
                            }
                        }
                    }
                }
            }
            found
        }
    };

    if let Some(meta) = appstream_result {
        return Ok(meta);
    }

    // 4. Try Flathub API (many popular apps have Flatpak versions with rich metadata)
    let flathub_meta = flathub_state.get_metadata_for_package(&pkg_name).await;

    if let Some(meta) = flathub_meta {
        return Ok(crate::flathub_api::flathub_to_app_metadata(
            &meta, &pkg_name,
        ));
    }

    // 5. Try SCM Metadata (GitHub/GitLab)
    // This is very efficient for getting icons for AUR packages hosted on GitHub
    let scm_data = if let Some(url) = &upstream_url {
        scm_state.0.fetch_metadata(url).await
    } else {
        None
    };

    if let Some(scm) = &scm_data {
        if scm.icon_url.is_some() || scm.description.is_some() || !scm.screenshots.is_empty() {
            return Ok(AppMetadata {
                name: pkg_name.clone(),
                pkg_name: Some(pkg_name.clone()),
                icon_url: scm.icon_url.clone(),
                app_id: pkg_name.clone(),
                summary: scm.description.clone(),
                screenshots: scm.screenshots.clone(),
                version: None,
                maintainer: None,
                license: scm.license.clone(),
                last_updated: None,
                description: scm.description.clone(),
            });
        }
    }

    // 6. Try Chaotic-AUR Metadata
    let chaotic_pkg = chaotic_state.find_package(&pkg_name).await;

    if let Some(cp) = chaotic_pkg {
        if cp.metadata.is_some() {
            if let Some(meta) = cp.metadata {
                return Ok(AppMetadata {
                    name: pkg_name.clone(),
                    pkg_name: Some(pkg_name.clone()),
                    icon_url: None, // Chaotic doesn't seem to have icons in this metadata struct
                    app_id: pkg_name.clone(),
                    summary: meta.desc,
                    screenshots: vec![],
                    version: cp.version,
                    maintainer: None,
                    license: meta.license,
                    last_updated: None,
                    description: None,
                });
            }
        }
    }

    // 7. Fallback: Try OpenGraph image, then Favicon from upstream URL
    let mut icon_url = None;
    let mut fallback_screenshots = vec![];

    // Try to use SCM data first
    if let Some(scm) = scm_data {
        icon_url = scm.icon_url;
        fallback_screenshots = scm.screenshots;
    }

    // If still no icon, try OpenGraph from upstream URL
    if icon_url.is_none() {
        if let Some(url) = &upstream_url {
            // Try OG image (async would be better but this is fallback)
            if let Some(og) = fetch_og_image(url).await {
                icon_url = Some(og);
            } else {
                // Last resort: favicon
                icon_url = Some(get_favicon_url(url));
            }
        }
    }

    Ok(AppMetadata {
        name: pkg_name.clone(),
        pkg_name: Some(pkg_name.clone()),
        icon_url,
        app_id: pkg_name,
        summary: None,
        screenshots: fallback_screenshots,
        version: None,
        maintainer: None,
        license: None,
        last_updated: None,
        description: None,
    })
}

#[tauri::command]
pub fn get_apps_by_category(state: State<MetadataState>, category: String) -> Vec<AppMetadata> {
    let loader = state.0.lock().unwrap();
    loader.get_apps_by_category(&category)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_extra_v3() {
        let path = PathBuf::from("extra_v5.xml");
        if !path.exists() {
            println!("extra_v5.xml not found, skipping test");
            return;
        }

        let content = std::fs::read_to_string(&path).unwrap();

        // Extract all components manually
        let re = regex::Regex::new(r#"(?s)<component\b.*?</component>"#).unwrap();
        let components: Vec<_> = re.find_iter(&content).map(|m| m.as_str()).collect();

        println!(
            "Found {} components. Testing individually...",
            components.len()
        );

        // Header for wrapping
        let header = r#"<?xml version="1.0" encoding="utf-8"?>
<components version="1.0" origin="archlinux-arch-extra">
"#;
        let footer = "</components>";

        for (i, comp_str) in components.iter().enumerate() {
            let wrapped = format!("{}{}{}", header, comp_str, footer);
            // Create a temp file for this component
            let temp_path = std::env::temp_dir().join(format!("temp_comp_{}.xml", i));
            std::fs::write(&temp_path, wrapped).unwrap();

            match Collection::from_path(temp_path.clone()) {
                Ok(_) => {
                    // Success
                }
                Err(e) => {
                    println!("Failed to parse component {}!", i);
                    println!("Error: {}", e);
                    // Print the first few lines of the component to identify it
                    let lines: Vec<&str> = comp_str.lines().take(5).collect();
                    println!("Component Content (Snipped):\n{}", lines.join("\n"));
                    std::fs::remove_file(temp_path).unwrap();
                    panic!("Found the validator!");
                }
            }
            std::fs::remove_file(temp_path).unwrap();
        }
        println!("All components parsed successfully individually!");
    }
}

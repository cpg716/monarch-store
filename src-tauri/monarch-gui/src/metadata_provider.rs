use crate::aur_api;
use crate::flathub_api::FlathubApiClient;
use crate::metadata::AppMetadata;
use lazy_static::lazy_static;
use regex::Regex;

use std::path::{Path, PathBuf};

use tokio::fs; // Use tokio filesystem

lazy_static! {
    static ref RE_NAME: Regex = Regex::new(r"(?m)^Name=(.*)$").unwrap();
    static ref RE_COMMENT: Regex = Regex::new(r"(?m)^Comment=(.*)$").unwrap();
    static ref RE_ICON: Regex = Regex::new(r"(?m)^Icon=(.*)$").unwrap();
}

pub struct MetadataProvider {
    http_client: reqwest::Client,
    cache_dir: PathBuf,
}

impl Default for MetadataProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataProvider {
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("monarch/icons");

        // Ensure cache dir exists
        let _ = std::fs::create_dir_all(&cache_dir);

        Self {
            http_client: reqwest::Client::new(),
            cache_dir,
        }
    }

    /// Primary Resolution Method - Returns Full AppMetadata
    /// Primary Resolution Method - Returns Full AppMetadata
    pub async fn resolve(
        &self,
        pkg_name: &str,
        appstream_data: Option<AppMetadata>,
        flathub_client: &FlathubApiClient,
        skip_aur: bool,
    ) -> Option<AppMetadata> {
        let pkg_lower = pkg_name.to_lowercase();

        // Level 1: AppStream (Pre-fetched)
        if let Some(meta) = appstream_data {
            return Some(meta);
        }

        // Level 2: Local Desktop Files
        if let Some(desktop_meta) = self.scan_desktop_files(&pkg_lower).await {
            // If desktop gave generic app_id (no dots), prefer Flathub for ODRS + screenshots
            if !desktop_meta.app_id.contains('.') {
                if let Some(flathub_meta) = self.query_flathub(&pkg_lower, flathub_client).await {
                    return Some(flathub_meta);
                }
            }
            return Some(desktop_meta);
        }

        // Level 3: Flathub API
        if let Some(flathub_meta) = self.query_flathub(&pkg_lower, flathub_client).await {
            return Some(flathub_meta);
        }

        // Level 4: AUR RPC (Flathub already tried in Level 3)
        if !skip_aur {
            if let Some(aur_meta) = self.query_aur(&pkg_lower).await {
                return Some(aur_meta);
            }
        }

        // Fallback: None (Core will construct skeleton)
        None
    }

    // --- LEVEL 2: Desktop Files ---

    async fn scan_desktop_files(&self, pkg_name: &str) -> Option<AppMetadata> {
        let search_paths = vec![
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
            dirs::data_local_dir()
                .unwrap_or(PathBuf::from(""))
                .join("applications"),
        ];

        for dir in search_paths {
            if !dir.exists() {
                continue;
            }
            let mut best_match: Option<PathBuf> = None;

            if let Ok(mut entries) = fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if let Some(fname) = path.file_name().and_then(|f| f.to_str()) {
                        let fname_lower = fname.to_lowercase();
                        if fname_lower.contains(pkg_name) && fname_lower.ends_with(".desktop") {
                            if fname_lower == format!("{}.desktop", pkg_name) {
                                best_match = Some(path);
                                break;
                            }
                            if best_match.is_none() {
                                best_match = Some(path);
                            }
                        }
                    }
                }
            }

            if let Some(path) = best_match {
                return self.parse_desktop_file(&path, pkg_name).await;
            }
        }
        None
    }

    async fn parse_desktop_file(&self, path: &Path, pkg_name: &str) -> Option<AppMetadata> {
        let content = fs::read_to_string(path).await.ok()?;

        let name = RE_NAME
            .captures(&content)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_else(|| pkg_name.to_string());

        let comment = RE_COMMENT
            .captures(&content)
            .map(|c| c[1].trim().to_string());
        let icon_str = RE_ICON.captures(&content).map(|c| c[1].trim().to_string());

        let icon_path = if let Some(icon_name) = icon_str {
            self.resolve_icon_path(&icon_name).await
        } else {
            None
        };

        // Use Flathub mapping when desktop file stem is generic (no dots) so ODRS reviews work.
        // e.g. lutris.desktop -> "lutris" is wrong; net.lutris.Lutris is correct.
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let app_id = if stem.contains('.') {
            stem
        } else if let Some(flathub_id) = crate::flathub_api::get_flathub_app_id(pkg_name) {
            flathub_id
        } else {
            stem
        };

        Some(AppMetadata {
            name,
            pkg_name: Some(pkg_name.to_string()),
            icon_url: icon_path,
            app_id,
            summary: comment.clone(),
            description: comment,
            screenshots: vec![],
            version: None,
            maintainer: None,
            license: None,
            last_updated: None,
            is_local: true,
            available_sources: None,
            installed: None,
        })
    }

    async fn resolve_icon_path(&self, icon_name: &str) -> Option<String> {
        if icon_name.starts_with('/') {
            if fs::metadata(icon_name).await.is_ok() {
                return Some(format!("file://{}", icon_name));
            }
            return None;
        }

        let extensions = ["png", "svg", "xpm"];
        let base_dirs = [
            "/usr/share/pixmaps",
            "/usr/share/icons/hicolor/48x48/apps",
            "/usr/share/icons/hicolor/64x64/apps",
            "/usr/share/icons/hicolor/128x128/apps",
            "/usr/share/icons/hicolor/256x256/apps",
            "/usr/share/icons/hicolor/scalable/apps",
        ];

        for dir in base_dirs {
            for ext in extensions {
                let p = Path::new(dir).join(format!("{}.{}", icon_name, ext));
                if fs::metadata(&p).await.is_ok() {
                    return Some(format!("file://{}", p.to_string_lossy()));
                }
            }
        }
        None
    }

    // --- LEVEL 3: Flathub API ---

    async fn query_flathub(
        &self,
        pkg_name: &str,
        flathub_client: &FlathubApiClient,
    ) -> Option<AppMetadata> {
        let meta = flathub_client.get_metadata_for_package(pkg_name).await?;
        let app_id = meta.id.as_deref().unwrap_or(pkg_name);

        // Cache Icon
        let cached_icon = if let Some(url) = &meta.icon {
            self.cache_remote_icon(url, app_id).await
        } else {
            None
        };

        // Convert screenshots (supports both legacy keys and v2 sizes array)
        let screenshots = crate::flathub_api::screenshot_urls_from_flathub(&meta.screenshots);

        Some(AppMetadata {
            name: meta.name.clone().unwrap_or_else(|| pkg_name.to_string()),
            pkg_name: Some(pkg_name.to_string()),
            icon_url: cached_icon.or(meta.icon.clone()),
            app_id: app_id.to_string(), // Critical for reviews!
            summary: meta.summary.clone(),
            description: meta.description.clone().or(meta.summary.clone()),
            screenshots,
            version: None, // Flathub API doesn't always give version easily in this struct
            maintainer: meta.developer_name.clone(),
            license: meta.project_license.clone(),
            last_updated: None,
            is_local: false,
            available_sources: None,
            installed: None,
        })
    }

    async fn cache_remote_icon(&self, url: &str, app_id: &str) -> Option<String> {
        if !url.starts_with("http") {
            return Some(url.to_string());
        }

        let ext = if url.ends_with(".png") {
            "png"
        } else if url.ends_with(".svg") {
            "svg"
        } else {
            "png"
        };
        let filename = format!("{}.{}", app_id, ext);
        let path = self.cache_dir.join(&filename);

        if path.exists() {
            return Some(format!("file://{}", path.to_string_lossy()));
        }

        match self.http_client.get(url).send().await {
            Ok(resp) => {
                if let Ok(bytes) = resp.bytes().await {
                    if let Ok(_) = fs::write(&path, bytes).await {
                        return Some(format!("file://{}", path.to_string_lossy()));
                    }
                }
            }
            Err(e) => log::warn!("Failed to download icon for {}: {}", app_id, e),
        }
        None
    }

    // --- LEVEL 4: AUR RPC ---

    async fn query_aur(&self, pkg_name: &str) -> Option<AppMetadata> {
        match aur_api::get_multi_info(&[pkg_name]).await {
            Ok(pkgs) => {
                if let Some(pkg) = pkgs.first() {
                    return Some(AppMetadata {
                        name: pkg.name.clone(),
                        pkg_name: Some(pkg.name.clone()),
                        icon_url: None,
                        app_id: pkg.name.clone(),
                        summary: Some(pkg.description.clone()),
                        description: Some(pkg.description.clone()),
                        screenshots: vec![],
                        version: Some(pkg.version.clone()),
                        maintainer: pkg.maintainer.clone(),
                        license: pkg.license.as_ref().and_then(|l| l.first()).cloned(),
                        last_updated: pkg.last_modified.map(|t| t as u64),
                        is_local: false,
                        available_sources: None,
                        installed: None,
                    });
                }
            }
            Err(e) => log::warn!("AUR lookup failed for {}: {}", pkg_name, e),
        }
        None
    }
}

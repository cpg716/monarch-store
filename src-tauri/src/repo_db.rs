use crate::models::{Package, PackageSource};
use flate2::read::GzDecoder;
use reqwest::Client;
use std::io::Read;
use tar::Archive;

// Struct removed.

#[async_trait::async_trait]
pub trait RepoClient: Send + Sync {
    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String>;
}

pub struct RealRepoClient {
    client: Client,
}

impl RealRepoClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .http1_only()
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
            .pool_idle_timeout(Some(std::time::Duration::from_secs(60)))
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(reqwest::header::ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8".parse().unwrap());
                headers.insert(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.5".parse().unwrap());
                headers.insert(reqwest::header::CONNECTION, "keep-alive".parse().unwrap());
                headers
            })
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }
}

#[async_trait::async_trait]
impl RepoClient for RealRepoClient {
    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        match self.client.get(url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.bytes().await {
                        Ok(data) => Ok(data.to_vec()),
                        Err(e) => Err(format!("Bytes error: {}", e)),
                    }
                } else {
                    Err(format!("HTTP {}", resp.status()))
                }
            }
            Err(e) => Err(format!("Request error: {}", e)),
        }
    }
}

pub async fn fetch_repo_packages<C: RepoClient>(
    client: &C,
    mirror_url: &str,
    repo_name: &str,
    source: PackageSource,
    cache_dir: &std::path::Path,
    force: bool,
    interval_hours: u64,
) -> Result<Vec<Package>, String> {
    let file_name = format!("{}.db", repo_name);
    let cache_path = cache_dir.join(&file_name);

    // Check if cache is fresh (modified < interval_hours)
    let is_fresh = if force {
        false
    } else if cache_path.exists() {
        if let Ok(metadata) = std::fs::metadata(&cache_path) {
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
        }
    } else {
        false
    };

    let bytes = if is_fresh {
        // Load from disk
        std::fs::read(&cache_path).map_err(|e| e.to_string())?
    } else {
        // Download with Fallback
        let mut mirrors_to_try = vec![mirror_url.to_string()];

        if mirror_url.contains("cachyos.org") || mirror_url.contains("soulharsh007.dev") {
            // CachyOS Mirror Rotation
            let alternates = [
                "https://cdn77.cachyos.org",
                "https://cdn.cachyos.org",
                "https://us.cachyos.org",
                "https://mirror.cachyos.org",
                "https://at.cachyos.org",
                "https://de-nue.soulharsh007.dev/cachyos",
                "https://us-mnz.soulharsh007.dev/cachyos",
                "https://mirror.lesviallon.fr/cachy",
            ];
            for alt in alternates {
                if !mirror_url.starts_with(alt) {
                    if let Some(path_index) = mirror_url.find("/repo/") {
                        let path = &mirror_url[path_index..];
                        mirrors_to_try.push(format!("{}{}", alt, path));
                    }
                }
            }
        } else if mirror_url.contains("manjaro") {
            // Manjaro Mirror Rotation
            let alternates = [
                "https://mirror.easyname.at/manjaro",
                "https://mirror.dkm.cz/manjaro",
                "https://manjaro.lucassymons.net",
                "https://ftp.gwdg.de/pub/linux/manjaro",
                "https://mirror.init7.net/manjaro",
            ];
            for alt in alternates {
                if !mirror_url.starts_with(alt) {
                    if let Some(path_index) = mirror_url.find("/stable/") {
                        let path = &mirror_url[path_index..];
                        mirrors_to_try.push(format!("{}{}", alt, path));
                    }
                }
            }
        } else if mirror_url.contains("endeavouros") {
            // EndeavourOS Mirror Rotation
            let alternates = [
                "https://mirror.moson.org/endeavouros",
                "https://mirror.alpix.eu/endeavouros",
                "https://ca.mirror.babylonix.io/endeavouros",
                "https://mirror.jingk.ai/endeavouros",
            ];
            for alt in alternates {
                if !mirror_url.starts_with(alt) {
                    if let Some(path_index) = mirror_url.find("/repo/") {
                        let path = &mirror_url[path_index..];
                        mirrors_to_try.push(format!("{}{}", alt, path));
                    }
                }
            }
        }

        let mut accumulated_errors = Vec::new();
        let mut success_data = None;

        for (i, url) in mirrors_to_try.iter().enumerate() {
            if i > 0 {
                // println!("Retrying with mirror: {}", url);
            }

            match client.fetch_bytes(url).await {
                Ok(data) => {
                    success_data = Some(data);
                    break;
                }
                Err(e) => {
                    let err = format!("Error from {}: {}", url, e);
                    accumulated_errors.push(err);
                }
            }
        }

        match success_data {
            Some(data) => {
                // Save to cache
                let _ = std::fs::write(&cache_path, &data);
                data
            }
            None => {
                return Err(format!(
                    "All mirrors failed for {}. Errors: [{}]",
                    repo_name,
                    accumulated_errors.join("; ")
                ))
            }
        }
    };

    // Decompress bytes (bytes is Vec<u8> or Bytes)
    let _bytes_len = bytes.len(); // Moved outside closure capture

    // CPU-bound parsing moved to blocking thread to avoid stalling async runtime
    let packages = tokio::task::spawn_blocking(move || {
        // Detect compression based on magic bytes
        let reader: Box<dyn Read + Send> = if bytes.starts_with(&[0x1f, 0x8b]) {
            Box::new(GzDecoder::new(&bytes[..]))
        } else if bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
            match zstd::stream::read::Decoder::new(&bytes[..]) {
                Ok(d) => Box::new(d),
                Err(e) => return Err(e.to_string()),
            }
        } else if bytes.starts_with(&[0xfd, 0x37, 0x7a, 0x58]) {
            Box::new(xz2::read::XzDecoder::new(&bytes[..]))
        } else {
            Box::new(&bytes[..])
        };

        let mut archive = Archive::new(reader);
        let mut packages = Vec::new();

        // Iterate over archive
        // Note: archive.entries() does I/O reading the tar headers
        let entries = archive.entries().map_err(|e| e.to_string())?;

        for file in entries {
            let mut file = file.map_err(|e| e.to_string())?;
            let path = file.path().map_err(|e| e.to_string())?.into_owned();

            if path.file_name().and_then(|n| n.to_str()) == Some("desc") {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    if let Some(pkg) = parse_desc(&content, source.clone()) {
                        packages.push(pkg);
                    }
                }
            }
        }
        Ok(packages)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;

    Ok(packages)
}

fn parse_desc(content: &str, source: PackageSource) -> Option<Package> {
    let mut lines = content.lines();

    let mut name = None;
    let mut version = None;
    let mut desc = None;
    let mut url = None;
    let mut last_modified = None;
    let mut license = Vec::new();
    let mut provides: Option<Vec<String>> = None;

    while let Some(line) = lines.next() {
        match line.trim() {
            "%NAME%" => name = lines.next().map(|s| s.to_string()),
            "%VERSION%" => version = lines.next().map(|s| s.to_string()),
            "%DESC%" => desc = lines.next().map(|s| s.to_string()),
            "%URL%" => url = lines.next().map(|s| s.to_string()),
            "%BUILDDATE%" => {
                if let Some(s) = lines.next() {
                    last_modified = s.parse::<i64>().ok();
                }
            }
            "%LICENSE%" => {
                for l in lines.by_ref() {
                    if l.is_empty() {
                        break;
                    }
                    license.push(l.to_string());
                }
            }
            "%PROVIDES%" => {
                let mut p_list = Vec::new();
                for l in lines.by_ref() {
                    if l.is_empty() {
                        break;
                    }
                    // Strip version info (e.g., "sh=1.0" -> "sh")
                    let clean_name = l.split_once('=').map(|(n, _)| n).unwrap_or(l);
                    p_list.push(clean_name.to_string());
                }
                provides = Some(p_list);
            }
            _ => {}
        }
    }

    if let (Some(name), Some(version)) = (name, version) {
        Some(Package {
            name,
            display_name: None,
            version,
            description: desc.unwrap_or_default(),
            source,
            maintainer: None,
            license: Some(license),
            url,
            last_modified, // Populated from %BUILDDATE%
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: None,
            icon: None,
            screenshots: None,
            provides,
            app_id: None,
        })
    } else {
        None
    }
}

// ----------------------
// Mocks & Tests
// ----------------------

#[cfg(test)]
pub struct MockRepoClient {
    pub responses: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, Result<Vec<u8>, String>>>,
    >,
}

#[cfg(test)]
impl MockRepoClient {
    pub fn new() -> Self {
        Self {
            responses: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn mock_response(&self, url: &str, data: Vec<u8>) {
        self.responses
            .lock()
            .unwrap()
            .insert(url.to_string(), Ok(data));
    }

    pub fn mock_error(&self, url: &str, error: &str) {
        self.responses
            .lock()
            .unwrap()
            .insert(url.to_string(), Err(error.to_string()));
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl RepoClient for MockRepoClient {
    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let responses = self.responses.lock().unwrap();
        if let Some(res) = responses.get(url) {
            res.clone()
        } else {
            Err(format!("Mock 404: {}", url))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PackageSource;

    #[tokio::test]
    async fn test_fetch_repo_packages_success_mock() {
        let mock_client = MockRepoClient::new();
        // Emulate empty response (valid empty file)
        let url = "https://example.com/repo.db";
        mock_client.mock_response(url, vec![]);

        let temp_dir = tempfile::tempdir().unwrap();
        let cache_path = temp_dir.path();

        let result = fetch_repo_packages(
            &mock_client,
            url,
            "test_repo",
            PackageSource::CachyOS,
            cache_path,
            true,
            0,
        )
        .await;

        // An empty file is a valid empty tar archive, so this should succeed.
        // This confirms the network mock delivered the payload.
        assert!(result.is_ok());
        let pkgs = result.unwrap();
        assert!(pkgs.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_repo_all_mirrors_fail() {
        let mock_client = MockRepoClient::new();
        let url = "https://example.com/repo.db";
        mock_client.mock_error(url, "Connection Timeout");

        let temp_dir = tempfile::tempdir().unwrap();
        let cache_path = temp_dir.path();

        let result = fetch_repo_packages(
            &mock_client,
            url,
            "fail_repo",
            PackageSource::CachyOS,
            cache_path,
            true,
            0,
        )
        .await;

        // Verify correct error aggregation
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.contains("All mirrors failed"));
        assert!(err.contains("Connection Timeout"));
    }
}

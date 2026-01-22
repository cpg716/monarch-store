use crate::models::{Package, PackageSource};
use flate2::read::GzDecoder;
use reqwest::Client;
use std::io::Read;
use tar::Archive;

// Struct removed.

pub async fn fetch_repo_packages(
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
        // println!("Loading {} from cache", repo_name);
        std::fs::read(&cache_path).map_err(|e| e.to_string())?
    } else {
        // Download
        let client = Client::builder()
            .user_agent("AURStore/0.1.0 (Tauri; Arch Linux)")
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new());

        let target_url = if mirror_url.ends_with(".db") || mirror_url.ends_with(".db.tar.gz") {
            mirror_url.to_string()
        } else {
            format!("{}/{}.db", mirror_url.trim_end_matches('/'), repo_name)
        };

        let resp = client
            .get(&target_url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Failed to fetch DB: {}", resp.status()));
        }

        let data = resp.bytes().await.map_err(|e| e.to_string())?;

        // Save to cache
        let _ = std::fs::write(&cache_path, &data);

        data.to_vec()
    };

    // Decompress bytes (bytes is Vec<u8> or Bytes)
    let _bytes_len = bytes.len(); // Moved outside closure capture

    // CPU-bound parsing moved to blocking thread to avoid stalling async runtime
    let packages = tokio::task::spawn_blocking(move || {
        // Detect compression based on magic bytes
        let reader: Box<dyn Read + Send> = if bytes.starts_with(&[0x1f, 0x8b]) {
            // println!("DEBUG: Detected Gzip compression for {}", repo_name);
            Box::new(GzDecoder::new(&bytes[..]))
        } else if bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
            // println!("DEBUG: Detected Zstd compression for {}", repo_name);
            match zstd::stream::read::Decoder::new(&bytes[..]) {
                Ok(d) => Box::new(d),
                Err(e) => return Err(e.to_string()),
            }
        } else if bytes.starts_with(&[0xfd, 0x37, 0x7a, 0x58]) {
            // println!("DEBUG: Detected XZ compression for {}", repo_name);
            Box::new(xz2::read::XzDecoder::new(&bytes[..]))
        } else {
            // println!("DEBUG: Assuming uncompressed tar for {}", repo_name);
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

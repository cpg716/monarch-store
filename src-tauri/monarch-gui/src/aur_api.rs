use crate::models::{Package, PackageSource};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::time::Duration;

// Shared HTTP client - created once, reused for all requests
static AUR_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("MonARCH-Store/0.1.0 (Tauri; Arch Linux)")
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

#[derive(Deserialize, Debug)]
struct AurResponse {
    results: Vec<AurPackage>,
    #[serde(default)]
    _resultcount: u32,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct AurPackage {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Maintainer")]
    maintainer: Option<String>,
    #[serde(rename = "NumVotes")]
    num_votes: Option<u32>,
    #[serde(rename = "URL")]
    url: Option<String>,
    #[serde(rename = "License")]
    license: Option<Vec<String>>,
    #[serde(rename = "Keywords")]
    keywords: Option<Vec<String>>,
    #[serde(rename = "LastModified")]
    last_modified: Option<i64>,
    #[serde(rename = "FirstSubmitted")]
    first_submitted: Option<i64>,
    #[serde(rename = "OutOfDate")]
    out_of_date: Option<i64>,
    #[serde(rename = "Depends")]
    depends: Option<Vec<String>>,
    #[serde(rename = "MakeDepends")]
    make_depends: Option<Vec<String>>,
    #[serde(rename = "CheckDepends")]
    check_depends: Option<Vec<String>>,
    #[serde(rename = "Conflicts")]
    conflicts: Option<Vec<String>>,
    #[serde(rename = "Provides")]
    provides: Option<Vec<String>>,
}

// Common function to fetch from AUR API
async fn fetch_aur(url: &str) -> Result<Vec<AurPackage>, String> {
    let resp = AUR_CLIENT
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("AUR API returned error: {}", resp.status()));
    }

    let body: AurResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    if let Some(err) = body.error {
        return Err(format!("AUR API Error: {}", err));
    }

    Ok(body.results)
}

// Convert AurPackage to Package
fn aur_to_package(p: AurPackage) -> Package {
    Package {
        name: p.name,
        display_name: None,
        description: p.description.unwrap_or_default(),
        version: p.version,
        source: PackageSource::Aur,
        maintainer: p.maintainer,
        num_votes: p.num_votes,
        url: p.url,
        license: p.license,
        keywords: p.keywords,
        last_modified: p.last_modified,
        first_submitted: p.first_submitted,
        out_of_date: p.out_of_date,
        icon: None,
        screenshots: None,
        provides: p.provides,
        app_id: None,
        is_optimized: None,
        depends: p.depends,
        make_depends: p.make_depends,
        is_featured: None,
        installed: false,
        ..Default::default()
    }
}

pub async fn search_aur(query: &str) -> Result<Vec<Package>, String> {
    if query.len() < 2 {
        return Ok(vec![]);
    }

    let url = format!("https://aur.archlinux.org/rpc/v5/search/{}", query);
    let mut results = fetch_aur(&url).await?;

    // Sort by votes (popularity) descending
    results.sort_by(|a, b| b.num_votes.unwrap_or(0).cmp(&a.num_votes.unwrap_or(0)));

    Ok(results.into_iter().map(aur_to_package).collect())
}

#[allow(dead_code)]
pub async fn search_aur_by_provides(query: &str) -> Result<Vec<Package>, String> {
    if query.len() < 2 {
        return Ok(vec![]);
    }

    let url = format!(
        "https://aur.archlinux.org/rpc/v5/search/{}?by=provides",
        query
    );
    let mut results = fetch_aur(&url).await?;

    results.sort_by(|a, b| b.num_votes.unwrap_or(0).cmp(&a.num_votes.unwrap_or(0)));

    Ok(results.into_iter().map(aur_to_package).collect())
}

pub async fn get_multi_info(names: &[&str]) -> Result<Vec<Package>, String> {
    if names.is_empty() {
        return Ok(vec![]);
    }

    let mut url = "https://aur.archlinux.org/rpc/v5/info?".to_string();
    for name in names {
        url.push_str(&format!("arg[]={}&", name));
    }

    let results = fetch_aur(&url).await?;
    Ok(results.into_iter().map(aur_to_package).collect())
}

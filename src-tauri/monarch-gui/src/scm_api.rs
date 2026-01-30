// use log::{info, warn};
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScmMetadata {
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub stars: Option<u64>,
    pub license: Option<String>,
    pub last_updated: Option<String>,
    pub screenshots: Vec<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GithubRepo {
    description: Option<String>,
    stargazers_count: Option<u64>,
    updated_at: Option<String>,
    owner: GithubOwner,
    license: Option<GithubLicense>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GithubOwner {
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct GithubLicense {
    name: Option<String>,
}

/// Extract image URLs from Markdown content
fn extract_markdown_images(content: &str, base_raw_url: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"!\[.*?\]\((.*?)\)"#).unwrap();
    let mut images = Vec::new();

    for cap in re.captures_iter(content) {
        if let Some(url) = cap.get(1) {
            let url_str = url.as_str().trim();

            // Filter for image extensions
            let lower = url_str.to_lowercase();
            if !lower.ends_with(".png")
                && !lower.ends_with(".jpg")
                && !lower.ends_with(".jpeg")
                && !lower.ends_with(".gif")
                && !lower.ends_with(".webp")
                && !lower.ends_with(".svg")
            {
                continue;
            }

            // Skip badges (common pattern)
            if lower.contains("badge")
                || lower.contains("shield")
                || lower.contains("travis")
                || lower.contains("codecov")
            {
                continue;
            }

            // Resolve URL
            let resolved = if url_str.starts_with("http") {
                url_str.to_string()
            } else if url_str.starts_with('/') {
                format!("{}{}", base_raw_url.trim_end_matches('/'), url_str)
            } else {
                format!("{}/{}", base_raw_url, url_str)
            };

            images.push(resolved);

            // Limit to 5 screenshots
            if images.len() >= 5 {
                break;
            }
        }
    }

    images
}

pub struct ScmClient {
    cache: Mutex<HashMap<String, Option<ScmMetadata>>>,
    client: reqwest::Client,
}

impl Default for ScmClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ScmClient {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            client: reqwest::Client::builder()
                .user_agent("MonARCH-Store/1.0")
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_metadata(&self, url: &str) -> Option<ScmMetadata> {
        // Basic normalization
        let clean_url = url.trim_end_matches('/');

        // Check cache
        if let Ok(cache) = self.cache.lock() {
            if let Some(cached) = cache.get(clean_url) {
                return cached.clone();
            }
        }

        let metadata = if clean_url.contains("github.com") {
            self.fetch_github(clean_url).await
        } else if clean_url.contains("gitlab.com") {
            self.fetch_gitlab(clean_url).await
        } else {
            None
        };

        // Update cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(clean_url.to_string(), metadata.clone());
        }

        metadata
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    async fn fetch_github(&self, url: &str) -> Option<ScmMetadata> {
        // Parse owner/repo from URL
        // Format: https://github.com/{owner}/{repo}
        let parts: Vec<&str> = url.split("github.com/").collect();
        if parts.len() < 2 {
            return None;
        }

        let path_parts: Vec<&str> = parts[1].split('/').collect();
        if path_parts.len() < 2 {
            return None;
        }

        let owner = path_parts[0];
        let repo = path_parts[1].trim_end_matches(".git");

        // 1. Fast Path: Construct Avatar URL directly (No API rate limit)
        // This is extremely fast and reliable for basic icons
        let avatar_url = format!("https://github.com/{}.png", owner);

        // 2. Try API for rich data (Description, Stars, License)
        // Rate limited to 60/hr for unauth, but worth trying
        let api_url = format!("https://api.github.com/repos/{}/{}", owner, repo);

        let resp = self.client.get(&api_url).send().await;

        match resp {
            Ok(response) => {
                if response.status().is_success() {
                    if let Ok(repo_data) = response.json::<GithubRepo>().await {
                        // Fetch README for screenshots
                        let screenshots = self.fetch_github_readme_images(owner, repo).await;

                        return Some(ScmMetadata {
                            description: repo_data.description,
                            icon_url: Some(avatar_url),
                            stars: repo_data.stargazers_count,
                            license: repo_data.license.map(|l| l.name).unwrap_or(None),
                            last_updated: repo_data.updated_at,
                            screenshots,
                        });
                    }
                }
            }
            Err(_) => {
                // If API fails (rate limit/network), at least return the avatar!
                // This ensures "Fast, Smooth, Efficient"
                return Some(ScmMetadata {
                    description: None,
                    icon_url: Some(avatar_url),
                    stars: None,
                    license: None,
                    last_updated: None,
                    screenshots: vec![],
                });
            }
        }

        // Fallback if API failed (e.g. 404) but we still might want the avatar?
        // If the repo is 404, the user might still exist.
        Some(ScmMetadata {
            description: None,
            icon_url: Some(avatar_url),
            stars: None,
            license: None,
            last_updated: None,
            screenshots: vec![],
        })
    }

    async fn fetch_gitlab(&self, url: &str) -> Option<ScmMetadata> {
        // GitLab's API is similar but less strict on User-Agent.
        // Public API: https://gitlab.com/api/v4/projects/{owner}%2F{repo}

        let parts: Vec<&str> = url.split("gitlab.com/").collect();
        if parts.len() < 2 {
            return None;
        }

        let path = parts[1].trim_end_matches(".git").trim_end_matches('/');
        // URL encode the path for the API
        let encoded_path = path.replace('/', "%2F");

        let api_url = format!("https://gitlab.com/api/v4/projects/{}", encoded_path);

        // GitLab doesn't have a simple deterministic avatar URL pattern for projects
        // without knowing the project ID or checking the API.

        if let Ok(response) = self.client.get(&api_url).send().await {
            if response.status().is_success() {
                // Quick parse for just essential fields using untyped json value
                // to avoid defining complex structs for GitLab
                if let Ok(val) = response.json::<serde_json::Value>().await {
                    let description = val
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let avatar = val
                        .get("avatar_url")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let stars = val.get("star_count").and_then(|v| v.as_u64());

                    // Fetch README for screenshots
                    let screenshots = self.fetch_gitlab_readme_images(&encoded_path).await;

                    return Some(ScmMetadata {
                        description,
                        icon_url: avatar,
                        stars,
                        license: None,
                        last_updated: val
                            .get("last_activity_at")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        screenshots,
                    });
                }
            }
        }

        None
    }

    /// Fetch README from GitHub and extract images
    async fn fetch_github_readme_images(&self, owner: &str, repo: &str) -> Vec<String> {
        // Try common README filenames via raw.githubusercontent.com
        let branches = ["main", "master"];
        let readme_names = ["README.md", "readme.md", "Readme.md"];

        for branch in branches {
            for readme in readme_names {
                let raw_url = format!(
                    "https://raw.githubusercontent.com/{}/{}/{}/{}",
                    owner, repo, branch, readme
                );

                if let Ok(resp) = self
                    .client
                    .get(&raw_url)
                    .header(USER_AGENT, "MonARCH-Store/1.0")
                    .send()
                    .await
                {
                    if resp.status().is_success() {
                        if let Ok(content) = resp.text().await {
                            let base_raw = format!(
                                "https://raw.githubusercontent.com/{}/{}/{}",
                                owner, repo, branch
                            );
                            let images = extract_markdown_images(&content, &base_raw);
                            if !images.is_empty() {
                                return images;
                            }
                        }
                    }
                }
            }
        }

        vec![]
    }

    /// Fetch README from GitLab and extract images
    async fn fetch_gitlab_readme_images(&self, encoded_path: &str) -> Vec<String> {
        // GitLab API for raw file content
        let readme_url = format!(
            "https://gitlab.com/api/v4/projects/{}/repository/files/README.md/raw?ref=main",
            encoded_path
        );

        // Try main branch first
        if let Ok(resp) = self.client.get(&readme_url).send().await {
            if resp.status().is_success() {
                if let Ok(content) = resp.text().await {
                    let base_raw = format!(
                        "https://gitlab.com/api/v4/projects/{}/repository/files",
                        encoded_path
                    );
                    let images = extract_markdown_images(&content, &base_raw);
                    if !images.is_empty() {
                        return images;
                    }
                }
            }
        }

        // Try master branch
        let readme_url_master = format!(
            "https://gitlab.com/api/v4/projects/{}/repository/files/README.md/raw?ref=master",
            encoded_path
        );

        if let Ok(resp) = self.client.get(&readme_url_master).send().await {
            if resp.status().is_success() {
                if let Ok(content) = resp.text().await {
                    let base_raw = format!(
                        "https://gitlab.com/api/v4/projects/{}/repository/files",
                        encoded_path
                    );
                    return extract_markdown_images(&content, &base_raw);
                }
            }
        }

        vec![]
    }
}

//! Operation Town Crier: Distro-aware news aggregation and safety gate.
//! Fetches the correct RSS/Atom feeds for the host OS and marks critical items
//! (Manual Intervention, Stable Update, Security/CVE) so the Updates page can block blind updates.

use crate::distro_context::DistroContext;
use feed_rs::parser;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::io::Cursor;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::State;

static FEED_WARNING_CACHE: Lazy<Mutex<std::collections::HashMap<String, Instant>>> =
    Lazy::new(|| Mutex::new(std::collections::HashMap::new()));
const FEED_WARNING_TTL: Duration = Duration::from_secs(600);

/// news category for grouping in the UI.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum NewsCategory {
    Critical,
    System,
    Discovery,
}

/// Single news item from any distro or Flathub feed.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub link: String,
    pub pub_date: String,
    pub source_label: String,
    pub is_critical: bool,
    pub category: NewsCategory,
    /// Article body/summary for inline display (from feed content or summary).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Feed URL and label for distro-agnostic fetch.
struct FeedSpec {
    url: &'static str,
    label: &'static str,
}

fn feed_specs_for_distro(distro_id: &str) -> Vec<FeedSpec> {
    let mut specs = vec![FeedSpec {
        url: "https://flathub.org/api/v2/feed/new",
        label: "Flathub",
    }];

    match distro_id {
        "arch" => {
            specs.push(FeedSpec {
                url: "https://archlinux.org/feeds/news/",
                label: "Arch News",
            });
        }
        "manjaro" => {
            specs.push(FeedSpec {
                url: "https://forum.manjaro.org/c/announcements.rss",
                label: "Manjaro Stable",
            });
        }
        "garuda" => {
            specs.push(FeedSpec {
                url: "https://forum.garudalinux.org/c/announcements.rss",
                label: "Garuda News",
            });
        }
        "endeavouros" => {
            specs.push(FeedSpec {
                url: "https://endeavouros.com/feed/",
                label: "EndeavourOS",
            });
        }
        "cachyos" => {
            specs.push(FeedSpec {
                url: "https://discuss.cachyos.org/c/announcements.rss",
                label: "CachyOS",
            });
            specs.push(FeedSpec {
                url: "https://archlinux.org/feeds/news/",
                label: "Arch News",
            });
        }
        _ => {
            specs.push(FeedSpec {
                url: "https://archlinux.org/feeds/news/",
                label: "Arch News",
            });
        }
    }

    specs
}

fn log_feed_warning_once(url: &str, message: impl FnOnce() -> String) {
    let Ok(mut cache) = FEED_WARNING_CACHE.lock() else {
        log::warn!("{}", message());
        return;
    };

    let now = Instant::now();
    cache.retain(|_, last_seen| now.duration_since(*last_seen) < FEED_WARNING_TTL);

    if let Some(last_seen) = cache.get(url) {
        if now.duration_since(*last_seen) < FEED_WARNING_TTL {
            return;
        }
    }

    cache.insert(url.to_string(), now);
    log::warn!("{}", message());
}

fn is_critical_title(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.contains("manual intervention")
        || lower.contains("stable update")
        || lower.contains("security")
        || lower.contains("cve")
        || lower.contains("vulnerability")
}

async fn fetch_one_feed(client: &reqwest::Client, url: &str, label: &str) -> Vec<NewsItem> {
    let resp = match client
        .get(url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            let status = r.status();
            log_feed_warning_once(url, || format!("News feed {} returned status {}", url, status));
            return vec![];
        }
        Err(e) => {
            log_feed_warning_once(url, || format!("News feed {} failed: {}", url, e));
            return vec![];
        }
    };

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            log_feed_warning_once(url, || format!("News feed {} body read failed: {}", url, e));
            return vec![];
        }
    };

    let feed = match parser::parse(Cursor::new(bytes.as_ref())) {
        Ok(f) => f,
        Err(e) => {
            log_feed_warning_once(url, || format!("News feed {} parse failed: {}", url, e));
            return vec![];
        }
    };

    let mut items = Vec::new();
    let category = if label.to_lowercase().contains("flathub") {
        NewsCategory::Discovery
    } else {
        NewsCategory::System
    };

    for entry in feed.entries {
        let title = entry
            .title
            .map(|t| t.content)
            .unwrap_or_else(|| "Untitled".to_string());
        let link = entry
            .links
            .first()
            .map(|l| l.href.clone())
            .unwrap_or_else(String::new);
        let pub_date = entry
            .updated
            .or(entry.published)
            .map(|d| d.to_rfc2822())
            .unwrap_or_else(String::new);
        let content = entry
            .content
            .as_ref()
            .and_then(|c| c.body.clone())
            .or_else(|| entry.summary.as_ref().map(|s| s.content.clone()));
        let id = if entry.id.is_empty() {
            if link.is_empty() {
                format!("{}:{}", label, title.chars().take(64).collect::<String>())
            } else {
                link.clone()
            }
        } else {
            entry.id
        };

        let is_critical = is_critical_title(&title);
        let final_category = if is_critical {
            NewsCategory::Critical
        } else {
            category.clone()
        };

        items.push(NewsItem {
            id,
            title,
            link,
            pub_date,
            source_label: label.to_string(),
            is_critical,
            category: final_category,
            content,
        });
    }
    items
}

fn parse_rfc2822_opt(s: &str) -> Option<std::time::SystemTime> {
    use std::time::UNIX_EPOCH;
    chrono::DateTime::parse_from_rfc2822(s).ok().map(|dt| {
        let secs = dt.timestamp();
        let nsecs = dt.timestamp_subsec_nanos();
        UNIX_EPOCH + std::time::Duration::new(secs as u64, nsecs)
    })
}

/// Fetches and normalizes news from distro-specific feeds plus Flathub.
#[tauri::command]
#[specta::specta]
pub async fn fetch_news(state_distro: State<'_, DistroContext>) -> Result<Vec<NewsItem>, String> {
    let distro_id = state_distro.id_str();
    let specs = feed_specs_for_distro(distro_id);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("MonARCH-Store/1.0 (https://github.com/cpg716/monarch-store)")
        .build()
        .map_err(|e| e.to_string())?;

    let mut all = Vec::new();
    let mut tasks = Vec::new();

    for spec in specs {
        tasks.push(fetch_one_feed(&client, spec.url, spec.label));
    }

    let results = futures::future::join_all(tasks).await;
    for items in results {
        all.extend(items);
    }

    all.sort_by(|a, b| {
        // First sort by category priority
        let a_cat_pri = match a.category {
            NewsCategory::Critical => 0,
            NewsCategory::System => 1,
            NewsCategory::Discovery => 2,
        };
        let b_cat_pri = match b.category {
            NewsCategory::Critical => 0,
            NewsCategory::System => 1,
            NewsCategory::Discovery => 2,
        };

        if a_cat_pri != b_cat_pri {
            return a_cat_pri.cmp(&b_cat_pri);
        }

        // Then by date
        let a_ts = parse_rfc2822_opt(&a.pub_date);
        let b_ts = parse_rfc2822_opt(&b.pub_date);
        match (a_ts, b_ts) {
            (Some(at), Some(bt)) => bt.cmp(&at),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    Ok(all)
}

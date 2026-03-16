use crate::models::{NewsCategory, NewsItem};
use feed_rs::parser;
use once_cell::sync::Lazy;
use std::io::Cursor;
use std::sync::Mutex;
use std::time::{Duration, Instant};

static FEED_WARNING_CACHE: Lazy<Mutex<std::collections::HashMap<String, Instant>>> =
    Lazy::new(|| Mutex::new(std::collections::HashMap::new()));
const FEED_WARNING_TTL: Duration = Duration::from_secs(600);

struct FeedSpec {
    url: &'static str,
    label: &'static str,
}

pub async fn fetch_news() -> Result<Vec<NewsItem>, String> {
    let specs = feed_specs_for_distro(&current_distro_id());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("MonARCH-Store-GTK/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let mut tasks = Vec::new();
    for spec in specs {
        tasks.push(fetch_one_feed(&client, spec.url, spec.label));
    }

    let mut all = Vec::new();
    for items in futures::future::join_all(tasks).await {
        all.extend(items);
    }

    all.sort_by(|a, b| {
        let a_priority = category_priority(&a.category);
        let b_priority = category_priority(&b.category);
        a_priority
            .cmp(&b_priority)
            .then_with(|| parse_rfc2822_opt(&b.pub_date).cmp(&parse_rfc2822_opt(&a.pub_date)))
    });

    Ok(all)
}

fn feed_specs_for_distro(distro_id: &str) -> Vec<FeedSpec> {
    let mut specs = vec![FeedSpec {
        url: "https://flathub.org/api/v2/feed/new",
        label: "Flathub",
    }];

    match distro_id {
        "arch" => specs.push(FeedSpec {
            url: "https://archlinux.org/feeds/news/",
            label: "Arch News",
        }),
        "manjaro" => specs.push(FeedSpec {
            url: "https://forum.manjaro.org/c/announcements.rss",
            label: "Manjaro Stable",
        }),
        "garuda" => specs.push(FeedSpec {
            url: "https://forum.garudalinux.org/c/announcements.rss",
            label: "Garuda News",
        }),
        "endeavouros" => specs.push(FeedSpec {
            url: "https://endeavouros.com/feed/",
            label: "EndeavourOS",
        }),
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
        _ => specs.push(FeedSpec {
            url: "https://archlinux.org/feeds/news/",
            label: "Arch News",
        }),
    }

    specs
}

fn current_distro_id() -> String {
    let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    for line in os_release.lines() {
        if let Some(value) = line.strip_prefix("ID=") {
            return value.trim_matches('"').to_lowercase();
        }
    }
    "arch".to_string()
}

async fn fetch_one_feed(client: &reqwest::Client, url: &str, label: &str) -> Vec<NewsItem> {
    let response = match client.get(url).send().await {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => {
            log_feed_warning_once(url, || format!("News feed {} returned {}", url, response.status()));
            return Vec::new();
        }
        Err(error) => {
            log_feed_warning_once(url, || format!("News feed {} failed: {}", url, error));
            return Vec::new();
        }
    };

    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            log_feed_warning_once(url, || format!("News feed {} body read failed: {}", url, error));
            return Vec::new();
        }
    };

    let feed = match parser::parse(Cursor::new(bytes.as_ref())) {
        Ok(feed) => feed,
        Err(error) => {
            log_feed_warning_once(url, || format!("News feed {} parse failed: {}", url, error));
            return Vec::new();
        }
    };

    let mut items = Vec::new();
    for entry in feed.entries {
        let title = entry
            .title
            .map(|title| title.content)
            .unwrap_or_else(|| "Untitled".to_string());
        let link = entry.links.first().map(|link| link.href.clone()).unwrap_or_default();
        let pub_date = entry
            .updated
            .or(entry.published)
            .map(|date| date.to_rfc2822())
            .unwrap_or_default();
        let content = entry
            .content
            .as_ref()
            .and_then(|content| content.body.clone())
            .or_else(|| entry.summary.as_ref().map(|summary| summary.content.clone()));
        let is_critical = is_critical_title(&title);
        let category = if is_critical {
            NewsCategory::Critical
        } else if label.eq_ignore_ascii_case("Flathub") {
            NewsCategory::Discovery
        } else {
            NewsCategory::System
        };

        items.push(NewsItem {
            id: if entry.id.is_empty() {
                if link.is_empty() {
                    format!("{}:{}", label, title)
                } else {
                    link.clone()
                }
            } else {
                entry.id
            },
            title,
            link,
            pub_date,
            source_label: label.to_string(),
            is_critical,
            category,
            content,
        });
    }

    items
}

fn category_priority(category: &NewsCategory) -> u8 {
    match category {
        NewsCategory::Critical => 0,
        NewsCategory::System => 1,
        NewsCategory::Discovery => 2,
    }
}

fn is_critical_title(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.contains("manual intervention")
        || lower.contains("stable update")
        || lower.contains("security")
        || lower.contains("cve")
        || lower.contains("vulnerability")
}

fn parse_rfc2822_opt(value: &str) -> Option<std::time::SystemTime> {
    use std::time::UNIX_EPOCH;
    chrono::DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|date| UNIX_EPOCH + std::time::Duration::new(date.timestamp() as u64, date.timestamp_subsec_nanos()))
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

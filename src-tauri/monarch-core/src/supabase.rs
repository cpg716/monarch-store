//! Supabase integration for MonARCH community reviews.
//!
//! Reads `SUPABASE_URL` and `SUPABASE_ANON_KEY` from env; falls back to defaults when unset.
//! TLS uses both webpki-roots and system certificates (rustls-tls-native-roots) for compatibility.
//! Set `SUPABASE_DISABLED=1` (or `true`) to skip all requests. Failures are logged with connect/timeout/request detail.

use crate::models::PackageReview;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

const DEFAULT_URL: &str = "https://tcmbahxvwhcetfbtnxlj.supabase.co";
const DEFAULT_ANON_KEY: &str = "sb_publishable_H5McpvxB2eoujN9LNzhy1w_lJup-ZcX";

#[derive(Debug, Deserialize)]
struct SupabaseReviewRow {
    #[allow(dead_code)]
    id: Option<serde_json::Value>,
    package_name: Option<String>,
    rating: Option<u32>,
    comment: Option<String>,
    user_name: Option<String>,
    created_at: Option<String>,
}

/// If set to "1" or "true", skip all Supabase requests (e.g. when offline or keys unavailable).
fn disabled() -> bool {
    std::env::var("SUPABASE_DISABLED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn config() -> (String, String) {
    let url = std::env::var("SUPABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
    let key = std::env::var("SUPABASE_ANON_KEY").unwrap_or_else(|_| DEFAULT_ANON_KEY.to_string());
    (url, key)
}

fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("reqwest client")
}

/// Log the underlying cause of a reqwest/network error for debugging.
fn log_send_error(e: &reqwest::Error, url: &str) {
    if e.is_connect() {
        log::warn!("Supabase connect error (no route to host, refused, or DNS): {}", e);
    } else if e.is_timeout() {
        log::warn!("Supabase timeout: {}", e);
    } else if e.is_request() {
        log::warn!("Supabase request error: {}", e);
    } else {
        log::warn!("Supabase error for {}: {} (status: {:?})", url, e, e.status());
    }
}

/// Fetch reviews for a package from Supabase. Returns empty vec on missing config or API error.
pub async fn fetch_reviews(app_id: &str) -> Result<Vec<PackageReview>, String> {
    if disabled() {
        return Ok(vec![]);
    }
    let (base_url, anon_key) = config();
    // PostgREST filter: package_name=eq."value" for app ids with dots; quote and encode only chars that break URL
    let raw = app_id.trim();
    let value = if raw.contains('.') || raw.contains('"') || raw.contains(' ') {
        let safe = raw
            .replace('"', "%22")
            .replace('&', "%26")
            .replace('=', "%3D")
            .replace(' ', "%20");
        format!("%22{}%22", safe)
    } else {
        raw.to_string()
    };
    let url = format!(
        "{}/rest/v1/reviews?package_name=eq.{}&order=created_at.desc",
        base_url.trim_end_matches('/'),
        value,
    );
    let client = client();
    let resp = client
        .get(&url)
        .header("apikey", &anon_key)
        .header("Authorization", format!("Bearer {}", anon_key))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| {
            log_send_error(&e, &url);
            format!("Supabase fetch_reviews: {} (url: {})", e, url)
        })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.as_u16() == 503 {
            log::warn!("Supabase returned 503 (project may have been paused). Unpause in the Supabase dashboard and try again.");
        }
        return Err(format!("Supabase reviews fetch failed: {} {}", status, body));
    }
    let rows: Vec<SupabaseReviewRow> = resp.json().await.map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let app_id = r.package_name.unwrap_or_default();
        let rating = r.rating.unwrap_or(0).clamp(1, 5);
        let description = r.comment.unwrap_or_default();
        let user_display = r
            .user_name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "MonARCH user".to_string());
        let date_created = r
            .created_at
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc).timestamp() as f64);
        out.push(PackageReview {
            review_id: None,
            app_id,
            user_display: Some(user_display),
            summary: None,
            description: if description.trim().is_empty() {
                None
            } else {
                Some(description)
            },
            rating: Some(rating),
            date_created,
            version: Some("MonARCH".to_string()),
            distro: Some("MonARCH".to_string()),
            locale: None,
        });
    }
    Ok(out)
}

/// Submit a review to Supabase. Fails if the API request fails.
pub async fn submit_review(
    app_id: &str,
    rating: u32,
    comment: &str,
    user_name: &str,
) -> Result<(), String> {
    if disabled() {
        return Err("Supabase is disabled (SUPABASE_DISABLED)".to_string());
    }
    let (base_url, anon_key) = config();
    let url = format!("{}/rest/v1/reviews", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "package_name": app_id.trim(),
        "rating": rating.clamp(1, 5),
        "comment": comment.trim(),
        "user_name": user_name.trim()
    });
    let client = client();
    let resp = client
        .post(&url)
        .header("apikey", &anon_key)
        .header("Authorization", format!("Bearer {}", anon_key))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=minimal")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| {
            log_send_error(&e, &url);
            e.to_string()
        })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Supabase submit failed: {} {}", status, text));
    }
    Ok(())
}

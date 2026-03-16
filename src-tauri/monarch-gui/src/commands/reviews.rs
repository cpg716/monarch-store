use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tauri::AppHandle;
use tauri::Manager;

const SUPABASE_URL: &str = "https://tcmbahxvwhcetfbtnxlj.supabase.co";
const SUPABASE_ANON_KEY: &str = "sb_publishable_H5McpvxB2eoujN9LNzhy1w_lJup-ZcX";

async fn submit_review_to_supabase(
    app_id: &str,
    rating: u32,
    comment: &str,
    user_name: &str,
) -> Result<(), String> {
    if std::env::var("SUPABASE_DISABLED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Err("Supabase disabled".into());
    }
    let url = std::env::var("SUPABASE_URL").unwrap_or_else(|_| SUPABASE_URL.to_string());
    let key = std::env::var("SUPABASE_ANON_KEY").unwrap_or_else(|_| SUPABASE_ANON_KEY.to_string());
    let url = format!("{}/rest/v1/reviews", url.trim_end_matches('/'));
    let body = serde_json::json!({
        "package_name": app_id.trim(),
        "rating": rating.clamp(1, 5),
        "comment": comment.trim(),
        "user_name": user_name.trim()
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .header("apikey", &key)
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=minimal")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Supabase submit failed: {} {}", status, text));
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct LocalReview {
    pub app_id: String,
    pub rating: u32,
    pub summary: String,
    pub description: String,
    pub user_display: String,
    pub date_created: u64,
}

fn get_reviews_path(app: &AppHandle) -> PathBuf {
    let mut path = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.push("reviews.json");
    path
}

#[tauri::command]
#[specta::specta]
pub async fn submit_review(
    app: AppHandle,
    app_id: String,
    rating: u32,
    summary: String,
    description: String,
    user_display: String,
) -> Result<(), String> {
    let path = get_reviews_path(&app);
    let mut reviews: Vec<LocalReview> = if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    let new_review = LocalReview {
        app_id: app_id.clone(),
        rating,
        summary: summary.clone(),
        description: description.clone(),
        user_display: user_display.clone(),
        date_created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    reviews.push(new_review);
    let content = serde_json::to_string_pretty(&reviews).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())?;

    // Also push to Supabase (MonARCH community backend). Local save already succeeded.
    if let Err(e) = submit_review_to_supabase(&app_id, rating, &description, &user_display).await {
        log::warn!("Supabase review submit failed (local save ok): {}", e);
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_local_reviews(app: AppHandle, app_id: String) -> Result<Vec<LocalReview>, String> {
    let path = get_reviews_path(&app);
    if !path.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let reviews: Vec<LocalReview> = serde_json::from_str(&content).unwrap_or_default();

    Ok(reviews.into_iter().filter(|r| r.app_id == app_id).collect())
}

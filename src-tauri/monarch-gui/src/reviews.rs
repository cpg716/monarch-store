use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalReview {
    pub app_id: String,
    pub rating: u32,
    pub summary: String,
    pub description: String,
    pub user_display: String,
    pub date_created: u64,
}

fn get_reviews_path(app: &AppHandle) -> PathBuf {
    let mut path = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.push("reviews.json");
    path
}

#[tauri::command]
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
        app_id,
        rating,
        summary,
        description,
        user_display,
        date_created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    reviews.push(new_review);
    let content = serde_json::to_string_pretty(&reviews).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_local_reviews(app: AppHandle, app_id: String) -> Result<Vec<LocalReview>, String> {
    let path = get_reviews_path(&app);
    if !path.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let reviews: Vec<LocalReview> = serde_json::from_str(&content).unwrap_or_default();
    
    Ok(reviews.into_iter().filter(|r| r.app_id == app_id).collect())
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct OdrsResponse {
    #[serde(flatten)]
    pub ratings: HashMap<String, OdrsRating>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OdrsRating {
    pub star1: u32,
    pub star2: u32,
    pub star3: u32,
    pub star4: u32,
    pub star5: u32,
    pub total: u32,
    pub score: Option<f64>, // ODRS returns 'score' (average) sometimes or we calc it
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Review {
    pub review_id: Option<u64>,
    pub app_id: String,
    pub user_display: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub rating: Option<u32>,
    pub date_created: Option<f64>,
    pub version: Option<String>,
    pub distro: Option<String>,
}

// Fetch basic rating summary
#[tauri::command]
pub async fn get_app_rating(app_id: String) -> Result<Option<OdrsRating>, String> {
    let url = format!("https://odrs.gnome.org/1.0/reviews/api/ratings/{}", app_id);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return Ok(None), // Silence timeouts/network errors
    };

    if !resp.status().is_success() {
        return Ok(None);
    }

    let body: OdrsResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body.ratings.get(&app_id).cloned())
}

#[tauri::command]
pub async fn get_app_ratings_batch(
    app_ids: Vec<String>,
) -> Result<HashMap<String, OdrsRating>, String> {
    let futures = app_ids.into_iter().map(|id| {
        let id_clone = id.clone();
        async move {
            match get_app_rating(id_clone.clone()).await {
                Ok(Some(rating)) => Some((id_clone, rating)),
                _ => None,
            }
        }
    });

    let results = futures::future::join_all(futures).await;

    let mut map = HashMap::new();
    for res in results.into_iter().flatten() {
        map.insert(res.0, res.1);
    }

    Ok(map)
}

// Fetch detailed reviews
#[tauri::command]
pub async fn get_app_reviews(app_id: String) -> Result<Vec<Review>, String> {
    let url = format!("https://odrs.gnome.org/1.0/reviews/api/app/{}", app_id);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return Ok(vec![]), // Silence timeouts/network errors
    };

    if !resp.status().is_success() {
        // Silence 5xx (Server/Gateway) and 404 (Not Found)
        if !resp.status().is_server_error() && resp.status() != reqwest::StatusCode::NOT_FOUND {
            println!("ODRS Note: {} returned {}", app_id, resp.status());
        }
        return Ok(vec![]);
    }

    let text = resp.text().await.map_err(|e| e.to_string())?;

    let reviews: Vec<Review> = serde_json::from_str(&text).map_err(|e| {
        // Only log parsing errors for successful responses
        println!("ODRS Parse Error for {}: {}", app_id, e);
        e.to_string()
    })?;

    Ok(reviews)
}

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
pub async fn get_app_rating(app_id: &str) -> Result<Option<OdrsRating>, String> {
    let url = format!("https://odrs.gnome.org/1.0/reviews/api/ratings/{}", app_id);

    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Ok(None); // No ratings found often returns 404 or empty
    }

    // ODRS returns a map { "app.id": { ... } }
    let body: HashMap<String, OdrsRating> = resp.json().await.map_err(|e| e.to_string())?;

    Ok(body.get(app_id).cloned())
}

// Fetch detailed reviews
pub async fn get_app_reviews(app_id: &str) -> Result<Vec<Review>, String> {
    // ODRS usually returns reviews via a different endpoint or POST
    // For simplicity, we might just stick to ratings first, but let's try the fetch
    // URL: https://odrs.gnome.org/1.0/reviews/api/app/{app_id}

    let url = format!("https://odrs.gnome.org/1.0/reviews/api/app/{}", app_id);
    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Ok(vec![]);
    }

    // Debug: Print raw body to see what failed
    let text = resp.text().await.map_err(|e| e.to_string())?;
    // println!("ODRS Response for {}: {}", app_id, text); // Uncomment for verbose debug

    // Try to parse text as string
    let reviews: Vec<Review> = serde_json::from_str(&text).map_err(|e| {
        println!("ODRS Parse Error for {}: {}", app_id, e);
        // println!("Raw Body: {}", text);
        e.to_string()
    })?;
    Ok(reviews)
}

use crate::models::{OdrsRating, PackageReview};
use std::collections::HashMap;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct OdrsResponse {
    #[serde(flatten)]
    ratings: HashMap<String, OdrsRating>,
}

fn sanitize_f64(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite())
}

pub async fn get_app_rating(app_id: impl Into<String>) -> Result<Option<OdrsRating>, String> {
    let app_id = app_id.into();
    let url = format!("https://odrs.gnome.org/1.0/reviews/api/ratings/{app_id}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = match client.get(&url).send().await {
        Ok(resp) => resp,
        Err(_) => return Ok(None),
    };
    if !resp.status().is_success() {
        return Ok(None);
    }

    let body: OdrsResponse = resp.json().await.map_err(|e| e.to_string())?;
    let mut rating = body.ratings.get(&app_id).cloned();
    if let Some(ref mut value) = rating {
        value.score = sanitize_f64(value.score);
    }
    Ok(rating)
}

type OdrsRatingsCache =
    std::sync::RwLock<Option<(std::time::Instant, HashMap<String, OdrsRating>)>>;
static RATINGS_CACHE: std::sync::OnceLock<OdrsRatingsCache> = std::sync::OnceLock::new();

fn get_ratings_cache() -> &'static OdrsRatingsCache {
    RATINGS_CACHE.get_or_init(|| std::sync::RwLock::new(None))
}

pub async fn get_all_ratings() -> Result<HashMap<String, OdrsRating>, String> {
    if let Some((timestamp, ref cache)) = *get_ratings_cache().read().unwrap() {
        if timestamp.elapsed() < std::time::Duration::from_secs(3600 * 24) {
            return Ok(cache.clone());
        }
    }

    let url = "https://odrs.gnome.org/1.0/reviews/api/ratings";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = match client.get(url).send().await {
        Ok(resp) => resp,
        Err(_) => return Ok(HashMap::new()),
    };
    if !resp.status().is_success() {
        return Ok(HashMap::new());
    }

    let text = resp.text().await.map_err(|e| e.to_string())?;
    let mut ratings: HashMap<String, OdrsRating> = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Ok(HashMap::new()),
    };
    for rating in ratings.values_mut() {
        rating.score = sanitize_f64(rating.score);
    }

    *get_ratings_cache().write().unwrap() = Some((std::time::Instant::now(), ratings.clone()));
    Ok(ratings)
}

pub async fn get_app_ratings_batch(
    app_ids: Vec<String>,
) -> Result<HashMap<String, OdrsRating>, String> {
    let all = match get_all_ratings().await {
        Ok(all) => all,
        Err(_) => return Ok(HashMap::new()),
    };

    let mut result = HashMap::new();
    for id in app_ids {
        let mut check_keys = vec![id.clone()];
        if !id.ends_with(".desktop") {
            check_keys.push(format!("{id}.desktop"));
        }
        if let Some(last) = id.split('.').next_back() {
            if last != id && !last.is_empty() {
                check_keys.push(last.to_string());
                if !last.ends_with(".desktop") {
                    check_keys.push(format!("{last}.desktop"));
                }
            }
        }

        for key in check_keys {
            if let Some(rating) = all.get(&key) {
                result.insert(id.clone(), rating.clone());
                break;
            }
        }
    }
    Ok(result)
}

pub async fn get_app_reviews(app_id: impl Into<String>) -> Result<Vec<PackageReview>, String> {
    let app_id = app_id.into();
    let url = format!("https://odrs.gnome.org/1.0/reviews/api/app/{app_id}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = match client.get(&url).send().await {
        Ok(resp) => resp,
        Err(_) => return Ok(Vec::new()),
    };
    if !resp.status().is_success() {
        return Ok(Vec::new());
    }

    let text = resp.text().await.map_err(|e| e.to_string())?;
    let mut reviews: Vec<PackageReview> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    for review in &mut reviews {
        review.date_created = sanitize_f64(review.date_created);
    }
    Ok(reviews)
}

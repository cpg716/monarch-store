use crate::models::LocalReview;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LocalReviewStore {
    path: PathBuf,
}

impl LocalReviewStore {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            path: reviews_path()?,
        })
    }

    pub fn load_for_app(&self, app_id: &str) -> Result<Vec<LocalReview>, String> {
        let normalized = app_id.trim();
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self
            .load_all()?
            .into_iter()
            .filter(|review| review.app_id == normalized)
            .collect())
    }

    pub fn submit(
        &self,
        app_id: String,
        rating: u32,
        summary: String,
        description: String,
        user_display: String,
    ) -> Result<LocalReview, String> {
        let normalized_app_id = app_id.trim().to_string();
        if normalized_app_id.is_empty() {
            return Err("Review app ID cannot be empty.".to_string());
        }

        let normalized_summary = summary.trim().to_string();
        let normalized_description = description.trim().to_string();
        if normalized_summary.is_empty() && normalized_description.is_empty() {
            return Err("Review text cannot be empty.".to_string());
        }

        let review = LocalReview {
            app_id: normalized_app_id,
            rating: rating.clamp(1, 5),
            summary: normalized_summary,
            description: normalized_description,
            user_display: user_display.trim().to_string(),
            date_created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_secs(),
        };

        let mut reviews = self.load_all()?;
        reviews.push(review.clone());
        write_reviews(&self.path, &reviews)?;
        Ok(review)
    }

    fn load_all(&self) -> Result<Vec<LocalReview>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }
}

fn reviews_path() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("monarch-store");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("reviews.json"))
}

fn write_reviews(path: &PathBuf, reviews: &[LocalReview]) -> Result<(), String> {
    let content = serde_json::to_string_pretty(reviews).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_review_store_filters_by_app_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("reviews.json");
        write_reviews(
            &path,
            &[
                LocalReview {
                    app_id: "firefox".to_string(),
                    rating: 5,
                    summary: "Great".to_string(),
                    description: "Solid browser".to_string(),
                    user_display: "Alice".to_string(),
                    date_created: 1,
                },
                LocalReview {
                    app_id: "vlc".to_string(),
                    rating: 4,
                    summary: "Good".to_string(),
                    description: "Plays everything".to_string(),
                    user_display: "Bob".to_string(),
                    date_created: 2,
                },
            ],
        )
        .expect("write reviews");

        let store = LocalReviewStore { path };
        let reviews = store.load_for_app("firefox").expect("load reviews");
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].user_display, "Alice");
    }
}

//! Minimal Flathub API fallback to fetch screenshots when registry/bootstrap have none.
//! Used so repo/AUR packages with app_id (e.g. org.ardour.Ardour) can show Flathub screenshots.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FlathubScreenshotSize {
    width: String,
    #[allow(dead_code)]
    height: String,
    src: String,
}

#[derive(Debug, Deserialize)]
struct FlathubScreenshot {
    #[serde(rename = "624x351", default)]
    size_624: Option<String>,
    #[serde(rename = "752x423", default)]
    size_752: Option<String>,
    #[serde(rename = "1248x702", default)]
    size_1248: Option<String>,
    #[serde(default)]
    sizes: Option<Vec<FlathubScreenshotSize>>,
}

#[derive(Debug, Deserialize)]
struct FlathubAppstream {
    #[serde(default)]
    screenshots: Vec<FlathubScreenshot>,
}

/// Fetch screenshot URLs for a Flathub app_id. Returns empty vec on any error.
/// Prefer 1248, then 752, then 624; one URL per screenshot.
pub async fn fetch_screenshots_for_app_id(app_id: &str) -> Vec<String> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Vec::new();
    }
    let url = format!("https://flathub.org/api/v2/appstream/{}", app_id);
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let body: FlathubAppstream = match response.json().await {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let preferred_widths = [1248, 752, 624];
    body.screenshots
        .iter()
        .filter_map(|s| {
            if let Some(ref sizes) = s.sizes {
                let mut by_width: Vec<(u32, &str)> = sizes
                    .iter()
                    .filter_map(|sz| sz.width.parse::<u32>().ok().map(|w| (w, sz.src.as_str())))
                    .collect();
                by_width.sort_by_key(|(w, _)| std::cmp::Reverse(*w));
                preferred_widths
                    .iter()
                    .find_map(|&pw| {
                        by_width
                            .iter()
                            .find(|(w, _)| *w == pw)
                            .map(|(_, u)| u.to_string())
                    })
                    .or_else(|| by_width.first().map(|(_, u)| u.to_string()))
            } else {
                s.size_1248
                    .clone()
                    .or_else(|| s.size_752.clone())
                    .or_else(|| s.size_624.clone())
            }
        })
        .collect()
}

use crate::models;
use serde::Deserialize;

const PKGSTATS_API_URL: &str = "https://pkgstats.archlinux.de/api/packages";

#[derive(Debug, Deserialize)]
struct PkgStatsResponse {
    #[serde(rename = "packagePopularities")]
    package_popularities: Vec<PkgStatsPackage>,
}

#[derive(Debug, Deserialize)]
struct PkgStatsPackage {
    name: String,
    popularity: f32,
}

pub async fn fetch_top_packages(limit: u32) -> Result<Vec<models::Package>, String> {
    let url = format!("{}?limit={}&sort=popularity", PKGSTATS_API_URL, limit);

    let response = reqwest::get(&url)
        .await
        .map_err(|e| e.to_string())?
        .json::<PkgStatsResponse>()
        .await
        .map_err(|e| e.to_string())?;

    let packages = response
        .package_popularities
        .into_iter()
        .map(|p| models::Package {
            name: p.name.clone(),
            display_name: Some(p.name.clone()), // We might prettify this later
            description: format!("Popularity: {:.2}%", p.popularity), // Placeholder desc until metdata hydrates
            version: "latest".to_string(),
            source: models::PackageSource::Official,
            maintainer: None,
            license: None,
            url: None,
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: None, // We could map popularity here but formatting differs
            icon: None,
            screenshots: None,
            provides: None,
            app_id: None,
            is_optimized: None,
            depends: None,
            make_depends: None,
            is_featured: Some(true),
            installed: false,
            ..Default::default()
        })
        .collect();

    Ok(packages)
}

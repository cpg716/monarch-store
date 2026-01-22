use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PackageSource {
    #[serde(rename = "chaotic")]
    Chaotic,
    #[serde(rename = "aur")]
    Aur,
    #[serde(rename = "official")]
    Official,
    #[serde(rename = "cachyos")]
    CachyOS,
    #[serde(rename = "garuda")]
    Garuda,
    #[serde(rename = "endeavour")]
    Endeavour,
    #[serde(rename = "manjaro")]
    Manjaro,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Package {
    pub name: String,
    pub display_name: Option<String>,
    pub description: String,
    pub version: String,
    pub source: PackageSource,
    pub maintainer: Option<String>,
    pub license: Option<Vec<String>>,
    pub url: Option<String>,
    pub last_modified: Option<i64>,
    pub first_submitted: Option<i64>,
    pub out_of_date: Option<i64>,
    pub keywords: Option<Vec<String>>,
    pub num_votes: Option<u32>,
    pub icon: Option<String>,
    pub screenshots: Option<Vec<String>>,
    pub provides: Option<Vec<String>>,
    pub app_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageVariant {
    pub source: PackageSource,
    pub version: String,
    pub repo_name: Option<String>,
    pub pkg_name: Option<String>, // Actual package name (e.g. firefox-nightly)
}

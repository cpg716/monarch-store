use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum PackageSource {
    #[default]
    #[serde(rename = "official")]
    Official,
    #[serde(rename = "chaotic")]
    Chaotic,
    #[serde(rename = "aur")]
    Aur,
    #[serde(rename = "cachyos")]
    CachyOS,
    #[serde(rename = "garuda")]
    Garuda,
    #[serde(rename = "endeavour")]
    Endeavour,
    #[serde(rename = "manjaro")]
    Manjaro,
    #[serde(rename = "local")]
    Local,
}

impl PackageSource {
    pub fn priority(&self) -> u8 {
        match self {
            PackageSource::Official => 1,
            PackageSource::Chaotic => 1,
            PackageSource::CachyOS => 1,
            PackageSource::Manjaro => 1,
            PackageSource::Garuda => 1,
            PackageSource::Endeavour => 1,
            PackageSource::Aur => 2,
            PackageSource::Local => 3,
        }
    }

    /// Map sync DB / repo name to the correct source. Use this whenever we get a package from a repo
    /// so CachyOS, Chaotic, Manjaro, etc. are labeled correctly instead of everything as "official".
    pub fn from_repo_name(name: &str) -> Self {
        match name {
            "chaotic-aur" => PackageSource::Chaotic,
            "monarch" => PackageSource::Official,
            n if n.starts_with("cachyos") => PackageSource::CachyOS,
            n if n.starts_with("manjaro") => PackageSource::Manjaro,
            n if n.starts_with("garuda") => PackageSource::Garuda,
            n if n.starts_with("endeavour") => PackageSource::Endeavour,
            "core" | "extra" | "community" | "multilib" => PackageSource::Official,
            _ => PackageSource::Official, // custom/user repos and unknown → official (repo, not AUR)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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
    pub is_optimized: Option<bool>,
    pub depends: Option<Vec<String>>,
    pub make_depends: Option<Vec<String>>,
    pub is_featured: Option<bool>,
    pub installed: bool,
    pub download_size: Option<u64>,
    pub installed_size: Option<u64>,
    pub alternatives: Option<Vec<Package>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageVariant {
    pub source: PackageSource,
    pub version: String,
    pub repo_name: Option<String>,
    pub pkg_name: Option<String>, // Actual package name (e.g. firefox-nightly)
}

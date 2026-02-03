use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct PackageSource {
    pub source_type: String, // "repo", "aur", "flatpak", "local"
    pub id: String,          // "core", "extra", "flathub", "chaotic-aur"
    pub version: String,     // Version available in this source
    pub label: String,       // "Manjaro Official", "Flatpak (Sandboxed)", etc.
}

impl PackageSource {
    pub fn new(source_type: &str, id: &str, version: &str, label: &str) -> Self {
        Self {
            source_type: source_type.to_string(),
            id: id.to_string(),
            version: version.to_string(),
            label: label.to_string(),
        }
    }

    pub fn priority(&self) -> u8 {
        match self.source_type.as_str() {
            "repo" => {
                // Give priority to optimized repos?
                match self.id.as_str() {
                    "chaotic-aur" | "cachyos" | "cachyos-v3" => 1,
                    _ => 2, // Standard repos
                }
            }
            "flatpak" => 3,
            "aur" => 4,
            _ => 5,
        }
    }

    /// Map sync DB / repo name to the correct source. Uses Grand Unification labels
    /// so CachyOS, Chaotic, Manjaro, SteamOS, etc. are labeled per distro identity.
    pub fn from_repo_name(
        name: &str,
        version: &str,
        distro: &crate::distro_context::DistroContext,
    ) -> Self {
        let source_type = if name == "aur" { "aur" } else { "repo" };
        let id = match name {
            n if n.starts_with("cachyos") => "cachyos",
            n if n.starts_with("manjaro") => "manjaro",
            n if n.starts_with("garuda") => "garuda",
            n if n.starts_with("endeavour") => "endeavour",
            "core" | "extra" | "community" | "multilib" => name,
            _ => name,
        };
        let label = crate::labels::get_friendly_label(name, distro.id_str());

        PackageSource::new(source_type, id, version, label)
    }

    pub fn official() -> Self {
        Self::new("repo", "core", "latest", "Official Repository")
    }

    pub fn chaotic() -> Self {
        Self::new("repo", "chaotic-aur", "latest", "Chaotic-AUR")
    }

    pub fn cachyos() -> Self {
        Self::new("repo", "cachyos", "latest", "CachyOS")
    }

    #[allow(dead_code)]
    pub fn aur() -> Self {
        Self::new("aur", "aur", "latest", "AUR")
    }

    #[allow(dead_code)]
    pub fn manjaro() -> Self {
        Self::new("repo", "manjaro", "latest", "Manjaro Official")
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
    pub available_sources: Option<Vec<PackageSource>>, // For consolidated search results
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageVariant {
    pub source: PackageSource,
    pub version: String,
    pub repo_name: Option<String>,
    pub pkg_name: Option<String>, // Actual package name (e.g. firefox-nightly)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateItem {
    pub name: String,
    pub current_version: String,
    pub new_version: String,
    pub source: PackageSource, // "official", "aur", "flatpak"
    pub size: Option<u64>,
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TransactionManifest {
    pub update_system: bool,          // Should we run -Syu?
    pub refresh_db: bool,             // Should we run -Sy?
    pub clear_cache: bool,            // Should we run -Sc?
    pub remove_lock: bool,            // Should we remove pacman lock?
    pub install_targets: Vec<String>, // List of repo packages
    pub remove_targets: Vec<String>,  // List of packages to remove
    pub local_paths: Vec<String>,     // List of pre-built AUR packages (.pkg.tar.zst) to install
}

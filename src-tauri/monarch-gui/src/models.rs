use serde::{Deserialize, Serialize};

use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, Default, Type)]
pub struct PackageSource {
    pub source_type: String, // "repo", "aur", "flatpak", "local"
    pub id: String,          // "core", "extra", "flathub", "chaotic-aur"
    pub version: String,     // Version available in this source
    pub label: String,       // "Manjaro Official", "Flatpak (Sandboxed)", etc.
    #[serde(default)]
    pub package_name: Option<String>, // Actual package name (e.g. "discord" vs "com.discordapp.Discord")
}

impl PackageSource {
    pub fn new(source_type: &str, id: &str, version: &str, label: &str) -> Self {
        Self {
            source_type: source_type.to_string(),
            id: id.to_string(),
            version: version.to_string(),
            label: label.to_string(),
            package_name: None,
        }
    }

    pub fn new_with_name(
        source_type: &str,
        id: &str,
        version: &str,
        label: &str,
        name: &str,
    ) -> Self {
        Self {
            source_type: source_type.to_string(),
            id: id.to_string(),
            version: version.to_string(),
            label: label.to_string(),
            package_name: Some(name.to_string()),
        }
    }

    pub fn priority(&self) -> u8 {
        match self.source_type.as_str() {
            "repo" => {
                // Give priority to optimized repos?
                match self.id.as_str() {
                    "chaotic-aur" | "cachyos" => 1, // cachyos = any CachyOS repo (v3/v4/znver4)
                    _ => 2,                         // Standard repos
                }
            }
            "flatpak" => 3,
            "aur" => 4,
            _ => 5,
        }
    }

    /// Map sync DB / repo name to the correct source. Uses Grand Unification labels
    /// so CachyOS, Chaotic, Manjaro, SteamOS, etc. are labeled per distro identity.
    /// Normalizes empty or "unknown" repo names to "other" so the UI never shows "Unknown Repository".
    pub fn from_repo_name(
        name: &str,
        version: &str,
        distro: &crate::distro_context::DistroContext,
        pkg_name: &str,
    ) -> Self {
        let name_repo = if name.trim().is_empty() || name.eq_ignore_ascii_case("unknown") {
            "other"
        } else {
            name
        };
        let source_type = if name_repo == "aur" { "aur" } else { "repo" };
        let id = match name_repo {
            n if n.starts_with("cachyos") => "cachyos",
            n if n.starts_with("manjaro") => "manjaro",
            n if n.starts_with("garuda") => "garuda",
            n if n.starts_with("endeavour") => "endeavour",
            "core" | "extra" | "community" | "multilib" => name_repo,
            _ => name_repo,
        };
        let label = crate::labels::get_friendly_label(name_repo, distro.id_str());

        PackageSource::new_with_name(source_type, id, version, label, pkg_name)
    }

    pub fn official(name: &str) -> Self {
        Self::new_with_name("repo", "core", "latest", "Arch Official", name)
    }

    pub fn chaotic(name: &str) -> Self {
        Self::new_with_name("repo", "chaotic-aur", "latest", "Chaotic-AUR", name)
    }

    pub fn cachyos(name: &str) -> Self {
        Self::new_with_name("repo", "cachyos", "latest", "CachyOS", name)
    }

    #[allow(dead_code)]
    pub fn aur(name: &str) -> Self {
        Self::new_with_name("aur", "aur", "latest", "AUR", name)
    }

    #[allow(dead_code)]
    pub fn manjaro(name: &str) -> Self {
        Self::new_with_name("repo", "manjaro", "latest", "Manjaro Official", name)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Type)]
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
    /// Canonical key used for merge deduplication (e.g. "discord"). Set during merge; used as React key.
    #[serde(default)]
    pub canonical_id: String,
    pub is_optimized: Option<bool>,
    pub depends: Option<Vec<String>>,
    pub make_depends: Option<Vec<String>>,
    pub is_featured: Option<bool>,
    pub installed: bool,
    pub download_size: Option<u64>,
    pub installed_size: Option<u64>,
    pub alternatives: Option<Vec<Package>>,
    pub available_sources: Option<Vec<PackageSource>>, // For consolidated search results
    pub rating: Option<crate::odrs_api::OdrsRating>,
    pub long_description: Option<String>,
    #[serde(default)]
    pub installed_sources: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PackageVariant {
    pub source: PackageSource,
    pub version: String,
    pub repo_name: Option<String>,
    pub pkg_name: Option<String>, // Actual package name (e.g. firefox-nightly)
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct UpdateItem {
    pub name: String,
    pub display_name: Option<String>,
    pub current_version: String,
    pub new_version: String,
    pub source: PackageSource, // "official", "aur", "flatpak"
    pub size: Option<u64>,
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Type)]
pub struct TransactionManifest {
    pub update_system: bool,          // Should we run -Syu?
    pub refresh_db: bool,             // Should we run -Sy?
    pub clear_cache: bool,            // Should we run -Sc?
    pub remove_lock: bool,            // Should we remove pacman lock?
    pub remove_orphans: bool,         // Should we remove unused dependencies?
    pub install_targets: Vec<String>, // List of repo packages
    pub remove_targets: Vec<String>,  // List of packages to remove
    pub local_paths: Vec<String>,     // List of pre-built AUR packages (.pkg.tar.zst) to install
    pub parallel_downloads: Option<u32>,
    pub cpu_optimization: Option<String>,
    pub target_repo: Option<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone, Default, Type)]
pub struct CacheStats {
    pub total_size_bytes: u64,
    pub package_count: u32,
}

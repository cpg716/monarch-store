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
                let id = self.id.to_lowercase();
                if id.contains("cachyos")
                    || id.contains("manjaro")
                    || id.contains("garuda")
                    || id.contains("endeavour")
                {
                    1 // Distro-native repos
                } else if matches!(id.as_str(), "core" | "extra" | "community" | "multilib" | "official") {
                    2 // Official Arch repos
                } else if id.contains("chaotic") {
                    3 // Chaotic-AUR (after distro-native and official)
                } else {
                    2 // Other configured repos default near official tier
                }
            }
            "flatpak" => 4,
            "aur" => 5,
            _ => 6,
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
    #[serde(default)]
    pub display_title: Option<String>,
    pub description: String,
    pub version: String,
    pub source: PackageSource,
    #[serde(default)]
    pub primary_action: Option<String>,
    #[serde(default)]
    pub primary_action_label: Option<String>,
    #[serde(default)]
    pub source_summary: Option<String>,
    #[serde(default)]
    pub trust_level: Option<String>,
    #[serde(default)]
    pub security_summary: Option<String>,
    pub maintainer: Option<String>,
    pub license: Option<Vec<String>>,
    pub url: Option<String>,
    pub last_modified: Option<i64>,
    #[serde(default)]
    pub last_modified_unix: Option<i64>,
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
    #[serde(default)]
    pub download_size_bytes: Option<u64>,
    #[serde(default)]
    pub installed_size_bytes: Option<u64>,
    pub alternatives: Option<Vec<Package>>,
    pub available_sources: Option<Vec<PackageSource>>, // For consolidated search results
    pub rating: Option<crate::odrs_api::OdrsRating>,
    pub long_description: Option<String>,
    #[serde(default)]
    pub installed_sources: Option<Vec<String>>,
    #[serde(default)]
    pub launch_target: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PackageVariant {
    pub source: PackageSource,
    pub version: String,
    pub repo_name: Option<String>,
    pub pkg_name: Option<String>, // Actual package name (e.g. firefox-nightly)
    pub download_size: Option<u64>,
    pub installed_size: Option<u64>,
    pub maintainer: Option<String>,
    pub license: Option<Vec<String>>,
    pub description: Option<String>,
    pub screenshots: Option<Vec<String>>,
    pub security: Option<PackageSecuritySummary>,
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

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct UpdateSnapshotItem {
    pub package: Package,
    pub current_version: String,
    pub new_version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct UpdateSourceStatus {
    pub source: String,
    pub status: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct UpdateSnapshot {
    pub items: Vec<UpdateSnapshotItem>,
    pub sources: Vec<UpdateSourceStatus>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct DiscoveryIntent {
    pub id: String,
    pub label: String,
    pub description: String,
    pub query: Option<String>,
    pub category: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct DiscoveryHomeSnapshot {
    pub essentials: Vec<Package>,
    pub trending: Vec<Package>,
    pub quick_starts: Vec<DiscoveryIntent>,
    pub generated_at: i64,
    pub stale: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct SearchSuggestion {
    pub label: String,
    pub query: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct SearchResponse {
    pub packages: Vec<Package>,
    pub suggestions: Vec<SearchSuggestion>,
    pub query_interpretation: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PackageSecuritySummary {
    pub trust_tier: String,
    pub system_access: String,
    pub maintainer_known: bool,
    pub verification_note: String,
    pub user_action_note: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Type)]
pub struct PackagePresentation {
    pub display_title: Option<String>,
    pub icon: Option<String>,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub screenshots: Vec<String>,
    pub app_id: Option<String>,
    pub developer_name: Option<String>,
    pub donation_url: Option<String>,
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

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct FullPackageDetails {
    pub package: Option<Package>,
    pub presentation: Option<PackagePresentation>,
    pub installed_status: crate::commands::package::PackageInstallStatus,
    pub all_installed_variants: Vec<crate::commands::package::PackageInstallStatus>,
    pub flatpak_permissions: Option<Vec<String>>,
    pub all_variants: Vec<PackageVariant>,
    pub display_title: Option<String>,
    pub primary_action: Option<String>,
    pub primary_action_label: Option<String>,
    pub selected_default_source: Option<PackageSource>,
    pub source_summary: Option<String>,
    pub security_summary: Option<String>,
    pub installed_source_label: Option<String>,
    pub source_switch_policy: Option<String>,
    pub source_switch_notice: Option<String>,
    pub security: Option<PackageSecuritySummary>,
    pub developer_name: Option<String>,
    pub donation_url: Option<String>,
}

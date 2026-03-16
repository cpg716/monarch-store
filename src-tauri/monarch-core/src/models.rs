use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct PackageSource {
    pub source_type: String,
    pub id: String,
    pub version: String,
    pub label: String,
    #[serde(default)]
    pub package_name: Option<String>,
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
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    pub keywords: Option<Vec<String>>,
    pub num_votes: Option<u32>,
    pub icon: Option<String>,
    pub screenshots: Option<Vec<String>>,
    pub provides: Option<Vec<String>>,
    pub app_id: Option<String>,
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
    pub available_sources: Option<Vec<PackageSource>>,
    pub rating: Option<OdrsRating>,
    pub long_description: Option<String>,
    #[serde(default)]
    pub installed_sources: Option<Vec<String>>,
    #[serde(default)]
    pub launch_target: Option<String>,
    #[serde(default)]
    pub discovered_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct OdrsRating {
    pub star1: u32,
    pub star2: u32,
    pub star3: u32,
    pub star4: u32,
    pub star5: u32,
    pub total: u32,
    pub score: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PackageSecuritySummary {
    pub trust_tier: String,
    pub system_access: String,
    pub maintainer_known: bool,
    pub verification_note: String,
    pub user_action_note: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PackageVariant {
    pub source: PackageSource,
    pub version: String,
    pub repo_name: Option<String>,
    pub pkg_name: Option<String>,
    pub download_size: Option<u64>,
    pub installed_size: Option<u64>,
    pub maintainer: Option<String>,
    pub license: Option<Vec<String>>,
    pub description: Option<String>,
    pub screenshots: Option<Vec<String>>,
    pub security: Option<PackageSecuritySummary>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PackageInstallStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub repo: Option<String>,
    pub source: Option<PackageSource>,
    pub actual_package_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FullPackageDetails {
    pub package: Option<Package>,
    pub presentation: Option<PackagePresentation>,
    pub installed_status: PackageInstallStatus,
    pub all_installed_variants: Vec<PackageInstallStatus>,
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PackageReview {
    pub review_id: Option<u64>,
    pub app_id: String,
    pub user_display: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub rating: Option<u32>,
    pub date_created: Option<f64>,
    pub version: Option<String>,
    pub distro: Option<String>,
    pub locale: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct LocalReview {
    pub app_id: String,
    pub rating: u32,
    pub summary: String,
    pub description: String,
    pub user_display: String,
    pub date_created: u64,
}

impl Package {
    pub fn effective_title(&self) -> String {
        self.display_title
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .or_else(|| {
                self.display_name
                    .as_ref()
                    .filter(|value| !value.trim().is_empty())
                    .cloned()
            })
            .unwrap_or_else(|| self.name.clone())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SearchOptions {
    pub flatpak_enabled: Option<bool>,
    pub aur_enabled: Option<bool>,
    pub chaotic_enabled: Option<bool>,
    pub show_system_apps: Option<bool>,
    pub source_filter: Option<String>,
    pub category_filter: Option<String>,
    pub installed_only: Option<bool>,
    pub sort_mode: Option<SearchSortMode>,
    pub for_installed_lookup: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchSortMode {
    #[default]
    Relevance,
    Name,
    Newest,
    Updated,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UpdateItem {
    pub name: String,
    pub display_name: Option<String>,
    pub current_version: String,
    pub new_version: String,
    pub source: PackageSource,
    pub size: Option<u64>,
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UpdateSnapshotItem {
    pub package: Package,
    pub current_version: String,
    pub new_version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UpdateSourceStatus {
    pub source: String,
    pub status: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UpdateSnapshot {
    pub items: Vec<UpdateSnapshotItem>,
    pub sources: Vec<UpdateSourceStatus>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TransactionManifest {
    pub update_system: bool,
    pub refresh_db: bool,
    pub clear_cache: bool,
    pub remove_lock: bool,
    pub remove_orphans: bool,
    pub install_targets: Vec<String>,
    pub remove_targets: Vec<String>,
    pub local_paths: Vec<String>,
    pub parallel_downloads: Option<u32>,
    pub cpu_optimization: Option<String>,
    pub target_repo: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum NewsCategory {
    #[default]
    Discovery,
    Critical,
    System,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub link: String,
    pub pub_date: String,
    pub source_label: String,
    pub is_critical: bool,
    pub category: NewsCategory,
    pub content: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DiscoveryIntent {
    pub id: String,
    pub label: String,
    pub description: String,
    pub query: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HomeSnapshot {
    pub suggested_searches: Vec<String>,
    pub featured: Vec<Package>,
    pub native: Vec<Package>,
    pub chaotic: Vec<Package>,
    pub flatpak: Vec<Package>,
    pub aur: Vec<Package>,
    pub trending: Vec<Package>,
    pub popular: Vec<Package>,
    pub new: Vec<Package>,
    pub updated: Vec<Package>,
    pub categories: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct StartupStatus {
    pub missing_required_bins: Vec<String>,
    pub stale_pacman_lock: bool,
    pub registry_empty: bool,
    pub helper_available: bool,
    pub policy_installed: bool,
    pub keyring_ready: bool,
    pub sync_db_healthy: bool,
    pub warnings: Vec<String>,
    pub onboarding_completed: bool,
    pub distro: DistroProfile,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SystemHealthView {
    pub startup: StartupStatus,
    pub pending_updates: usize,
    pub alpm_status: String,
    pub update_status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotTool {
    Snapper,
    Timeshift,
    #[default]
    None,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SnapshotStatus {
    pub tool: SnapshotTool,
    pub is_configured: bool,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SystemInfo {
    pub kernel: String,
    pub distro: String,
    pub pacman_version: String,
    pub chaotic_enabled: bool,
    pub cpu_optimization: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CacheSizeResult {
    pub size_bytes: u64,
    pub human_readable: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct OrphansWithSizeResult {
    pub orphans: Vec<String>,
    pub total_size_bytes: u64,
    pub human_readable: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MirrorTestResult {
    pub url: String,
    pub latency_ms: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UpdateWarnings {
    pub reboot_required: bool,
    pub pacnew_warnings: Vec<String>,
    pub restart_required_services: Vec<String>,
    pub critical_advisories: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SettingsView {
    pub settings: GtkSettings,
    pub startup: Option<StartupStatus>,
    pub system_health: Option<SystemHealthView>,
    pub system_info: Option<SystemInfo>,
    pub snapshot_status: Option<SnapshotStatus>,
    pub cache: Option<CacheSizeResult>,
    pub orphans: Option<OrphansWithSizeResult>,
    pub update_warnings: Option<UpdateWarnings>,
    pub mirror_rank_tool: Option<String>,
    pub notices: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChaoticSupport {
    #[default]
    Allowed,
    Blocked,
    Native,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DistroProfile {
    pub id: String,
    pub pretty_name: String,
    pub chaotic_support: ChaoticSupport,
    pub chaotic_configured: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct GtkSettings {
    pub aur_enabled: bool,
    pub flatpak_enabled: bool,
    pub chaotic_enabled: bool,
    pub show_system_apps: bool,
    pub one_click_enabled: bool,
    pub reduce_password_prompts: bool,
    pub automatic_housekeeping: bool,
    pub sync_on_startup: bool,
    pub verbose_logs: bool,
    pub telemetry_enabled: bool,
    pub clean_build: bool,
    pub parallel_downloads: u32,
    pub onboarding_completed: bool,
    pub read_news_ids: Vec<String>,
    pub theme_mode: String,
    pub accent_color: Option<String>,
    pub sidebar_expanded: bool,
    pub alpha_notice_dismissed: bool,
    pub search_history: Vec<String>,
    pub active_tab: String,
    pub sync_interval_hours: u32,
    pub notifications_enabled: bool,
    pub advanced_mode: bool,
}

impl Default for GtkSettings {
    fn default() -> Self {
        Self {
            aur_enabled: true,
            flatpak_enabled: true,
            chaotic_enabled: false,
            show_system_apps: false,
            one_click_enabled: false,
            reduce_password_prompts: false,
            automatic_housekeeping: false,
            sync_on_startup: true,
            verbose_logs: false,
            telemetry_enabled: true,
            clean_build: false,
            parallel_downloads: 3,
            onboarding_completed: false,
            read_news_ids: Vec::new(),
            theme_mode: "system".to_string(),
            accent_color: None,
            sidebar_expanded: true,
            alpha_notice_dismissed: false,
            search_history: Vec::new(),
            active_tab: "home".to_string(),
            sync_interval_hours: 6,
            notifications_enabled: true,
            advanced_mode: false,
        }
    }
}

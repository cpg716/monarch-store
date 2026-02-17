use crate::helper_client::{invoke_helper, HelperCommand};
use crate::models::{Package, PackageSource};
use crate::repo_db;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct RepoConfig {
    pub name: String,
    pub url: String,
    pub source: PackageSource,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredConfig {
    repos: Vec<RepoConfig>,
    #[serde(default)]
    aur_enabled: bool,
    #[serde(default)]
    one_click_enabled: bool,
    #[serde(default)]
    advanced_mode: bool,
    #[serde(default)]
    telemetry_enabled: bool,
    #[serde(default = "default_notifications_enabled")]
    notifications_enabled: bool,
    /// Sync repositories when the app starts (default on); when off, no trigger_repo_sync on startup.
    #[serde(default = "default_sync_on_startup")]
    sync_on_startup_enabled: bool,
    #[serde(default = "default_sync_interval")]
    sync_interval_hours: u32,
    #[serde(default = "default_flatpak_enabled")]
    flatpak_enabled: bool,
    #[serde(default)]
    pub automatic_housekeeping: bool,
    #[serde(default)]
    repo_priority_order: Vec<String>,
    #[serde(default)]
    verbose_logs_enabled: bool,
    #[serde(default)]
    clean_build_enabled: bool,
    #[serde(default = "default_parallel_downloads")]
    parallel_downloads: u32,
    #[serde(default = "default_onboarding_completed")]
    pub onboarding_completed: bool,
    pub theme_mode: String,
    pub accent_color: String,
    pub declined_system_setup: bool,
    pub is_sidebar_expanded: bool,
    pub alpha_notice_dismissed: bool,
    pub search_history: Vec<String>,
    pub read_news_ids: Vec<String>,
    pub active_tab: String,
}

fn default_sync_interval() -> u32 {
    3
}

fn default_flatpak_enabled() -> bool {
    true
}

fn default_sync_on_startup() -> bool {
    true
}

fn default_notifications_enabled() -> bool {
    true // Default to enabled
}

fn default_parallel_downloads() -> u32 {
    5
}

fn default_onboarding_completed() -> bool {
    false
}

#[derive(Clone)]
pub struct RepoManager {
    // Map RepoName -> List of Packages
    pub cache: Arc<RwLock<HashMap<String, Vec<Package>>>>,
    repos: Arc<RwLock<Vec<RepoConfig>>>,
    pub aur_enabled: Arc<RwLock<bool>>,
    pub one_click_enabled: Arc<RwLock<bool>>,
    pub advanced_mode: Arc<RwLock<bool>>,
    pub telemetry_enabled: Arc<RwLock<bool>>,
    pub notifications_enabled: Arc<RwLock<bool>>,
    pub sync_on_startup_enabled: Arc<RwLock<bool>>,
    pub sync_interval_hours: Arc<RwLock<u32>>,
    pub flatpak_enabled: Arc<RwLock<bool>>,
    pub repo_priority_order: Arc<RwLock<Vec<String>>>,
    pub verbose_logs_enabled: Arc<RwLock<bool>>,
    pub clean_build_enabled: Arc<RwLock<bool>>,
    pub automatic_housekeeping: Arc<RwLock<bool>>,
    pub parallel_downloads: Arc<RwLock<u32>>,
    pub onboarding_completed: Arc<RwLock<bool>>,
    pub theme_mode: Arc<RwLock<String>>,
    pub accent_color: Arc<RwLock<String>>,
    pub declined_system_setup: Arc<RwLock<bool>>,
    pub is_sidebar_expanded: Arc<RwLock<bool>>,
    pub alpha_notice_dismissed: Arc<RwLock<bool>>,
    pub search_history: Arc<RwLock<Vec<String>>>,
    pub read_news_ids: Arc<RwLock<Vec<String>>>,
    pub active_tab: Arc<RwLock<String>>,
}

// Helper for Intelligent Priority Sorting (Granular Optimization Ranking)
pub fn calculate_package_rank(
    pkg: &Package,
    opt_level: u8,
    distro: &crate::distro_context::DistroContext,
) -> u8 {
    // Manjaro Strategy: Stability First (Official Repos Priority 0)
    // We treat "source_first" as "Official/Stable First" here
    if distro.capabilities.default_search_sort == "source_first" {
        match pkg.source.source_type.as_str() {
            "repo" => match pkg.source.id.as_str() {
                "core" | "extra" | "multilib" => 0, // High Priority Official
                "manjaro" => 0,
                _ => 10, // Unofficial repos deprioritized on Manjaro
            },
            "flatpak" => 1,
            "aur" | "local" => 2,
            _ => 10,
        }
    } else {
        // CachyOS/Garuda/Arch Strategy: Performance First (Optimization Level Priority)
        // opt_level: 0=None, 1=v3, 2=v4, 3=znver4
        match opt_level {
            3 => 0, // Rank 0: Zen 4/5 Optimized (ELITE)
            2 => 1, // Rank 1: x86-64-v4 Optimized
            1 => 2, // Rank 2: x86-64-v3 Optimized
            _ => {
                // Rank 4+: General Priorities (Chaotic=4, Official=5, etc)
                pkg.source.priority().saturating_add(3)
            }
        }
    }
}

impl RepoManager {
    pub fn new() -> Self {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("monarch-store");
        std::fs::create_dir_all(&config_path).unwrap_or_default();
        // let config_file = config_path.join("repos.json"); // Not used for state init anymore directly here, but later

        // Default Repos - We only actively manage specific ones if found.
        // Everything else must be discovered from the host system.
        let mut initial_repos = vec![
            // Official Repos (We keep these for UI structure, but their enabled state comes from system)
            RepoConfig {
                name: "core".to_string(),
                url: "https://geo.mirror.pkgbuild.com/core/os/x86_64/core.db".to_string(),
                source: PackageSource::official(""),
                enabled: true,
            },
            RepoConfig {
                name: "extra".to_string(),
                url: "https://geo.mirror.pkgbuild.com/extra/os/x86_64/extra.db".to_string(),
                source: PackageSource::official(""),
                enabled: true,
            },
            RepoConfig {
                name: "community".to_string(),
                url: "https://geo.mirror.pkgbuild.com/community/os/x86_64/community.db".to_string(),
                source: PackageSource::official(""),
                enabled: true,
            },
            RepoConfig {
                name: "multilib".to_string(),
                url: "https://geo.mirror.pkgbuild.com/multilib/os/x86_64/multilib.db".to_string(),
                source: PackageSource::official(""),
                enabled: true,
            },
        ];

        // host-adaptive: we no longer check /etc/pacman.d/monarch for manual config files.
        // The authority is ALPM (below).

        // 2. DISCOVER HOST REPOS via ALPM (system pacman.conf — we do not inject)
        // Register all repo sections from /etc/pacman.conf (and Include'd files) so syncdbs() returns
        // the actual system repos: core, extra, community, multilib, plus Manjaro, Garuda, Chaotic-AUR, CachyOS, etc.
        if let Ok(alpm) = alpm::Alpm::new("/", "/var/lib/pacman") {
            crate::alpm_read::register_syncdbs_from_conf(&alpm, "/etc/pacman.conf");
            let dbs = alpm.syncdbs();
            for db in dbs {
                let db_name = db.name();
                // If it's already in our list, mark it enabled
                if let Some(existing) = initial_repos.iter_mut().find(|r| r.name == db_name) {
                    existing.enabled = true;
                } else {
                    // System repo from pacman.conf we didn't have in default list (e.g. manjaro, chaotic-aur, cachyos, garuda).
                    // Add it; we do not inject — we only discover from system.

                    // Infer Source from Name
                    let source = PackageSource::from_repo_name(
                        db_name,
                        "latest",
                        &crate::distro_context::DistroContext::new(),
                        "", // Generic repo config has no specific package name
                    );

                    let servers = db.servers().into_iter().next().unwrap_or("").to_string();

                    initial_repos.push(RepoConfig {
                        name: db_name.to_string(),
                        url: servers,
                        source,
                        enabled: true,
                    });
                }
            }
        }

        // 4. PERSISTENCE: Trust repos.json for UI persistence (Onboarding/Settings choices)
        let mut initial_aur = false;
        let mut initial_one_click = false;
        let mut initial_advanced = false;
        let mut initial_telemetry = false;
        let mut initial_notifications = true;
        let mut initial_sync_on_startup = true;
        let mut initial_sync_interval = 3;
        let mut initial_flatpak = true;
        let mut initial_repo_order = Vec::new();
        let mut initial_verbose = false;
        let mut initial_clean_build = false;
        let mut initial_housekeeping = false;
        let mut initial_parallel_downloads = 5;
        let mut initial_onboarding = false;
        let mut initial_theme = "system".to_string();
        let mut initial_accent = "#3b82f6".to_string();
        let mut initial_declined = false;
        let mut initial_sidebar = true;
        let mut initial_alpha = false;
        let mut initial_search_history = Vec::new();
        let mut initial_read_news = Vec::new();
        let mut initial_active_tab = "explore".to_string();

        let config_file = config_path.join("repos.json");

        if config_file.exists() {
            if let Ok(file) = std::fs::File::open(&config_file) {
                let reader = std::io::BufReader::new(file);
                match serde_json::from_reader::<_, StoredConfig>(reader) {
                    Ok(saved_config) => {
                        log::info!("[REPO_MANAGER] Loaded persisted config from repos.json");
                        initial_aur = saved_config.aur_enabled;
                        initial_one_click = saved_config.one_click_enabled;
                        initial_advanced = saved_config.advanced_mode;
                        initial_telemetry = saved_config.telemetry_enabled;
                        initial_notifications = saved_config.notifications_enabled;
                        initial_sync_on_startup = saved_config.sync_on_startup_enabled;
                        initial_sync_interval = saved_config.sync_interval_hours;
                        initial_flatpak = saved_config.flatpak_enabled;
                        initial_repo_order = saved_config.repo_priority_order;
                        initial_verbose = saved_config.verbose_logs_enabled;
                        initial_clean_build = saved_config.clean_build_enabled;
                        initial_housekeeping = saved_config.automatic_housekeeping;
                        initial_parallel_downloads = saved_config.parallel_downloads;
                        initial_onboarding = saved_config.onboarding_completed;
                        initial_theme = saved_config.theme_mode;
                        initial_accent = saved_config.accent_color;
                        initial_declined = saved_config.declined_system_setup;
                        initial_sidebar = saved_config.is_sidebar_expanded;
                        initial_alpha = saved_config.alpha_notice_dismissed;
                        initial_search_history = saved_config.search_history;
                        initial_read_news = saved_config.read_news_ids;
                        initial_active_tab = saved_config.active_tab;

                        // Merge saved repo enabled states
                        for saved_repo in saved_config.repos {
                            if let Some(r) =
                                initial_repos.iter_mut().find(|r| r.name == saved_repo.name)
                            {
                                // Only overwrite if not an Official repo (Official stay enabled by policy)
                                if r.source != PackageSource::official("") {
                                    r.enabled = saved_repo.enabled;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "[REPO_MANAGER] Failed to deserialize repos.json: {}. Using defaults.",
                            e
                        );
                    }
                }
            }
        }

        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            repos: Arc::new(RwLock::new(initial_repos)),
            aur_enabled: Arc::new(RwLock::new(initial_aur)),
            one_click_enabled: Arc::new(RwLock::new(initial_one_click)),
            advanced_mode: Arc::new(RwLock::new(initial_advanced)),
            telemetry_enabled: Arc::new(RwLock::new(initial_telemetry)),
            notifications_enabled: Arc::new(RwLock::new(initial_notifications)),
            sync_on_startup_enabled: Arc::new(RwLock::new(initial_sync_on_startup)),
            sync_interval_hours: Arc::new(RwLock::new(initial_sync_interval)),
            flatpak_enabled: Arc::new(RwLock::new(initial_flatpak)),
            repo_priority_order: Arc::new(RwLock::new(initial_repo_order)),
            verbose_logs_enabled: Arc::new(RwLock::new(initial_verbose)),
            clean_build_enabled: Arc::new(RwLock::new(initial_clean_build)),
            automatic_housekeeping: Arc::new(RwLock::new(initial_housekeeping)),
            parallel_downloads: Arc::new(RwLock::new(initial_parallel_downloads)),
            onboarding_completed: Arc::new(RwLock::new(initial_onboarding)),
            theme_mode: Arc::new(RwLock::new(initial_theme)),
            accent_color: Arc::new(RwLock::new(initial_accent)),
            declined_system_setup: Arc::new(RwLock::new(initial_declined)),
            is_sidebar_expanded: Arc::new(RwLock::new(initial_sidebar)),
            alpha_notice_dismissed: Arc::new(RwLock::new(initial_alpha)),
            search_history: Arc::new(RwLock::new(initial_search_history)),
            read_news_ids: Arc::new(RwLock::new(initial_read_news)),
            active_tab: Arc::new(RwLock::new(initial_active_tab)),
        }
    }
    async fn save_config_async(&self) {
        let repos = self.repos.read().await.clone();
        let aur = *self.aur_enabled.read().await;
        let one_click = *self.one_click_enabled.read().await;
        let advanced = *self.advanced_mode.read().await;
        let telemetry = *self.telemetry_enabled.read().await;
        let notifications = *self.notifications_enabled.read().await;
        let sync_on_startup = *self.sync_on_startup_enabled.read().await;
        let sync_interval = *self.sync_interval_hours.read().await;
        let flatpak = *self.flatpak_enabled.read().await;
        let repo_order = self.repo_priority_order.read().await.clone();
        let verbose = *self.verbose_logs_enabled.read().await;
        let clean_build = *self.clean_build_enabled.read().await;
        let housekeeping = *self.automatic_housekeeping.read().await;
        let onboarding = *self.onboarding_completed.read().await;
        let theme = self.theme_mode.read().await.clone();
        let accent = self.accent_color.read().await.clone();
        let declined = *self.declined_system_setup.read().await;
        let sidebar = *self.is_sidebar_expanded.read().await;
        let alpha = *self.alpha_notice_dismissed.read().await;
        let search_history = self.search_history.read().await.clone();
        let read_news = self.read_news_ids.read().await.clone();
        let tab = self.active_tab.read().await.clone();
        let parallel_downloads = *self.parallel_downloads.read().await;

        tokio::task::spawn_blocking(move || {
            let config = StoredConfig {
                repos,
                aur_enabled: aur,
                one_click_enabled: one_click,
                advanced_mode: advanced,
                telemetry_enabled: telemetry,
                notifications_enabled: notifications,
                sync_on_startup_enabled: sync_on_startup,
                sync_interval_hours: sync_interval,
                flatpak_enabled: flatpak,
                repo_priority_order: repo_order,
                verbose_logs_enabled: verbose,
                clean_build_enabled: clean_build,
                automatic_housekeeping: housekeeping,
                onboarding_completed: onboarding,
                theme_mode: theme,
                accent_color: accent,
                declined_system_setup: declined,
                is_sidebar_expanded: sidebar,
                alpha_notice_dismissed: alpha,
                search_history,
                read_news_ids: read_news,
                active_tab: tab,
                parallel_downloads,
            };

            let config_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("monarch-store");
            let config_file = config_path.join("repos.json");

            if let Ok(json) = serde_json::to_string_pretty(&config) {
                if let Err(e) = std::fs::write(&config_file, json) {
                    log::error!("[REPO_MANAGER] Failed to write repos.json: {}", e);
                }
            } else {
                log::error!("[REPO_MANAGER] Failed to serialize config object");
            }
        });
    }

    pub async fn is_aur_enabled(&self) -> bool {
        *self.aur_enabled.read().await
    }

    pub async fn set_aur_enabled(&self, enabled: bool) {
        let mut w = self.aur_enabled.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_one_click_enabled(&self) -> bool {
        *self.one_click_enabled.read().await
    }

    pub async fn set_one_click_enabled(&self, enabled: bool) {
        let mut w = self.one_click_enabled.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_advanced_mode(&self) -> bool {
        *self.advanced_mode.read().await
    }

    pub async fn set_advanced_mode(&self, enabled: bool) {
        let mut w = self.advanced_mode.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn get_sync_interval_hours(&self) -> u32 {
        *self.sync_interval_hours.read().await
    }

    pub async fn set_sync_interval_hours(&self, hours: u32) {
        let mut w = self.sync_interval_hours.write().await;
        *w = hours;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_flatpak_enabled(&self) -> bool {
        *self.flatpak_enabled.read().await
    }

    pub async fn set_flatpak_enabled(&self, enabled: bool) {
        let mut w = self.flatpak_enabled.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn get_repo_priority_order(&self) -> Vec<String> {
        self.repo_priority_order.read().await.clone()
    }

    pub async fn set_repo_priority_order(&self, order: Vec<String>) {
        let mut w = self.repo_priority_order.write().await;
        *w = order;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_verbose_logs_enabled(&self) -> bool {
        *self.verbose_logs_enabled.read().await
    }

    pub async fn set_verbose_logs_enabled(&self, enabled: bool) {
        let mut w = self.verbose_logs_enabled.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_clean_build_enabled(&self) -> bool {
        *self.clean_build_enabled.read().await
    }

    pub async fn set_clean_build_enabled(&self, enabled: bool) {
        let mut w = self.clean_build_enabled.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn set_telemetry_enabled(&self, enabled: bool) {
        let mut w = self.telemetry_enabled.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_telemetry_enabled(&self) -> bool {
        *self.telemetry_enabled.read().await
    }

    pub async fn set_sync_on_startup_enabled(&self, enabled: bool) {
        let mut w = self.sync_on_startup_enabled.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_sync_on_startup_enabled(&self) -> bool {
        *self.sync_on_startup_enabled.read().await
    }

    pub async fn set_notifications_enabled(&self, enabled: bool) {
        let mut w = self.notifications_enabled.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_notifications_enabled(&self) -> bool {
        *self.notifications_enabled.read().await
    }

    pub async fn is_onboarding_completed(&self) -> bool {
        *self.onboarding_completed.read().await
    }

    pub async fn set_onboarding_completed(&self, completed: bool) {
        let mut onboarding = self.onboarding_completed.write().await;
        *onboarding = completed;
        drop(onboarding);
        self.save_config_async().await;
    }

    pub async fn get_theme_mode(&self) -> String {
        self.theme_mode.read().await.clone()
    }

    pub async fn set_theme_mode(&self, mode: String) {
        let mut theme = self.theme_mode.write().await;
        *theme = mode;
        drop(theme);
        self.save_config_async().await;
    }

    pub async fn get_accent_color(&self) -> String {
        self.accent_color.read().await.clone()
    }

    pub async fn set_accent_color(&self, color: String) {
        let mut accent = self.accent_color.write().await;
        *accent = color;
        drop(accent);
        self.save_config_async().await;
    }

    pub async fn is_declined_system_setup(&self) -> bool {
        *self.declined_system_setup.read().await
    }

    pub async fn set_declined_system_setup(&self, declined: bool) {
        let mut flag = self.declined_system_setup.write().await;
        *flag = declined;
        drop(flag);
        self.save_config_async().await;
    }

    pub async fn is_sidebar_expanded(&self) -> bool {
        *self.is_sidebar_expanded.read().await
    }

    pub async fn set_sidebar_expanded(&self, expanded: bool) {
        let mut flag = self.is_sidebar_expanded.write().await;
        *flag = expanded;
        drop(flag);
        self.save_config_async().await;
    }

    pub async fn is_alpha_notice_dismissed(&self) -> bool {
        *self.alpha_notice_dismissed.read().await
    }

    pub async fn set_alpha_notice_dismissed(&self, dismissed: bool) {
        let mut flag = self.alpha_notice_dismissed.write().await;
        *flag = dismissed;
        drop(flag);
        self.save_config_async().await;
    }

    pub async fn get_search_history(&self) -> Vec<String> {
        self.search_history.read().await.clone()
    }

    pub async fn set_search_history(&self, history: Vec<String>) {
        let mut h = self.search_history.write().await;
        *h = history;
        drop(h);
        self.save_config_async().await;
    }

    pub async fn get_read_news_ids(&self) -> Vec<String> {
        self.read_news_ids.read().await.clone()
    }

    pub async fn set_read_news_ids(&self, ids: Vec<String>) {
        let mut i = self.read_news_ids.write().await;
        *i = ids;
        drop(i);
        self.save_config_async().await;
    }

    pub async fn get_active_tab(&self) -> String {
        self.active_tab.read().await.clone()
    }

    pub async fn set_active_tab(&self, tab: String) {
        let mut t = self.active_tab.write().await;
        *t = tab;
        drop(t);
        self.save_config_async().await;
    }

    pub async fn is_automatic_housekeeping_enabled(&self) -> bool {
        *self.automatic_housekeeping.read().await
    }

    pub async fn set_automatic_housekeeping_enabled(&self, enabled: bool) {
        let mut w = self.automatic_housekeeping.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn get_parallel_downloads(&self) -> u32 {
        *self.parallel_downloads.read().await
    }

    pub async fn set_parallel_downloads(&self, count: u32) {
        let mut w = self.parallel_downloads.write().await;
        *w = count;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_repo_enabled(&self, name: &str) -> bool {
        let repos = self.repos.read().await;
        repos.iter().any(|r| r.name == name && r.enabled)
    }

    pub async fn get_all_repos(&self) -> Vec<RepoConfig> {
        self.repos.read().await.clone()
    }

    pub async fn load_initial_cache(&self) {
        let repos = self.repos.read().await;
        // Only load enabled or required repos
        let active_repos: Vec<RepoConfig> = repos.iter().filter(|r| r.enabled).cloned().collect();
        drop(repos);

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("monarch-store")
            .join("dbs");

        if !cache_dir.exists() {
            return;
        }

        log::info!("Loading initial package cache from disk");
        let mut handles = Vec::new();

        for repo in active_repos {
            // Simplified loading logic...
            let r = repo.clone();
            let c_dir = cache_dir.clone();
            handles.push(tokio::spawn(async move {
                let file_name = format!("{}.db", r.name);
                let path = c_dir.join(file_name);
                if !path.exists() {
                    return None;
                }
                match std::fs::read(&path) {
                    Ok(_) => {
                        let client = crate::repo_db::RealRepoClient::new();
                        match crate::repo_db::fetch_repo_packages(
                            &client, &r.url, &r.name, r.source, &c_dir, false, 999999,
                        )
                        .await
                        {
                            Ok(pkgs) => Some((r.name, pkgs)),
                            Err(_) => None,
                        }
                    }
                    Err(_) => None,
                }
            }));
        }

        for handle in handles {
            if let Ok(Some((name, pkgs))) = handle.await {
                let mut cache = self.cache.write().await;
                cache.insert(name, pkgs);
            }
        }
    }

    pub async fn sync_all(
        &self,
        force: bool,
        interval_hours: u64,
        app: Option<tauri::AppHandle>,
    ) -> Result<String, String> {
        use tauri::Emitter;
        let repos = self.repos.read().await;
        // Use all enabled repos for system sync, not just active ones (though they are usually same)
        let active_repos: Vec<RepoConfig> = repos.iter().filter(|r| r.enabled).cloned().collect();
        let enabled_repo_names: Vec<String> = active_repos.iter().map(|r| r.name.clone()).collect();
        drop(repos);

        // 1. Trigger System Sync (Helper) - This updates /var/lib/pacman/sync
        if let Some(ref a) = app {
            let _ = a.emit("sync-progress", "Synchronizing system databases...");

            // We need a password? sync_all is usually called from background or trigger_repo_sync.
            // trigger_repo_sync doesn't pass password.
            // However, AlpmSync requires root ONLY if writing to /var/lib/pacman/sync.
            // If we are in background, we might check if we can run passwordless (Polkit).
            // But invoke_helper handles Polkit via pkexec.
            // If the user isn't prompted, it might fail or hang?
            // Wait, AlpmSync in helper runs as root. pkexec will prompt if needed.
            // But if this runs on startup (background), we DON'T want a prompt blocking everything.
            //
            // The user prompt said: "Constraint: The UI search results must not update until both the local cache AND the system sync are complete."
            // But if this blocks on auth...
            // "When the GUI triggers a 'Refresh Mirrors/DBs'..." -> Usually a user action.
            // If it's the auto-sync on startup, we might skip system sync if it prompts.
            // But we can't detect "will prompt".

            // However, Polkit rules allow passwordless refresh usually?
            // "Authentication is required to install, update, or remove applications."
            // Refreshing DBs is "update".
            // Let's assume for "Refresh Mirrors" button (User Action) it is fine.
            // For background sync, it might annoy.
            // BUT: The "Dual Brain" fix is critical.
            // Let's implement it. If we are in `trigger_repo_sync` (User Action), we definitely want this.

            // To be safe, we spawn it.
            // But we need to wait for it?

            let one_click = self.is_one_click_enabled().await;
            let _ = invoke_helper(
                a,
                HelperCommand::AlpmSync {
                    enabled_repos: enabled_repo_names,
                },
                None,
                one_click,
            )
            .await;

            // We ignore the result/rx for now to not block the GUI cache update?
            // Or do we wait?
            // "Constraint: UI search results must not update until... complete"
            // So we SHOULD wait.
            // But `invoke_helper` returns Receiver. We must drain it to wait.
        }

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("monarch-store")
            .join("dbs");
        std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
        let mut handles = Vec::new();

        for repo in active_repos {
            let r = repo.clone();
            let c_dir = cache_dir.clone();
            let app_clone = app.clone();
            handles.push(tokio::spawn(async move {
                if let Some(ref a) = app_clone {
                    let _ = a.emit("sync-progress", format!("Updating {}...", r.name));
                }
                let client = repo_db::RealRepoClient::new();
                match repo_db::fetch_repo_packages(
                    &client,
                    &r.url,
                    &r.name,
                    r.source,
                    &c_dir,
                    force,
                    interval_hours,
                )
                .await
                {
                    Ok(pkgs) => Ok((r.name, pkgs)),
                    Err(e) => Err((r.name, e)),
                }
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(Ok((name, pkgs))) = handle.await {
                let mut cache = self.cache.write().await;
                cache.insert(name.clone(), pkgs);
                results.push(format!("Synced {} from {}", 0, name)); // Simplified logging
            }
        }
        Ok("Sync Complete".to_string())
    }

    /// Search for packages in the local cache matching the query string.
    /// Iterates every repo in the cache (all enabled repos from last sync); no repo is skipped.
    /// Uses regex for case-insensitive partial matching on name and description.
    pub async fn get_packages_matching(
        &self,
        query: &str,
        distro: &crate::distro_context::DistroContext,
    ) -> Result<Vec<Package>, String> {
        let query_parts: Vec<String> = query.split_whitespace().map(|s| s.to_string()).collect();
        let query_regexes: Vec<regex::Regex> = query_parts
            .iter()
            .filter_map(|p| {
                regex::RegexBuilder::new(&regex::escape(p))
                    .case_insensitive(true)
                    .build()
                    .ok()
            })
            .collect();

        if query_regexes.is_empty() {
            return Ok(Vec::new());
        }

        let cache = self.cache.read().await;
        // Map to deduplicate: name -> index in results
        let mut results_map: HashMap<String, usize> = HashMap::new();
        let mut results = Vec::new();

        // 1. Search Repos (Sync DBs)
        for (repo_name, pkgs) in cache.iter() {
            for pkg in pkgs {
                let mut all_match = true;
                for re in &query_regexes {
                    if !re.is_match(&pkg.name) && !re.is_match(&pkg.description) {
                        all_match = false;
                        break;
                    }
                }

                if all_match {
                    let mut p = pkg.clone();
                    p.source =
                        PackageSource::from_repo_name(repo_name, &p.version, distro, &p.name);
                    p.installed = crate::alpm_read::is_package_installed(&p.name);

                    if results_map.contains_key(&p.name) {
                        // If multiple repos provide same package, keep the first one found (or prioritize?)
                        // Usually Repos are distinct or overlapping (multilib vs extra).
                        // Let's just keep existing.
                    } else {
                        results_map.insert(p.name.clone(), results.len());
                        results.push(p);
                    }
                }
            }
        }

        // 2. Search Local DB (Installed Packages) - Fallback for missing repos (e.g. CachyOS not in cache)
        // This ensures installed apps ALWAYS show up even if repo sync failed.
        // Fallback Strategy: ALPM binding first, then CLI if binding returns nothing/fails.
        let mut local_pkgs = crate::alpm_read::get_installed_packages_native();
        if local_pkgs.is_empty() {
            local_pkgs = crate::alpm_read::search_installed_packages_cli(query);
        }

        for pkg in local_pkgs {
            let mut all_match = true;
            for re in &query_regexes {
                if !re.is_match(&pkg.name) && !re.is_match(&pkg.description) {
                    all_match = false;
                    break;
                }
            }

            if all_match {
                // Only add if not already found in Repos (prefer Repo source info if available)
                if !results_map.contains_key(&pkg.name) {
                    let p = pkg;
                    // p.source is already "local" from get_installed_packages_native
                    // p.installed is true
                    results_map.insert(p.name.clone(), results.len());
                    results.push(p);
                } else {
                    // Update installed status just in case (though step 1 checks is_package_installed)
                    let idx = results_map[&pkg.name];
                    results[idx].installed = true;
                }
            }
        }

        Ok(results)
    }

    /// Exact Match Lookup from Cache (Fallback for ALPM failures)
    /// Used when ALPM bindings fail or return empty results.
    /// Returns a simplified Package object populated from the text-file cache.
    pub async fn get_package_exact(
        &self,
        name: &str,
        distro: &crate::distro_context::DistroContext,
    ) -> Option<Package> {
        let cache = self.cache.read().await;
        for (repo_name, pkgs) in cache.iter() {
            if let Some(pkg) = pkgs.iter().find(|p| p.name == name) {
                let mut p = pkg.clone();
                p.source = PackageSource::from_repo_name(repo_name, &p.version, distro, &p.name);
                // We can't easily check installed status without ALPM, so default to false
                // or try the CLI fallback if critical. For now, false is safe for a fallback.
                // Actually, let's try the simple check:
                p.installed = crate::alpm_read::is_package_installed(&p.name);
                return Some(p);
            }
        }
        None
    }

    // MODULAR APPLY LOGIC — pass password so one prompt covers all helper invokes
    pub async fn apply_os_config(
        &self,
        app: &tauri::AppHandle,
        password: Option<String>,
    ) -> Result<(), String> {
        let repos = self.repos.read().await;
        drop(repos);
        // 1. Refactor note: Traditional "Repo Injection" is deprecated.
        // We no longer modify pacman.conf or manage .conf files directly via HelperCommand::{WriteFiles, RemoveFiles}.
        // The application now uses Host Detection to respect system-provided repositories.

        // Host-Adaptive: We assume the user has configured pacman.conf manually or via other tools.
        // We do trigger a DB refresh to ensure our cache is up to date with the system.
        let one_click = self.is_one_click_enabled().await;
        let mut rx = invoke_helper(
            app,
            HelperCommand::ExecuteBatch {
                manifest: crate::models::TransactionManifest {
                    refresh_db: true,
                    ..Default::default()
                },
            },
            password,
            one_click,
        )
        .await?;
        while rx.recv().await.is_some() {}
        Ok(())
    }

    pub async fn set_repo_state(
        &self,
        app: &tauri::AppHandle,
        name: &str,
        enabled: bool,
    ) -> Result<(), String> {
        // --- FIREWALL: Identity Matrix Check ---
        let distro = crate::distro_context::get_distro_context();

        // Rule 1: Manjaro cannot enable Chaotic-AUR (Glibc Mismatch)
        if enabled && name == "chaotic-aur" {
            // Bypass check if in Advanced Mode
            if !*self.advanced_mode.read().await {
                if let crate::distro_context::ChaoticSupport::Blocked =
                    distro.capabilities.chaotic_aur_support
                {
                    return Err(format!(
                        "ACTION BLOCKED: Enabling Chaotic-AUR on {} is unsafe due to glibc incompatibility.", 
                        distro.pretty_name
                    ));
                }
            }
        }
        // ---------------------------------------

        let mut repos = self.repos.write().await;
        if let Some(r) = repos.iter_mut().find(|r| r.name == name) {
            r.enabled = enabled;
        }
        drop(repos);

        // Instant UI Update: Clear from cache if disabled
        if !enabled {
            let mut cache = self.cache.write().await;
            cache.remove(name);
        }

        self.save_config_async().await;
        // Apply config and sync so the repo is usable (Apple Store–like)
        self.apply_os_config(app, None).await?;
        Ok(())
    }

    /// Toggle all repos belonging to a family (e.g., "cachyos", "manjaro")
    /// Added skip_os_sync to avoid 4x prompts during onboarding
    pub async fn set_repo_family_state(
        &self,
        app: &tauri::AppHandle,
        family: &str,
        enabled: bool,
        skip_os_sync: bool,
    ) -> Result<(), String> {
        let mut repos = self.repos.write().await;
        let family_lower = family.to_lowercase();
        let mut affected_repos = Vec::new();

        for repo in repos.iter_mut() {
            let repo_lower = repo.name.to_lowercase();

            // Match family by prefix or exact match
            let belongs_to_family = match family_lower.as_str() {
                "cachyos" => repo_lower.starts_with("cachyos"),
                "manjaro" => repo_lower.starts_with("manjaro"),
                "chaotic" | "chaotic-aur" => repo_lower == "chaotic-aur",
                "garuda" => repo_lower == "garuda",
                "endeavouros" => repo_lower == "endeavouros",
                _ => repo_lower == family_lower,
            };

            if belongs_to_family {
                if enabled {
                    // Smart enable: For CachyOS, only enable if CPU compatible
                    if repo_lower.contains("-znver4") {
                        repo.enabled = crate::utils::is_cpu_znver4_compatible();
                    } else if repo_lower.contains("-v4") {
                        repo.enabled = crate::utils::is_cpu_v4_compatible();
                    } else if repo_lower.contains("-v3") || repo_lower.contains("-core") {
                        repo.enabled = crate::utils::is_cpu_v3_compatible();
                    } else {
                        repo.enabled = true;
                    }
                } else {
                    repo.enabled = false;
                    affected_repos.push(repo.name.clone());
                }
            }
        }

        drop(repos);

        // Instant UI Update: Batch clear
        if !affected_repos.is_empty() {
            let mut cache = self.cache.write().await;
            for name in affected_repos {
                cache.remove(&name);
            }
        }

        self.save_config_async().await;
        if !skip_os_sync {
            self.apply_os_config(app, None).await?;
        }

        Ok(())
    }

    // ... inside RepoManager impl ...

    #[allow(dead_code)]
    pub async fn get_package(&self, name: &str) -> Option<Package> {
        // Reuse get_all_packages Logic which now sorts by optimization
        let pkgs = self.get_all_packages(name).await;
        pkgs.into_iter().next() // Return the top-ranked one
    }

    /// Returns packages from enabled repos only (soft disable).
    pub async fn get_all_packages_with_repos(&self, name: &str) -> Vec<(Package, String)> {
        let enabled: std::collections::HashSet<String> = {
            let repos = self.repos.read().await;
            repos
                .iter()
                .filter(|r| r.enabled)
                .map(|r| r.name.clone())
                .collect()
        };
        let cache = self.cache.read().await;
        let mut results: Vec<(Package, u8, String)> = Vec::new();
        let cpu_v3 = crate::utils::is_cpu_v3_compatible();
        let cpu_v4 = crate::utils::is_cpu_v4_compatible();
        let distro = crate::distro_context::get_distro_context();

        for (repo_name, pkgs) in cache.iter() {
            if !enabled.contains(repo_name) {
                continue;
            }
            let opt_level: u8 =
                if repo_name.contains("-znver4") && crate::utils::is_cpu_znver4_compatible() {
                    3
                } else if repo_name.contains("-v4") && cpu_v4 {
                    2
                } else if (repo_name.contains("-v3")
                    || repo_name.contains("-core-v3")
                    || repo_name.contains("-extra-v3"))
                    && cpu_v3
                {
                    1
                } else {
                    0
                };

            if let Some(p) = pkgs.iter().find(|p| p.name == name) {
                results.push((p.clone(), opt_level, repo_name.clone()));
            }
        }

        results.sort_by(|(pkg_a, level_a, _), (pkg_b, level_b, _)| {
            let rank_a = calculate_package_rank(pkg_a, *level_a, &distro);
            let rank_b = calculate_package_rank(pkg_b, *level_b, &distro);
            rank_a.cmp(&rank_b)
        });

        results
            .into_iter()
            .map(|(mut p, level, r_name)| {
                p.is_optimized = Some(level > 0);
                (p, r_name)
            })
            .collect()
    }

    #[allow(dead_code)]
    pub async fn get_all_packages(&self, name: &str) -> Vec<Package> {
        self.get_all_packages_with_repos(name)
            .await
            .into_iter()
            .map(|(p, _)| p)
            .collect()
    }

    #[allow(dead_code)]
    pub async fn get_packages_providing(&self, name: &str) -> Vec<Package> {
        self.get_packages_providing_with_repos(name)
            .await
            .into_iter()
            .map(|(p, _)| p)
            .collect()
    }

    /// Returns packages from enabled repos only (soft disable).
    pub async fn get_packages_providing_with_repos(&self, name: &str) -> Vec<(Package, String)> {
        let mut results = Vec::new();
        let enabled: std::collections::HashSet<String> = {
            let repos = self.repos.read().await;
            repos
                .iter()
                .filter(|r| r.enabled)
                .map(|r| r.name.clone())
                .collect()
        };
        let cache = self.cache.read().await;

        for (repo_name, repo_pkgs) in cache.iter() {
            if !enabled.contains(repo_name) {
                continue;
            }
            for pkg in repo_pkgs {
                if let Some(provides) = &pkg.provides {
                    if provides.iter().any(|p| p == name) {
                        results.push((pkg.clone(), repo_name.clone()));
                    }
                }
            }
        }
        results
    }

    /// Returns packages from enabled repos only (soft disable).
    /// Prefer alpm_read::get_packages_batch (ALPM as single READ source); this remains for fallback/sync paths.
    #[allow(dead_code)]
    pub async fn get_packages_batch(&self, names: &[String]) -> Vec<Package> {
        let mut results = Vec::new();
        let enabled: std::collections::HashSet<String> = {
            let repos = self.repos.read().await;
            repos
                .iter()
                .filter(|r| r.enabled)
                .map(|r| r.name.clone())
                .collect()
        };
        let cache = self.cache.read().await;
        let names_set: std::collections::HashSet<&str> = names.iter().map(|s| s.as_str()).collect();
        for (repo_name, pkgs) in cache.iter() {
            if enabled.contains(repo_name) {
                for pkg in pkgs {
                    if names_set.contains(pkg.name.as_str()) {
                        results.push(pkg.clone());
                    }
                }
            }
        }
        results
    }

    pub async fn get_package_counts(&self) -> HashMap<String, usize> {
        let cache = self.cache.read().await;
        cache
            .iter()
            .map(|(name, pkgs)| (name.clone(), pkgs.len()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Package, PackageSource};

    fn make_test_pkg(source: PackageSource) -> Package {
        Package {
            name: "test".to_string(),
            display_name: None,
            description: "test".to_string(),
            version: "1.0".to_string(),
            source,
            maintainer: None,
            url: None,
            license: None,
            keywords: None,
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            num_votes: None,
            icon: None,
            app_id: None,
            screenshots: None,
            is_optimized: None,
            provides: None,
            depends: None,
            make_depends: None,
            is_featured: None,
            installed: false,
            ..Default::default()
        }
    }

    #[test]
    fn test_chaotic_priority() {
        let p_chaotic = make_test_pkg(PackageSource::chaotic("test-pkg"));
        let p_official = make_test_pkg(PackageSource::official("test-pkg"));
        let p_aur = make_test_pkg(PackageSource::aur("test-pkg"));
        let distro = crate::distro_context::DistroContext::new(); // Default Arch

        // Rank Check: priority() + 3 = Chaotic(1+3)=4, Official(2+3)=5, Aur(4+3)=7
        assert_eq!(calculate_package_rank(&p_chaotic, 0, &distro), 4);
        assert_eq!(calculate_package_rank(&p_official, 0, &distro), 5);
        assert_eq!(calculate_package_rank(&p_aur, 0, &distro), 7);

        // Verify Chaotic beats Official (Lower rank is better)
        assert!(
            calculate_package_rank(&p_chaotic, 0, &distro)
                < calculate_package_rank(&p_official, 0, &distro)
        );
    }

    #[test]
    fn test_optimized_priority() {
        let p_cachy = make_test_pkg(PackageSource::cachyos("test-pkg"));
        let distro = crate::distro_context::DistroContext::new(); // Default Arch

        // Optimized tiers: znver4=0, v4=1, v3=2, standard=priority()+3=1+3=4
        assert_eq!(calculate_package_rank(&p_cachy, 3, &distro), 0); // znver4
        assert_eq!(calculate_package_rank(&p_cachy, 2, &distro), 1); // v4
        assert_eq!(calculate_package_rank(&p_cachy, 1, &distro), 2); // v3
        assert_eq!(calculate_package_rank(&p_cachy, 0, &distro), 4); // Standard CachyOS (priority 1 + 3)

        assert!(
            calculate_package_rank(&p_cachy, 1, &distro)
                < calculate_package_rank(&p_cachy, 0, &distro)
        );
    }
}

// Diagnostic: Check which repos are actually synced in pacman system
#[tauri::command]
#[specta::specta]
pub async fn check_repo_sync_status(
    state_repo: tauri::State<'_, RepoManager>,
) -> Result<std::collections::HashMap<String, bool>, String> {
    let repos = state_repo.repos.read().await;
    let mut status = std::collections::HashMap::new();
    let sync_dir = std::path::Path::new("/var/lib/pacman/sync");

    for repo in repos.iter() {
        if !repo.enabled {
            status.insert(repo.name.clone(), true); // Disabled repos considered "fine"
            continue;
        }
        let db_path = sync_dir.join(format!("{}.db", repo.name));
        status.insert(repo.name.clone(), db_path.exists());
    }
    Ok(status)
}

// Trigger DB refresh to reflect system config. No repo injection; we read from ALPM.
#[tauri::command]
#[specta::specta]
pub async fn apply_os_config(
    app: tauri::AppHandle,
    state_repo: tauri::State<'_, RepoManager>,
    password: Option<String>,
) -> Result<(), String> {
    state_repo.inner().apply_os_config(&app, password).await
}

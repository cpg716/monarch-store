use crate::models::{Package, PackageSource};
use crate::repo_db;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepoConfig {
    pub name: String,
    pub url: String,
    pub source: PackageSource,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize)]
struct StoredConfig {
    repos: Vec<RepoConfig>,
    #[serde(default)]
    aur_enabled: bool,
    #[serde(default)]
    one_click_enabled: bool,
}

#[derive(Clone)]
pub struct RepoManager {
    // Map RepoName -> List of Packages
    cache: Arc<RwLock<HashMap<String, Vec<Package>>>>,
    repos: Arc<RwLock<Vec<RepoConfig>>>,
    pub aur_enabled: Arc<RwLock<bool>>,
    pub one_click_enabled: Arc<RwLock<bool>>,
}

// Helper for Intelligent Priority Sorting (Chaotic-First)
pub fn calculate_package_rank(pkg: &Package, is_opt: bool) -> u8 {
    if is_opt {
        return 0; // Rank 0: Hardware Optimized (GOD TIER)
    }
    pkg.source.priority()
}

impl RepoManager {
    pub fn new() -> Self {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("monarch-store");
        std::fs::create_dir_all(&config_path).unwrap_or_default();
        let config_file = config_path.join("repos.json");

        // Default Repos - Chaotic-AUR is PRIMARY
        // CRITICAL: Use valid pacman repo names (lowercase, no spaces)
        let defaults = vec![
            RepoConfig {
                name: "chaotic-aur".to_string(),
                url: "https://cdn-mirror.chaotic.cx/chaotic-aur/x86_64/chaotic-aur.db".to_string(),
                source: PackageSource::Chaotic,
                enabled: true,
            },
            RepoConfig {
                name: "core".to_string(),
                url: "https://geo.mirror.pkgbuild.com/core/os/x86_64/core.db".to_string(),
                source: PackageSource::Official,
                enabled: true,
            },
            RepoConfig {
                name: "extra".to_string(),
                url: "https://geo.mirror.pkgbuild.com/extra/os/x86_64/extra.db".to_string(),
                source: PackageSource::Official,
                enabled: true,
            },
            RepoConfig {
                name: "multilib".to_string(),
                url: "https://geo.mirror.pkgbuild.com/multilib/os/x86_64/multilib.db".to_string(),
                source: PackageSource::Official,
                enabled: true,
            },
            RepoConfig {
                name: "cachyos".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64/cachyos/cachyos.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: true,
            },
            RepoConfig {
                name: "cachyos-v3".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v3/cachyos-v3/cachyos-v3.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: crate::utils::is_cpu_v3_compatible(),
            },
            RepoConfig {
                name: "cachyos-core-v3".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v3/cachyos-core-v3/cachyos-core-v3.db"
                    .to_string(),
                source: PackageSource::CachyOS,
                enabled: crate::utils::is_cpu_v3_compatible(),
            },
            RepoConfig {
                name: "cachyos-extra-v3".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v3/cachyos-extra-v3/cachyos-extra-v3.db"
                    .to_string(),
                source: PackageSource::CachyOS,
                enabled: crate::utils::is_cpu_v3_compatible(),
            },
            RepoConfig {
                name: "cachyos-v4".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v4/cachyos-v4/cachyos-v4.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: crate::utils::is_cpu_v4_compatible(),
            },
            RepoConfig {
                name: "cachyos-core-v4".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v4/cachyos-core-v4/cachyos-core-v4.db"
                    .to_string(),
                source: PackageSource::CachyOS,
                enabled: crate::utils::is_cpu_v4_compatible(),
            },
            RepoConfig {
                name: "cachyos-extra-v4".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v4/cachyos-extra-v4/cachyos-extra-v4.db"
                    .to_string(),
                source: PackageSource::CachyOS,
                enabled: crate::utils::is_cpu_v4_compatible(),
            },
            RepoConfig {
                name: "cachyos-extra-znver4".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v4/cachyos-extra-znver4/cachyos-extra-znver4.db"
                    .to_string(),
                source: PackageSource::CachyOS,
                enabled: crate::utils::is_cpu_znver4_compatible(),
            },
            RepoConfig {
                name: "cachyos-core-znver4".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v4/cachyos-core-znver4/cachyos-core-znver4.db"
                    .to_string(),
                source: PackageSource::CachyOS,
                enabled: crate::utils::is_cpu_znver4_compatible(),
            },
            RepoConfig {
                name: "garuda".to_string(),
                url: "https://builds.garudalinux.org/repos/garuda/x86_64/garuda.db".to_string(),
                source: PackageSource::Garuda,
                enabled: true,
            },
            RepoConfig {
                name: "endeavouros".to_string(),
                url: "https://mirror.moson.org/endeavouros/repo/endeavouros/x86_64/endeavouros.db"
                    .to_string(),
                source: PackageSource::Endeavour,
                enabled: true,
            },
            RepoConfig {
                name: "manjaro-core".to_string(),
                url: "https://mirror.init7.net/manjaro/stable/core/x86_64/core.db".to_string(),
                source: PackageSource::Manjaro,
                enabled: true,
            },
            RepoConfig {
                name: "manjaro-extra".to_string(),
                url: "https://mirror.init7.net/manjaro/stable/extra/x86_64/extra.db".to_string(),
                source: PackageSource::Manjaro,
                enabled: true,
            },
        ];

        let mut initial_repos = defaults.clone();
        let mut initial_aur = false;
        let mut initial_one_click = false;

        // Try Load Config
        if config_file.exists() {
            if let Ok(file) = std::fs::File::open(&config_file) {
                let reader = std::io::BufReader::new(file);
                if let Ok(saved_config) = serde_json::from_reader::<_, StoredConfig>(reader) {
                    initial_aur = saved_config.aur_enabled;
                    initial_one_click = saved_config.one_click_enabled;

                    // MIGRATION FIX:
                    // Only merge saved config if the name EXACTLY matches a known valid default.
                    // This filters out legacy "Arch Multilib" entries which won't match "multilib".
                    // OR we could try to map them, but resetting to defaults is cleaner for fixing this bug.
                    for repo in &mut initial_repos {
                        // Check for legacy matches to migrating enabled status
                        let legacy_name = match repo.name.as_str() {
                            "multilib" => "Arch Multilib",
                            "core" => "Arch Core",
                            "extra" => "Arch Extra",
                            "chaotic-aur" => "Chaotic-AUR",
                            _ => "",
                        };

                        if let Some(saved_repo) = saved_config.repos.iter().find(|r| {
                            r.name == repo.name
                                || (!legacy_name.is_empty() && r.name == legacy_name)
                        }) {
                            repo.enabled = saved_repo.enabled;
                        }

                        // CRITICAL: Strict Hardware Enforcement
                        // If a repo is enabled but incompatible with the detected CPU, force it off.
                        let is_cachy = repo.name.to_lowercase().starts_with("cachyos");
                        if repo.enabled && is_cachy {
                            let repo_lower = repo.name.to_lowercase();
                            let is_compatible = if repo_lower.contains("-znver4") {
                                crate::utils::is_cpu_znver4_compatible()
                            } else if repo_lower.contains("-v4") {
                                crate::utils::is_cpu_v4_compatible()
                            } else if repo_lower.contains("-v3") || repo_lower.contains("-core") {
                                crate::utils::is_cpu_v3_compatible()
                            } else {
                                true // standard x86-64
                            };

                            if !is_compatible {
                                println!(
                                    "WARNING: Disabling incompatible repo '{}' for current CPU",
                                    repo.name
                                );
                                repo.enabled = false;
                            }
                        }
                    }
                }
            }
        }

        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            repos: Arc::new(RwLock::new(initial_repos)),
            aur_enabled: Arc::new(RwLock::new(initial_aur)),
            one_click_enabled: Arc::new(RwLock::new(initial_one_click)),
        }
    }

    async fn save_config_async(&self) {
        let repos = self.repos.read().await.clone();
        let aur = *self.aur_enabled.read().await;
        let one_click = *self.one_click_enabled.read().await;

        // Spawn blocking task for file I/O
        tokio::task::spawn_blocking(move || {
            let config = StoredConfig {
                repos,
                aur_enabled: aur,
                one_click_enabled: one_click,
            };

            let config_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("monarch-store");
            let _ = std::fs::create_dir_all(&config_path);
            let config_file = config_path.join("repos.json");

            if let Ok(file) = std::fs::File::create(config_file) {
                let _ = serde_json::to_writer_pretty(file, &config);
            }
        });
    }

    pub async fn set_aur_enabled(&self, enabled: bool) {
        let mut w = self.aur_enabled.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_aur_enabled(&self) -> bool {
        *self.aur_enabled.read().await
    }

    pub async fn set_one_click_enabled(&self, enabled: bool) {
        let mut w = self.one_click_enabled.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_one_click_enabled(&self) -> bool {
        *self.one_click_enabled.read().await
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

        println!("Loading initial package cache from disk...");
        let mut handles = Vec::new();

        for repo in active_repos {
            let r = repo.clone();
            let c_dir = cache_dir.clone();

            // Spawn parsing tasks
            handles.push(tokio::spawn(async move {
                let file_name = format!("{}.db", r.name);
                let path = c_dir.join(file_name);

                if !path.exists() {
                    return None;
                }

                match std::fs::read(&path) {
                    Ok(_) => {
                        // We use a simplified parsing or reuse repo_db logic?
                        // We need to decode the tar.gz/zst. repo_db code is reusable if refactored,
                        // but for now let's call fetch_repo_packages with force=false and huge interval?
                        // No, fetch_repo_packages does network logic.
                        // Let's copy the extraction logic or move it to a public helper in repo_db?
                        // For expediency, we can just call fetch_repo_packages with a VERY LARGE interval (e.g. 100000 hours)
                        // This ensures it treats disk as fresh.
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
        println!("Initial cache load complete.");
    }

    pub async fn sync_all(
        &self,
        force: bool,
        interval_hours: u64,
        app: Option<tauri::AppHandle>,
    ) -> Result<String, String> {
        use tauri::Emitter;
        let repos = self.repos.read().await;
        // Only sync enabled repos
        let active_repos: Vec<RepoConfig> = repos.iter().filter(|r| r.enabled).cloned().collect();
        drop(repos); // Release lock

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
                // Pass cache_dir, force flag, and interval_hours
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

        // Await all tasks
        for handle in handles {
            match handle.await {
                Ok(task_res) => {
                    match task_res {
                        Ok((name, pkgs)) => {
                            let count = pkgs.len();
                            // Update Cache
                            let mut cache = self.cache.write().await;
                            cache.insert(name.clone(), pkgs);
                            results.push(format!("Synced {} packages from {}", count, name));
                        }
                        Err((name, e)) => {
                            eprintln!("Failed to sync {}: {}", name, e);
                            results.push(format!("Failed {}: {}", name, e));
                        }
                    }
                }
                Err(e) => {
                    results.push(format!("Task execution failed: {}", e));
                }
            }
        }

        Ok(results.join(", "))
    }

    pub async fn apply_os_config(&self, password: Option<String>) -> Result<(), String> {
        let repos = self.repos.read().await;
        // SOFT TOGGLE: We keep ALL repos enabled in pacman.conf even if disabled in UI
        // This ensures background updates work.
        let active_repos: Vec<RepoConfig> = repos.iter().cloned().collect();
        drop(repos);

        // 1. Generate Config Content (Modular - one file per repo or one consolidated monarch file)
        // We'll use one consolidated file in the modular directory for simplicity and avoiding orphan files.
        let mut config_content =
            String::from("# Generated by MonARCH Store (Infrastructure 2.0)\n");

        // We also need to build a list of "Special Sync" commands for the script
        let mut manual_sync_cmds = String::new();

        for repo in &active_repos {
            if repo.source == PackageSource::Official {
                continue;
            } // Don't duplicate official repos

            // Detect Naming Mismatch (e.g., [manjaro-core] vs .../core.db)
            let url_filename = repo.url.split('/').last().unwrap_or("");
            let expected_filename = format!("{}.db", repo.name);

            // Check if we need the Local Mirror Workaround
            // Case: URL ends in .db BUT the filename doesn't match the repo section name
            if repo.url.ends_with(".db") && url_filename != expected_filename {
                // LOCAL MIRROR STRATEGY (Split-Mirror)
                // 1. Local DB (file:///) -> Satisfies -Sy (DB update)
                // 2. Upstream (http://...) -> Satisfies -S (Package download)

                // Calculate stripped upstream URL for the second Server line
                let upstream_server_url = if repo.url.ends_with(".db") {
                    let parts: Vec<&str> = repo.url.split('/').collect();
                    if parts.len() > 1 {
                        parts[..parts.len() - 1].join("/")
                    } else {
                        repo.url.clone()
                    }
                } else {
                    repo.url.clone()
                };

                // Config: Prioritize Local DB, fallback to Upstream for packages
                config_content.push_str(&format!(
                    "\n[{}]\nServer = file:///var/lib/monarch/dbs\nServer = {}\n",
                    repo.name, upstream_server_url
                ));

                // Add command to manually download the DB to the STASH path
                manual_sync_cmds.push_str(&format!(
                    "echo 'Step: Stashing matched DB for {}...'\n",
                    repo.name
                ));

                // Ensure stashing directory exists
                manual_sync_cmds.push_str("mkdir -p /var/lib/monarch/dbs\n");

                manual_sync_cmds.push_str(&format!(
                    "curl -f -L -s -o /var/lib/monarch/dbs/{}.db '{}' || echo 'Failed to stash {}'\n",
                    repo.name, repo.url, repo.name
                ));

                // Also download the .sig file if signature checking is required (optional but good practice)
                // actually, for now let's just do the DB to fix the 404.
            } else {
                // STANDARD STRATEGY
                // Pacman Server directive must be a DIRECTORY, not the full file path.
                let server_url = if repo.url.ends_with(".db") {
                    let parts: Vec<&str> = repo.url.split('/').collect();
                    if parts.len() > 1 {
                        parts[..parts.len() - 1].join("/")
                    } else {
                        repo.url.clone()
                    }
                } else {
                    repo.url.clone()
                };

                config_content.push_str(&format!("\n[{}]\nServer = {}\n", repo.name, server_url));
            }

            if repo.name == "chaotic-aur" || repo.name.starts_with("cachyos") {
                config_content.push_str("SigLevel = PackageRequired\n");
            } else {
                config_content.push_str("SigLevel = Optional TrustAll\n");
            }
        }

        // 2. Build BATCH script
        let mut script = String::from("echo '--- MonARCH System Integration ---'\n");

        // Ensure directory exists and is CLEAN to avoid stale/broken configs (like the EndeavourOS mirrorlist error)
        script.push_str("mkdir -p /etc/pacman.d/monarch\n");
        script.push_str("rm -f /etc/pacman.d/monarch/*.conf\n");

        // Infrastructure 2.0: Ensure Include is in pacman.conf
        // We also clean up legacy direct entries
        script.push_str(r#"
if ! grep -q "/etc/pacman.d/monarch/\*.conf" /etc/pacman.conf; then
    echo -e "\n# MonARCH Managed Repositories\nInclude = /etc/pacman.d/monarch/*.conf" >> /etc/pacman.conf
fi
# Best effort cleanup of legacy direct entries
sed -i '/\[chaotic-aur\]/,/^\s*$/{d}' /etc/pacman.conf
sed -i '/\[cachyos\]/,/^\s*$/{d}' /etc/pacman.conf
"#);

        // Chaotic-AUR Key Import (Reliable method)
        if active_repos.iter().any(|r| r.name == "chaotic-aur") {
            script.push_str("echo 'Syncing Chaotic-AUR Keys...'\n");
            script.push_str(
                "pacman-key --recv-key 3056513887B78AEB --keyserver keyserver.ubuntu.com || true\n",
            );
            script.push_str("pacman-key --lsign-key 3056513887B78AEB\n");
        }

        // CachyOS Key Import
        if active_repos.iter().any(|r| r.name.starts_with("cachyos")) {
            script.push_str("echo 'Syncing CachyOS Keys...'\n");
            script.push_str(
                "pacman-key --recv-key F3B607488DB35A47 --keyserver keyserver.ubuntu.com || true\n",
            );
            script.push_str("pacman-key --lsign-key F3B607488DB35A47\n");
        }

        // Write the modular config
        script.push_str(&format!(
            "cat <<EOF | tee /etc/pacman.d/monarch/monarch_repos.conf > /dev/null\n{}\nEOF\n",
            config_content
        ));

        // Execute Manual Sync Commands (for Manjaro/Mismatched repos)
        // We do this BEFORE standard pacman -Sy
        if !manual_sync_cmds.is_empty() {
            script.push_str(&manual_sync_cmds);
        }

        // Sync databases to make changes effective immediately
        script.push_str("pacman -Sy --noconfirm\n");

        // 3. Execute via run_privileged_script
        crate::utils::run_privileged_script(&script, password, false)
            .await
            .map(|_| ())
    }

    pub async fn set_repo_state(&self, name: &str, enabled: bool) {
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
        // Default behavior (no skip args in struct so we assume standard apply)
        // If we want to skip, we need arguments. But this is internal API.
        // We will call apply explicitly from commands if needed.
        let _ = self.apply_os_config(None).await;
    }

    /// Toggle all repos belonging to a family (e.g., "cachyos", "manjaro")
    /// Added skip_os_sync to avoid 4x prompts during onboarding
    pub async fn set_repo_family_state(&self, family: &str, enabled: bool, skip_os_sync: bool) {
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
            let _ = self.apply_os_config(None).await;
        }
    }

    /// Targeted Install Helper: Safely ensure a specific repo database exists
    /// Handles Local Mirror strategy for mismatched repos (Manjaro) automatically.
    #[allow(dead_code)]
    pub async fn ensure_repo_sync(
        &self,
        repo_name: &str,
        password: Option<String>,
    ) -> Result<(), String> {
        // Self-Healing: Verify config exists, otherwise everything fails.
        let config_path = std::path::Path::new("/etc/pacman.d/monarch/monarch_repos.conf");
        if !config_path.exists() {
            eprintln!("WARN: Config missing during ensure_repo_sync. Regenerating...");
            self.apply_os_config(password.clone()).await?;
        }

        let repos = self.repos.read().await;
        let repo = repos.iter().find(|r| r.name == repo_name).cloned();
        drop(repos);

        let repo = match repo {
            Some(r) => r,
            Option::None => {
                return Err(format!(
                    "Repository '{}' not found in configuration",
                    repo_name
                ))
            }
        };

        // Detect Naming Mismatch (Manjaro Strategy)
        let url_filename = repo.url.split('/').last().unwrap_or("");
        let expected_filename = format!("{}.db", repo.name);

        let is_mismatch = repo.url.ends_with(".db") && url_filename != expected_filename;

        // Build Command
        let mut script = String::from("echo '--- MonARCH Targeted Sync ---'\n");

        if is_mismatch {
            // Manual Download (Idempotent: Only if missing)
            // We check for the DB file existence in shell to avoid unnecessary network calls
            // Note: We check the STASH path now, because that's what our "Server = file://..." points to.
            script.push_str(&format!(
                "if [ ! -f /var/lib/monarch/dbs/{}.db ]; then\n",
                repo.name
            ));
            script.push_str(&format!(
                "    echo 'Detected Mismatched DB. manually stashing {}...'\n",
                repo.name
            ));
            script.push_str("    mkdir -p /var/lib/monarch/dbs\n");
            script.push_str(&format!(
                "    curl -f -L -s -o /var/lib/monarch/dbs/{}.db '{}' || exit 1\n",
                repo.name, repo.url
            ));
            script.push_str("fi\n");
        } else {
            // Standard Sync (Idempotent: Only if missing)
            // This prevents running full -Sy on every install, while ensuring the DB exists.
            script.push_str(&format!(
                "if [ ! -f /var/lib/pacman/sync/{}.db ]; then\n",
                repo.name
            ));
            script.push_str(&format!(
                "    echo 'DB missing for {}. Syncing...'\n",
                repo.name
            ));
            script.push_str("    pacman -Sy --noconfirm\n"); // We could target just this repo if we knew the syntax, but -Sy is safest fallack
            script.push_str("fi\n");
        }

        crate::utils::run_privileged_script(&script, password, false)
            .await
            .map(|_| ())
    }

    pub async fn search(&self, query: &str) -> Vec<Package> {
        let cache = self.cache.read().await;
        // Store (Package, is_optimized) tuples
        let mut results: Vec<(Package, bool)> = Vec::new();
        let q = query.to_lowercase();

        let cpu_v3 = crate::utils::is_cpu_v3_compatible();
        let cpu_v4 = crate::utils::is_cpu_v4_compatible();

        for (repo_name, pkgs) in cache.iter() {
            // Determine optimization status for this repo
            let is_optimized = if repo_name.contains("-v4") {
                cpu_v4
            } else if repo_name.contains("-v3") || repo_name.contains("-znver4") {
                // treat znver4 as v3+ equivalent for ranking or strict v4 check?
                // Using v3 check for generic v3 boost, or strict check if we want to be safe.
                // The repo enable logic handles strict compatibility.
                // Here we just want to know "is this a fancy repo".
                // If it's enabled and in cache, it MUST be compatible (enforced by set_repo_family_state).
                // So checking contains is enough?
                // Let's be explicit and re-verify hardware to be safe/correct conceptually.
                if repo_name.contains("-znver4") {
                    crate::utils::is_cpu_znver4_compatible()
                } else {
                    cpu_v3
                }
            } else {
                false
            };

            for p in pkgs {
                let name_match = p.name.to_lowercase().contains(&q);
                let display_match = p
                    .display_name
                    .as_ref()
                    .map(|dn| dn.to_lowercase().contains(&q))
                    .unwrap_or(false);

                if name_match || display_match {
                    results.push((p.clone(), is_optimized));
                }
            }
        }

        // Sort results by Intelligent Power Standard:
        // 1. Optimized Hardware Repo (CachyOS v3/v4)
        // 2. Chaotic / CachyOS (Instant)
        // 3. Official Arch
        // 4. AUR
        results.sort_by(|(pkg_a, opt_a), (pkg_b, opt_b)| {
            // Helper to get sort rank
            let get_rank = |pkg: &Package, is_opt: bool| -> u8 {
                if is_opt {
                    return 0;
                } // Rank 0: Hardware Optimized (GOD TIER)
                pkg.source.priority()
            };

            let rank_a = get_rank(pkg_a, *opt_a);
            let rank_b = get_rank(pkg_b, *opt_b);

            if rank_a != rank_b {
                return rank_a.cmp(&rank_b);
            }

            // Tie-breaker: Name length (shorter is usually more relevant)
            pkg_a.name.len().cmp(&pkg_b.name.len())
        });

        // Unpack and Populate
        results
            .into_iter()
            .map(|(mut p, opt)| {
                p.is_optimized = Some(opt);
                p
            })
            .collect()
    }

    #[allow(dead_code)]
    pub async fn get_package(&self, name: &str) -> Option<Package> {
        // Reuse get_all_packages Logic which now sorts by optimization
        let pkgs = self.get_all_packages(name).await;
        pkgs.into_iter().next() // Return the top-ranked one
    }

    pub async fn get_all_packages_with_repos(&self, name: &str) -> Vec<(Package, String)> {
        let cache = self.cache.read().await;
        // (Package, is_optimized, repo_name)
        let mut results: Vec<(Package, bool, String)> = Vec::new();

        let cpu_v3 = crate::utils::is_cpu_v3_compatible();
        let cpu_v4 = crate::utils::is_cpu_v4_compatible();

        for (repo_name, pkgs) in cache.iter() {
            let is_optimized = if repo_name.contains("-v4") {
                cpu_v4
            } else if repo_name.contains("-v3") || repo_name.contains("-znver4") {
                if repo_name.contains("-znver4") {
                    crate::utils::is_cpu_znver4_compatible()
                } else {
                    cpu_v3
                }
            } else {
                false
            };

            if let Some(p) = pkgs.iter().find(|p| p.name == name) {
                results.push((p.clone(), is_optimized, repo_name.clone()));
            }
        }

        results.sort_by(|(pkg_a, opt_a, _), (pkg_b, opt_b, _)| {
            let rank_a = calculate_package_rank(pkg_a, *opt_a);
            let rank_b = calculate_package_rank(pkg_b, *opt_b);
            rank_a.cmp(&rank_b)
        });

        results
            .into_iter()
            .map(|(mut p, opt, r_name)| {
                p.is_optimized = Some(opt);
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

    pub async fn get_packages_providing_with_repos(&self, name: &str) -> Vec<(Package, String)> {
        // Optimization logic could be applied here too if needed, but less critical.
        // For now keeping it simple.
        let mut results = Vec::new();
        let cache = self.cache.read().await;

        for (repo_name, repo_pkgs) in cache.iter() {
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

    pub async fn get_packages_batch(&self, names: &[String]) -> Vec<Package> {
        let mut results = Vec::new();
        let cache = self.cache.read().await;
        for pkgs in cache.values() {
            for pkg in pkgs {
                if names.contains(&pkg.name) {
                    results.push(pkg.clone());
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
            alternatives: None,
        }
    }

    #[test]
    fn test_chaotic_priority() {
        let p_chaotic = make_test_pkg(PackageSource::Chaotic);
        let p_official = make_test_pkg(PackageSource::Official);
        let p_aur = make_test_pkg(PackageSource::Aur);

        // Rank Check directly
        assert_eq!(calculate_package_rank(&p_chaotic, false), 1);
        assert_eq!(calculate_package_rank(&p_official, false), 2);
        assert_eq!(calculate_package_rank(&p_aur, false), 4);

        // Verify Chaotic beats Official (Lower rank is better)
        assert!(
            calculate_package_rank(&p_chaotic, false) < calculate_package_rank(&p_official, false)
        );
    }

    #[test]
    fn test_optimized_priority() {
        let p_cachy = make_test_pkg(PackageSource::CachyOS);

        // Optimized Cachy vs Standard Cachy
        assert_eq!(calculate_package_rank(&p_cachy, true), 0); // God Tier
        assert_eq!(calculate_package_rank(&p_cachy, false), 1); // Standard Tier

        assert!(calculate_package_rank(&p_cachy, true) < calculate_package_rank(&p_cachy, false));
    }
}

// Diagnostic: Check which repos are actually synced in pacman system
#[tauri::command]
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

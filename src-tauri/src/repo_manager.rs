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
    aur_enabled: bool,
}

pub struct RepoManager {
    // Map RepoName -> List of Packages
    cache: Arc<RwLock<HashMap<String, Vec<Package>>>>,
    repos: Arc<RwLock<Vec<RepoConfig>>>,
    pub aur_enabled: Arc<RwLock<bool>>,
}

impl RepoManager {
    pub fn new() -> Self {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("monarch-store");
        std::fs::create_dir_all(&config_path).unwrap_or_default();
        let config_file = config_path.join("repos.json");

        // Default Repos - Chaotic-AUR is PRIMARY
        let defaults = vec![
            RepoConfig {
                name: "Chaotic-AUR".to_string(),
                url: "https://cdn-mirror.chaotic.cx/chaotic-aur/x86_64/chaotic-aur.db".to_string(),
                source: PackageSource::Chaotic,
                enabled: true,
            },
            RepoConfig {
                name: "Arch Core".to_string(),
                url: "https://geo.mirror.pkgbuild.com/core/os/x86_64/core.db".to_string(),
                source: PackageSource::Official,
                enabled: true,
            },
            RepoConfig {
                name: "Arch Extra".to_string(),
                url: "https://geo.mirror.pkgbuild.com/extra/os/x86_64/extra.db".to_string(),
                source: PackageSource::Official,
                enabled: true,
            },
            RepoConfig {
                name: "Arch Multilib".to_string(),
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
                url: "https://mirror.easyname.at/manjaro/stable/core/x86_64/core.db".to_string(),
                source: PackageSource::Manjaro,
                enabled: false,
            },
            RepoConfig {
                name: "manjaro-extra".to_string(),
                url: "https://mirror.easyname.at/manjaro/stable/extra/x86_64/extra.db".to_string(),
                source: PackageSource::Manjaro,
                enabled: false,
            },
            RepoConfig {
                name: "manjaro-multilib".to_string(),
                url: "https://mirror.easyname.at/manjaro/stable/multilib/x86_64/multilib.db"
                    .to_string(),
                source: PackageSource::Manjaro,
                enabled: false,
            },
        ];

        let mut initial_repos = defaults.clone();
        let mut initial_aur = false;

        // Try Load Config
        if config_file.exists() {
            if let Ok(file) = std::fs::File::open(&config_file) {
                let reader = std::io::BufReader::new(file);
                if let Ok(saved_config) = serde_json::from_reader::<_, StoredConfig>(reader) {
                    initial_aur = saved_config.aur_enabled;

                    // Merge saved states with defaults
                    for repo in &mut initial_repos {
                        if let Some(saved_repo) =
                            saved_config.repos.iter().find(|r| r.name == repo.name)
                        {
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
        }
    }

    async fn save_config_async(&self) {
        let repos = self.repos.read().await.clone();
        let aur = *self.aur_enabled.read().await;

        // Spawn blocking task for file I/O
        tokio::task::spawn_blocking(move || {
            let config = StoredConfig {
                repos,
                aur_enabled: aur,
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

    pub async fn get_all_repos(&self) -> Vec<RepoConfig> {
        self.repos.read().await.clone()
    }

    pub async fn sync_all(&self, force: bool, interval_hours: u64) -> Result<String, String> {
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

            handles.push(tokio::spawn(async move {
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
                            println!("Failed to sync {}: {}", name, e);
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

    pub async fn set_repo_state(&self, name: &str, enabled: bool) {
        let mut repos = self.repos.write().await;
        if let Some(r) = repos.iter_mut().find(|r| r.name == name) {
            r.enabled = enabled;
        }
        drop(repos);
        self.save_config_async().await;
    }

    /// Toggle all repos belonging to a family (e.g., "cachyos", "manjaro")
    /// For CachyOS, intelligently enables only CPU-compatible variants
    pub async fn set_repo_family_state(&self, family: &str, enabled: bool) {
        let mut repos = self.repos.write().await;
        let family_lower = family.to_lowercase();

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
                }
            }
        }

        drop(repos);
        self.save_config_async().await;
    }

    pub async fn search(&self, query: &str) -> Vec<Package> {
        let cache = self.cache.read().await;
        let mut results = Vec::new();
        let q = query.to_lowercase();

        for pkgs in cache.values() {
            for p in pkgs {
                let name_match = p.name.to_lowercase().contains(&q);
                let display_match = p
                    .display_name
                    .as_ref()
                    .map(|dn| dn.to_lowercase().contains(&q))
                    .unwrap_or(false);

                if name_match || display_match {
                    results.push(p.clone());
                }
            }
        }

        // Sort results by Source Priority: Official > Others
        // This ensures that when we iterate later, Official is seen first.
        results.sort_by(|a, b| {
            let priority = |s: &PackageSource| match s {
                PackageSource::Official => 0,
                PackageSource::Chaotic => 1,
                PackageSource::CachyOS => 2,
                PackageSource::Manjaro => 3, // Prioritize Manjaro over Garuda/Endeavour if present for balance
                PackageSource::Garuda => 4,
                PackageSource::Endeavour => 5,
                PackageSource::Aur => 6,
            };
            priority(&a.source).cmp(&priority(&b.source))
        });

        results
    }

    pub async fn get_package(&self, name: &str) -> Option<Package> {
        let cache = self.cache.read().await;
        for pkgs in cache.values() {
            if let Some(p) = pkgs.iter().find(|p| p.name == name) {
                return Some(p.clone());
            }
        }
        None
    }

    pub async fn get_all_packages(&self, name: &str) -> Vec<Package> {
        let cache = self.cache.read().await;
        let mut results = Vec::new();
        for pkgs in cache.values() {
            if let Some(p) = pkgs.iter().find(|p| p.name == name) {
                results.push(p.clone());
            }
        }
        results
    }

    pub async fn get_packages_providing(&self, name: &str) -> Vec<Package> {
        let mut results = Vec::new();
        let cache = self.cache.read().await;

        for repo_pkgs in cache.values() {
            for pkg in repo_pkgs {
                // Check if package provides the target name
                if let Some(provides) = &pkg.provides {
                    if provides.iter().any(|p| p == name) {
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

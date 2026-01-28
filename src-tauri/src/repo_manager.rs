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
}

#[derive(Clone)]
pub struct RepoManager {
    // Map RepoName -> List of Packages
    cache: Arc<RwLock<HashMap<String, Vec<Package>>>>,
    repos: Arc<RwLock<Vec<RepoConfig>>>,
    pub aur_enabled: Arc<RwLock<bool>>,
    pub one_click_enabled: Arc<RwLock<bool>>,
    pub advanced_mode: Arc<RwLock<bool>>,
    pub telemetry_enabled: Arc<RwLock<bool>>,
}

// Helper for Intelligent Priority Sorting (Granular Optimization Ranking)
pub fn calculate_package_rank(pkg: &Package, opt_level: u8, distro: &crate::distro_context::DistroContext) -> u8 {
    // Manjaro Strategy: Stability First (Official Repos Priority 0)
    // We treat "source_first" as "Official/Stable First" here
    if distro.capabilities.default_search_sort == "source_first" {
         match pkg.source {
            PackageSource::Official | PackageSource::Manjaro => 0, // Highest Priority
            PackageSource::Aur => 2,
            PackageSource::Chaotic | PackageSource::CachyOS | PackageSource::Garuda | PackageSource::Endeavour => {
                 // Deprioritize unofficial binaries massively to warn user
                10 
            }
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

        // Default Repos - Chaotic-AUR is PRIMARY
        let mut initial_repos = vec![
            RepoConfig {
                name: "chaotic-aur".to_string(),
                url: "https://cdn-mirror.chaotic.cx/chaotic-aur/x86_64/chaotic-aur.db".to_string(),
                source: PackageSource::Chaotic,
                enabled: false, // Default to false, check disk
            },
            RepoConfig {
                name: "cachyos".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64/cachyos/cachyos.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: false, 
            },
            RepoConfig {
                name: "cachyos-v3".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v3/cachyos-v3/cachyos-v3.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: false,
            },
            RepoConfig {
                name: "cachyos-core-v3".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v3/cachyos-core-v3/cachyos-core-v3.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: false,
            },
            RepoConfig {
                name: "cachyos-extra-v3".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v3/cachyos-extra-v3/cachyos-extra-v3.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: false,
            },
            RepoConfig {
                name: "cachyos-v4".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v4/cachyos-v4/cachyos-v4.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: false,
            },
            RepoConfig {
                name: "cachyos-core-v4".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v4/cachyos-core-v4/cachyos-core-v4.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: false,
            },
            RepoConfig {
                name: "cachyos-extra-v4".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v4/cachyos-extra-v4/cachyos-extra-v4.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: false,
            },
             RepoConfig {
                name: "cachyos-extra-znver4".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v4/cachyos-extra-znver4/cachyos-extra-znver4.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: false,
            },
            RepoConfig {
                name: "cachyos-core-znver4".to_string(),
                url: "https://cdn77.cachyos.org/repo/x86_64_v4/cachyos-core-znver4/cachyos-core-znver4.db".to_string(),
                source: PackageSource::CachyOS,
                enabled: false,
            },
            RepoConfig {
                name: "garuda".to_string(),
                url: "https://builds.garudalinux.org/repos/garuda/x86_64/garuda.db".to_string(),
                source: PackageSource::Garuda,
                enabled: false,
            },
            RepoConfig {
                name: "endeavouros".to_string(),
                url: "https://mirror.moson.org/endeavouros/repo/endeavouros/x86_64/endeavouros.db".to_string(),
                source: PackageSource::Endeavour,
                enabled: false,
            },
            RepoConfig {
                name: "manjaro-core".to_string(),
                url: "https://mirror.init7.net/manjaro/stable/core/x86_64/core.db".to_string(),
                source: PackageSource::Manjaro,
                enabled: false,
            },
            RepoConfig {
                name: "manjaro-extra".to_string(),
                url: "https://mirror.init7.net/manjaro/stable/extra/x86_64/extra.db".to_string(),
                source: PackageSource::Manjaro,
                enabled: false,
            },
            // Official Repos are Always True (Logic handled in filtering usually, but let's keep them here)
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
        ];

        // TRUTH FROM DISK (Modular Config Strategy)
        // Check /etc/pacman.d/monarch/50-{name}.conf
        let monarch_conf_dir = std::path::Path::new("/etc/pacman.d/monarch");
        
        for repo in &mut initial_repos {
            if repo.source == PackageSource::Official {
                continue; // Always enabled
            }

            let conf_name = format!("50-{}.conf", repo.name);
            let path = monarch_conf_dir.join(conf_name);
            if path.exists() {
                // If the file exists, the repo is enabled in the system.
                // We trust the disk over everything else.
                repo.enabled = true;
            } else {
                repo.enabled = false;
            }
        }

        // We load repos.json ONLY for AUR/One-Click preferences, NOT for repo state
        let mut initial_aur = false;
        let mut initial_one_click = false;
        let mut initial_advanced = false;
        let mut initial_telemetry = false; // Default to FALSE (Strict Opt-In) usually, but user requested consistent experience. 
        // Plan said: "Default: false (Strict Opt-In) OR true (Opt-Out) — Decision: Set to true but force the User to see the Onboarding Modal where they can uncheck it."
        // We will default to false here for safety, onboarding modal is responsible for setting it to true if user consents.
        
        let config_file = config_path.join("repos.json");
        
        if config_file.exists() {
             if let Ok(file) = std::fs::File::open(&config_file) {
                let reader = std::io::BufReader::new(file);
                if let Ok(saved_config) = serde_json::from_reader::<_, StoredConfig>(reader) {
                    initial_aur = saved_config.aur_enabled;
                    initial_one_click = saved_config.one_click_enabled;
                    initial_advanced = saved_config.advanced_mode;
                    initial_telemetry = saved_config.telemetry_enabled;
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
        }
    }

    async fn save_config_async(&self) {
        let repos = self.repos.read().await.clone();
        let aur = *self.aur_enabled.read().await;
        let one_click = *self.one_click_enabled.read().await;
        let advanced = *self.advanced_mode.read().await;
        let telemetry = *self.telemetry_enabled.read().await;

        tokio::task::spawn_blocking(move || {
            let config = StoredConfig {
                repos,
                aur_enabled: aur,
                one_click_enabled: one_click,
                advanced_mode: advanced,
                telemetry_enabled: telemetry,
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

    pub async fn set_advanced_mode(&self, enabled: bool) {
        let mut w = self.advanced_mode.write().await;
        *w = enabled;
        drop(w);
        self.save_config_async().await;
    }

    pub async fn is_advanced_mode(&self) -> bool {
        *self.advanced_mode.read().await
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

        println!("Loading initial package cache from disk...");
        let mut handles = Vec::new();

        for repo in active_repos {
            // Simplified loading logic...
            let r = repo.clone();
            let c_dir = cache_dir.clone();
             handles.push(tokio::spawn(async move {
                let file_name = format!("{}.db", r.name);
                let path = c_dir.join(file_name);
                if !path.exists() { return None; }
                 match std::fs::read(&path) {
                    Ok(_) => {
                        let client = crate::repo_db::RealRepoClient::new();
                        match crate::repo_db::fetch_repo_packages(
                            &client, &r.url, &r.name, r.source, &c_dir, false, 999999,
                        ).await {
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

    pub async fn sync_all(&self, force: bool, interval_hours: u64, app: Option<tauri::AppHandle>) -> Result<String, String> {
         use tauri::Emitter;
        let repos = self.repos.read().await;
        let active_repos: Vec<RepoConfig> = repos.iter().filter(|r| r.enabled).cloned().collect();
        drop(repos);

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
                match repo_db::fetch_repo_packages(&client, &r.url, &r.name, r.source, &c_dir, force, interval_hours).await {
                    Ok(pkgs) => Ok((r.name, pkgs)),
                    Err(e) => Err((r.name, e)),
                }
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
             match handle.await {
                Ok(Ok((name, pkgs))) => {
                    let mut cache = self.cache.write().await;
                    let val = if pkgs.len() > 0 { pkgs } else { Vec::new() };
                    cache.insert(name.clone(), val);
                     results.push(format!("Synced {} from {}", 0, name)); // Simplified logging
                }
                _ => {}
            }
        }
        Ok("Sync Complete".to_string())
    }

    // MODULAR APPLY LOGIC
    pub async fn apply_os_config(&self, password: Option<String>) -> Result<(), String> {
        let repos = self.repos.read().await;
        let all_repos: Vec<RepoConfig> = repos.iter().cloned().collect();
        drop(repos);

        let mut script = String::from("echo '--- MonARCH Modular Sync ---'\n");
        script.push_str("mkdir -p /etc/pacman.d/monarch\n");

        // 1. Ensure Include exists in pacman.conf (high priority)
        script.push_str(r#"
if ! grep -q "/etc/pacman.d/monarch/\*.conf" /etc/pacman.conf; then
    # Insert before [core] for high priority
    sed -i '/\[core\]/i # MonARCH Managed Repositories\nInclude = /etc/pacman.d/monarch/*.conf\n' /etc/pacman.conf
fi
# Best effort cleanup of legacy direct entries
sed -i '/\[chaotic-aur\]/,/^\s*$/{d}' /etc/pacman.conf
sed -i '/\[cachyos\]/,/^\s*$/{d}' /etc/pacman.conf
# Aggressive cleanup of orphaned Includes (Fixes "Server not recognized in options")
sed -i '/Include.*cachyos-.*mirrorlist/d' /etc/pacman.conf
sed -i '/\[garuda\]/,/^\s*$/{d}' /etc/pacman.conf
sed -i '/\[endeavouros\]/,/^\s*$/{d}' /etc/pacman.conf
sed -i '/\[manjaro/D' /etc/pacman.conf
"#);

        // 2. Manage Individual Files
        for repo in all_repos {
            if repo.source == PackageSource::Official { continue; }
            
            let filename = format!("50-{}.conf", repo.name);
            let path = format!("/etc/pacman.d/monarch/{}", filename);

            if repo.enabled {
                // Generate Content
                let mut content = String::new();
                
                 // Local Mirror Logic (Same as before)
                let url_filename = repo.url.split('/').last().unwrap_or("");
                let expected_filename = format!("{}.db", repo.name);

                 if repo.url.ends_with(".db") && url_filename != expected_filename {
                     let upstream_server_url = if repo.url.ends_with(".db") {
                        let parts: Vec<&str> = repo.url.split('/').collect();
                        if parts.len() > 1 { parts[..parts.len() - 1].join("/") } else { repo.url.clone() }
                    } else { repo.url.clone() };
                    
                    content.push_str(&format!("[{}]\nServer = file:///var/lib/monarch/dbs\nServer = {}\n", repo.name, upstream_server_url));
                    
                    // Add stash command to script
                    script.push_str(&format!("mkdir -p /var/lib/monarch/dbs\n"));
                    script.push_str(&format!("curl -f -L -s -o /var/lib/monarch/dbs/{}.db '{}' || true\n", repo.name, repo.url));

                 } else {
                     let server_url = if repo.url.ends_with(".db") {
                        let parts: Vec<&str> = repo.url.split('/').collect();
                        if parts.len() > 1 { parts[..parts.len() - 1].join("/") } else { repo.url.clone() }
                    } else { repo.url.clone() };
                     content.push_str(&format!("[{}]\nServer = {}\n", repo.name, server_url));
                 }

                 if repo.name == "chaotic-aur" || repo.name.starts_with("cachyos") {
                    content.push_str("SigLevel = PackageRequired\n");
                } else {
                    content.push_str("SigLevel = Optional TrustAll\n");
                }

                // Write File
                script.push_str(&format!("cat <<EOF > {}\n{}\nEOF\n", path, content));

            } else {
                // Remove File
                script.push_str(&format!("rm -f {}\n", path));
            }
        }

        // 3. Sync
        script.push_str("pacman -Sy --noconfirm\n");

        crate::utils::run_privileged_script(&script, password, false).await.map(|_| ())
    }

    pub async fn set_repo_state(&self, name: &str, enabled: bool) -> Result<(), String> {
        // --- FIREWALL: Identity Matrix Check ---
        let distro = crate::distro_context::get_distro_context();
        
        // Rule 1: Manjaro cannot enable Chaotic-AUR (Glibc Mismatch)
        if enabled && name == "chaotic-aur" {
            // Bypass check if in Advanced Mode
            if !*self.advanced_mode.read().await {
                if let crate::distro_context::ChaoticSupport::Blocked = distro.capabilities.chaotic_aur_support {
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
        // Default behavior (no skip args in struct so we assume standard apply)
        // If we want to skip, we need arguments. But this is internal API.
        // We will call apply explicitly from commands if needed.
        let _ = self.apply_os_config(None).await;
        
        Ok(())
    }

    /// Toggle all repos belonging to a family (e.g., "cachyos", "manjaro")
    /// Added skip_os_sync to avoid 4x prompts during onboarding
    pub async fn set_repo_family_state(&self, family: &str, enabled: bool, skip_os_sync: bool) -> Result<(), String> {
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

        Ok(())
    }

    /// Targeted Install Helper: Safely ensure a specific repo database exists
    /// Handles Local Mirror strategy for mismatched repos (Manjaro) automatically.
    #[allow(dead_code)]
    pub async fn ensure_repo_sync(
        &self,
        repo_name: &str,
        password: Option<String>,
    ) -> Result<(), String> {
        // Self-Healing: Verify modular directory exists, otherwise everything fails.
        let config_dir = std::path::Path::new("/etc/pacman.d/monarch");
        if !config_dir.exists() {
            eprintln!("WARN: MonARCH Repo Infrastructure missing during ensure_repo_sync. Regenerating...");
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

// ... inside RepoManager impl ...

    pub async fn search(&self, query: &str) -> Vec<Package> {
        let cache = self.cache.read().await;
        // Store (Package, opt_level) tuples
        let mut results: Vec<(Package, u8)> = Vec::new();
        let q = query.to_lowercase();

        let cpu_v3 = crate::utils::is_cpu_v3_compatible();
        let cpu_v4 = crate::utils::is_cpu_v4_compatible();
        let distro = crate::distro_context::get_distro_context();

        for (repo_name, pkgs) in cache.iter() {
            // Determine optimization level for this repo
            let opt_level: u8 = if repo_name.contains("-znver4") && crate::utils::is_cpu_znver4_compatible() {
                3
            } else if repo_name.contains("-v4") && cpu_v4 {
                2
            } else if (repo_name.contains("-v3") || repo_name.contains("-core-v3") || repo_name.contains("-extra-v3")) && cpu_v3 {
                1
            } else {
                0
            };

            for p in pkgs {
                let name_match = p.name.to_lowercase().contains(&q);
                let display_match = p
                    .display_name
                    .as_ref()
                    .map(|dn| dn.to_lowercase().contains(&q))
                    .unwrap_or(false);

                if name_match || display_match {
                    results.push((p.clone(), opt_level));
                }
            }
        }

        // Sort results by Intelligent Power Standard + Distro Context
        results.sort_by(|(pkg_a, level_a), (pkg_b, level_b)| {
            let rank_a = calculate_package_rank(pkg_a, *level_a, &distro);
            let rank_b = calculate_package_rank(pkg_b, *level_b, &distro);

            if rank_a != rank_b {
                return rank_a.cmp(&rank_b);
            }

            // Tie-breaker: Name length (shorter is usually more relevant)
            pkg_a.name.len().cmp(&pkg_b.name.len())
        });

        // Unpack and Populate
        results
            .into_iter()
            .map(|(mut p, level)| {
                p.is_optimized = Some(level > 0);
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
        // (Package, opt_level, repo_name)
        let mut results: Vec<(Package, u8, String)> = Vec::new();

        let cpu_v3 = crate::utils::is_cpu_v3_compatible();
        let cpu_v4 = crate::utils::is_cpu_v4_compatible();
        let distro = crate::distro_context::get_distro_context();

        for (repo_name, pkgs) in cache.iter() {
            let opt_level: u8 = if repo_name.contains("-znver4") && crate::utils::is_cpu_znver4_compatible() {
                3
            } else if repo_name.contains("-v4") && cpu_v4 {
                2
            } else if (repo_name.contains("-v3") || repo_name.contains("-core-v3") || repo_name.contains("-extra-v3")) && cpu_v3 {
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
        let distro = crate::distro_context::DistroContext::new(); // Default Arch

        // Rank Check directly (Standard Priorities: Chaotic=4, Official=5, Aur=8)
        assert_eq!(calculate_package_rank(&p_chaotic, 0, &distro), 4);
        assert_eq!(calculate_package_rank(&p_official, 0, &distro), 5);
        assert_eq!(calculate_package_rank(&p_aur, 0, &distro), 8);

        // Verify Chaotic beats Official (Lower rank is better)
        assert!(
            calculate_package_rank(&p_chaotic, 0, &distro) < calculate_package_rank(&p_official, 0, &distro)
        );
    }

    #[test]
    fn test_optimized_priority() {
        let p_cachy = make_test_pkg(PackageSource::CachyOS);
        let distro = crate::distro_context::DistroContext::new(); // Default Arch

        // Optimized tiers vs Standard Cachy (Cachy standard is priority 3+3=6)
        assert_eq!(calculate_package_rank(&p_cachy, 3, &distro), 0); // znver4
        assert_eq!(calculate_package_rank(&p_cachy, 2, &distro), 1); // v4
        assert_eq!(calculate_package_rank(&p_cachy, 1, &distro), 2); // v3
        assert_eq!(calculate_package_rank(&p_cachy, 0, &distro), 6); // Standard Cachy

        assert!(calculate_package_rank(&p_cachy, 1, &distro) < calculate_package_rank(&p_cachy, 0, &distro));
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

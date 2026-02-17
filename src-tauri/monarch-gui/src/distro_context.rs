//! Distro detection and capabilities. The app is distro-aware: repos are discovered from
//! system pacman.conf (we do not inject). Manjaro, Garuda (uses Chaotic-AUR), CachyOS, etc.
//! are detected via /etc/os-release; repo list comes from ALPM + pacman.conf.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DistroId {
    Arch,
    Manjaro,
    #[serde(rename = "endeavouros")]
    EndeavourOS,
    Garuda,
    #[serde(rename = "cachyos")]
    CachyOS,
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RepoManagementMode {
    Unlocked, // User can do anything (Arch)
    Locked,   // User cannot change base repos (Manjaro)
    Managed,  // Pre-configured but flexible (Cachy/Garuda)
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ChaoticSupport {
    Allowed, // Can be enabled (Arch/Endeavour)
    Blocked, // DANGER: Glibc mismatch (Manjaro)
    Native,  // Pre-installed: Garuda and CachyOS ship Chaotic-AUR in default pacman.conf
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct DistroCapabilities {
    pub repo_management: RepoManagementMode,
    pub chaotic_aur_support: ChaoticSupport,
    pub default_search_sort: String, // "binary_first" | "source_first"
    pub description: String,
    pub icon_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct DistroContext {
    pub id: DistroId,
    pub pretty_name: String,
    pub capabilities: DistroCapabilities,
    pub cpu_tier: String, // "v1" | "v2" | "v3" | "v4"
    pub active_repos: Vec<String>,
}

impl DistroContext {
    /// True if the host may enable Chaotic-AUR (Arch, CachyOS, Garuda, EndeavourOS). Blocked on Manjaro.
    pub fn is_chaotic_compatible(&self) -> bool {
        !matches!(
            self.capabilities.chaotic_aur_support,
            ChaoticSupport::Blocked
        )
    }

    /// Returns distro ID as &str for label resolution (Grand Unification).
    pub fn id_str(&self) -> &str {
        match &self.id {
            DistroId::Manjaro => "manjaro",
            DistroId::Garuda => "garuda",
            DistroId::CachyOS => "cachyos",
            DistroId::EndeavourOS => "endeavouros",
            DistroId::Arch => "arch",
            DistroId::Unknown(s) => s.as_str(),
        }
    }

    pub fn new() -> Self {
        let (id, name) = detect_os_release();
        let capabilities = match id {
            DistroId::Manjaro => DistroCapabilities {
                repo_management: RepoManagementMode::Locked,
                chaotic_aur_support: ChaoticSupport::Blocked,
                default_search_sort: "source_first".to_string(), // Manjaro users should prefer AUR builds or Flatpaks
                description: "Manjaro Stability Guard Active.".to_string(),
                icon_key: "shield".to_string(),
            },
            // Garuda ships Chaotic-AUR in default pacman.conf; we discover repos from system (no injection).
            DistroId::Garuda => DistroCapabilities {
                repo_management: RepoManagementMode::Managed,
                chaotic_aur_support: ChaoticSupport::Native, // Garuda uses Chaotic-AUR repos
                default_search_sort: "binary_first".to_string(),
                description: "Garuda Gaming Edition.".to_string(),
                icon_key: "eagle".to_string(),
            },
            DistroId::CachyOS => DistroCapabilities {
                repo_management: RepoManagementMode::Managed,
                chaotic_aur_support: ChaoticSupport::Native,
                default_search_sort: "binary_first".to_string(), // Optimized binaries priority
                description: "Powered by CachyOS.".to_string(),
                icon_key: "rocket".to_string(),
            },
            DistroId::EndeavourOS => DistroCapabilities {
                repo_management: RepoManagementMode::Unlocked,
                chaotic_aur_support: ChaoticSupport::Allowed,
                default_search_sort: "binary_first".to_string(),
                description: "EndeavourOS Detected.".to_string(),
                icon_key: "ship".to_string(),
            },
            DistroId::Arch => DistroCapabilities {
                repo_management: RepoManagementMode::Unlocked,
                chaotic_aur_support: ChaoticSupport::Allowed,
                default_search_sort: "binary_first".to_string(),
                description: "Standard Arch System.".to_string(),
                icon_key: "arch".to_string(),
            },
            DistroId::Unknown(_) => DistroCapabilities {
                repo_management: RepoManagementMode::Unlocked,
                chaotic_aur_support: ChaoticSupport::Allowed,
                default_search_sort: "binary_first".to_string(),
                description: "Unknown Arch-based Distro.".to_string(),
                icon_key: "arch".to_string(),
            },
        };

        let cpu_tier = detect_cpu_tier();
        let active_repos = discover_active_repos();

        Self {
            id,
            pretty_name: name,
            capabilities,
            cpu_tier,
            active_repos,
        }
    }
}

fn detect_cpu_tier() -> String {
    // Check if x86-64-v3 or v4 is supported (Crucial for CachyOS Optimized tiering)
    if std::arch::is_x86_feature_detected!("avx512f") {
        "v4".to_string()
    } else if std::arch::is_x86_feature_detected!("avx2") {
        "v3".to_string()
    } else if std::arch::is_x86_feature_detected!("sse4.2") {
        "v2".to_string()
    } else {
        "v1".to_string()
    }
}

fn discover_active_repos() -> Vec<String> {
    let mut repos = Vec::new();
    if let Ok(alpm) = alpm::Alpm::new("/", "/var/lib/pacman") {
        crate::alpm_read::register_syncdbs_from_conf(&alpm, "/etc/pacman.conf");
        for db in alpm.syncdbs() {
            repos.push(db.name().to_string());
        }
    }
    // Fallback if ALPM fails (rare on Arch)
    if repos.is_empty() {
        repos.push("core".to_string());
        repos.push("extra".to_string());
    }
    repos
}

fn detect_os_release() -> (DistroId, String) {
    let path = Path::new("/etc/os-release");
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            return (
                DistroId::Unknown("unknown".to_string()),
                "Unknown Linux".to_string(),
            )
        }
    };

    let mut id_val = String::new();
    let mut id_like = String::new();
    let mut name_val = String::new();

    for line in content.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let clean_value = value.trim_matches('"');
            match key {
                "ID" => id_val = clean_value.to_lowercase(),
                "ID_LIKE" => id_like = clean_value.to_lowercase(),
                "PRETTY_NAME" => name_val = clean_value.to_string(),
                _ => {}
            }
        }
    }

    // Explicit IDs first (CachyOS, Garuda, Manjaro, EndeavourOS, Arch).
    // Then "archlinux" (some installs report this). Then ID_LIKE fallback for Arch-based unknowns
    // and for CachyOS/Garuda when ID is "arch" (e.g. ID=arch ID_LIKE="arch cachyos").
    let id_like_words: std::collections::HashSet<&str> = id_like.split_whitespace().collect();
    let distro_id = match id_val.as_str() {
        "manjaro" => DistroId::Manjaro,
        "garuda" => DistroId::Garuda,
        "cachyos" => DistroId::CachyOS,
        "endeavouros" => DistroId::EndeavourOS,
        "arch" | "archlinux" => {
            if id_like_words.contains("cachyos") {
                DistroId::CachyOS
            } else if id_like_words.contains("garuda") {
                DistroId::Garuda
            } else {
                DistroId::Arch
            }
        }
        _ => {
            if id_like_words.contains("cachyos") {
                DistroId::CachyOS
            } else if id_like_words.contains("garuda") {
                DistroId::Garuda
            } else if id_like_words.contains("manjaro") {
                DistroId::Manjaro
            } else if id_like_words.contains("endeavouros") {
                DistroId::EndeavourOS
            } else if id_like_words.contains("arch") {
                DistroId::Arch
            } else {
                DistroId::Unknown(id_val)
            }
        }
    };

    (distro_id, name_val)
}

#[tauri::command]
#[specta::specta]
pub fn get_distro_context() -> DistroContext {
    DistroContext::new()
}

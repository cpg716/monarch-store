use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DistroId {
    Arch,
    Manjaro,
    EndeavourOS,
    Garuda,
    CachyOS,
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoManagementMode {
    Unlocked, // User can do anything (Arch)
    Locked,   // User cannot change base repos (Manjaro)
    Managed,  // Pre-configured but flexible (Cachy/Garuda)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChaoticSupport {
    Allowed, // Can be enabled (Arch/Endeavour)
    Blocked, // DANGER: Glibc mismatch (Manjaro)
    Native,  // Pre-installed (Garuda/Cachy)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistroCapabilities {
    pub repo_management: RepoManagementMode,
    pub chaotic_aur_support: ChaoticSupport,
    pub default_search_sort: String, // "binary_first" | "source_first"
    pub description: String,
    pub icon_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistroContext {
    pub id: DistroId,
    pub pretty_name: String,
    pub capabilities: DistroCapabilities,
}

impl DistroContext {
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
            DistroId::Garuda => DistroCapabilities {
                repo_management: RepoManagementMode::Managed,
                chaotic_aur_support: ChaoticSupport::Native,
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

        Self {
            id,
            pretty_name: name,
            capabilities,
        }
    }
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
    let mut name_val = String::new();

    for line in content.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let clean_value = value.trim_matches('"');
            match key {
                "ID" => id_val = clean_value.to_lowercase(),
                "PRETTY_NAME" => name_val = clean_value.to_string(),
                _ => {}
            }
        }
    }

    let distro_id = match id_val.as_str() {
        "manjaro" => DistroId::Manjaro,
        "garuda" => DistroId::Garuda,
        "cachyos" => DistroId::CachyOS,
        "endeavouros" => DistroId::EndeavourOS,
        "arch" => DistroId::Arch,
        _ => DistroId::Unknown(id_val),
    };

    (distro_id, name_val)
}

#[tauri::command]
pub fn get_distro_context() -> DistroContext {
    DistroContext::new()
}

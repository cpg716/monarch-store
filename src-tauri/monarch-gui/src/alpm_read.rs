use crate::models::{Package, PackageSource};
use alpm::{Alpm, PackageReason, SigLevel};
use std::path::Path;
use std::process::Command;

/// Collect all repository section names from pacman.conf and any Include'd files
/// (e.g. /etc/pacman.d/monarch/*.conf) so core, extra, community, multilib are
/// registered when using modular Include.
fn collect_repo_sections_from_conf(conf_path: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let content = match std::fs::read_to_string(conf_path) {
        Ok(c) => c,
        Err(_) => return sections,
    };
    let conf_dir = Path::new(conf_path)
        .parent()
        .unwrap_or_else(|| Path::new("/etc"));
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let section = line[1..line.len() - 1].trim();
            if section != "options" && !sections.contains(&section.to_string()) {
                sections.push(section.to_string());
            }
            continue;
        }
        if line.to_lowercase().starts_with("include") {
            let rest = line[6..]
                .trim_start_matches(|c: char| c == '=' || c == ' ')
                .trim();
            let path = rest.trim_matches(|c| c == '"' || c == '\'');
            let full = if path.starts_with('/') {
                path.to_string()
            } else {
                conf_dir.join(path).to_string_lossy().into_owned()
            };
            for included_path in glob_includes(&full) {
                for s in collect_repo_sections_from_conf(&included_path) {
                    if !sections.contains(&s) {
                        sections.push(s);
                    }
                }
            }
        }
    }
    sections
}

fn glob_includes(pattern: &str) -> Vec<String> {
    let path = Path::new(pattern);
    if !pattern.contains('*') {
        return if path.exists() && path.is_file() {
            vec![pattern.to_string()]
        } else {
            Vec::new()
        };
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("/"));
    let file_pattern = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let suffix = file_pattern
        .find('*')
        .map(|i| &file_pattern[i + 1..])
        .unwrap_or("");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if (suffix.is_empty() || name.ends_with(suffix)) && p.is_file() {
                if let Some(s) = p.to_str() {
                    out.push(s.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

/// Register all repo sections from system pacman.conf (and Include'd files).
/// Call this before iterating syncdbs() so Manjaro, Garuda, Chaotic-AUR, CachyOS, etc. are discovered.
pub fn register_syncdbs_from_conf(alpm: &Alpm, conf_path: &str) {
    let sections = collect_repo_sections_from_conf(conf_path);
    if sections.is_empty() {
        let _ = alpm.register_syncdb("core", SigLevel::PACKAGE_OPTIONAL);
        let _ = alpm.register_syncdb("extra", SigLevel::PACKAGE_OPTIONAL);
        let _ = alpm.register_syncdb("community", SigLevel::PACKAGE_OPTIONAL);
        let _ = alpm.register_syncdb("multilib", SigLevel::PACKAGE_OPTIONAL);
        return;
    }
    for section in sections {
        let _ = alpm.register_syncdb(section.as_str(), SigLevel::PACKAGE_OPTIONAL);
    }
}

/// Returns true if [chaotic-aur] is present in the loaded ALPM sync DBs (from pacman.conf).
/// Used by check_chaotic_status to define "enabled" as ALPM-visible, not just file presence.
pub fn chaotic_aur_in_syncdbs(conf_path: &str) -> bool {
    let alpm = match Alpm::new("/", "/var/lib/pacman") {
        Ok(a) => a,
        Err(_) => return false,
    };
    register_syncdbs_from_conf(&alpm, conf_path);
    alpm.syncdbs()
        .into_iter()
        .any(|d| d.name() == "chaotic-aur")
}

pub fn vercmp_greater(v1: &str, v2: &str) -> bool {
    alpm::vercmp(v1, v2) == std::cmp::Ordering::Greater
}

pub fn get_package_native(name: &str) -> Option<Package> {
    let alpm = Alpm::new("/", "/var/lib/pacman").ok()?;

    register_syncdbs_from_conf(&alpm, "/etc/pacman.conf");
    let installed_version = alpm
        .localdb()
        .pkg(name)
        .ok()
        .map(|p| p.version().to_string());
    let distro = crate::distro_context::DistroContext::new();

    // Prefer the syncdb whose package version matches the installed version (likely install origin)
    let mut first: Option<(String, String, String, u64, u64)> = None;
    for db in alpm.syncdbs() {
        if let Ok(pkg) = db.pkg(name) {
            let version_matches = installed_version
                .as_ref()
                .map(|v| v == pkg.version().as_str())
                .unwrap_or(false);
            if version_matches {
                let installed = alpm.localdb().pkg(name).is_ok();
                return Some(Package {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    description: pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
                    source: PackageSource::from_repo_name(
                        db.name(),
                        pkg.version().as_str(),
                        &distro,
                        &pkg.name(),
                    ),
                    installed,
                    download_size: Some(pkg.download_size() as u64),
                    installed_size: Some(pkg.isize() as u64),
                    depends: Some(pkg.depends().iter().map(|d| d.to_string()).collect()),
                    make_depends: Some(pkg.makedepends().iter().map(|d| d.to_string()).collect()),
                    ..Default::default()
                });
            }
            if first.is_none() {
                first = Some((
                    db.name().to_string(),
                    pkg.version().to_string(),
                    pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
                    pkg.download_size() as u64,
                    pkg.isize() as u64,
                ));
            }
        }
    }
    if let Some((db_name, version, description, download_size, isize)) = first {
        let installed = alpm.localdb().pkg(name).is_ok();
        return Some(Package {
            name: name.to_string(),
            version: version.clone(),
            description,
            source: PackageSource::from_repo_name(&db_name, &version, &distro, name),
            installed,
            download_size: Some(download_size),
            installed_size: Some(isize),
            depends: alpm.syncdbs().iter().find(|db| db.name() == db_name)
                .and_then(|db| db.pkg(name).ok())
                .map(|p| p.depends().iter().map(|d| d.to_string()).collect()),
            ..Default::default()
        });
    }

    // Installed but not in any sync DB (e.g. AUR-only): return localdb package; assume AUR
    if let Ok(pkg) = alpm.localdb().pkg(name) {
        return Some(Package {
            name: pkg.name().to_string(),
            version: pkg.version().to_string(),
            description: pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
            source: PackageSource::new("local", "local", pkg.version().as_str(), "Local"),
            installed: true,
            installed_size: Some(pkg.isize() as u64),
            ..Default::default()
        });
    }

    None
}

pub fn get_installed_packages_native() -> Vec<Package> {
    let alpm = match Alpm::new("/", "/var/lib/pacman") {
        Ok(a) => a,
        Err(e) => {
            log::warn!("Alpm::new failed in get_installed_packages_native: {}", e);
            return Vec::new(); // Caller handles empty by trying search_installed_packages_cli
        }
    };

    alpm.localdb()
        .pkgs()
        .iter()
        .map(|pkg| Package {
            name: pkg.name().to_string(),
            version: pkg.version().to_string(),
            description: pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
            installed: true,
            installed_size: Some(pkg.isize() as u64),
            ..Default::default()
        })
        .collect()
}

/// Batch lookup from ALPM sync DBs (and localdb for installed). Single source of truth for READ;
/// same data install uses, so packages we show are always findable. Only returns packages from
/// repos in `enabled_repos` (empty = no filter, use all registered syncdbs).
pub fn get_packages_batch(names: &[String], enabled_repos: &[String]) -> Vec<Package> {
    if names.is_empty() {
        return Vec::new();
    }
    let alpm = match Alpm::new("/", "/var/lib/pacman") {
        Ok(a) => a,
        Err(e) => {
            log::error!("[ALPM] Alpm::new failed: {}", e);
            return Vec::new();
        }
    };

    register_syncdbs_from_conf(&alpm, "/etc/pacman.conf");

    let names_set: std::collections::HashSet<&str> = names.iter().map(|s| s.as_str()).collect();
    let mut results = Vec::new();

    let dbs: Vec<String> = alpm.syncdbs().iter().map(|d| d.name().to_string()).collect();
    if dbs.is_empty() {
        log::warn!("[ALPM] No Sync DBs registered! Check pacman.conf or permissions.");
        // If we can't find any DBs, attempting to register them explicitly again might help debugging
        // but for now just warn.
    }
    
    // Debug log for Essentials debugging
    if names.contains(&"firefox".to_string()) {
       log::debug!(
           "[ALPM] get_packages_batch searching for 'firefox'. Registered DBs: {:?}. Enabled filter: {:?}", 
           dbs, 
           enabled_repos
       );
    }

    for db in alpm.syncdbs() {
        let db_name = db.name();
        if !enabled_repos.is_empty() && !enabled_repos.iter().any(|r| r == db_name) {
            continue;
        }
        for pkg in db.pkgs() {
            if names_set.contains(pkg.name()) {
                // log::debug!("[ALPM] Found {} in {}", pkg.name(), db_name);
                let is_installed = alpm.localdb().pkg(pkg.name()).is_ok();
                results.push(Package {
                    name: pkg.name().to_string(),
                    display_name: Some(crate::utils::to_pretty_name(pkg.name())),
                    description: pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
                    version: pkg.version().to_string(),
                    source: PackageSource::from_repo_name(
                        db_name,
                        pkg.version().as_str(),
                        &crate::distro_context::DistroContext::new(),
                        pkg.name(),
                    ),
                    installed: is_installed,
                    download_size: Some(pkg.download_size() as u64),
                    installed_size: Some(pkg.isize() as u64),
                    depends: Some(pkg.depends().iter().map(|d| d.to_string()).collect()),
                    make_depends: Some(pkg.makedepends().iter().map(|d| d.to_string()).collect()),
                    last_modified: None,
                    ..Default::default()
                });
            }
        }
    }

    for pkg in alpm.localdb().pkgs() {
        if names_set.contains(pkg.name()) && !results.iter().any(|r| r.name == pkg.name()) {
            results.push(Package {
                name: pkg.name().to_string(),
                display_name: Some(crate::utils::to_pretty_name(pkg.name())),
                description: pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
                version: pkg.version().to_string(),
                source: PackageSource::new("local", "local", pkg.version().as_str(), "Local"),
                installed: true,
                installed_size: Some(pkg.isize() as u64),
                ..Default::default()
            });
        }
    }

    results
}

/// Returns true if a package of the given name is installed (localdb).
/// Replaces read-only `pacman -Q <name>` checks.
pub fn is_package_installed(name: &str) -> bool {
    // 1. Try ALPM Binding
    if let Ok(alpm) = Alpm::new("/", "/var/lib/pacman") {
        if alpm.localdb().pkg(name).is_ok() {
            return true;
        }
    }
    // 2. Fallback to pacman -Q (CLI) if binding fails or returns false (double check)
    match Command::new("pacman").arg("-Q").arg(name).output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Returns true if the package exists in any sync database (official or enabled repos).
/// Replaces read-only `pacman -Si <name>` for "in repo" checks.
pub fn is_package_in_syncdb(name: &str) -> bool {
    let alpm = match Alpm::new("/", "/var/lib/pacman") {
        Ok(a) => a,
        Err(_) => return false,
    };
    register_syncdbs_from_conf(&alpm, "/etc/pacman.conf");
    for db in alpm.syncdbs() {
        if db.pkg(name).is_ok() {
            return true;
        }
    }
    false
}

/// Returns true if the dependency `name` is satisfied: installed or provided by some installed package.
/// Replaces read-only `pacman -T <name>` for dependency checks.
pub fn is_dep_satisfied(name: &str) -> bool {
    let alpm = match Alpm::new("/", "/var/lib/pacman") {
        Ok(a) => a,
        Err(_) => return false,
    };
    if alpm.localdb().pkg(name).is_ok() {
        return true;
    }
    for pkg in alpm.localdb().pkgs() {
        for provide in pkg.provides() {
            let prov_name = provide.name().split('=').next().unwrap_or(provide.name());
            if prov_name == name {
                return true;
            }
        }
    }
    false
}

/// Returns (name, version) of installed packages that are not in any sync DB (foreign/AUR).
/// Replaces read-only `pacman -Qm`.
pub fn get_foreign_installed_packages() -> Vec<(String, String)> {
    let alpm = match Alpm::new("/", "/var/lib/pacman") {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    register_syncdbs_from_conf(&alpm, "/etc/pacman.conf");
    let in_sync = |n: &str| {
        for db in alpm.syncdbs() {
            if db.pkg(n).is_ok() {
                return true;
            }
        }
        false
    };
    alpm.localdb()
        .pkgs()
        .iter()
        .filter(|pkg| !in_sync(pkg.name()))
        .map(|pkg| (pkg.name().to_string(), pkg.version().to_string()))
        .collect()
}

/// Returns names of orphan packages (installed as dependency but no longer required by any package).
/// Replaces read-only `pacman -Qtdq`.
pub fn get_orphans_native() -> Vec<String> {
    let alpm = match Alpm::new("/", "/var/lib/pacman") {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    let mut required = std::collections::HashSet::new();
    for pkg in alpm.localdb().pkgs() {
        for dep in pkg.depends() {
            required.insert(dep.name().to_string());
        }
        for provide in pkg.provides() {
            let name = provide.name().split('=').next().unwrap_or(provide.name());
            required.insert(name.to_string());
        }
    }
    alpm.localdb()
        .pkgs()
        .iter()
        .filter(|pkg| pkg.reason() == PackageReason::Depend && !required.contains(pkg.name()))
        .map(|pkg| pkg.name().to_string())
        .collect()
}

/// Returns a list of packages that have upgrades available in the sync databases.
/// Replicates `pacman -Qu`. Used for the Updates page; intentionally uses full pacman.conf
/// (no filter by app "enabled" state) so that installed packages from any repo, including
/// Chaotic-AUR, always get updates—for updates a repo is never "turned off".
/// Distro-agnostic: we read the system pacman.conf and follow Include directives, so all
/// distro repos (Arch core/extra, Manjaro, Garuda, CachyOS, Chaotic-AUR, EOS, etc.) are included.
pub fn get_host_updates() -> Vec<crate::models::UpdateItem> {
    let alpm = match Alpm::new("/", "/var/lib/pacman") {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    register_syncdbs_from_conf(&alpm, "/etc/pacman.conf");

    let mut updates = Vec::new();
    let localdb = alpm.localdb();

    for db in alpm.syncdbs() {
        let db_name = db.name();
        for pkg in db.pkgs() {
            if let Ok(local_pkg) = localdb.pkg(pkg.name()) {
                if alpm::vercmp(pkg.version().as_str(), local_pkg.version().as_str())
                    == std::cmp::Ordering::Greater
                {
                    updates.push(crate::models::UpdateItem {
                        name: pkg.name().to_string(), // Package Name
                        current_version: local_pkg.version().to_string(),
                        new_version: pkg.version().to_string(),
                        source: PackageSource::from_repo_name(
                            db_name,
                            pkg.version().as_str(),
                            &crate::distro_context::DistroContext::new(),
                            pkg.name(),
                        ),
                        size: Some(pkg.download_size() as u64),
                        icon: None,
                        display_name: None,
                    });
                }
            }
        }
    }
    updates
}

/// Fallback Search via CLI `pacman -Qs`
pub fn search_installed_packages_cli(query: &str) -> Vec<Package> {
    let output = match std::process::Command::new("pacman")
        .arg("-Qs")
        .arg(query)
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut packages = Vec::new();
    let lines: Vec<&str> = stdout.lines().collect();

    let mut current_pkg: Option<Package> = None;

    for line in lines {
        if line.starts_with("local/") {
            // Push previous if complete
            if let Some(pkg) = current_pkg.take() {
                packages.push(pkg);
            }

            // Parse new package line: "local/name version"
            // Example: local/heroic-games-launcher-bin 2.19.0-1
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let full_name = parts[0].trim_start_matches("local/");
                let version = parts[1];

                let mut p = Package::default();
                p.name = full_name.to_string();
                p.version = version.to_string();
                p.installed = true;
                p.source = PackageSource::new("local", "local", version, "Installed (Local)");
                current_pkg = Some(p);
            }
        } else if line.starts_with("    ") {
            // Description line
            if let Some(pkg) = &mut current_pkg {
                pkg.description = line.trim().to_string();
            }
        }
    }

    // Push last one
    if let Some(pkg) = current_pkg {
        packages.push(pkg);
    }

    packages
}

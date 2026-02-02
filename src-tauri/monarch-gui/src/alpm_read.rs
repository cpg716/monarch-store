use crate::models::{Package, PackageSource};
use alpm::{Alpm, PackageReason, SigLevel};
use std::path::Path;

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

fn register_syncdbs_from_conf(alpm: &Alpm, conf_path: &str) {
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


pub fn get_package_native(name: &str) -> Option<Package> {
    let alpm = Alpm::new("/", "/var/lib/pacman").ok()?;

    // Register all repos (including from Include directives) and try sync DBs for real source
    register_syncdbs_from_conf(&alpm, "/etc/pacman.conf");
    for db in alpm.syncdbs() {
        if let Ok(pkg) = db.pkg(name) {
            let installed = alpm.localdb().pkg(name).is_ok();
            return Some(Package {
                name: pkg.name().to_string(),
                version: pkg.version().to_string(),
                description: pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
                source: PackageSource::from_repo_name(
                    db.name(),
                    pkg.version().as_str(),
                    &crate::distro_context::DistroContext::new(),
                ),
                installed,
                download_size: Some(pkg.download_size() as u64),
                installed_size: Some(pkg.isize() as u64),
                ..Default::default()
            });
        }
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
        Err(_) => return Vec::new(),
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
        Err(_) => return Vec::new(),
    };

    register_syncdbs_from_conf(&alpm, "/etc/pacman.conf");

    let names_set: std::collections::HashSet<&str> = names.iter().map(|s| s.as_str()).collect();
    let mut results = Vec::new();

    for db in alpm.syncdbs() {
        let db_name = db.name();
        if !enabled_repos.is_empty() && !enabled_repos.iter().any(|r| r == db_name) {
            continue;
        }
        for pkg in db.pkgs() {
            if names_set.contains(pkg.name()) {
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
                    ),
                    installed: is_installed,
                    download_size: Some(pkg.download_size() as u64),
                    installed_size: Some(pkg.isize() as u64),
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
    let alpm = match Alpm::new("/", "/var/lib/pacman") {
        Ok(a) => a,
        Err(_) => return false,
    };
    alpm.localdb().pkg(name).is_ok()
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
/// Replicates `pacman -Qu`.
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
                        ),
                        size: Some(pkg.download_size() as u64),
                        icon: None,
                    });
                }
            }
        }
    }
    updates
}

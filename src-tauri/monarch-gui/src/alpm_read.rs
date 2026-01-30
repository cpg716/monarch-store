use crate::models::{Package, PackageSource};
use alpm::{Alpm, SigLevel};

pub fn search_local_dbs(query: &str) -> Vec<Package> {
    let alpm = match Alpm::new("/", "/var/lib/pacman") {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };

    // Dynamic Repository Registration (Domain 2)
    let last_check = std::fs::metadata("/var/lib/pacman/db.lck")
        .and_then(|m| m.modified())
        .map(|t| t.elapsed().unwrap_or_default().as_secs())
        .unwrap_or(3601);

    if last_check < 3600 {
        // Passive check using checkupdates if checked < 1 hour ago
        let output = std::process::Command::new("checkupdates").output();
        if let Ok(o) = output {
            if o.status.success() {
                // Return cached version or just continue
            }
        }
    }

    if let Ok(conf) = std::fs::read_to_string("/etc/pacman.conf") {
        for line in conf.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                let section = &line[1..line.len() - 1];
                if section != "options" {
                    let _ = alpm.register_syncdb(section, SigLevel::PACKAGE_OPTIONAL);
                }
            }
        }
    } else {
        // Fallback
        let _ = alpm.register_syncdb("core", SigLevel::PACKAGE_OPTIONAL);
        let _ = alpm.register_syncdb("extra", SigLevel::PACKAGE_OPTIONAL);
        let _ = alpm.register_syncdb("chaotic-aur", SigLevel::PACKAGE_OPTIONAL);
    }

    let mut results = Vec::new();
    let query_lower = query.to_lowercase();

    for db in alpm.syncdbs() {
        for pkg in db.pkgs() {
            if pkg.name().contains(&query_lower)
                || pkg
                    .desc()
                    .map(|d| d.contains(&query_lower))
                    .unwrap_or(false)
            {
                let is_installed = alpm.localdb().pkg(pkg.name()).is_ok();
                results.push(Package {
                    name: pkg.name().to_string(),
                    display_name: Some(crate::utils::to_pretty_name(pkg.name())),
                    description: pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
                    version: pkg.version().to_string(),
                    source: match db.name() {
                        "chaotic-aur" => PackageSource::Chaotic,
                        _ => PackageSource::Official,
                    },
                    installed: is_installed,
                    download_size: Some(pkg.download_size() as u64),
                    installed_size: Some(pkg.isize() as u64),
                    ..Default::default()
                });
            }
        }
    }

    // Also search localdb directly for packages not in syncdbs (e.g. custom AUR builds)
    for pkg in alpm.localdb().pkgs() {
        if !results.iter().any(|r| r.name == pkg.name()) {
            if pkg.name().contains(&query_lower)
                || pkg
                    .desc()
                    .map(|d| d.contains(&query_lower))
                    .unwrap_or(false)
            {
                results.push(Package {
                    name: pkg.name().to_string(),
                    display_name: Some(crate::utils::to_pretty_name(pkg.name())),
                    description: pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
                    version: pkg.version().to_string(),
                    source: PackageSource::Aur, // Default source for local-only if we don't know better
                    installed: true,
                    download_size: Some(pkg.download_size() as u64),
                    installed_size: Some(pkg.isize() as u64),
                    ..Default::default()
                });
            }
        }
    }

    results
}

pub fn get_package_native(name: &str) -> Option<Package> {
    let alpm = Alpm::new("/", "/var/lib/pacman").ok()?;

    // Check local database first
    if let Ok(pkg) = alpm.localdb().pkg(name) {
        return Some(Package {
            name: pkg.name().to_string(),
            version: pkg.version().to_string(),
            description: pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
            installed: true,
            installed_size: Some(pkg.isize() as u64),
            ..Default::default()
        });
    }

    // Check sync databases
    if let Ok(conf) = std::fs::read_to_string("/etc/pacman.conf") {
        for line in conf.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                let section = &line[1..line.len() - 1];
                if section != "options" {
                    if let Ok(db) = alpm.register_syncdb(section, SigLevel::PACKAGE_OPTIONAL) {
                        if let Ok(pkg) = db.pkg(name) {
                            return Some(Package {
                                name: pkg.name().to_string(),
                                version: pkg.version().to_string(),
                                description: pkg.desc().map(|d| d.to_string()).unwrap_or_default(),
                                source: match db.name() {
                                    "chaotic-aur" => PackageSource::Chaotic,
                                    _ => PackageSource::Official,
                                },
                                download_size: Some(pkg.download_size() as u64),
                                installed_size: Some(pkg.isize() as u64),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }
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

use crate::models::{Package, PackageSource};
use crate::registry::RegistryManager;
use alpm::{Alpm, SigLevel};
use appstream::{enums::Icon, Collection, Component};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use roxmltree::Document;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
struct AppMetadata {
    name: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    icon: Option<String>,
    screenshots: Vec<String>,
    app_id: Option<String>,
    maintainer: Option<String>,
    license: Option<Vec<String>>,
    categories: Vec<String>,
}

#[derive(Clone, Debug)]
struct FlatpakAvailability {
    canonical_id: String,
    app_id: String,
    remote: String,
    label: String,
}

pub fn hydrate_registry_from_live_system(registry: &RegistryManager) -> Result<usize, String> {
    let icon_index = build_icon_name_index();
    let metadata = build_metadata_index(&icon_index);
    let flatpak_sources = build_flatpak_source_index();
    let alpm = Alpm::new("/", "/var/lib/pacman").map_err(|e| e.to_string())?;
    register_syncdbs_from_conf(&alpm, "/etc/pacman.conf");

    let installed_names = alpm
        .localdb()
        .pkgs()
        .iter()
        .map(|pkg| pkg.name().to_string())
        .collect::<HashSet<_>>();
    let installed_versions = alpm
        .localdb()
        .pkgs()
        .iter()
        .map(|pkg| (pkg.name().to_string(), pkg.version().to_string()))
        .collect::<HashMap<_, _>>();

    let mut packages = HashMap::<String, Package>::new();

    for db in alpm.syncdbs() {
        let repo_name = db.name().to_string();
        for repo_pkg in db.pkgs() {
            let name = repo_pkg.name().to_string();
            let version = repo_pkg.version().to_string();
            let source = repo_source(&repo_name, &version, &name);
            let meta = find_metadata(&metadata, &name, None);
            let canonical_id =
                canonical_id_for(&name, meta.as_ref().and_then(|m| m.app_id.as_deref()));
            let installed = installed_names.contains(&name);
            let selected = packages
                .get(&canonical_id)
                .map(|existing| {
                    candidate_priority(&source, installed)
                        > candidate_priority(&existing.source, existing.installed)
                })
                .unwrap_or(true);

            let entry = packages
                .entry(canonical_id.clone())
                .or_insert_with(|| Package {
                    canonical_id: canonical_id.clone(),
                    name: name.clone(),
                    display_name: meta
                        .as_ref()
                        .and_then(|m| m.name.clone())
                        .or_else(|| Some(pretty_name(&name))),
                    description: meta
                        .as_ref()
                        .and_then(|m| m.summary.clone())
                        .unwrap_or_else(|| {
                            repo_pkg.desc().map(|d| d.to_string()).unwrap_or_default()
                        }),
                    version: version.clone(),
                    source: source.clone(),
                    maintainer: meta.as_ref().and_then(|m| m.maintainer.clone()),
                    license: meta.as_ref().and_then(|m| m.license.clone()),
                    categories: meta.as_ref().map(|m| m.categories.clone()),
                    icon: meta.as_ref().and_then(|m| m.icon.clone()),
                    screenshots: meta.as_ref().map(|m| m.screenshots.clone()),
                    app_id: meta.as_ref().and_then(|m| m.app_id.clone()),
                    long_description: meta.as_ref().and_then(|m| m.description.clone()),
                    installed,
                    download_size: Some(repo_pkg.download_size() as u64),
                    installed_size: Some(repo_pkg.isize() as u64),
                    download_size_bytes: Some(repo_pkg.download_size() as u64),
                    installed_size_bytes: Some(repo_pkg.isize() as u64),
                    available_sources: Some(vec![source.clone()]),
                    ..Package::default()
                });

            merge_metadata_into_package(entry, meta.as_ref(), &name);
            fill_icon_from_identity(entry, &name, &icon_index);
            fill_icon_from_app_id(entry);

            if entry.categories.as_ref().map_or(true, |c| c.is_empty()) {
                let groups: Vec<String> = repo_pkg.groups().iter().map(|s| s.to_lowercase()).collect();
                entry.categories = Some(fallback_categories_from_pacman_groups(&groups));
            }

            if selected {
                entry.name = name.clone();
                entry.version = if installed {
                    installed_versions
                        .get(&name)
                        .cloned()
                        .unwrap_or_else(|| version.clone())
                } else {
                    version.clone()
                };
                entry.source = source.clone();
                entry.installed = installed;
                if entry.description.trim().is_empty() {
                    entry.description = repo_pkg.desc().map(|d| d.to_string()).unwrap_or_default();
                }
                if entry.display_name.is_none() {
                    entry.display_name = Some(pretty_name(&name));
                }
                entry.download_size = Some(repo_pkg.download_size() as u64);
                entry.installed_size = Some(repo_pkg.isize() as u64);
                entry.download_size_bytes = Some(repo_pkg.download_size() as u64);
                entry.installed_size_bytes = Some(repo_pkg.isize() as u64);
                merge_metadata_into_package(entry, meta.as_ref(), &name);
                fill_icon_from_identity(entry, &name, &icon_index);
                fill_icon_from_app_id(entry);
            }

            let sources = entry.available_sources.get_or_insert_with(Vec::new);
            if !sources.iter().any(|existing| {
                existing.id == source.id && existing.package_name == source.package_name
            }) {
                sources.push(source);
            }
        }
    }

    let in_sync = packages
        .values()
        .map(|package| package.name.clone())
        .collect::<HashSet<_>>();

    for local_pkg in alpm.localdb().pkgs() {
        if in_sync.contains(local_pkg.name()) {
            continue;
        }
        let name = local_pkg.name().to_string();
        let meta = find_metadata(&metadata, &name, None);
        let canonical_id = canonical_id_for(&name, meta.as_ref().and_then(|m| m.app_id.as_deref()));
        let version = local_pkg.version().to_string();
        let source = PackageSource {
            source_type: "aur".to_string(),
            id: "aur".to_string(),
            version: version.clone(),
            label: "AUR (Community)".to_string(),
            package_name: Some(name.clone()),
        };
        let mut package = Package {
            canonical_id,
            name: name.clone(),
            display_name: meta
                .as_ref()
                .and_then(|m| m.name.clone())
                .or_else(|| Some(pretty_name(&name))),
            description: meta
                .as_ref()
                .and_then(|m| m.summary.clone())
                .unwrap_or_else(|| local_pkg.desc().map(|d| d.to_string()).unwrap_or_default()),
            version,
            source: source.clone(),
            maintainer: meta.as_ref().and_then(|m| m.maintainer.clone()),
            license: meta.as_ref().and_then(|m| m.license.clone()),
            categories: meta.as_ref().map(|m| m.categories.clone()),
            icon: meta.as_ref().and_then(|m| m.icon.clone()),
            screenshots: meta.as_ref().map(|m| m.screenshots.clone()),
            app_id: meta.as_ref().and_then(|m| m.app_id.clone()),
            long_description: meta.as_ref().and_then(|m| m.description.clone()),
            installed: true,
            installed_size: Some(local_pkg.isize() as u64),
            installed_size_bytes: Some(local_pkg.isize() as u64),
            available_sources: Some(vec![source]),
            ..Package::default()
        };
        merge_metadata_into_package(&mut package, meta.as_ref(), &name);
        fill_icon_from_identity(&mut package, &name, &icon_index);
        fill_icon_from_app_id(&mut package);
        if package.categories.as_ref().map_or(true, |c| c.is_empty()) {
            let groups: Vec<String> = local_pkg.groups().iter().map(|s| s.to_lowercase()).collect();
            package.categories = Some(fallback_categories_from_pacman_groups(&groups));
        }
        packages.insert(package.canonical_id.clone(), package);
    }

    attach_flatpak_sources(&mut packages, &flatpak_sources, &metadata);

    let mut values = packages.into_values().collect::<Vec<_>>();
    values.sort_by_key(|package| package.effective_title().to_lowercase());
    registry.replace_all_packages(&values)?;
    Ok(values.len())
}

/// Pamac-style: when AppStream has no categories, derive at least one from pacman groups
/// so category browse shows many more apps (like Bazaar/Pamac).
fn fallback_categories_from_pacman_groups(groups: &[String]) -> Vec<String> {
    let mut out: Vec<&'static str> = Vec::new();
    for g in groups {
        let g = g.trim().to_lowercase();
        if g.is_empty() {
            continue;
        }
        if g.contains("game") || g == "games" {
            out.push("Games");
        } else if g.contains("office") || g.contains("productivity") || g.contains("pim") {
            out.push("Productivity");
        } else if g.contains("network") || g.contains("browser") || g.contains("communication") {
            out.push("Internet");
        } else if g.contains("audio") || g.contains("video") || g.contains("multimedia") {
            out.push("Multimedia");
        } else if g.contains("graphic") || g.contains("design") || g == "art" {
            out.push("Graphics & Design");
        } else if g.contains("education") {
            out.push("Utilities");
        } else if g.contains("development") || g.contains("sdk") || g.contains("-devel") {
            out.push("Development");
        } else if g.contains("science") {
            out.push("Utilities");
        } else if g.contains("system") || g.contains("utility") || g.contains("tool")
            || g == "xorg"
            || g.contains("gnome")
            || g.contains("kde-")
        {
            out.push("System Tools");
        }
    }
    out.sort();
    out.dedup();
    if out.is_empty() {
        vec!["Utilities".to_string()]
    } else {
        out.into_iter().map(String::from).collect()
    }
}

fn build_metadata_index(icon_index: &HashMap<String, PathBuf>) -> HashMap<String, AppMetadata> {
    let mut index = HashMap::new();
    for path in appstream_collection_paths() {
        if !path.exists() {
            continue;
        }
        let Ok(collection) = Collection::from_path(path) else {
            continue;
        };
        for component in &collection.components {
            add_component_metadata(&mut index, component, icon_index);
        }
    }

    for path in native_component_paths() {
        if !path.exists() {
            continue;
        }
        if let Ok(component) = Component::from_path(path.clone()) {
            add_component_metadata(&mut index, &component, icon_index);
            continue;
        }
        if let Some(metadata) = raw_component_metadata_from_xml(&path, icon_index) {
            for key in fallback_metadata_keys(&path, &metadata) {
                merge_metadata_candidate(&mut index, key, metadata.clone());
            }
        }
    }
    index
}

fn add_component_metadata(
    index: &mut HashMap<String, AppMetadata>,
    component: &Component,
    icon_index: &HashMap<String, PathBuf>,
) {
    let metadata = component_to_metadata(component, icon_index);
    let keys = metadata_keys(component, &metadata);
    for key in keys {
        merge_metadata_candidate(index, key, metadata.clone());
    }
}

fn appstream_collection_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let legacy_and_new_bases = vec![
        PathBuf::from("/usr/share/app-info/xmls"),
        PathBuf::from("/var/lib/swcatalog/xml"),
        PathBuf::from("/usr/share/swcatalog/xml"),
    ];

    for base in legacy_and_new_bases {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .is_some_and(|ext| ext == "gz" || ext == "xml")
                {
                    paths.push(path);
                }
            }
        }
    }

    for base in flatpak_appstream_bases() {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let remote_dir = entry.path();
            let target = remote_dir.join("x86_64/active/appstream.xml.gz");
            if target.exists() {
                paths.push(target);
                continue;
            }
            let Ok(sub) = std::fs::read_dir(&remote_dir) else {
                continue;
            };
            for child in sub.flatten() {
                let deep = child.path().join("active/appstream.xml.gz");
                if deep.exists() {
                    paths.push(deep);
                }
            }
        }
    }

    paths
}

fn flatpak_appstream_bases() -> Vec<PathBuf> {
    let mut bases = vec![PathBuf::from("/var/lib/flatpak/appstream")];
    if let Some(home) = dirs::home_dir() {
        bases.push(home.join(".local/share/flatpak/appstream"));
    }
    bases
}

fn flatpak_collection_paths() -> Vec<(PathBuf, String)> {
    let mut paths = Vec::new();

    for base in flatpak_appstream_bases() {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let remote_dir = entry.path();
            let remote = remote_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("flathub")
                .to_string();
            let target = remote_dir.join("x86_64/active/appstream.xml.gz");
            if target.exists() {
                paths.push((target, remote.clone()));
                continue;
            }
            let Ok(sub) = std::fs::read_dir(&remote_dir) else {
                continue;
            };
            for child in sub.flatten() {
                let deep = child.path().join("active/appstream.xml.gz");
                if deep.exists() {
                    paths.push((deep, remote.clone()));
                }
            }
        }
    }

    paths.sort_by(|left, right| left.0.cmp(&right.0));
    paths.dedup_by(|left, right| left.0 == right.0);
    paths
}

fn build_flatpak_source_index() -> HashMap<String, Vec<FlatpakAvailability>> {
    let mut index = HashMap::<String, Vec<FlatpakAvailability>>::new();

    for (path, remote) in flatpak_collection_paths() {
        let Ok(collection) = Collection::from_path(path) else {
            continue;
        };
        for component in &collection.components {
            let app_id = component.id.to_string();
            if app_id.trim().is_empty() {
                continue;
            }
            let canonical_id = canonical_id_for(
                component.pkgname.as_deref().unwrap_or(&app_id),
                Some(&app_id),
            );
            if canonical_id.trim().is_empty() || is_prerelease_canonical(&canonical_id) {
                continue;
            }
            let label = if remote == "flathub-beta" {
                "Flathub Beta".to_string()
            } else {
                "Flathub".to_string()
            };
            let source = FlatpakAvailability {
                canonical_id: canonical_id.clone(),
                app_id,
                remote: remote.clone(),
                label,
            };
            push_flatpak_source(&mut index, canonical_id, source);
        }
    }

    for source in flatpak_remote_ls_sources() {
        push_flatpak_source(&mut index, source.canonical_id.clone(), source);
    }

    index
}

fn push_flatpak_source(
    index: &mut HashMap<String, Vec<FlatpakAvailability>>,
    canonical_id: String,
    source: FlatpakAvailability,
) {
    let entry = index.entry(canonical_id).or_default();
    if !entry.iter().any(|existing| {
        existing.app_id.eq_ignore_ascii_case(&source.app_id)
            && existing.remote.eq_ignore_ascii_case(&source.remote)
    }) {
        entry.push(source);
    }
}

fn flatpak_remote_ls_sources() -> Vec<FlatpakAvailability> {
    let output = match std::process::Command::new("flatpak")
        .args(["remote-ls", "--app", "--columns=application,name,origin"])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let app_id = parts.next()?.trim();
            if app_id.is_empty() {
                return None;
            }
            let name = parts.next().unwrap_or_default().trim();
            let remote = parts.next().unwrap_or("flathub").trim();
            let canonical_id =
                canonical_id_for(if name.is_empty() { app_id } else { name }, Some(app_id));
            if canonical_id.is_empty() || is_prerelease_canonical(&canonical_id) {
                return None;
            }
            Some(FlatpakAvailability {
                canonical_id,
                app_id: app_id.to_string(),
                remote: remote.to_string(),
                label: if remote.eq_ignore_ascii_case("flathub-beta") {
                    "Flathub Beta".to_string()
                } else {
                    "Flathub".to_string()
                },
            })
        })
        .collect()
}

fn attach_flatpak_sources(
    packages: &mut HashMap<String, Package>,
    flatpak_sources: &HashMap<String, Vec<FlatpakAvailability>>,
    metadata: &HashMap<String, AppMetadata>,
) {
    for package in packages.values_mut() {
        if is_prerelease_canonical(&package.canonical_id) {
            continue;
        }
        let Some(sources) = flatpak_sources.get(&package.canonical_id) else {
            continue;
        };
        let available_sources = package.available_sources.get_or_insert_with(Vec::new);
        for source in sources {
            let flatpak_source = PackageSource {
                source_type: "flatpak".to_string(),
                id: source.remote.clone(),
                version: package.version.clone(),
                label: source.label.clone(),
                package_name: Some(source.app_id.clone()),
            };
            if !available_sources.iter().any(|existing| {
                existing.source_type == "flatpak"
                    && existing.id.eq_ignore_ascii_case(&flatpak_source.id)
                    && existing.package_name == flatpak_source.package_name
            }) {
                available_sources.push(flatpak_source);
            }
        }

        if let Some(source) = sources.first() {
            if metadata_text_score(Some(source.app_id.as_str()))
                > metadata_text_score(package.app_id.as_deref())
            {
                package.app_id = Some(source.app_id.clone());
            }
        }
        if let Some(meta) = find_metadata(metadata, &package.name, package.app_id.as_deref()) {
            let package_name = package.name.clone();
            merge_metadata_into_package(package, Some(&meta), &package_name);
        }
        fill_icon_from_app_id(package);
    }
}

fn native_component_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut bases = vec![
        PathBuf::from("/usr/share/metainfo"),
        PathBuf::from("/usr/share/appdata"),
        PathBuf::from("/usr/local/share/metainfo"),
        PathBuf::from("/usr/local/share/appdata"),
    ];
    if let Some(home) = dirs::home_dir() {
        bases.push(home.join(".local/share/metainfo"));
        bases.push(home.join(".local/share/appdata"));
    }

    for base in bases {
        let Ok(entries) = std::fs::read_dir(base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let is_component = name.ends_with(".xml")
                || name.ends_with(".metainfo.xml")
                || name.ends_with(".appdata.xml");
            if is_component {
                paths.push(path);
            }
        }
    }

    paths.sort();
    paths.dedup();
    paths
}

fn build_icon_name_index() -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    let mut roots = vec![
        PathBuf::from("/usr/share/pixmaps"),
        PathBuf::from("/usr/share/icons/hicolor/scalable/apps"),
        PathBuf::from("/usr/share/icons/hicolor/512x512/apps"),
        PathBuf::from("/usr/share/icons/hicolor/256x256/apps"),
        PathBuf::from("/usr/share/icons/hicolor/192x192/apps"),
        PathBuf::from("/usr/share/icons/hicolor/128x128/apps"),
        PathBuf::from("/usr/share/icons/hicolor/64x64/apps"),
        PathBuf::from("/usr/share/icons/hicolor/48x48/apps"),
        PathBuf::from("/usr/share/icons/hicolor/32x32/apps"),
        PathBuf::from("/usr/share/icons/hicolor/24x24/apps"),
        PathBuf::from("/usr/share/icons/hicolor/22x22/apps"),
        PathBuf::from("/usr/share/icons/hicolor/16x16/apps"),
        PathBuf::from("/usr/share/icons"),
    ];
    roots.extend(flatpak_icon_roots());
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".local/share/icons"));
    }

    for root in roots {
        walk_icon_dir(&root, &mut index, 0);
    }
    index
}

fn flatpak_icon_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for base in flatpak_appstream_bases() {
        collect_flatpak_icon_dirs(&base, &mut roots, 0);
    }
    roots.sort();
    roots.dedup();
    roots
}

fn collect_flatpak_icon_dirs(base: &Path, roots: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().and_then(|value| value.to_str()) == Some("icons") {
            roots.push(path);
            continue;
        }
        collect_flatpak_icon_dirs(&path, roots, depth + 1);
    }
}

fn walk_icon_dir(root: &Path, index: &mut HashMap<String, PathBuf>, depth: usize) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_icon_dir(&path, index, depth + 1);
            continue;
        }
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "png" | "svg" | "xpm"))
        {
            continue;
        }
        let Some(filename) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let filename = filename.to_ascii_lowercase();
        let stem = stem.to_ascii_lowercase();
        let is_symbolic = stem.ends_with("-symbolic");
        let symbolic_alias = stem.strip_suffix("-symbolic").map(str::to_string);
        let primary_keys = [filename, stem];
        for key in primary_keys {
            insert_icon_candidate(index, key, &path, is_symbolic);
        }
        if let Some(alias) = symbolic_alias {
            insert_icon_candidate(index, alias, &path, true);
        }
    }
}

fn insert_icon_candidate(
    index: &mut HashMap<String, PathBuf>,
    key: String,
    candidate: &Path,
    candidate_is_symbolic: bool,
) {
    if should_skip_identity_icon(candidate, candidate_is_symbolic) {
        return;
    }
    let replace = index
        .get(&key)
        .map(|existing| {
            icon_candidate_score(candidate, candidate_is_symbolic)
                > icon_candidate_score(existing, path_is_symbolic(existing))
        })
        .unwrap_or(true);
    if replace {
        index.insert(key, candidate.to_path_buf());
    }
}

fn icon_candidate_score(path: &Path, is_symbolic: bool) -> i32 {
    let lowered = path.to_string_lossy().to_ascii_lowercase();
    let mut score = 0;

    if !is_symbolic {
        score += 1000;
    }
    if lowered.contains("/apps/") {
        score += 240;
    }
    if lowered.contains("/hicolor/") {
        score += 180;
    }
    if lowered.contains("/scalable/") {
        score += 120;
    }
    if lowered.ends_with(".png") {
        score += 40;
    } else if lowered.ends_with(".svg") {
        score += 30;
    }
    if lowered.contains("/char-white/") {
        score -= 1200;
    }
    if lowered.contains("/status/")
        || lowered.contains("/panel/")
        || lowered.contains("/mimetypes/")
    {
        score -= 900;
    }
    let size_hint = extract_icon_size_hint(&lowered);
    if size_hint > 0 && size_hint < 24 {
        score -= 320;
    }

    score + size_hint
}

fn should_skip_identity_icon(path: &Path, is_symbolic: bool) -> bool {
    if is_symbolic {
        return true;
    }
    let lowered = path.to_string_lossy().to_ascii_lowercase();
    lowered.contains("/char-white/")
        || lowered.contains("/status/")
        || lowered.contains("/panel/")
        || lowered.contains("/mimetypes/")
}

fn path_is_symbolic(path: &Path) -> bool {
    path.file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|stem| stem.to_ascii_lowercase().ends_with("-symbolic"))
}

fn extract_icon_size_hint(path: &str) -> i32 {
    let mut best = 0;
    for segment in path.split('/') {
        if let Some((width, height)) = segment.split_once('x') {
            let width = width.parse::<i32>().ok();
            let height = height.parse::<i32>().ok();
            if let (Some(width), Some(height)) = (width, height) {
                best = best.max(width.min(height));
            }
        }
    }
    best
}

fn component_to_metadata(
    component: &Component,
    icon_index: &HashMap<String, PathBuf>,
) -> AppMetadata {
    let mut icons = component.icons.clone();
    icons.sort_by_key(|b| std::cmp::Reverse(icon_size(b)));

    AppMetadata {
        name: component.name.0.values().next().cloned(),
        summary: component
            .summary
            .as_ref()
            .and_then(|s| s.0.values().next().cloned()),
        description: component
            .description
            .as_ref()
            .and_then(|d| d.0.values().next().cloned()),
        icon: icons
            .iter()
            .find_map(|icon| preferred_icon_to_string(icon, component.id.0.as_str(), icon_index))
            .or_else(|| flatpak_appstream_icon_uri(component.id.0.as_str()))
            .or_else(|| {
                icons.iter().find_map(|icon| match icon {
                    Icon::Stock(_) => icon_to_string(icon, icon_index),
                    _ => None,
                })
            })
            .or_else(|| heuristic_component_icon(component, icon_index)),
        screenshots: component
            .screenshots
            .iter()
            .filter_map(|shot| {
                shot.images
                    .iter()
                    .find(|image| image.kind == appstream::enums::ImageKind::Source)
                    .or_else(|| shot.images.first())
                    .map(|image| image.url.to_string())
            })
            .collect(),
        app_id: Some(component.id.to_string()),
        maintainer: component
            .developer_name
            .as_ref()
            .and_then(|name| name.0.values().next().cloned()),
        license: component.project_license.as_ref().map(|value| {
            value
                .0
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        }),
        categories: component
            .categories
            .iter()
            .map(|value| value.to_string())
            .filter(|value| !value.trim().is_empty())
            .collect(),
    }
}

fn metadata_keys(component: &Component, metadata: &AppMetadata) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(pkg_name) = &component.pkgname {
        keys.push(pkg_name.to_lowercase());
        keys.push(canonical_id_for(pkg_name, metadata.app_id.as_deref()));
    }
    if let Some(app_id) = &metadata.app_id {
        keys.push(app_id.to_lowercase());
        keys.push(canonical_id_for(app_id, Some(app_id)));
    }
    if let Some(name) = &metadata.name {
        keys.push(name.to_lowercase());
        keys.push(canonical_id_for(name, metadata.app_id.as_deref()));
    }
    keys.sort();
    keys.dedup();
    keys
}

fn icon_size(icon: &Icon) -> u32 {
    match icon {
        Icon::Cached { width, .. } => width.unwrap_or(0),
        Icon::Local { width, .. } => width.unwrap_or(0),
        _ => 0,
    }
}

fn icon_to_string(icon: &Icon, icon_index: &HashMap<String, PathBuf>) -> Option<String> {
    match icon {
        Icon::Stock(name) => icon_name_to_uri(name, icon_index),
        Icon::Remote { url, .. } => Some(url.to_string()),
        Icon::Cached { path, .. } | Icon::Local { path, .. } => icon_path_to_uri(path),
    }
}

fn preferred_icon_to_string(
    icon: &Icon,
    app_id: &str,
    icon_index: &HashMap<String, PathBuf>,
) -> Option<String> {
    match icon {
        Icon::Remote { .. } | Icon::Cached { .. } | Icon::Local { .. } => {
            icon_to_string(icon, icon_index)
        }
        Icon::Stock(name) => {
            flatpak_appstream_icon_uri(app_id).or_else(|| icon_name_to_uri(name, icon_index))
        }
    }
}

fn heuristic_component_icon(
    component: &Component,
    icon_index: &HashMap<String, PathBuf>,
) -> Option<String> {
    for candidate in component_icon_candidates(component) {
        if let Some(icon) = icon_name_to_uri(&candidate, icon_index) {
            return Some(icon);
        }
    }
    None
}

fn component_icon_candidates(component: &Component) -> Vec<String> {
    let mut candidates = Vec::new();

    candidates.push(component.id.to_string());
    candidates.push(
        component
            .id
            .to_string()
            .trim_end_matches(".desktop")
            .to_string(),
    );

    if let Some(pkg_name) = component.pkgname.as_deref() {
        candidates.push(pkg_name.to_string());
        candidates.push(strip_package_suffix(&pkg_name.to_ascii_lowercase()).to_string());
    }

    for launchable in &component.launchables {
        if let appstream::enums::Launchable::DesktopId(id) = launchable {
            candidates.push(id.clone());
            candidates.push(id.trim_end_matches(".desktop").to_string());
        }
    }

    if let Some(name) = component.name.0.values().next() {
        candidates.push(name.clone());
        candidates.push(name.to_ascii_lowercase().replace(' ', "-"));
        candidates.push(name.to_ascii_lowercase().replace(' ', ""));
    }

    let mut expanded = Vec::new();
    for candidate in candidates {
        if candidate.trim().is_empty() {
            continue;
        }
        expanded.push(candidate.clone());
        let lowered = candidate.to_ascii_lowercase();
        if lowered != candidate {
            expanded.push(lowered.clone());
        }
        if let Some(tail) = lowered.split('.').next_back() {
            expanded.push(tail.to_string());
        }
        if let Some(base) = lowered.strip_suffix(".desktop") {
            expanded.push(base.to_string());
        }
    }
    expanded.sort();
    expanded.dedup();
    expanded
}

fn package_icon_candidates(
    package_name: &str,
    display_name: Option<&str>,
    app_id: Option<&str>,
) -> Vec<String> {
    let mut candidates = Vec::new();

    candidates.push(package_name.to_string());
    candidates.push(strip_package_suffix(&package_name.to_ascii_lowercase()).to_string());
    candidates.push(canonical_id_for(package_name, app_id));

    if let Some(app_id) = app_id {
        candidates.push(app_id.to_string());
        candidates.push(app_id.trim_end_matches(".desktop").to_string());
        if let Some(tail) = app_id
            .trim_end_matches(".desktop")
            .split('.')
            .next_back()
            .filter(|value| value.len() >= 3)
        {
            candidates.push(tail.to_string());
        }
    }

    if let Some(display_name) = display_name {
        candidates.push(display_name.to_string());
        candidates.push(display_name.to_ascii_lowercase().replace(' ', "-"));
        candidates.push(display_name.to_ascii_lowercase().replace(' ', ""));
    }

    let mut expanded = Vec::new();
    for candidate in candidates {
        if candidate.trim().is_empty() {
            continue;
        }
        expanded.push(candidate.clone());
        let lowered = candidate.to_ascii_lowercase();
        if lowered != candidate {
            expanded.push(lowered.clone());
        }
        if let Some(base) = lowered.strip_suffix(".desktop") {
            expanded.push(base.to_string());
        }
    }
    expanded.sort();
    expanded.dedup();
    expanded
}

fn fill_icon_from_identity(
    package: &mut Package,
    package_name: &str,
    icon_index: &HashMap<String, PathBuf>,
) {
    let candidate = icon_for_package_identity(
        package_name,
        package.display_name.as_deref(),
        package.app_id.as_deref(),
        icon_index,
    );
    if metadata_icon_score(candidate.as_deref()) > metadata_icon_score(package.icon.as_deref()) {
        package.icon = candidate;
    }
}

fn fill_icon_from_app_id(package: &mut Package) {
    let candidate = package
        .app_id
        .as_deref()
        .and_then(flatpak_appstream_icon_uri);
    if metadata_icon_score(candidate.as_deref()) > metadata_icon_score(package.icon.as_deref()) {
        package.icon = candidate;
    }
}

fn icon_for_package_identity(
    package_name: &str,
    display_name: Option<&str>,
    app_id: Option<&str>,
    icon_index: &HashMap<String, PathBuf>,
) -> Option<String> {
    for candidate in package_icon_candidates(package_name, display_name, app_id) {
        if let Some(icon) = icon_name_to_uri(&candidate, icon_index) {
            return Some(icon);
        }
    }
    None
}

fn icon_name_to_uri(icon_name: &str, icon_index: &HashMap<String, PathBuf>) -> Option<String> {
    let raw = icon_name.trim();
    if raw.is_empty() {
        return None;
    }

    let lowered = raw.to_ascii_lowercase();
    let mut candidates = vec![lowered.clone()];
    if !lowered.ends_with(".png") && !lowered.ends_with(".svg") && !lowered.ends_with(".xpm") {
        candidates.push(format!("{lowered}.png"));
        candidates.push(format!("{lowered}.svg"));
        candidates.push(format!("{lowered}.xpm"));
    }
    if let Some(base) = lowered.strip_suffix(".desktop") {
        candidates.push(base.to_string());
        candidates.push(format!("{base}.png"));
        candidates.push(format!("{base}.svg"));
        candidates.push(format!("{base}.xpm"));
    }

    for candidate in candidates {
        if let Some(path) = icon_index.get(&candidate) {
            if let Some(uri) = icon_path_to_uri(path) {
                return Some(uri);
            }
        }
    }
    None
}

fn flatpak_appstream_icon_uri(app_id: &str) -> Option<String> {
    let lowered = app_id.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return None;
    }

    let mut best: Option<PathBuf> = None;
    let mut best_score = i32::MIN;
    for root in flatpak_icon_roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let Ok(files) = std::fs::read_dir(&path) else {
                    continue;
                };
                for file in files.flatten() {
                    let file_path = file.path();
                    let stem = file_path
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .map(|value| value.to_ascii_lowercase());
                    if stem.as_deref() != Some(lowered.as_str()) {
                        continue;
                    }
                    let score = icon_candidate_score(&file_path, path_is_symbolic(&file_path));
                    if score > best_score {
                        best_score = score;
                        best = Some(file_path);
                    }
                }
            }
        }
    }

    best.as_deref().and_then(icon_path_to_uri)
}

fn icon_path_to_uri(path: &Path) -> Option<String> {
    let actual = if path.is_absolute() && path.exists() {
        path.to_path_buf()
    } else {
        let filename = path.file_name()?;
        let cached = icons_dir().join(filename);
        if cached.exists() {
            cached
        } else {
            return None;
        }
    };

    if actual
        .extension()
        .is_some_and(|ext| ext == "svg" || ext == "png")
    {
        let bytes = std::fs::read(&actual).ok()?;
        let mime = if actual.extension().is_some_and(|ext| ext == "svg") {
            if svg_bytes_are_symbolic(&bytes) {
                return None;
            }
            "image/svg+xml"
        } else {
            "image/png"
        };
        return Some(format!(
            "data:{};base64,{}",
            mime,
            BASE64_STANDARD.encode(bytes)
        ));
    }

    Some(actual.to_string_lossy().to_string())
}

fn svg_bytes_are_symbolic(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    text.contains("currentcolor") || text.contains("colorscheme-text") || text.contains("-symbolic")
}

fn icons_dir() -> PathBuf {
    let mut path = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    path.push("monarch-store");
    path.push("icons");
    let _ = std::fs::create_dir_all(&path);
    path
}

fn raw_component_metadata_from_xml(
    path: &Path,
    icon_index: &HashMap<String, PathBuf>,
) -> Option<AppMetadata> {
    let text = std::fs::read_to_string(path).ok()?;
    let document = Document::parse(&text).ok()?;
    let component = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "component")?;

    let app_id = first_child_text(component, "id");
    let name = first_child_text(component, "name");
    let summary = first_child_text(component, "summary");
    let description = first_descendant_text(component, "description");
    let maintainer = first_child_text(component, "developer_name")
        .or_else(|| developer_name_from_node(component));
    let license = first_child_text(component, "project_license")
        .map(|value| {
            value
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty());
    let screenshots = component
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "image")
        .filter_map(node_text)
        .map(|value| value.trim().to_string())
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .collect::<Vec<_>>();
    let categories = component
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "category")
        .filter_map(node_text)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let explicit_icon = component
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "icon")
        .and_then(|node| {
            let raw = node_text(node)?;
            if matches!(node.attribute("type"), Some("stock")) {
                icon_name_to_uri(&raw, icon_index)
            } else if raw.starts_with("http://") || raw.starts_with("https://") {
                Some(raw)
            } else {
                icon_name_to_uri(&raw, icon_index).or_else(|| icon_path_to_uri(Path::new(&raw)))
            }
        });

    let metadata = AppMetadata {
        name: name.clone(),
        summary,
        description,
        icon: explicit_icon.or_else(|| {
            icon_for_package_identity(
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default(),
                name.as_deref(),
                app_id.as_deref(),
                icon_index,
            )
        }),
        screenshots,
        app_id,
        maintainer,
        license,
        categories,
    };

    if metadata.name.is_none()
        && metadata.summary.is_none()
        && metadata.description.is_none()
        && metadata.icon.is_none()
        && metadata.app_id.is_none()
        && metadata.screenshots.is_empty()
    {
        return None;
    }

    Some(metadata)
}

fn fallback_metadata_keys(path: &Path, metadata: &AppMetadata) -> Vec<String> {
    let mut keys = Vec::new();

    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
        keys.push(stem.to_ascii_lowercase());
        keys.push(canonical_id_for(stem, metadata.app_id.as_deref()));
    }

    if let Some(app_id) = metadata.app_id.as_deref() {
        keys.push(app_id.to_ascii_lowercase());
        keys.push(canonical_id_for(app_id, Some(app_id)));
    }

    if let Some(name) = metadata.name.as_deref() {
        keys.push(name.to_ascii_lowercase());
        keys.push(canonical_id_for(name, metadata.app_id.as_deref()));
    }

    keys.sort();
    keys.dedup();
    keys
}

fn first_child_text(node: roxmltree::Node<'_, '_>, tag_name: &str) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == tag_name)
        .and_then(node_text)
}

fn first_descendant_text(node: roxmltree::Node<'_, '_>, tag_name: &str) -> Option<String> {
    node.descendants()
        .find(|child| child.is_element() && child.tag_name().name() == tag_name)
        .and_then(node_text)
}

fn developer_name_from_node(component: roxmltree::Node<'_, '_>) -> Option<String> {
    component
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == "developer")
        .and_then(|developer| first_child_text(developer, "name"))
}

fn node_text(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let mut text = String::new();
    for descendant in node.descendants() {
        if let Some(value) = descendant.text() {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(trimmed);
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn find_metadata(
    index: &HashMap<String, AppMetadata>,
    package_name: &str,
    app_id: Option<&str>,
) -> Option<AppMetadata> {
    let key = package_name.trim().to_lowercase();
    index
        .get(&key)
        .cloned()
        .or_else(|| {
            let stripped = strip_package_suffix(&key).to_string();
            index.get(&stripped).cloned()
        })
        .or_else(|| {
            let canonical = canonical_id_for(package_name, app_id);
            index.get(&canonical).cloned()
        })
}

fn merge_metadata_candidate(
    index: &mut HashMap<String, AppMetadata>,
    key: String,
    candidate: AppMetadata,
) {
    match index.get_mut(&key) {
        Some(existing) => merge_app_metadata(existing, candidate),
        None => {
            index.insert(key, candidate);
        }
    }
}

fn merge_app_metadata(existing: &mut AppMetadata, incoming: AppMetadata) {
    if metadata_text_score(incoming.name.as_deref()) > metadata_text_score(existing.name.as_deref())
    {
        existing.name = incoming.name;
    }
    if metadata_text_score(incoming.summary.as_deref())
        > metadata_text_score(existing.summary.as_deref())
    {
        existing.summary = incoming.summary;
    }
    if metadata_text_score(incoming.description.as_deref())
        > metadata_text_score(existing.description.as_deref())
    {
        existing.description = incoming.description;
    }
    if metadata_icon_score(incoming.icon.as_deref()) > metadata_icon_score(existing.icon.as_deref())
    {
        existing.icon = incoming.icon;
    }
    if incoming.screenshots.len() > existing.screenshots.len() {
        existing.screenshots = incoming.screenshots;
    }
    if metadata_text_score(incoming.app_id.as_deref())
        > metadata_text_score(existing.app_id.as_deref())
    {
        existing.app_id = incoming.app_id;
    }
    if metadata_text_score(incoming.maintainer.as_deref())
        > metadata_text_score(existing.maintainer.as_deref())
    {
        existing.maintainer = incoming.maintainer;
    }
    if incoming
        .license
        .as_ref()
        .map(|items| items.len())
        .unwrap_or(0)
        > existing
            .license
            .as_ref()
            .map(|items| items.len())
            .unwrap_or(0)
    {
        existing.license = incoming.license;
    }
    if incoming.categories.len() > existing.categories.len() {
        existing.categories = incoming.categories;
    }
}

fn metadata_text_score(value: Option<&str>) -> usize {
    value.map(|text| text.trim().len()).unwrap_or_default()
}

fn metadata_icon_score(value: Option<&str>) -> i32 {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return 0;
    };

    let mut score = 0;
    if value.starts_with("data:image/png") {
        score += 620;
    } else if value.starts_with("data:image/svg+xml") {
        score += 520;
    } else if value.starts_with("file://") || value.starts_with('/') {
        score += 420;
    } else if value.starts_with("http://") || value.starts_with("https://") {
        score += 360;
    } else {
        score += 180;
    }

    if value.to_ascii_lowercase().contains("symbolic") {
        score -= 900;
    }

    score + ((value.len().min(32_768) / 64) as i32)
}

fn strip_package_suffix(name: &str) -> &str {
    const SUFFIXES: &[&str] = &[
        "-bin",
        "-git",
        "-appimage",
        "-flatpak",
        "-beta",
        "-nightly",
        "-stable",
        "-gtk3",
    ];
    for suffix in SUFFIXES {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped;
        }
    }
    name
}

fn pretty_name(pkg_name: &str) -> String {
    let last = pkg_name.split('.').next_back().unwrap_or(pkg_name);
    last.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn canonical_id_for(name: &str, app_id: Option<&str>) -> String {
    canonical_merge_key(name, app_id)
}

fn canonical_merge_key(name: &str, app_id: Option<&str>) -> String {
    let mut key = canonical_merge_key_raw(name, app_id).to_lowercase();
    key.retain(|ch| ch.is_ascii_alphanumeric());
    key
}

fn canonical_merge_key_raw(name: &str, app_id: Option<&str>) -> String {
    let name_trim = name.trim();

    if let Some(id) = app_id.map(str::trim).filter(|id| id.contains('.')) {
        if let Some(canonical) = known_app_id_to_canonical(id) {
            if !should_keep_prerelease_separate(name_trim, id, canonical) {
                return canonical.to_string();
            }
        }

        let id = id.strip_suffix(".desktop").unwrap_or(id);
        if let Some(tail) = id.split('.').next_back() {
            let mut tail = tail.trim().to_lowercase();
            let is_generic = matches!(tail.as_str(), "desktop" | "git" | "bin" | "stable");
            if tail.len() < 3 || is_generic {
                let segments = id.split('.').collect::<Vec<_>>();
                if segments.len() > 1 {
                    tail = segments[segments.len() - 2].to_lowercase();
                }
            }
            if !tail.is_empty() {
                if should_keep_prerelease_separate(name_trim, id, &tail) {
                    return name_trim.to_lowercase();
                }
                return tail;
            }
        }
    }

    if let Some(canonical) = known_package_name_to_canonical(name_trim) {
        if !should_keep_prerelease_separate(name_trim, app_id.unwrap_or_default(), canonical) {
            return canonical.to_string();
        }
    }

    if name_trim.contains('.') {
        if let Some(canonical) = known_app_id_to_canonical(name_trim) {
            if !should_keep_prerelease_separate(name_trim, name_trim, canonical) {
                return canonical.to_string();
            }
        }
    }

    let mut clean_name = name_trim.to_lowercase();
    let packaging_suffixes = [
        "-bin",
        "-git",
        "-official",
        "-repo",
        "-stable",
        "-appimage",
        ".desktop",
        "-desktop",
        "-hg",
        "-svn",
    ];

    loop {
        let mut changed = false;
        for suffix in packaging_suffixes {
            if clean_name.ends_with(suffix) {
                clean_name = clean_name
                    .strip_suffix(suffix)
                    .unwrap_or(clean_name.as_str())
                    .to_string();
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }

    clean_name
}

fn should_keep_prerelease_separate(name: &str, app_id: &str, canonical: &str) -> bool {
    (is_prerelease_channel(name) || is_prerelease_channel(app_id))
        && !is_prerelease_channel(canonical)
}

fn is_prerelease_canonical(canonical: &str) -> bool {
    is_prerelease_channel(canonical)
}

fn is_prerelease_channel(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase().replace(['.', '_'], "-");
    [
        "alpha",
        "beta",
        "canary",
        "daily",
        "dev",
        "developer",
        "edge-dev",
        "insiders",
        "nightly",
        "preview",
        "ptb",
        "rc",
        "unstable",
    ]
    .iter()
    .any(|token| {
        normalized == *token
            || normalized.starts_with(&format!("{token}-"))
            || normalized.ends_with(&format!("-{token}"))
            || normalized.contains(&format!("-{token}-"))
    })
}

fn known_package_name_to_canonical(name: &str) -> Option<&'static str> {
    match name.trim().to_ascii_lowercase().as_str() {
        "discord_arch_electron" => Some("discord"),
        "google-chrome-stable" => Some("google-chrome"),
        "heroic-games-launcher-bin" => Some("heroic-games-launcher"),
        "libreoffice-fresh" | "libreoffice-still" => Some("libreoffice"),
        "microsoft-edge-stable" => Some("microsoft-edge"),
        "visual-studio-code-bin" => Some("visual-studio-code"),
        _ => None,
    }
}

fn known_app_id_to_canonical(app_id: &str) -> Option<&'static str> {
    match app_id.trim().to_lowercase().as_str() {
        "com.google.chrome" => Some("google-chrome"),
        "org.mozilla.firefox" => Some("firefox"),
        "org.chromium.chromium" => Some("chromium"),
        "com.brave.browser" => Some("brave"),
        "com.microsoft.edge" => Some("microsoft-edge"),
        "io.gitlab.librewolf-community" => Some("librewolf"),
        "com.spotify.client" => Some("spotify"),
        "com.discordapp.discord" => Some("discord"),
        "com.discordapp.discordcanary" => Some("discord-canary"),
        "com.discordapp.discordptb" => Some("discord-ptb"),
        "org.telegram.desktop" => Some("telegram"),
        "org.signal.signal" => Some("signal"),
        "org.videolan.vlc" => Some("vlc"),
        "org.mpv.mpv" => Some("mpv"),
        "com.obsproject.studio" => Some("obs-studio"),
        "org.gimp.gimp" => Some("gimp"),
        "org.inkscape.inkscape" => Some("inkscape"),
        "org.blender.blender" => Some("blender"),
        "org.kde.kdenlive" => Some("kdenlive"),
        "com.visualstudio.code" => Some("visual-studio-code"),
        "com.visualstudio.code-oss" => Some("code"),
        "com.valvesoftware.steam" => Some("steam"),
        "com.valvesoftware.steam.desktop" => Some("steam"),
        "com.heroicgameslauncher.hgl" => Some("heroic-games-launcher"),
        "com.mojang.minecraft" => Some("minecraft-launcher"),
        "org.libreoffice.libreoffice" => Some("libreoffice"),
        "org.keepassxc.keepassxc" => Some("keepassxc"),
        "com.bitwarden.desktop" => Some("bitwarden"),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        canonical_id_for, is_prerelease_channel, known_package_name_to_canonical,
        merge_app_metadata, merge_metadata_into_package, AppMetadata,
    };
    use crate::models::{Package, PackageSource};

    #[test]
    fn stable_and_prerelease_variants_do_not_merge() {
        assert_eq!(
            canonical_id_for("firefox", Some("org.mozilla.firefox")),
            "firefox"
        );
        assert_eq!(
            canonical_id_for("firefox-nightly", Some("org.mozilla.firefox")),
            "firefoxnightly"
        );
        assert_eq!(
            canonical_id_for("discord-canary", Some("com.discordapp.discordcanary")),
            "discordcanary"
        );
    }

    #[test]
    fn known_aliases_merge_into_stable_canonical_ids() {
        assert_eq!(
            known_package_name_to_canonical("discord_arch_electron"),
            Some("discord")
        );
        assert_eq!(canonical_id_for("discord_arch_electron", None), "discord");
        assert_eq!(
            canonical_id_for("heroic-games-launcher-bin", None),
            "heroicgameslauncher"
        );
    }

    #[test]
    fn prerelease_detector_catches_common_channel_names() {
        assert!(is_prerelease_channel("nightly"));
        assert!(is_prerelease_channel("firefox-nightly"));
        assert!(is_prerelease_channel("discord_canary"));
        assert!(!is_prerelease_channel("discord"));
    }

    #[test]
    fn merge_app_metadata_prefers_richer_icon_payload() {
        let mut existing = AppMetadata {
            icon: Some("https://example.test/icon.png".to_string()),
            ..Default::default()
        };
        let incoming = AppMetadata {
            icon: Some(format!("data:image/png;base64,{}", "A".repeat(4096))),
            ..Default::default()
        };

        merge_app_metadata(&mut existing, incoming);

        assert!(existing.icon.unwrap().starts_with("data:image/png"));
    }

    #[test]
    fn merge_metadata_into_package_upgrades_existing_icon_when_incoming_is_better() {
        let mut package = Package {
            name: "discord".to_string(),
            description: "Chat app".to_string(),
            source: PackageSource::new("repo", "extra", "1.0", "Arch Official"),
            icon: Some("discord-symbolic".to_string()),
            ..Default::default()
        };
        let meta = AppMetadata {
            icon: Some(format!("data:image/png;base64,{}", "B".repeat(2048))),
            screenshots: vec!["https://example.test/shot.png".to_string()],
            ..Default::default()
        };

        merge_metadata_into_package(&mut package, Some(&meta), "discord");

        assert!(package.icon.unwrap().starts_with("data:image/png"));
        assert_eq!(package.screenshots.unwrap().len(), 1);
    }
}

fn merge_metadata_into_package(
    entry: &mut Package,
    meta: Option<&AppMetadata>,
    package_name: &str,
) {
    let Some(meta) = meta else {
        return;
    };

    if entry
        .display_name
        .as_deref()
        .map(|value| value.trim().is_empty() || value == pretty_name(package_name))
        .unwrap_or(true)
    {
        if let Some(name) = meta.name.as_ref().filter(|value| !value.trim().is_empty()) {
            entry.display_name = Some(name.clone());
        }
    }

    if let Some(summary) = meta
        .summary
        .as_deref()
        .map(sanitize_metadata_summary)
        .filter(|value| !value.trim().is_empty())
    {
        if metadata_text_score(Some(summary.as_str()))
            > metadata_text_score(Some(entry.description.as_str()))
        {
            entry.description = summary;
        }
    }

    let incoming_long = meta
        .description
        .as_deref()
        .map(sanitize_metadata_description)
        .filter(|value| !value.trim().is_empty());
    if metadata_text_score(incoming_long.as_deref())
        > metadata_text_score(entry.long_description.as_deref())
    {
        entry.long_description = incoming_long;
    }

    if metadata_icon_score(meta.icon.as_deref()) > metadata_icon_score(entry.icon.as_deref()) {
        entry.icon = meta.icon.clone();
    }

    if meta.screenshots.len()
        > entry
            .screenshots
            .as_ref()
            .map(|shots| shots.len())
            .unwrap_or_default()
    {
        entry.screenshots = Some(meta.screenshots.clone());
    }

    if metadata_text_score(meta.app_id.as_deref()) > metadata_text_score(entry.app_id.as_deref()) {
        entry.app_id = meta.app_id.clone();
    }

    if metadata_text_score(meta.maintainer.as_deref())
        > metadata_text_score(entry.maintainer.as_deref())
    {
        entry.maintainer = meta.maintainer.clone();
    }

    if meta.license.as_ref().map(|items| items.len()).unwrap_or(0)
        > entry.license.as_ref().map(|items| items.len()).unwrap_or(0)
    {
        entry.license = meta.license.clone();
    }

    if meta.categories.len()
        > entry
            .categories
            .as_ref()
            .map(|categories| categories.len())
            .unwrap_or_default()
    {
        entry.categories = Some(meta.categories.clone());
    }
}

fn sanitize_metadata_summary(value: &str) -> String {
    sanitize_metadata_text(value, 1)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn sanitize_metadata_description(value: &str) -> String {
    sanitize_metadata_text(value, 3)
}

fn sanitize_metadata_text(value: &str, max_paragraphs: usize) -> String {
    let mut text = value
        .replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n")
        .replace("</p>", "\n\n")
        .replace("<p>", "")
        .replace("<ul>", "\n")
        .replace("</ul>", "\n")
        .replace("<ol>", "\n")
        .replace("</ol>", "\n")
        .replace("<li>", "- ")
        .replace("</li>", "\n");

    text = strip_html_tags(&text);
    text = decode_html_entities(&text);

    let mut paragraphs = Vec::new();
    for paragraph in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if paragraphs
            .last()
            .is_some_and(|previous: &String| previous.eq_ignore_ascii_case(paragraph))
        {
            continue;
        }
        paragraphs.push(collapse_internal_whitespace(paragraph));
        if paragraphs.len() >= max_paragraphs {
            break;
        }
    }

    paragraphs.join("\n\n")
}

fn strip_html_tags(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut inside_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

fn decode_html_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn collapse_internal_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn candidate_priority(source: &PackageSource, installed: bool) -> i32 {
    if installed {
        return 1000;
    }
    match source.id.as_str() {
        "core" | "extra" | "community" | "multilib" => 500,
        "chaotic-aur" => 400,
        id if id.starts_with("cachyos") => 450,
        _ => 300,
    }
}

fn repo_source(repo_name: &str, version: &str, package_name: &str) -> PackageSource {
    let repo_id = repo_name.to_lowercase();
    let label = if matches!(
        repo_id.as_str(),
        "core" | "extra" | "community" | "multilib"
    ) {
        "Arch Official".to_string()
    } else if repo_id.contains("chaotic") {
        "Chaotic-AUR".to_string()
    } else if repo_id.starts_with("cachyos") {
        "CachyOS (Optimized)".to_string()
    } else if repo_id.starts_with("manjaro") {
        "Manjaro Official".to_string()
    } else if repo_id.starts_with("garuda") {
        "Garuda Linux".to_string()
    } else {
        pretty_name(repo_name)
    };

    PackageSource {
        source_type: "repo".to_string(),
        id: repo_id,
        version: version.to_string(),
        label,
        package_name: Some(package_name.to_string()),
    }
}

fn collect_repo_sections_from_conf(conf_path: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let content = match std::fs::read_to_string(conf_path) {
        Ok(content) => content,
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
            if section != "options" && !sections.iter().any(|item| item == section) {
                sections.push(section.to_string());
            }
            continue;
        }
        if line.to_lowercase().starts_with("include") {
            let rest = line[6..].trim_start_matches(['=', ' ']).trim();
            let path = rest.trim_matches(|ch| ch == '"' || ch == '\'');
            let full = if path.starts_with('/') {
                path.to_string()
            } else {
                conf_dir.join(path).to_string_lossy().into_owned()
            };
            for include in glob_includes(&full) {
                for section in collect_repo_sections_from_conf(&include) {
                    if !sections.iter().any(|item| item == &section) {
                        sections.push(section);
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
    let file_pattern = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let suffix = file_pattern
        .find('*')
        .map(|index| &file_pattern[index + 1..])
        .unwrap_or("");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let candidate = entry.path();
            let name = candidate
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if (suffix.is_empty() || name.ends_with(suffix)) && candidate.is_file() {
                if let Some(value) = candidate.to_str() {
                    out.push(value.to_string());
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

/// Looks up download and installed size for repo/chaotic packages from the sync databases.
/// Returns a map from (repo_id, pkg_name) to (download_size_bytes, installed_size_bytes).
/// Used when hydrating package details so each repo variant gets the correct size.
pub fn get_repo_package_sizes(
    requests: &[(String, String)],
    pacman_conf_path: &str,
) -> HashMap<(String, String), (u64, u64)> {
    let mut out = HashMap::new();
    if requests.is_empty() {
        return out;
    }
    let Ok(alpm) = Alpm::new("/", "/var/lib/pacman") else {
        return out;
    };
    register_syncdbs_from_conf(&alpm, pacman_conf_path);
    for (repo_id, pkg_name) in requests {
        for db in alpm.syncdbs() {
            if db.name() != repo_id.as_str() {
                continue;
            }
            if let Ok(pkg) = db.pkg(pkg_name.as_str()) {
                out.insert(
                    (repo_id.clone(), pkg_name.clone()),
                    (pkg.download_size() as u64, pkg.isize() as u64),
                );
                break;
            }
        }
    }
    out
}

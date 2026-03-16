use std::process::Stdio;
use tokio::io::AsyncWriteExt;

pub const MONARCH_PK_HELPER: &str = "/usr/lib/monarch-store/monarch-helper";

/// Single source of truth for the dev helper path. Same resolution order as helper_client so
/// install/update and onboarding deployment always use the same binary (e.g. src-tauri/target/debug when npm run tauri dev).
pub fn get_dev_helper_path() -> Option<std::path::PathBuf> {
    // 1. CARGO_TARGET_DIR (set by npm run tauri dev) — may be relative or absolute
    if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        let p = std::path::Path::new(&target_dir)
            .join("debug")
            .join("monarch-helper");
        if p.exists() {
            return Some(p.canonicalize().unwrap_or(p.to_path_buf()));
        }
        // If relative, try from cwd
        if !target_dir.starts_with('/') {
            if let Ok(cwd) = std::env::current_dir() {
                let p = cwd.join(&target_dir).join("debug").join("monarch-helper");
                if p.exists() {
                    return Some(p.canonicalize().unwrap_or(p.to_path_buf()));
                }
            }
        }
    }
    // 2. Same directory as this executable (works when both are in target/debug)
    if let Ok(exe_path) = std::env::current_exe() {
        let exe_canon = exe_path.canonicalize().unwrap_or(exe_path);
        if let Some(parent) = exe_canon.parent() {
            let p = parent.join("monarch-helper");
            if p.exists() {
                return Some(p.canonicalize().unwrap_or(p.to_path_buf()));
            }
        }
    }
    // 3. Relative fallbacks from cwd (project root when run via npm run tauri dev)
    for path in &[
        "src-tauri/target/debug/monarch-helper",
        "./src-tauri/target/debug/monarch-helper",
        "../target/debug/monarch-helper",
        "./target/debug/monarch-helper",
    ] {
        let p = std::path::Path::new(path);
        if p.exists() {
            if let Ok(canon) = p.canonicalize() {
                return Some(canon);
            }
        }
    }
    None
}

/// Returns true if the helper binary is available (production path or dev build path).
/// Use this for health checks so dev builds (npm run tauri dev) don't report "helper missing" every launch.
pub fn monarch_helper_available() -> bool {
    if std::path::Path::new(MONARCH_PK_HELPER).exists() {
        return true;
    }
    get_dev_helper_path().is_some()
}

lazy_static::lazy_static! {
    pub static ref PRIVILEGED_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::new(());
}

pub fn to_pretty_name(pkg_name: &str) -> String {
    // 0. Handle RDN App IDs (e.g. com.discordapp.Discord -> Discord)
    let name_to_process = if pkg_name.contains('.') {
        pkg_name.split('.').next_back().unwrap_or(pkg_name)
    } else {
        pkg_name
    };

    // 1. Basic cleaning and splitting
    let parts: Vec<&str> = name_to_process.split(['-', '_']).collect();

    // 2. Capitalization logic
    let pretty: Vec<String> = parts
        .into_iter()
        .map(|part| {
            match part.to_lowercase().as_str() {
                "cli" => "CLI".to_string(),
                "tui" => "TUI".to_string(),
                "gui" => "GUI".to_string(),
                "api" => "API".to_string(),
                "sdk" => "SDK".to_string(),
                "aur" => "AUR".to_string(),
                "git" => "Git".to_string(),
                "bin" => "".to_string(), // Strip common suffixes
                "" => "".to_string(),
                _ => {
                    let mut chars = part.chars();
                    match chars.next() {
                        core::option::Option::None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                }
            }
        })
        .filter(|p| !p.is_empty())
        .collect();

    if pretty.is_empty() {
        return name_to_process.to_string();
    }

    pretty.join(" ")
}

/// Strip HTML tags so descriptions never show literal `<p>` etc. on cards.
pub fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .trim()
        .to_string()
}

/// Truncate description for UI payload (max 200 chars) to reduce IPC and DOM size.
/// Strips HTML first so cards never show literal `<p>` or other tags.
pub fn truncate_description_for_ui(s: &str, max_chars: usize) -> String {
    let s = strip_html(s).trim().to_string();
    let s = s.as_str();
    if s.len() <= max_chars {
        s.to_string()
    } else {
        let mut end = max_chars;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

/// Strip HTML and truncate descriptions on a list of packages (for search, category, trending).
const DESC_MAX_UI: usize = 200;

fn package_primary_action(pkg: &crate::models::Package) -> (&'static str, &'static str) {
    if pkg.installed {
        ("launch", "Open")
    } else {
        ("install", "Install")
    }
}

fn package_source_summary(pkg: &crate::models::Package) -> String {
    if let Some(sources) = &pkg.available_sources {
        if sources.len() > 1 {
            return format!("{} sources available", sources.len());
        }
    }

    if pkg.is_optimized.unwrap_or(false) {
        return format!("Optimized for {}", pkg.source.label);
    }

    if !pkg.source.label.trim().is_empty() {
        return pkg.source.label.clone();
    }

    if !pkg.source.id.trim().is_empty() {
        return pkg.source.id.clone();
    }

    "Available now".to_string()
}

fn package_trust_level(pkg: &crate::models::Package) -> &'static str {
    match pkg.source.source_type.as_str() {
        "flatpak" => "sandboxed",
        "aur" => "community",
        "repo" => {
            let id = pkg.source.id.to_lowercase();
            if matches!(id.as_str(), "core" | "extra" | "community" | "multilib" | "official")
                || id.contains("manjaro")
                || id.contains("cachyos")
                || id.contains("garuda")
                || id.contains("endeavour")
            {
                "official"
            } else {
                "third_party"
            }
        }
        _ => "third_party",
    }
}

fn package_security_summary(pkg: &crate::models::Package) -> &'static str {
    match pkg.source.source_type.as_str() {
        "flatpak" => "Sandboxed package with portal-based access.",
        "aur" => "Community build script; review PKGBUILD before installing.",
        _ => "Native package with direct system access.",
    }
}

pub fn apply_package_ui_defaults(pkg: &mut crate::models::Package) {
    if pkg
        .display_title
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        let display_name = pkg
            .display_name
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| to_pretty_name(&pkg.name));
        pkg.display_title = Some(display_name);
    }

    let (action, label) = package_primary_action(pkg);
    if pkg
        .primary_action
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        pkg.primary_action = Some(action.to_string());
    }
    if pkg
        .primary_action_label
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        pkg.primary_action_label = Some(label.to_string());
    }

    if pkg
        .source_summary
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        pkg.source_summary = Some(package_source_summary(pkg));
    }

    if pkg
        .trust_level
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        pkg.trust_level = Some(package_trust_level(pkg).to_string());
    }

    if pkg
        .security_summary
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        pkg.security_summary = Some(package_security_summary(pkg).to_string());
    }

    if pkg.last_modified_unix.is_none() {
        pkg.last_modified_unix = pkg.last_modified;
    }
    if pkg.download_size_bytes.is_none() {
        pkg.download_size_bytes = pkg.download_size;
    }
    if pkg.installed_size_bytes.is_none() {
        pkg.installed_size_bytes = pkg.installed_size;
    }
}

pub fn prepare_package_descriptions_for_ui(packages: &mut [crate::models::Package]) {
    for pkg in packages.iter_mut() {
        if !pkg.description.is_empty() {
            pkg.description = truncate_description_for_ui(&pkg.description, DESC_MAX_UI);
        }
        apply_package_ui_defaults(pkg);
    }
}

lazy_static::lazy_static! {
    static ref VALIDATE_RE: regex::Regex = regex::Regex::new(r"^[a-zA-Z0-9@._+\-]+$").expect("valid package name regex");
}

static VALIDATE_CACHE: once_cell::sync::Lazy<moka::sync::Cache<String, Result<(), String>>> =
    once_cell::sync::Lazy::new(|| {
        moka::sync::Cache::builder()
            .max_capacity(2000)
            .time_to_live(std::time::Duration::from_secs(300))
            .build()
    });

fn validate_package_name_impl(name: &str) -> Result<(), String> {
    if !VALIDATE_RE.is_match(name) {
        return Err(format!(
            "Invalid package name: '{}'. Contains unsafe characters.",
            name
        ));
    }
    Ok(())
}

/// Validates package name (Arch standard). Results are memoized to avoid repeated regex checks during rapid search.
pub fn validate_package_name(name: &str) -> Result<(), String> {
    if let Some(cached) = VALIDATE_CACHE.get(name) {
        return cached;
    }
    let r = validate_package_name_impl(name);
    VALIDATE_CACHE.insert(name.to_string(), r.clone());
    r
}

use crate::models;

// Checks if the CPU supports x86-64-v3 (AVX2, FMA, BMI2, etc.)
pub fn is_cpu_v3_compatible() -> bool {
    let cpuid = raw_cpuid::CpuId::new();

    // v3 requires: AVX, AVX2, BMI1, BMI2, F16C, FMA, MOVBE, XSAVE, LZCNT (ABM)
    let has_v3_base = if let Some(feat) = cpuid.get_feature_info() {
        feat.has_avx() && feat.has_fma() && feat.has_f16c() && feat.has_movbe() && feat.has_xsave()
    } else {
        false
    };

    let has_v3_ext = if let Some(ext) = cpuid.get_extended_feature_info() {
        ext.has_avx2() && ext.has_bmi1() && ext.has_bmi2()
    } else {
        false
    };

    let has_lzcnt = if let Some(ext) = cpuid.get_extended_processor_and_feature_identifiers() {
        ext.has_lzcnt()
    } else {
        false
    };

    has_v3_base && has_v3_ext && has_lzcnt
}

// Checks if the CPU supports x86-64-v4 (AVX-512 foundation and major extensions)
pub fn is_cpu_v4_compatible() -> bool {
    let cpuid = raw_cpuid::CpuId::new();

    if let Some(ext) = cpuid.get_extended_feature_info() {
        // v4 requires v3 + AVX-512F, BW, CD, DQ, VL
        ext.has_avx512f()
            && ext.has_avx512bw()
            && ext.has_avx512cd()
            && ext.has_avx512dq()
            && ext.has_avx512vl()
    } else {
        false
    }
}

// Checks if the CPU is Zen 4 or Zen 5 (optimized)
pub fn is_cpu_znver4_compatible() -> bool {
    let cpuid = raw_cpuid::CpuId::new();

    // 1. Must support v4 features
    if !is_cpu_v4_compatible() {
        return false;
    }

    // 2. Check for AuthenticAMD vendor
    let is_amd = cpuid
        .get_vendor_info()
        .map(|v| v.as_str() == "AuthenticAMD")
        .unwrap_or(false);
    if !is_amd {
        return false;
    }

    // 3. Detect Zen 4/5 via Leaf 7 Sub-leaf 1 (AVX512-VNNI, BF16, etc.)
    // Zen 4 specific: AVX512_VNNI, AVX512_BF16, AVX512_VBMI2 etc.
    if let Some(ext) = cpuid.get_extended_feature_info() {
        // We look for flags introduced in Zen 4 (AVX512-VNNI is one, but Intel has it too)
        // AuthenticAMD + AVX512F + BIT ALGORITHM/VPOPCNTDQ is a good indicator of Zen 4
        ext.has_avx512vnni() && ext.has_avx512bitalg()
    } else {
        false
    }
}

/// Strips common package suffixes like -bin, -git, -nightly
pub fn strip_package_suffix(name: &str) -> &str {
    // Ordered by length (longest first) to match specific first?
    // Actually -bin and -git are most common.
    // If strict match needed, verify with list.
    let suffixes = [
        "-bin",
        "-git",
        "-nightly",
        "-beta",
        "-dev",
        "-pure",
        "-appimage",
        "-wayland",
        "-x11",
        "-hg",
        "-svn",
        "-cn",
        "-fresh",
        "-still",
        "-native",
        "-runtime",
        "-lts",
        "-edge",
        "-stable",
    ];

    for suffix in suffixes {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped;
        }
    }
    name
}

/// Known Flatpak app_id -> canonical package name so the same app from AUR, Flatpak, and Chaotic
/// merges into one card. Maps app_id (e.g. com.google.Chrome) to repo/AUR package name (e.g. google-chrome).
/// Reverse mapping: maps canonical/repo names to Flathub App IDs for metadata proxying.
pub fn canonical_to_flathub_id(canonical: &str) -> Option<String> {
    let c = canonical.trim().to_lowercase();
    let map: &[(&str, &str)] = &[
        ("google-chrome", "com.google.Chrome"),
        ("firefox", "org.mozilla.firefox"),
        ("brave", "com.brave.Browser"),
        ("spotify", "com.spotify.Client"),
        ("discord", "com.discordapp.Discord"),
        ("telegram-desktop", "org.telegram.desktop"),
        ("signal-desktop", "org.signal.Signal"),
        ("vlc", "org.videolan.VLC"),
        ("mpv", "io.mpv.Mpv"),
        ("obs-studio", "com.obsproject.Studio"),
        ("gimp", "org.gimp.GIMP"),
        ("kdenlive", "org.kde.kdenlive"),
        ("code", "com.visualstudio.code"),
        ("visual-studio-code", "com.visualstudio.code"),
        ("steam", "com.valvesoftware.Steam"),
        ("heroic-games-launcher", "com.heroicgameslauncher.hgl"),
        ("bitwarden", "com.bitwarden.desktop"),
        ("keepassxc", "org.keepassxc.KeePassXC"),
    ];
    for (k, v) in map {
        if c == *k {
            return Some((*v).to_string());
        }
    }
    None
}

pub fn known_app_id_to_canonical(app_id: &str) -> Option<String> {
    let id = app_id.trim().to_lowercase();
    let map: &[(&str, &str)] = &[
        // Browsers (last segment often != package name)
        ("com.google.chrome", "google-chrome"),
        ("org.mozilla.firefox", "firefox"),
        ("org.chromium.chromium", "chromium"),
        ("com.brave.browser", "brave"),
        ("com.microsoft.edge", "microsoft-edge"),
        ("com.vivaldi.vivaldi", "vivaldi"),
        ("io.gitlab.librewolf-community", "librewolf"),
        // Media / comms
        ("com.spotify.client", "spotify"),
        ("com.discordapp.discord", "discord"),
        ("com.discordapp.discordcanary", "discord-canary"),
        ("com.discordapp.discordptb", "discord-ptb"),
        ("io.github.spacingbat3.webcord", "webcord"),
        ("dev.vencord.vesktop", "vesktop"),
        ("org.telegram.desktop", "telegram-desktop"),
        ("org.signal.signal", "signal-desktop"),
        ("us.zoom.zoom", "zoom"),
        ("com.microsoft.teams", "teams"),
        ("com.slack.slack", "slack-desktop"),
        ("im.riot.riot", "element-desktop"),
        ("chat.zulip.zulip", "zulip-desktop"),
        // Media players / editors
        ("org.videolan.vlc", "vlc"),
        ("org.mpv.mpv", "mpv"),
        ("com.obsproject.studio", "obs-studio"),
        ("org.gimp.gimp", "gimp"),
        ("org.inkscape.inkscape", "inkscape"),
        ("org.blender.blender", "blender"),
        ("org.audacityteam.audacity", "audacity"),
        ("org.kde.kdenlive", "kdenlive"),
        // Development
        ("com.visualstudio.code", "visual-studio-code"),
        ("com.visualstudio.code-oss", "code"),
        (
            "com.jetbrains.intellij-idea-community",
            "intellij-idea-community-edition",
        ),
        (
            "com.jetbrains.pycharm-community",
            "pycharm-community-edition",
        ),
        ("com.jetbrains.toolbox", "jetbrains-toolbox"),
        ("com.sublimetext.three", "sublime-text-4"),
        ("com.getpostman.postman", "postman-bin"),
        // Gaming / office / utils
        ("com.valvesoftware.steam", "steam"),
        ("com.valvesoftware.steam.desktop", "steam"),
        ("net.lutris.lutris", "lutris"),
        ("net.lutris.lutris.desktop", "lutris"),
        ("com.heroicgameslauncher.hgl", "heroic-games-launcher"),
        ("com.mojang.minecraft", "minecraft-launcher"),
        ("org.libreoffice.libreoffice", "libreoffice"),
        ("org.onlyoffice.desktopeditors", "onlyoffice-bin"),
        ("com.bitwarden.desktop", "bitwarden"),
        ("org.keepassxc.keepassxc", "keepassxc"),
        ("org.mozilla.thunderbird", "thunderbird"),
        ("org.filezilla_project.filezilla", "filezilla"),
        ("org.qbittorrent.qbittorrent", "qbittorrent"),
        ("com.transmissionbt.transmission", "transmission-gtk"),
        ("org.virtualbox.virtualbox", "virtualbox"),
    ];
    for (k, v) in map {
        if id == *k {
            return Some((*v).to_string());
        }
    }
    None
}

/// Preferred display name for a canonical key so the app is always shown with one proper name
/// (e.g. "heroic" -> "Heroic Game Launcher" instead of sometimes "Heroic"). Display-only; merge key stays generic.
fn preferred_display_name(canonical_key: &str) -> Option<&'static str> {
    match canonical_key {
        "heroicgameslauncher" => Some("Heroic Game Launcher"),
        "heroic" => Some("Heroic Game Launcher"),
        "obsstudio" => Some("OBS Studio"),
        "obs" => Some("OBS Studio"),
        "visualstudiocode" | "code" => Some("Visual Studio Code"),
        "libreoffice" => Some("LibreOffice"),
        "brave" => Some("Brave Browser"),
        "googlechrome" | "chrome" => Some("Google Chrome"),
        "edge" => Some("Microsoft Edge"),
        "retroarch" => Some("RetroArch"),
        _ => None,
    }
}

/// Public accessor so search/category pipelines can apply preferred names (e.g. "Heroic" -> "Heroic Game Launcher").
pub fn get_preferred_display_name(canonical_key: &str) -> Option<&'static str> {
    preferred_display_name(canonical_key)
}

/// Extra search terms when the user query is a short name (e.g. "heroic") so we reliably
/// get repo/AUR/Chaotic packages (e.g. "heroic-games-launcher-bin" from CachyOS, Chaotic-AUR, or AUR).
/// Used for AUR search, Chaotic filter, and repo search so one card shows all sources.
pub fn aur_search_expansion_terms(query: &str) -> Vec<&'static str> {
    let q = query.trim().to_lowercase();
    // Normalize query to alphanumeric for mapping (so "heroic-games-launcher" and "heroicgameslauncher" both match)
    let mut q_norm = q.clone();
    q_norm.retain(|c| c.is_ascii_alphanumeric());

    if q.is_empty() {
        return vec![];
    }
    match q_norm.as_str() {
        "heroic" | "heroicgameslauncher" => vec!["heroic-games-launcher"],
        "obs" | "obsstudio" => vec!["obs-studio"],
        "code" | "vscode" | "visualstudiocode" => vec!["visual-studio-code"],
        "chrome" | "google" | "googlechrome" => vec!["google-chrome"],
        _ => vec![],
    }
}

/// Repo package names to include in get_packages_batch when resolving a canonical app (e.g. Discord from CachyOS).
/// So one card shows Official + CachyOS + Flatpak even when CachyOS names the pkg discord_arch_electron.
pub fn canonical_to_repo_lookup_names(canonical: &str) -> Vec<&'static str> {
    let c = canonical.trim().to_lowercase();
    let mut c_norm = c.clone();
    c_norm.retain(|c| c.is_ascii_alphanumeric());

    if c.is_empty() {
        return vec![];
    }
    match c_norm.as_str() {
        "discord" | "discordarchelectron" => vec!["discord", "discord_arch_electron"],
        "discordcanary" => vec!["discord-canary"],
        "discordptb" => vec!["discord-ptb"],
        "heroicgameslauncher" => vec!["heroic-games-launcher", "heroic-games-launcher-bin"],
        "obsstudio" => vec!["obs-studio"],
        "code" | "visualstudiocode" => vec!["visual-studio-code", "code"],
        "telegram" | "telegramdesktop" => vec!["telegram-desktop"],
        "signal" | "signaldesktop" => vec!["signal-desktop"],
        "google" | "googlechrome" => vec!["google-chrome", "google-chrome-stable"],
        "googlechromecanary" => vec!["google-chrome-canary"],
        "microsoftedge" => vec![
            "microsoft-edge-stable",
            "microsoft-edge-dev",
            "microsoft-edge",
        ],
        "libreoffice" => vec!["libreoffice-fresh", "libreoffice-still"],
        "vlc" => vec!["vlc"],
        "steam" | "steamnative" => vec!["steam", "steam-native-runtime"],
        "minecraft" | "minecraftlauncher" | "prism" | "prismlauncher" => {
            vec!["minecraft-launcher", "poly-mc-launcher", "prism-launcher"]
        }
        "proton" | "protonvpn" => vec!["proton-vpn-gtk", "proton-vpn"],
        "linux" | "linuxcachyos" => vec!["linux-cachyos", "linux"],
        "cachyos" | "cachyossettings" => vec!["cachyos-settings"],
        "simplenote" | "simplenoteelectron" => vec!["simplenote-electron-bin"],
        "bitwarden" => vec!["bitwarden"],
        "keepass" | "keepassxc" => vec!["keepassxc"],
        "thunderbird" => vec!["thunderbird"],
        _ => vec![],
    }
}

pub fn is_package_or_alias_installed(name: &str) -> bool {
    if crate::alpm_read::is_package_installed(name) {
        return true;
    }

    let canonical = canonical_merge_key(name, None);
    for alias in canonical_to_repo_lookup_names(&canonical) {
        if alias != name && crate::alpm_read::is_package_installed(alias) {
            return true;
        }
    }

    false
}

pub fn installed_source_for_package(name: &str, app_id: Option<&str>) -> Option<crate::models::PackageSource> {
    let canonical = canonical_merge_key(name, app_id);
    let mut candidates = vec![name.to_string()];

    for alias in canonical_to_repo_lookup_names(&canonical) {
        let alias_name = alias.to_string();
        if !candidates.contains(&alias_name) {
            candidates.push(alias_name);
        }
    }

    for candidate in candidates {
        if let Some(pkg) = crate::alpm_read::get_package_native(&candidate) {
            if pkg.installed {
                return Some(pkg.source);
            }
        }
    }

    None
}

/// Returns a canonical key for merge deduplication. Variants (firefox, firefox-developer-edition,
/// discord-bin) and Flatpak app_id (com.discordapp.Discord) map to the same key so they merge
/// into one entry with multiple sources.
/// 1. Prioritize AppID if it exists: check known map (e.g. com.obsproject.Studio -> obs-studio), else last RDN segment.
/// 2. Fallback to package name with aggressive suffix stripping (-bin, -git, etc.).
///    For multi-segment names we use the first segment as key when valid (so "heroic" and "heroic-games-launcher" merge without a per-app list).
pub fn canonical_merge_key(name: &str, app_id: Option<&str>) -> String {
    let mut key = canonical_merge_key_raw(name, app_id).to_lowercase();
    // STRICT IRON CORE RULE: Retain ONLY alphanumeric, stripping dots, hyphens, underscores.
    // This ensures parity with frontend getPackageListKey (which uses replace(/[^a-z0-9]/g, '')).
    key.retain(|c| c.is_ascii_alphanumeric());
    key
}

/// Returns a "clean" canonical key that preserves hyphens/dots for search and grouping logic.
/// Matches the result of canonical_merge_key_raw but explicitly lowercased.
pub fn canonical_search_base(name: &str, app_id: Option<&str>) -> String {
    canonical_merge_key_raw(name, app_id).to_lowercase()
}

pub fn finalize_package_contract(pkg: &mut models::Package) {
    if pkg.canonical_id.trim().is_empty() {
        pkg.canonical_id = canonical_merge_key(&pkg.name, pkg.app_id.as_deref());
    } else {
        pkg.canonical_id = canonical_merge_key(&pkg.canonical_id, None);
    }

    pkg.app_id = pkg.app_id.as_ref().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    let mut normalized_sources = Vec::new();
    let mut seen_sources = std::collections::HashSet::new();
    let mut push_source = |source: models::PackageSource| {
        let key = format!(
            "{}:{}:{}",
            source.source_type,
            source.id,
            source.package_name.clone().unwrap_or_default()
        );
        if seen_sources.insert(key) {
            normalized_sources.push(source);
        }
    };

    if let Some(sources) = pkg.available_sources.take() {
        for source in sources {
            push_source(source);
        }
    }
    push_source(pkg.source.clone());
    pkg.available_sources = Some(normalized_sources);

    if let Some(primary) = pkg
        .available_sources
        .as_ref()
        .and_then(|sources| {
            sources.iter().find(|source| {
                source.source_type == pkg.source.source_type
                    && source.id == pkg.source.id
                    && source.package_name == pkg.source.package_name
            })
        })
        .cloned()
    {
        pkg.source = primary;
    } else if let Some(primary) = pkg
        .available_sources
        .as_ref()
        .and_then(|sources| sources.first())
        .cloned()
    {
        pkg.source = primary;
    }

    if let Some(existing) = pkg.installed_sources.as_mut() {
        existing.retain(|value| !value.trim().is_empty());
        existing.sort();
        existing.dedup();
        if existing.is_empty() {
            pkg.installed_sources = None;
        }
    }

    if pkg
        .installed_sources
        .as_ref()
        .map(|sources| !sources.is_empty())
        .unwrap_or(false)
    {
        pkg.installed = true;
    }

    if pkg.installed && pkg.installed_sources.is_none() {
        let installed_name = pkg
            .source
            .package_name
            .clone()
            .or_else(|| pkg.app_id.clone())
            .unwrap_or_else(|| pkg.name.clone());
        pkg.installed_sources = Some(vec![installed_name]);
    }

    if pkg.launch_target.as_ref().map(|value| value.trim().is_empty()).unwrap_or(true) {
        let fallback = if pkg.source.source_type == "flatpak" {
            pkg.app_id.clone().unwrap_or_else(|| pkg.name.clone())
        } else {
            pkg.source
                .package_name
                .clone()
                .unwrap_or_else(|| pkg.name.clone())
        };
        pkg.launch_target = Some(fallback);
    }

    #[cfg(debug_assertions)]
    {
        if pkg.canonical_id.trim().is_empty() {
            log::error!(
                "[IRON-CORE] package left finalizer without canonical_id: name={} app_id={:?}",
                pkg.name,
                pkg.app_id
            );
        }

        if let Some(sources) = &pkg.available_sources {
            let mut unique = std::collections::HashSet::new();
            for source in sources {
                let key = format!(
                    "{}:{}:{}",
                    source.source_type,
                    source.id,
                    source.package_name.clone().unwrap_or_default()
                );
                if !unique.insert(key) {
                    log::warn!(
                        "[IRON-CORE] duplicate source slot for canonical_id={}",
                        pkg.canonical_id
                    );
                }
            }
            let source_present = sources.iter().any(|source| {
                source.source_type == pkg.source.source_type
                    && source.id == pkg.source.id
                    && source.package_name == pkg.source.package_name
            });
            if !source_present {
                log::warn!(
                    "[IRON-CORE] primary source missing from available_sources for canonical_id={}",
                    pkg.canonical_id
                );
            }
        }
    }
}

pub fn finalize_packages_contract(packages: &mut [models::Package]) {
    for pkg in packages {
        finalize_package_contract(pkg);
    }
}

fn canonical_merge_key_raw(name: &str, app_id: Option<&str>) -> String {
    // 1. Prioritize AppID if it exists and looks like a Reverse Domain Name (RDN)
    if let Some(id) = app_id {
        let id_trim = id.trim();
        if id_trim.contains('.') {
            if let Some(canonical) = known_app_id_to_canonical(id_trim) {
                return canonical.to_string();
            }
            if let Some(tail) = id_trim
                .strip_suffix(".desktop")
                .unwrap_or(id_trim)
                .split('.')
                .next_back()
            {
                let mut t = tail.trim().to_lowercase();
                // If tail is too generic (desktop, git, bin) or too short, move back one segment
                let is_generic = matches!(
                    t.as_str(),
                    "desktop" | "git" | "bin" | "stable"
                );
                if t.len() < 3 || is_generic {
                    let segments: Vec<&str> = id_trim
                        .strip_suffix(".desktop")
                        .unwrap_or(id_trim)
                        .split('.')
                        .collect();
                    if segments.len() > 1 {
                        t = segments[segments.len() - 2].to_lowercase();
                    }
                }
                if !t.is_empty() {
                    return t;
                }
            }
        }
    }

    // 2. If name itself looks like an App ID (e.g. com.discordapp.Discord), resolve so repo and Flatpak merge
    let name_trim = name.trim();
    if name_trim.contains('.')
        && name_trim
            .find('.')
            .is_some_and(|i| i > 0 && i < name_trim.len() - 1)
    {
        if let Some(canonical) = known_app_id_to_canonical(name_trim) {
            return canonical.to_string();
        }
    }

    // 3. Fallback to package name with packaging suffix stripping only.
    // Channel/release variants (beta/canary/nightly/dev/ptb/esr/developer-edition/insider)
    // are preserved as distinct product identities.
    let mut clean_name = name_trim.to_lowercase();
    let variant_suffixes = [
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
        for suffix in &variant_suffixes {
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

/// Deduplicates packages by canonical_merge_key and merges available_sources.
/// Use after merge_search_results so the same app (e.g. Discord from AUR + Flatpak) never appears twice.
/// Prefers keeping the package that has available_sources (unified card) so the UI shows a friendly name and dropdown.
pub fn deduplicate_by_canonical_key(packages: Vec<models::Package>) -> Vec<models::Package> {
    use std::collections::HashMap;
    let mut by_key: HashMap<String, models::Package> = HashMap::new();
    for pkg in packages {
        let key = canonical_merge_key(&pkg.name, pkg.app_id.as_deref());
        if let Some(existing) = by_key.get_mut(&key) {
            // Merge available_sources from both
            let incoming_sources = pkg
                .available_sources
                .clone()
                .unwrap_or_else(|| vec![pkg.source.clone()]);
            let existing_sources = existing.available_sources.get_or_insert_with(Vec::new);

            // Ensure existing's primary source is also in its list if it was empty
            if existing_sources.is_empty() {
                existing_sources.push(existing.source.clone());
            }

            for s in incoming_sources {
                if !existing_sources.iter().any(|e| {
                    e.id == s.id
                        && e.source_type == s.source_type
                        && e.package_name == s.package_name
                }) {
                    existing_sources.push(s.clone());
                } else if let Some(existing_src) = existing_sources.iter_mut().find(|e| {
                    e.id == s.id
                        && e.source_type == s.source_type
                        && e.package_name == s.package_name
                }) {
                    // Same source slot: keep the newest version.
                    if s.version > existing_src.version {
                        *existing_src = s.clone();
                    }
                }
            }
            // Prefer friendly name/display_name from the unified entry (often has available_sources)
            let new_has_sources = pkg.available_sources.as_ref().map_or(0, |v| v.len()) > 0;
            let existing_has_sources =
                existing.available_sources.as_ref().map_or(0, |v| v.len()) > 0;
            if new_has_sources && !existing_has_sources {
                existing.name = pkg.name;
                existing.display_name = pkg.display_name.or(existing.display_name.clone());
            } else {
                if existing.display_name.is_none() && pkg.display_name.is_some() {
                    existing.display_name = pkg.display_name.clone();
                } else if let (Some(ref ex), Some(ref inc)) =
                    (&existing.display_name, &pkg.display_name)
                {
                    if inc.len() > ex.len() {
                        existing.display_name = pkg.display_name.clone();
                    }
                }
                if existing.name.contains('.') && !pkg.name.contains('.') {
                    existing.name = pkg.name;
                }
            }
            if existing.app_id.is_none() && pkg.app_id.is_some() {
                existing.app_id = pkg.app_id;
            }
            if existing.is_featured.is_none() && pkg.is_featured == Some(true) {
                existing.is_featured = Some(true);
            }
        } else {
            let mut pkg = pkg;
            pkg.canonical_id = key.clone();
            by_key.insert(key, pkg);
        }
    }
    // One proper name per app: prefer known full name for this canonical key, else pretty name from pkg.name
    by_key
        .into_values()
        .map(|mut pkg| {
            let preferred = preferred_display_name(&pkg.canonical_id);
            if let Some(full) = preferred {
                pkg.display_name = Some(String::from(full));
            } else if pkg.display_name.as_ref().is_none_or(|s| s.is_empty())
                && !pkg.name.contains('.')
            {
                pkg.display_name = Some(to_pretty_name(&pkg.name));
            }
            pkg
        })
        .collect()
}

/// Merges official/appstream packages with repository packages, handling deduplication.
/// This logic was extracted from lib.rs to allow for unit testing.
#[allow(dead_code)]
pub fn merge_and_deduplicate(
    mut base_packages: Vec<models::Package>,
    repo_results: Vec<models::Package>,
) -> Vec<models::Package> {
    // Track seen App IDs or Normalized Names to prevent duplicates
    let mut grouping_map: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for (i, p) in base_packages.iter().enumerate() {
        if let Some(id) = &p.app_id {
            grouping_map.insert(id.clone(), i);
        } else {
            // Fallback: use normalized name
            grouping_map.insert(strip_package_suffix(&p.name).to_string(), i);
        }
    }

    for mut pkg in repo_results {
        // 1. Check Exact Name Match
        if let Some(idx) = base_packages.iter().position(|p| p.name == pkg.name) {
            // Merge logic...
            if pkg.source.priority() < base_packages[idx].source.priority() {
                let mut old_primary = std::mem::replace(&mut base_packages[idx], pkg);
                let alternatives = old_primary.alternatives.take().unwrap_or_default();
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .extend(alternatives);
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .push(old_primary);
            } else {
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .push(pkg);
            }
            continue;
        }

        // 2. Check Grouping Match (App ID or Normalized Name)
        let group_key = pkg
            .app_id
            .clone()
            .unwrap_or_else(|| strip_package_suffix(&pkg.name).to_string());

        if let Some(&idx) = grouping_map.get(&group_key) {
            // Priority Swap Logic
            if pkg.source.priority() < base_packages[idx].source.priority() {
                let mut old_primary = std::mem::replace(&mut base_packages[idx], pkg);
                let alternatives = old_primary.alternatives.take().unwrap_or_default();
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .extend(alternatives);
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .push(old_primary);
            } else {
                base_packages[idx]
                    .alternatives
                    .get_or_insert_with(Vec::new)
                    .push(pkg);
            }
            continue;
        }

        // 3. New Entry
        pkg.display_name = Some(to_pretty_name(&pkg.name));
        pkg.alternatives = None;
        grouping_map.insert(group_key, base_packages.len());
        base_packages.push(pkg);
    }

    base_packages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Package, PackageSource};

    // Need to make sort_packages_by_relevance pub(crate) or use public API?
    // It's pub in utils.rs, so super::* should cover it.
    // Wait, check if utils.rs has it as pub.

    fn make_pkg(name: &str, source: PackageSource, votes: Option<u32>) -> Package {
        Package {
            name: name.to_string(),
            display_name: None,
            description: "".to_string(),
            version: "1.0".to_string(),
            source,
            maintainer: None,
            license: None,
            url: None,
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: votes,
            icon: None,
            screenshots: None,
            provides: None,
            app_id: None,
            is_optimized: None,
            depends: None,
            make_depends: None,
            is_featured: None,
            installed: false,
            ..Default::default()
        }
    }

    #[test]
    fn test_canonical_merge_key_variants() {
        // Packaging variants map to same canonical key for merge deduplication.
        assert_eq!(canonical_merge_key("firefox", None), "firefox");
        assert_eq!(canonical_merge_key("firefox-bin", None), "firefox");
        assert_eq!(canonical_merge_key("firefox-git", None), "firefox");

        // Channel/release variants are distinct products.
        assert_eq!(
            canonical_merge_key("firefox-developer-edition", None),
            "firefoxdeveloperedition"
        );
        assert_eq!(canonical_merge_key("firefox-esr", None), "firefoxesr");
        assert_eq!(canonical_merge_key("discord-canary", None), "discordcanary");
        assert_eq!(canonical_merge_key("discord-ptb", None), "discordptb");
        assert_eq!(canonical_merge_key("google-chrome-canary", None), "googlechromecanary");

        assert_eq!(canonical_merge_key("brave-bin", None), "brave");
        assert_eq!(
            canonical_merge_key("visual-studio-code-bin", None),
            "visualstudiocode"
        );

        // Fix for Zettlr/Desktop collision: .desktop suffix should be ignored when app_id looks like a filename
        assert_eq!(canonical_merge_key("zettlr.desktop", None), "zettlr");
        assert_eq!(
            canonical_merge_key("org.foo.bar.desktop", None),
            "orgfoobar"
        );

        // App ID takes precedence (reverse-DNS last segment)
        assert_eq!(
            canonical_merge_key("firefox", Some("org.mozilla.firefox")),
            "firefox"
        );
        assert_eq!(
            canonical_merge_key("Firefox", Some("org.mozilla.firefox")),
            "firefox"
        );

        // Stable cross-source merge.
        assert_eq!(
            canonical_merge_key("OBS Studio", Some("com.obsproject.Studio")),
            "obsstudio"
        );
        assert_eq!(canonical_merge_key("obs-studio", None), "obsstudio");

        // "heroic-games-launcher" keeps full identity.
        assert_eq!(canonical_merge_key("heroic", None), "heroic");
        assert_eq!(
            canonical_merge_key("heroic-games-launcher-bin", None),
            "heroicgameslauncher"
        );
        assert_eq!(
            canonical_merge_key("Heroic Game Launcher", Some("com.heroicgameslauncher.hgl")),
            "heroicgameslauncher"
        );

        // Name-as-app-id: repo package named "com.discordapp.Discord" (e.g. from metadata) merges with Flatpak
        assert_eq!(
            canonical_merge_key("com.discordapp.Discord", None),
            "discord"
        );
        assert_eq!(
            canonical_merge_key("Discord", Some("com.discordapp.Discord")),
            "discord"
        );

        assert_eq!(
            canonical_merge_key("librewolf", Some("io.gitlab.librewolf-community")),
            "librewolf"
        );
        assert_eq!(
            canonical_merge_key("io.gitlab.librewolf-community", None),
            "librewolf"
        );
    }

    #[test]
    fn test_finalize_package_contract_normalizes_identity_and_sources() {
        let mut pkg = Package {
            name: "firefox-bin".to_string(),
            app_id: Some(" org.mozilla.firefox ".to_string()),
            canonical_id: "".to_string(),
            source: PackageSource::new("repo", "extra", "1.0", "Arch Official"),
            available_sources: Some(vec![
                PackageSource::new("repo", "extra", "1.0", "Arch Official"),
                PackageSource::new("repo", "extra", "1.0", "Arch Official"),
            ]),
            installed: true,
            installed_sources: None,
            launch_target: None,
            description: String::new(),
            version: "1.0".to_string(),
            display_name: None,
            maintainer: None,
            license: None,
            url: None,
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: None,
            icon: None,
            screenshots: None,
            provides: None,
            is_optimized: None,
            depends: None,
            make_depends: None,
            is_featured: None,
            ..Default::default()
        };

        finalize_package_contract(&mut pkg);

        assert_eq!(pkg.canonical_id, "firefox");
        assert_eq!(pkg.app_id.as_deref(), Some("org.mozilla.firefox"));
        assert_eq!(pkg.available_sources.as_ref().map(|s| s.len()), Some(1));
        assert_eq!(pkg.installed_sources.as_ref().map(|s| s.len()), Some(1));
        assert_eq!(pkg.launch_target.as_deref(), Some("firefox-bin"));
    }

    #[test]
    fn test_finalize_package_contract_preserves_flatpak_launch_target() {
        let mut pkg = Package {
            name: "io.gitlab.librewolf-community".to_string(),
            app_id: Some("io.gitlab.librewolf-community".to_string()),
            source: PackageSource::new("flatpak", "flathub", "1.0", "Flatpak"),
            description: String::new(),
            version: "1.0".to_string(),
            display_name: None,
            maintainer: None,
            license: None,
            url: None,
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: None,
            icon: None,
            screenshots: None,
            provides: None,
            is_optimized: None,
            depends: None,
            make_depends: None,
            is_featured: None,
            installed: true,
            ..Default::default()
        };

        finalize_package_contract(&mut pkg);

        assert_eq!(pkg.canonical_id, "librewolf");
        assert_eq!(pkg.launch_target.as_deref(), Some("io.gitlab.librewolf-community"));
    }

    #[test]
    fn test_deduplication_priority_swap() {
        // Manjaro (distro-native repo, highest priority in this pair)
        let manjaro = make_pkg("spotify", PackageSource::manjaro("spotify"), None);
        // Chaotic (Lower priority than distro-native repos)
        let chaotic = make_pkg("spotify", PackageSource::chaotic("spotify"), None);

        let results = merge_and_deduplicate(vec![manjaro], vec![chaotic]);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source.source_type, "repo");
        assert_eq!(results[0].source.id, "manjaro");
        assert_eq!(results[0].alternatives.as_ref().unwrap().len(), 1);
        assert_eq!(
            results[0].alternatives.as_ref().unwrap()[0].source,
            PackageSource::chaotic("spotify")
        );
    }

    #[test]
    fn test_to_pretty_name() {
        assert_eq!(to_pretty_name("discord"), "Discord");
        assert_eq!(to_pretty_name("visual-studio-code"), "Visual Studio Code");
        // Test RDN logic
        assert_eq!(to_pretty_name("com.discordapp.Discord"), "Discord");
        assert_eq!(to_pretty_name("org.mozilla.firefox"), "Firefox");
        assert_eq!(to_pretty_name("io.github.spacingbat3.webcord"), "Webcord");
        // Test mixed
        assert_eq!(to_pretty_name("com.example.my-cool-app"), "My Cool App");
    }
}

pub async fn run_privileged_script(
    script: &str,
    password: Option<String>,
    bypass_helper: bool,
) -> Result<String, String> {
    let wrapper_path = "/usr/lib/monarch-store/monarch-wrapper";
    let wrapper_exists = std::path::Path::new(wrapper_path).exists();
    let helper_exists = std::path::Path::new(MONARCH_PK_HELPER).exists();

    // Acquire global lock to serialize privileged prompts
    let _guard = PRIVILEGED_LOCK.lock().await;

    let (program, args) = if password.is_some() {
        ("sudo", vec!["-S", "bash", "-s"])
    } else if wrapper_exists && !bypass_helper {
        // Use wrapper so Polkit action com.monarch.store.script applies; DE agent = once-per-session.
        (
            "pkexec",
            vec!["--disable-internal-agent", wrapper_path, "bash", "-s"],
        )
    } else if helper_exists && !bypass_helper {
        (
            "pkexec",
            vec!["--disable-internal-agent", MONARCH_PK_HELPER, "bash", "-s"],
        )
    } else {
        (
            "pkexec",
            vec!["--disable-internal-agent", "/bin/bash", "-s"],
        )
    };

    let mut child = tokio::process::Command::new(program)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", program, e))?;

    if let Some(mut stdin) = child.stdin.take() {
        if let Some(pwd) = &password {
            let _ = stdin.write_all(format!("{}\n", pwd).as_bytes()).await;
        }
        let _ = stdin.write_all(script.as_bytes()).await;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("Failed to wait on {}: {}", program, e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "Privileged Action Failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub enum InstallMode {
    System,
    Portable,
    Dev, // Useful for debugging
}

pub fn get_install_mode() -> InstallMode {
    if let Ok(exe_path) = std::env::current_exe() {
        let path_str = exe_path.to_string_lossy();

        // 1. System Install (Pacman)
        // Usually /usr/bin/monarch-store
        if path_str.starts_with("/usr/bin") || path_str.starts_with("/bin") {
            return InstallMode::System;
        }

        // 2. AppImage (Mounted)
        // Usually /tmp/.mount_monarcXXXXXX/usr/bin/monarch-store or similar
        // BUT the actual AppImage *file* is what we care about updates for.
        // However, we just need to know "Are we managed by Pacman?".
        // If we are NOT in /usr/bin, we are likely portable or dev.

        // 3. Dev Mode
        if path_str.contains("/target/debug/") || path_str.contains("/target/release/") {
            return InstallMode::Dev;
        }
    }

    // Default to Portable for AppImages, manual builds in /home, etc.
    InstallMode::Portable
}

/// Maps event name to a category for Aptabase dashboard segmentation (filter/group by event_category).
fn event_category_and_label(event: &str) -> (&'static str, &'static str) {
    match event {
        "app_started" => ("lifecycle", "App started"),
        "store_installed" => ("lifecycle", "Store installed"),
        "search" | "search_query" => ("search", "Search"),
        "onboarding_completed" => ("engagement", "Onboarding completed"),
        "review_submitted" => ("engagement", "Review submitted"),
        "install_package" => ("install", "Package installed"),
        "uninstall_package" => ("install", "Package uninstalled"),
        "error_reported" => ("error", "Error reported"),
        "panic" => ("error", "App panic"),
        _ => ("other", "other"),
    }
}

/// Safely tracks an event ONLY if telemetry is enabled in configuration.
/// Injects event_category and event_label into every payload so Aptabase dashboard can segment
/// and display each event type as its own box with richer filtering.
pub async fn track_event_safe(
    app: &tauri::AppHandle,
    event: &str,
    payload: Option<serde_json::Value>,
) {
    use crate::repo_manager::RepoManager;
    use serde_json::Value;
    use tauri::Manager;
    use tauri_plugin_aptabase::EventTracker;

    let state = app.state::<RepoManager>();
    if state.is_telemetry_enabled().await {
        let (category, label) = event_category_and_label(event);
        let mut map: serde_json::Map<String, Value> = match payload.as_ref() {
            Some(Value::Object(m)) => m.clone(),
            _ => serde_json::Map::new(),
        };
        map.insert(
            "event_category".to_string(),
            Value::String(category.to_string()),
        );
        map.insert("event_label".to_string(), Value::String(label.to_string()));
        let enriched = Value::Object(map);

        #[cfg(debug_assertions)]
        log::debug!("Telemetry sending: {} {:?}", event, enriched);

        let _ = app.track_event(event, Some(enriched));
    } else {
        #[cfg(debug_assertions)]
        log::debug!("Telemetry blocked (consent denied): {}", event);
    }
}
pub async fn run_pacman_command_transparent(
    app: tauri::AppHandle,
    action_args: Vec<String>,
    password: Option<String>,
) -> Result<(), String> {
    use crate::distro_context::DistroContext;
    use crate::distro_context::DistroId;
    use crate::error_classifier::ClassifiedError;
    use std::sync::Arc;
    use tauri::Emitter;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::sync::Mutex;

    // 1. Manjaro Safety Guard (Protocol v0.3.5 Phase 4)
    let distro = DistroContext::new();
    if distro.id == DistroId::Manjaro {
        let has_sy = action_args
            .iter()
            .any(|a| a.contains("-Sy") || a.contains("-Syy"));
        let has_u = action_args.iter().any(|a| a.contains("u"));
        if has_sy && !has_u {
            let msg = "Manjaro Stability Guard: Partial upgrades (-Sy without -u) are blocked to prevent system breakage.".to_string();
            let _ = app.emit("install-output", &msg);
            return Err(msg);
        }
    }

    // 2. Build the command
    let (binary, args) = crate::commands::cmd_helpers::build_pacman_cmd(
        &action_args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &password,
    );

    // Acquire global lock to serialize privileged prompts
    let _guard = PRIVILEGED_LOCK.lock().await;

    let mut child = tokio::process::Command::new(binary)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn pacman command: {}", e))?;

    // 3. Handle password if using sudo
    if let Some(pwd) = password {
        if let Some(mut s) = child.stdin.take() {
            if let Err(e) =
                tokio::io::AsyncWriteExt::write_all(&mut s, format!("{}\n", pwd).as_bytes()).await
            {
                let err_msg = if e.kind() == std::io::ErrorKind::BrokenPipe {
                    "Authentication failed or cancelled (Broken pipe).".to_string()
                } else {
                    format!("Failed to write password to stdin: {}", e)
                };
                return Err(err_msg);
            }
            let _ = tokio::io::AsyncWriteExt::flush(&mut s).await;
            let _ = tokio::io::AsyncWriteExt::shutdown(&mut s).await;
        }
    }

    // 4. Stream Output with Error Collection
    // Collect stderr for error classification
    let error_buffer: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let app_clone = app.clone();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture stdout (was not piped)".to_string())?;
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = app_clone.emit("install-output", line);
        }
    });

    let app_clone = app.clone();
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture stderr (was not piped)".to_string())?;
    let error_buffer_clone = error_buffer.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            // Store for later classification
            {
                let mut buf = error_buffer_clone.lock().await;
                buf.push(line.clone());
            }
            let _ = app_clone.emit("install-output", format!("ERROR: {}", line));
        }
    });

    // 5. Wait for completion
    let status = child.wait().await.map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        // 6. Classify the error and emit structured event for UI recovery actions
        let errors = error_buffer.lock().await;
        let combined_output = errors.join("\n");

        if let Some(classified) = ClassifiedError::from_output(&combined_output) {
            // Emit structured error event for the UI to show recovery options
            let _ = app.emit("install-error-classified", &classified);
            Err(format!("{}: {}", classified.title, classified.description))
        } else {
            Err("Pacman operation failed. Check logs for details.".to_string())
        }
    }
}

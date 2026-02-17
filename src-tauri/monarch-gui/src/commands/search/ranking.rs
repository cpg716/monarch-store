use crate::{metadata, models::Package};

/// Open Core & Proprietary Fork siblings: proprietary/paid builds vs open-source base or community fork.
/// When the user searches for any of the terms, all packages in the group get a relevance boost
/// so both show at the top (e.g. Chrome + Chromium, VS Code + VSCodium, Plex + Kodi).
/// Model: Google Chrome (Proprietary) vs Chromium (Open Source); wrappers/builds (VS Code → VSCodium).
pub const SIBLING_GROUPS: &[(&[&str], &[&str])] = &[
    // (package names / display variants in group, search terms that trigger boost)
    // --- Web Browsers (Chromium-based: proprietary on open Chromium) ---
    (
        &["google-chrome", "chromium", "google chrome"],
        &["chrome", "chromium"],
    ),
    (
        &[
            "microsoft-edge",
            "microsoft-edge-dev",
            "edge-bin",
            "microsoft edge",
        ],
        &["edge", "chromium"],
    ),
    (
        &["vivaldi", "vivaldi-bin", "vivaldi-stable"],
        &["vivaldi", "chromium"],
    ),
    (
        &["opera", "opera-ffmpeg-codecs", "opera-developer"],
        &["opera", "chromium"],
    ),
    (
        &["brave", "brave-bin", "brave-browser"],
        &["brave", "chromium"],
    ),
    (
        &["yandex-browser", "yandex-browser-beta", "chromium"],
        &["yandex", "chromium"],
    ),
    (
        &[
            "firefox",
            "librewolf",
            "firefox-esr",
            "firefox-developer-edition",
        ],
        &["firefox", "librewolf"],
    ),
    (
        &["tor-browser", "tor-browser-launcher"],
        &["tor", "tor browser"],
    ),
    // --- Development & IDEs (Open Core: Pro/Ultimate vs Community/Open build) ---
    (
        &[
            "code",
            "visual-studio-code-bin",
            "vscodium",
            "vscodium-bin",
            "visual studio code",
        ],
        &["code", "vscode", "vscodium", "visual studio code"],
    ),
    (
        &[
            "pycharm-professional",
            "pycharm-community-edition",
            "pycharm",
        ],
        &["pycharm", "py charm"],
    ),
    (
        &[
            "intellij-idea-ultimate-edition",
            "intellij-idea-community-edition",
            "intellij",
        ],
        &["intellij", "idea", "jetbrains"],
    ),
    (
        &["android-studio", "android-sdk", "android-tools"],
        &["android studio", "android sdk"],
    ),
    (
        &[
            "jdk",
            "jdk-openjdk",
            "jdk8-openjdk",
            "jdk11-openjdk",
            "jdk17-openjdk",
            "openjdk",
        ],
        &["jdk", "openjdk", "java"],
    ),
    (
        &["qt6-base", "qt5-base", "qt-creator", "qt6-creator"],
        &["qt", "qt creator"],
    ),
    (
        &[
            "sublime-text",
            "sublime-text-4",
            "sublime-merge",
            "atom",
            "pulsar-editor-bin",
            "pulsar",
            "lime",
        ],
        &["sublime", "atom", "pulsar", "lime", "sublime text"],
    ),
    (&["cursor", "cursor-bin", "cursor-appimage"], &["cursor"]),
    // --- DevOps & Infrastructure (Open Core: EE vs CE, proprietary vs engine/OSS) ---
    (&["gitlab", "gitlab-ce", "gitlab-ee"], &["gitlab"]),
    (
        &["docker", "docker-desktop", "podman", "docker-engine"],
        &["docker", "podman", "container"],
    ),
    (&["nginx", "nginx-mainline", "nginx-plus"], &["nginx"]),
    (
        &["redis", "redis-server", "valkey", "valkey-bin"],
        &["redis", "valkey"],
    ),
    (
        &["elasticsearch", "opensearch", "opensearch-dashboards"],
        &["elasticsearch", "opensearch", "elastic search"],
    ),
    (
        &["terraform", "opentofu", "tofu", "terraform-bin"],
        &["terraform", "opentofu", "tofu"],
    ),
    (
        &["mysql", "mariadb", "mysql57", "mysql80"],
        &["mysql", "mariadb"],
    ),
    (&["influxdb", "influxdb2", "influxdb-bin"], &["influxdb"]),
    (&["grafana", "grafana-bin"], &["grafana"]),
    (&["sonarqube", "sonar-scanner"], &["sonarqube", "sonar"]),
    (
        &["ansible", "ansible-core", "ansible-automation-platform"],
        &["ansible"],
    ),
    (
        &["puppet", "puppet-agent", "puppet-enterprise"],
        &["puppet"],
    ),
    (&["chef", "chef-workstation", "chef-infra"], &["chef"]),
    (&["metasploit", "metasploit-framework"], &["metasploit"]),
    // --- Communication & Collaboration (Open Core / proprietary server vs OSS client) ---
    (
        &["mattermost", "mattermost-desktop", "mattermost-server"],
        &["mattermost"],
    ),
    (
        &["rocket-chat", "rocket.chat", "rocketchat"],
        &["rocket", "rocket.chat"],
    ),
    (&["zulip", "zulip-desktop", "zulip-server"], &["zulip"]),
    (&["telegram-desktop", "telegram"], &["telegram"]),
    (
        &[
            "discord",
            "discord-canary",
            "discord-ptb",
            "webcord",
            "webcord-bin",
            "discord_arch_electron",
        ],
        &["discord", "webcord"],
    ),
    (
        &["slack", "slack-desktop", "mattermost"],
        &["slack", "mattermost"],
    ),
    (
        &["element-desktop", "element-web", "riot-desktop"],
        &["element", "matrix", "riot"],
    ),
    (&["signal-desktop", "signal-desktop-beta"], &["signal"]),
    (&["zoom", "zoom-wayland"], &["zoom"]),
    (&["teams", "teams-for-linux"], &["teams", "microsoft teams"]),
    (
        &["thunderbird", "betterbird"],
        &["thunderbird", "betterbird", "mail"],
    ),
    // --- Media & Streaming (Proprietary fork vs open source: Plex/Kodi, Emby/Jellyfin) ---
    (&["plex", "plex-media-server", "plexamp"], &["plex"]),
    (&["kodi", "kodi-git", "xbmc"], &["kodi", "xbmc", "plex"]),
    (&["emby", "emby-server", "emby-server-bin"], &["emby"]),
    (
        &["jellyfin", "jellyfin-server", "jellyfin-media-player"],
        &["jellyfin", "emby"],
    ),
    (
        &[
            "spotify",
            "spotify-adblock",
            "spotifywm",
            "spotify-launcher",
        ],
        &["spotify"],
    ),
    (
        &["vlc", "mpv", "mpv-mpris"],
        &["vlc", "mpv", "video player", "media player"],
    ),
    (&["obs-studio", "obs-studio-git"], &["obs", "streaming"]),
    (
        &["davinci-resolve", "davinci-resolve-studio"],
        &["davinci", "resolve"],
    ),
    (&["lightworks", "lightworks-bin"], &["lightworks"]),
    (
        &["kdenlive", "kdenlive-git", "shotcut"],
        &["kdenlive", "shotcut", "video editor"],
    ),
    (
        &["audacity", "tenacity", "audacity-git"],
        &["audacity", "tenacity", "audio editor"],
    ),
    (
        &["handbrake", "handbrake-cli"],
        &["handbrake", "video encoder"],
    ),
    (
        &["gimp", "gimp-git", "krita"],
        &["gimp", "krita", "image editor"],
    ),
    (&["inkscape", "inkscape-git"], &["inkscape", "vector"]),
    (&["blender", "blender-docs"], &["blender", "3d"]),
    (
        &["ardour", "reaper", "reaper-bin"],
        &["ardour", "reaper", "daw", "audio"],
    ),
    // --- Productivity & Office (Open Core: Enterprise vs Community/Desktop) ---
    (
        &[
            "onlyoffice",
            "onlyoffice-bin",
            "onlyoffice-desktopeditors",
            "onlyoffice-documentserver",
        ],
        &["onlyoffice", "only office"],
    ),
    (
        &[
            "libreoffice-fresh",
            "libreoffice-still",
            "libreoffice",
            "openoffice",
            "openoffice-bin",
        ],
        &["libreoffice", "openoffice", "libre office"],
    ),
    (&["wps-office", "wps-office-bin"], &["wps", "wps office"]),
    (&["zimbra", "zimbra-desktop"], &["zimbra"]),
    (
        &["odoo", "odoo-community", "odoo-enterprise"],
        &["odoo", "erp"],
    ),
    (&["magento", "magento-open-source"], &["magento"]),
    (&["sugarcrm", "suitecrm"], &["sugarcrm", "suitecrm", "crm"]),
    (
        &["bitwarden", "bitwarden-cli", "vaultwarden"],
        &["bitwarden", "vaultwarden", "password"],
    ),
    (
        &["standard-notes", "standard-notes-bin"],
        &["standard notes", "standardnotes"],
    ),
    (
        &["evince", "okular", "atril", "xreader"],
        &["pdf", "evince", "okular", "document viewer", "pdf viewer"],
    ),
    (&["zotero", "zotero-bin"], &["zotero", "reference"]),
    (
        &["notion", "notion-app", "obsidian", "obsidian-bin"],
        &["notion", "obsidian", "notes"],
    ),
    // --- Security & Virtualization (Open Core / base vs proprietary extension) ---
    (
        &[
            "virtualbox",
            "virtualbox-host-dkms",
            "virtualbox-ext-oracle",
            "virt-manager",
        ],
        &["virtualbox", "virtual machine", "virt"],
    ),
    (
        &["pfsense", "opnsense", "opnsense-update"],
        &["pfsense", "opnsense", "firewall"],
    ),
    (&["vmware-workstation", "vmware-player"], &["vmware"]),
    (
        &["teamviewer", "anydesk", "rustdesk", "rustdesk-bin"],
        &["teamviewer", "anydesk", "rustdesk", "remote"],
    ),
    (
        &["veracrypt", "truecrypt", "cryptsetup"],
        &["veracrypt", "truecrypt", "encryption"],
    ),
    (
        &["keepassxc", "keepass", "keepassxc-git"],
        &["keepass", "keepassxc", "password"],
    ),
    // --- Gaming / launchers ---
    (
        &["steam", "steam-native-runtime", "steam-manjaro"],
        &["steam"],
    ),
    (&["lutris", "lutris-ge"], &["lutris", "gaming"]),
    (
        &["heroic", "heroic-games-launcher-bin"],
        &["heroic", "epic", "games launcher"],
    ),
    (
        &[
            "minecraft",
            "minecraft-launcher",
            "prism-launcher",
            "poly mc",
        ],
        &["minecraft", "prism", "polymc"],
    ),
    // --- Other (dev tools, git clients, file transfer) ---
    (
        &["gitkraken", "gitg", "sourcetree", "git-cola", "lazygit"],
        &["gitkraken", "git gui", "sourcetree", "lazygit"],
    ),
    (
        &["postman", "postman-bin", "insomnia"],
        &["postman", "insomnia", "api"],
    ),
    (
        &["qbittorrent", "transmission-gtk", "deluge", "transmission"],
        &["qbittorrent", "transmission", "deluge", "torrent"],
    ),
    (&["filezilla", "filezilla-gtk3"], &["filezilla", "ftp"]),
];

pub const SIBLING_BOOST_SCORE: u32 = 85;

pub fn calculate_relevance(
    pkg: &Package,
    query: &str,
    metadata: &metadata::AppStreamLoader,
    popular_apps: &[String],
) -> u32 {
    let pkg_name_lower = pkg.name.to_lowercase();
    let display_name_lower = pkg.display_name.as_ref().map(|s| s.to_lowercase());
    let friendly_name = metadata
        .get_friendly_name(&pkg.name)
        .map(|s| s.to_lowercase());

    let matches_query = pkg_name_lower.contains(query)
        || display_name_lower.as_deref().unwrap_or("").contains(query)
        || friendly_name.as_deref().unwrap_or("").contains(query)
        || pkg
            .keywords
            .as_ref()
            .map(|k| k.iter().any(|w| w.to_lowercase().contains(query)))
            .unwrap_or(false);

    let is_popular = popular_apps.contains(&pkg_name_lower);

    let mut score = 0u32;

    // 1. Exact Name Match (Score 100)
    if pkg_name_lower == query {
        score = 100;
    }
    // 2. Exact Friendly Name Match (Score 100)
    else if let Some(friendly) = &friendly_name {
        if friendly == query {
            score = 100;
        } else if friendly.contains(query) && query.len() >= 3 {
            // 3. Partial Friendly Match (Score 80)
            score = 80;
        }
    }

    // 4. Exact App ID Match (Score 90)
    if score == 0 {
        if let Some(app_id) = &pkg.app_id {
            if app_id.to_lowercase() == query {
                score = 90;
            }
        }
    }

    // 5. Starts with Query (Score 50)
    if score == 0 && pkg_name_lower.starts_with(query) {
        score = 50;
    }

    // 6. Contains Query (Score 20)
    if score == 0 && matches_query {
        score = 20;
    }

    // 7. Popularity Bonus: +10 when in popular list and matches query
    if is_popular && matches_query && score > 0 {
        score = (score + 10).min(100);
    }

    // 8. Sibling boost: when query matches a proprietary/open pair (e.g. chrome/chromium),
    //    boost both so they show at top regardless of exact name match
    let display_lower = display_name_lower.as_deref().unwrap_or("");
    for (names, terms) in SIBLING_GROUPS {
        let in_group = names
            .iter()
            .any(|n| *n == pkg_name_lower || *n == display_lower);
        let query_matches = terms
            .iter()
            .any(|t| query == *t || query.contains(t) || t.contains(query));
        if in_group && query_matches {
            score = score.max(SIBLING_BOOST_SCORE);
        }
    }

    /* ----------------------------------------------------------------
       NEW: Word Match Boosting (Solves "google-chrome" vs "chrome")
       If the query appears as a distinct word in the name (split by -._),
       it's a much stronger match than a generic "contains" or even "starts with" on a long string.
    ---------------------------------------------------------------- */
    let delimiters = ['-', '_', '.', ' '];

    // Helper: Exact word match?
    let has_word = |text: &str| text.split(&delimiters[..]).any(|word| word == query);

    if score < 90 {
        if has_word(&pkg_name_lower) {
            // "google-chrome" contains word "chrome" -> Boost over "chrome-gnome-shell" (starts with)?
            // "chrome-gnome-shell" starts with "chrome" -> 50.
            // "google-chrome" word match "chrome" -> 60.
            score = score.max(60);
        }

        if has_word(display_lower) {
            // "Google Chrome" contains word "Chrome"
            score = score.max(55);
        }
    }

    /* ----------------------------------------------------------------
       NEW: Fallback Display Name Match
       If friendly_name was missing, but display_name is present (e.g. from AUR search),
       use it for strict matching.
    ---------------------------------------------------------------- */
    if score < 100 && friendly_name.is_none() {
        if display_lower == query {
            score = 95; // Almost perfect
        } else if display_lower.starts_with(query) {
            score = score.max(50);
        }
    }

    score
}

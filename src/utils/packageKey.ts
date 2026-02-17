
/**
 * Known App ID to Canonical Name mapping.
 * Ported from src-tauri/monarch-gui/src/utils.rs (known_app_id_to_canonical)
 */
const KNOWN_APP_ID_MAP: Record<string, string> = {
    // Browsers
    "com.google.chrome": "google-chrome",
    "org.mozilla.firefox": "firefox",
    "org.chromium.chromium": "chromium",
    "com.brave.browser": "brave",
    "com.microsoft.edge": "microsoft-edge",
    "com.vivaldi.vivaldi": "vivaldi",
    // Media / Comms
    "com.spotify.client": "spotify",
    "com.discordapp.discord": "discord",
    "com.discordapp.discordcanary": "discord-canary",
    "com.discordapp.discordptb": "discord-ptb",
    "io.github.spacingbat3.webcord": "webcord",
    "dev.vencord.vesktop": "vesktop",
    "org.telegram.desktop": "telegram-desktop",
    "org.signal.signal": "signal-desktop",
    "us.zoom.zoom": "zoom",
    "com.microsoft.teams": "teams",
    "com.slack.slack": "slack-desktop",
    "im.riot.riot": "element-desktop",
    "chat.zulip.zulip": "zulip-desktop",
    // Media Players / Editors
    "org.videolan.vlc": "vlc",
    "org.mpv.mpv": "mpv",
    "com.obsproject.studio": "obs-studio",
    "org.gimp.gimp": "gimp",
    "org.inkscape.inkscape": "inkscape",
    "org.blender.blender": "blender",
    "org.audacityteam.audacity": "audacity",
    "org.kde.kdenlive": "kdenlive",
    // Development
    "com.visualstudio.code": "visual-studio-code",
    "com.visualstudio.code-oss": "code",
    "com.jetbrains.intellij-idea-community": "intellij-idea-community-edition",
    "com.jetbrains.pycharm-community": "pycharm-community-edition",
    "com.jetbrains.toolbox": "jetbrains-toolbox",
    "com.sublimetext.three": "sublime-text-4",
    "com.getpostman.postman": "postman-bin",
    // Gaming / Office / Utils
    "com.valvesoftware.steam": "steam",
    "com.valvesoftware.steam.desktop": "steam",
    "net.lutris.lutris": "lutris",
    "net.lutris.lutris.desktop": "lutris",
    "com.heroicgameslauncher.hgl": "heroic-games-launcher",
    "com.mojang.minecraft": "minecraft-launcher",
    "org.libreoffice.libreoffice": "libreoffice",
    "org.onlyoffice.desktopeditors": "onlyoffice-bin",
    "com.bitwarden.desktop": "bitwarden",
    "org.keepassxc.keepassxc": "keepassxc",
    "org.mozilla.thunderbird": "thunderbird",
    "org.filezilla_project.filezilla": "filezilla",
    "org.qbittorrent.qbittorrent": "qbittorrent",
    "com.transmissionbt.transmission": "transmission-gtk",
    "org.virtualbox.virtualbox": "virtualbox",
};

const FIRST_SEGMENT_SKIP = ["lib", "lib32", "org", "com", "python", "perl", "php"];

const VARIANT_SUFFIXES = [
    "-bin", "-git", "-flatpak", "-official", "-repo", "-beta", "-nightly", "-stable",
    "-appimage", "-electron", "-developer-edition", "-esr", "-dev", "-wayland", "-x11",
    "-cn", "-fresh", "-still", "-native", "-runtime", "-lts", "-edge", ".desktop", "-desktop",
    "-hg", "-svn"
];

function knownAppIdToCanonical(appId: string): string | null {
    const id = appId.trim().toLowerCase();
    return KNOWN_APP_ID_MAP[id] || null;
}

function stripPackageSuffix(name: string): string {
    let cleanName = name.toLowerCase();
    while (true) {
        let changed = false;
        for (const suffix of VARIANT_SUFFIXES) {
            if (cleanName.endsWith(suffix)) {
                cleanName = cleanName.slice(0, -suffix.length);
                changed = true;
                break;
            }
        }
        if (!changed) break;
    }
    return cleanName;
}

function firstSegmentCanonical(nameWithSeparators: string): string | null {
    if (!nameWithSeparators.includes('-') && !nameWithSeparators.includes('_')) {
        return null;
    }
    const first = nameWithSeparators.split(/[-_]/)[0].trim().toLowerCase();
    const normalized = first.replace(/[-_]/g, '');

    if (normalized.length >= 3 && !FIRST_SEGMENT_SKIP.includes(normalized)) {
        return normalized;
    }
    return null;
}

/**
 * Generates the canonical merge key for a package using strict Rust-equivalent rules.
 * 
 * 1. Prioritize AppID if it exists and maps to a known canonical name.
 * 2. Fallback to package name with aggressive suffix stripping.
 * 3. Use first segment of multi-segment names (e.g. "heroic-games-launcher" -> "heroic").
 * 4. NORMALIZE: Retain ONLY alphanumeric characters (strip dots, hyphens, underscores).
 */
export function getPackageListKey(pkg: { canonical_id?: unknown; app_id?: string | null; name: string }): string {
    // 1. If backend already provided canonical_id, trust it (it uses the same logic).
    if (typeof pkg.canonical_id === 'string' && pkg.canonical_id.length > 0) {
        return pkg.canonical_id;
    }

    const appId = pkg.app_id?.trim();
    const name = pkg.name.trim();

    // 2. Logic: Prioritize AppID if it exists 
    if (appId && appId.includes('.')) {
        const canonical = knownAppIdToCanonical(appId);
        if (canonical) {
            return canonical.replace(/[^a-z0-9]/g, ''); // Strict alphanumeric
        }

        // Check for .desktop suffix or RDN structure
        let workingId = appId;
        if (workingId.endsWith('.desktop')) {
            workingId = workingId.slice(0, -8); // Remove .desktop
        }

        const segments = workingId.split('.');
        if (segments.length > 1) {
            let tail = segments[segments.length - 1].toLowerCase();
            const isGeneric = ["desktop", "git", "bin", "nightly", "beta", "stable"].includes(tail);

            if (tail.length < 3 || isGeneric) {
                // Move back one segment
                tail = segments[segments.length - 2].toLowerCase();
            }

            if (tail.length > 0) {
                return tail.replace(/[^a-z0-9]/g, '');
            }
        }
    }

    // 3. Logic: If name looks like AppID
    if (name.includes('.') && name.indexOf('.') > 0 && name.indexOf('.') < name.length - 1) {
        const canonical = knownAppIdToCanonical(name);
        if (canonical) {
            const seg = firstSegmentCanonical(canonical);
            if (seg) return seg;
            return canonical.replace(/[^a-z0-9]/g, '');
        }
    }

    // 4. Fallback: Name with suffix stripping
    let cleanName = stripPackageSuffix(name);

    // Final Normalize: Alphanumeric ONLY
    // Conservative: We no longer use firstSegmentCanonical for general merging as it causes collisions
    // for common prefixes like gnome-*, kde-*, texlive-*, etc.
    return cleanName.replace(/[^a-z0-9]/g, '');
}

/** Safe string key for PackageSource. Backend may return source_type/id as non-strings. */
export function getSourceKey(source: { source_type?: unknown; id?: unknown } | string, index?: number): string {
    if (typeof source === 'string') return source;
    const st = typeof source.source_type === 'string' ? source.source_type : '';
    const id = typeof source.id === 'string' ? source.id : '';
    const base = `${st}:${id}`;
    return index !== undefined ? `${base}-${index}` : base;
}

/** Ensures key is always a valid string; use index fallback when value is object/non-primitive. */
export function safeKey(value: unknown, index: number): string {
    if (value === null || value === undefined) return `key-${index}`;
    if (typeof value === 'string' || typeof value === 'number') return String(value);
    return `key-${index}`;
}

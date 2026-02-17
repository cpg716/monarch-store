# Essentials List — Homepage Discovery & Category Featured

**Last updated:** 2026-02-03

The **Recommended Essentials** row on the homepage shows a combined discovery pool (essentials + category featured), shuffled each launch. **Category view** “Featured” apps at the top of each category can be driven by the same remote list so both update when you edit the file — no app release required.

---

## How it works

1. **Backend** (`get_essentials_list` in `commands/package.rs`):
   - **System override:** If `/var/lib/monarch/dbs/essentials.db` exists and has content (one package name per line), that list is used. Distro packagers can customize.
   - **Remote list:** Otherwise the app fetches `docs/essentials.json` from the repo (raw GitHub URL). Result is cached under the user cache dir (`~/.cache/monarch/essentials.json`) for **7 days**. Supports either a flat array or an extended object (see below).
   - **Built-in default:** If the remote fetch fails (offline, URL change, invalid JSON), a built-in list is used.
   - The returned list is **essentials ∪ all featured** (per category), capped at 120, so the homepage row and category Featured stay in sync when you use the extended format.

2. **Category Featured** (`discovery_manager::get_featured_names_for_category`):
   - When resolving “featured” apps for a category (Games, Office, etc.), the app first checks the **same cache file** (`~/.cache/monarch/essentials.json`). If it’s fresh (within 7 days) and contains `featured_by_category`, that list is used for that category.
   - If the cache is missing, stale, or has no `featured_by_category`, built-in Rust lists per category are used.

3. **Frontend** (`useSmartEssentials`):
   - Calls `get_essentials_list()` and **shuffles** the result each launch. No filter by installed; installed apps show with an “Installed” badge.
   - **“See All”** uses the same pool.

---

## Remote format: flat array (current) or extended object

**Option A — Flat array (backward compatible):**  
Same as today. Only the Essentials pool is updated; Category Featured stays built-in.

```json
[
  "firefox",
  "google-chrome",
  "vlc",
  "steam",
  "discord",
  ...
]
```

**Option B — Extended object (Essentials + Featured in one update):**  
When you use this format, **both** the Essentials row and **Category Featured** lists are updated from the same file (after cache refresh).

```json
{
  "packages": [
    "firefox",
    "librewolf",
    "chromium",
    "google-chrome",
    "thunderbird",
    "telegram-desktop",
    "signal-desktop",
    "discord",
    "newsflash",
    "libreoffice-fresh",
    "obsidian",
    ...
  ],
  "featured_by_category": {
    "game": ["steam", "lutris", "heroic-games-launcher-bin", "discord", "minecraft-launcher", "wine", "protonup-qt", "retroarch", "gamemode", "mangohud", "r2modman-bin", "prismlauncher"],
    "network": ["google-chrome", "firefox", "brave-bin", "discord", "telegram-desktop", "signal-desktop", "zoom", "thunderbird", "qbittorrent", "transmission-gtk", "filezilla", "anydesk-bin"],
    "audiovideo": ["vlc", "obs-studio", "spotify", "gimp", "kdenlive", "blender", "audacity", "mpv", "inkscape", "handbrake", "ffmpeg", "krita"],
    "graphics": ["gimp", "blender", "inkscape", "krita", "darktable", "rawtherapee", "digikam", "glaxnimate"],
    "development": ["visual-studio-code-bin", "code", "git", "docker", "intellij-idea-community-edition", "pycharm-community-edition", "postman-bin", "sublime-text-4", "neovim", "vim", "cmake", "qtcreator"],
    "office": ["libreoffice-fresh", "obsidian", "notion-app-electron", "evince", "onlyoffice-bin", "simple-scan", "typora", "joplin", "okular"],
    "system": ["gparted", "timeshift", "bleachbit", "htop", "btop", "flatpak", "pacman", "virtualbox", "kvm", "qemu-full"],
    "utility": ["calculator", "gnome-calculator", "gnome-disk-utility", "file-roller", "spectacle", "flameshot", "ark", "kate", "gedit", "nano", "speedtest-cli", "neofetch", "fastfetch", "tree", "ripgrep", "bat", "eza", "fd", "fzf", "alacritty", "kitty"]
  }
}
```

- **Category keys** in `featured_by_category` must be the canonical names: `game`, `network`, `audiovideo`, `graphics`, `development`, `office`, `system`, `utility`. The UI maps “Games”, “Internet”, “Multimedia”, etc. to these keys.
- Use **canonical package names** (repo or AUR) that `get_packages_by_names` can resolve.
- After you push to `main`, users get the updated Essentials and Featured lists within 7 days (or on next cache clear). No app release required.

---

## Files

| File | Purpose |
|------|--------|
| **docs/essentials.json** | Remote list (flat array or extended object with `packages` + `featured_by_category`). |
| **src-tauri/monarch-gui/src/commands/package.rs** | `get_essentials_list`: DEFAULT_ESSENTIALS, fetch from URL, cache 7 days, merge with featured, fallback. |
| **src-tauri/monarch-gui/src/discovery_manager.rs** | `get_featured_names_for_category`: reads cache for `featured_by_category` when fresh; else built-in. |
| **src/constants.ts** | `ESSENTIALS_POOL` / `ESSENTIAL_IDS`: fallback when backend fails (e.g. offline). |
| **src/hooks/useSmartEssentials.ts** | Shuffles essentials list each launch; shows all (installed ones get “Installed” badge). |

---

## Current default 33 (built-in and docs/essentials.json)

**Web Browsers & Communication:** firefox, librewolf, chromium, google-chrome, thunderbird, telegram-desktop, signal-desktop, discord, newsflash.

**Office & Productivity:** libreoffice-fresh, obsidian, calibre, simplenote-electron-bin, okular, evince, foliate, keepassxc.

**Graphics & Design:** gimp, inkscape, blender, shutter, flameshot, krita, rawtherapee, gwenview.

**Multimedia & Audio:** vlc, audacity, obs-studio, handbrake, strawberry, easyeffects, ardour, shortwave.

All names are canonical for Arch repos (Extra, Community) or AUR. The homepage Essentials row shows the combined pool (essentials ∪ featured), shuffled each launch; installed apps are shown with an “Installed” badge.

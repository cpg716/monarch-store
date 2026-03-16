# Source logos (badges)

Logo-only badges on package cards and in the home "Source key" (24×24). Use the script below to fetch **official or high-quality** logos.

## Download new logos (run from repo root)

```bash
cd src/assets/source-logos
./fetch-source-logos.sh
```

Or run the `curl` commands below manually (save each to the filename on the right).

## Direct download URLs

| Source       | Save as            | URL |
|-------------|--------------------|-----|
| **Arch**    | `arch-official.svg` | Official: `https://archlinux.org/static/logos/archlinux-logo-dark-scalable.svg` (or light: `archlinux-logo-light-scalable.svg`). Alternative (Commons): `https://upload.wikimedia.org/wikipedia/commons/f/f9/Archlinux-logo-standard-version.svg` |
| **Flatpak** | `flatpak.svg`       | Official ZIP (contains SVGs): `https://flatpak.org/img/flatpak-logos.zip` — unzip and copy the SVG you want. Or Simple Icons (CC0): `https://cdn.jsdelivr.net/npm/simple-icons@v11/icons/flatpak.svg` |
| **AUR**     | `aur.svg`           | No official AUR logo; script fetches Simple Icons archlinux: `https://cdn.jsdelivr.net/npm/simple-icons@v11/icons/archlinux.svg` (CC0). |
| **Chaotic-AUR** | `chaotic-aur.png` | Site favicon (PNG): `https://aur.chaotic.cx/favicon-32x32.png`. No upstream SVG. |
| **CachyOS** | `cachyos.svg`       | Commons: `https://upload.wikimedia.org/wikipedia/commons/b/b8/CachyOS_Logo.svg` (GPL). |
| **Manjaro** | `manjaro.svg`       | Commons: `https://upload.wikimedia.org/wikipedia/commons/3/3e/Manjaro-logo.svg` (PD). |
| **EndeavourOS** | `endeavouros.svg` | GitHub: `https://raw.githubusercontent.com/endeavouros-team/Branding/main/icons/endeavouros.svg` (official Branding repo). |
| **Garuda**   | `garuda.svg`        | Commons: `https://upload.wikimedia.org/wikipedia/commons/8/88/Garuda-blue-sgs.svg` (CC BY-SA). |

## One-line fetch (copy-paste)

```bash
cd src/assets/source-logos

# Arch (official dark)
curl -sL -o arch-official.svg "https://archlinux.org/static/logos/archlinux-logo-dark-scalable.svg"

# Flatpak (Simple Icons CDN)
curl -sL -o flatpak.svg "https://cdn.jsdelivr.net/npm/simple-icons@v11/icons/flatpak.svg"

# CachyOS (Commons)
curl -sL -o cachyos.svg "https://upload.wikimedia.org/wikipedia/commons/b/b8/CachyOS_Logo.svg"

# Manjaro (Commons)
curl -sL -o manjaro.svg "https://upload.wikimedia.org/wikipedia/commons/3/3e/Manjaro-logo.svg"

# EndeavourOS (GitHub)
curl -sL -o endeavouros.svg "https://raw.githubusercontent.com/endeavouros-team/Branding/main/icons/endeavouros.svg"

# Garuda (Commons)
curl -sL -o garuda.svg "https://upload.wikimedia.org/wikipedia/commons/8/88/Garuda-blue-sgs.svg"

# AUR (Simple Icons archlinux)
curl -sL -o aur.svg "https://cdn.jsdelivr.net/npm/simple-icons@v11/icons/archlinux.svg"

# Chaotic-AUR (site favicon, PNG)
curl -sL -o chaotic-aur.png "https://aur.chaotic.cx/favicon-32x32.png"
```

If Arch official returns 500, use Commons:
```bash
curl -sL -o arch-official.svg "https://upload.wikimedia.org/wikipedia/commons/f/f9/Archlinux-logo-standard-version.svg"
```

## Licensing

- **Arch**: Trademark policy at https://terms.archlinux.org/docs/trademark-policy/
- **Flatpak**: CC BY 3.0 (official zip from flatpak.org/press/)
- **Simple Icons**: CC0-1.0 (https://simpleicons.org)
- **Commons/Wikimedia**: See each file’s license on the page (e.g. GPL, CC BY-SA).

Replace files in this directory; the app uses the filenames above (`arch-official.svg`, `flatpak.svg`, etc.).

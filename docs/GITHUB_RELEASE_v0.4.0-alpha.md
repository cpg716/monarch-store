# MonARCH Store v0.4.5-alpha — Release fallback

**Note:** The GitHub Release workflow now generates the release body **dynamically** from `RELEASE_NOTES.md` for the pushed tag. The text below is a fallback you can paste if you edit the draft manually.

---

Experimental Alpha Release. See assets below for installation.

**v0.4.5-alpha** — Version bump. One card per app, details dropdown, Operation Chaotic Good (Chaotic-AUR safe toggle, onboarding wizard), UI/UX labels. Repositories discovered from your system's `pacman.conf`.

## What's in this release

- **Mission Control (New Settings)** — Tabbed settings for Sources, AUR Builder, and System Maintenance. Chaotic-AUR safe toggle (traffic light: Active/Inactive/Blocked); we install keyring and mirrorlist only—you add the repo to pacman.conf manually.
- **Onboarding Wizard** — Multi-step flow: Welcome → Source Manager → Chaotic-AUR Setup (conditional) → Security & Privacy → Theme → Confirmation.
- **One card per app** — No duplicate cards; details page always shows a dropdown of all sources and prefers the card’s source.
- **Unified Update System** — Parallel checks Repo/AUR/Flatpak. Enforces full system upgrade (`-Syu`) when updating official packages (Safety Lock). "Built from Source" labels for AUR.
- **Legacy Code Audit** — Removed all "Ghost Commands" and legacy contexts for runtime stability.
- **Native AUR Builder** — Replaced yay wrapper with a native, user-level builder; build logs stream to the UI.
- **Flatpak Integration** — Install, remove, and update Flatpak apps as first-class citizens.
- **Manjaro Guard** — Automatically blocks `chaotic-aur` on Manjaro to prevent glibc breakage.
- **Silent Guard** — Complex operations prompt for a password at most once; Polkit remembered for 5 minutes.

## Installation

- **Arch Linux:** Download the `.pkg.tar.zst` (if attached) and run `sudo pacman -U monarch-store-*.pkg.tar.zst`
- **AppImage:** Download the `.AppImage`, make executable (`chmod +x`), and run.

⚠️ **Alpha:** Use with care on production systems. Ensure you have backups.

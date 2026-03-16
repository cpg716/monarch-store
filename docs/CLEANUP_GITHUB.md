# GitHub repo cleanup

## What the current build uses (GTK-only)

- **`src-tauri/`** — monarch-gtk (UI), monarch-helper (privileged), monarch-core (logic), monarch-gui (shared/icons, desktop file). This is the only active app.
- **`.github/workflows/release.yml`** — Builds in Arch container, publishes tarball on host.
- **Root:** README, LICENSE, RELEASE_NOTES, PKGBUILD, AGENTS.md, docs, scripts. Legacy Tauri/React files (e.g. `src/`, `index.html`, `package.json`, `vite.config.ts`) are kept for reference only and are not used by the release.

## Done (2026-03-16)

- **Tags:** Removed all tags except **v0.5.0-alpha** and **v0.4.7-alpha** (remote and local). Old 0.2.x, 0.3.x, 0.4.x tags are gone.
- **Branches:** Only `main` exists; no stale branches.
- **Workflows:** Only `.github/workflows/release.yml` is present (Build Builder Image was removed earlier).
- **Obsolete root files removed from repo:** `icons.txt`, `images.txt`, `urls.txt`, `odrs_debug_plan.txt`, `release_v0.3.00_alpha1.sh`, `User_Demands_Butterfly_Wings_Flap.mp4`, `GLM-V3.Modelfile` (v0.1/v0.2-era cruft; not used by the GTK build). They are in `.gitignore` so they stay ignored if present on disk.

## Optional cleanup on GitHub

- **Releases:** In the repo → **Releases**, you can delete old draft or published releases that pointed at removed tags. Keep **v0.5.0-alpha** (and optionally **v0.4.7-alpha** if you left that release).
- **Actions runs:** In **Actions**, you can delete old workflow runs to tidy the list (runs are just logs; deleting them does not affect tags or releases).

## Script

`scripts/cleanup-github-tags.sh` can be used in the future to trim tags again (it keeps v0.5.0-alpha and v0.4.7-alpha by default; edit `KEEP` to change).

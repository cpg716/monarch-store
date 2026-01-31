# Prepare for Push to GitHub — v0.3.5-alpha

**Last updated:** 2025-01-31

Use this checklist before pushing to GitHub (main and/or release tag).

---

## 1. Pre-push checklist

| Step | Command / action | Status |
|------|------------------|--------|
| **Build** | `npm run build` | ✅ Must pass (TS + Vite) |
| **Rust** | `cd src-tauri && cargo check` | ✅ Must pass |
| **.gitignore** | `.cursor` and build artifacts ignored | ✅ Done |
| **No secrets** | No API keys, tokens, or `.env` committed | ⬜ Verify |
| **Version** | package.json, tauri.conf.json, Cargo.toml, PKGBUILD = 0.3.5-alpha / 0.3.5_alpha | ✅ Synced |

---

## 2. What to commit

**Include:**
- All source code (src/, src-tauri/)
- Docs (docs/, root .md)
- Config (package.json, vite.config.ts, tsconfig.json, PKGBUILD, .SRCINFO)
- Scripts (scripts/)
- Screenshots (screenshots/)
- Security (security/)

**Exclude (via .gitignore):**
- `node_modules/`, `dist/`, `target/`
- `.cursor/` (IDE/agent context)
- `*.log`, `.env`, build artifacts

---

## 3. Commands to push

From repo root:

```bash
# 1. Stage all changes (respects .gitignore)
git add -A

# 2. Review what will be committed
git status

# 3. Commit (adjust message if needed)
git commit -m "Release v0.3.5-alpha: Omni-User, Titan Polish, Aptabase, docs"

# 4. Push main
git push origin main

# 5. (Optional) Create and push release tag
git tag -a v0.3.5_alpha -m "Release v0.3.5-alpha"
git push origin v0.3.5_alpha
```

If you use SSH for GitHub:

```bash
git remote set-url origin git@github.com:cpg716/monarch-store.git
git push origin main
git push origin v0.3.5_alpha
```

---

## 4. After pushing the tag

To switch PKGBUILD to the release tarball and update checksums:

```bash
chmod +x scripts/release-finalize-pkgbuild.sh
./scripts/release-finalize-pkgbuild.sh
```

Then push the updated PKGBUILD and .SRCINFO: `git push origin main`.

---

## 5. Optional: GitHub Release

On GitHub: **Releases** → **Draft a new release** → choose tag `v0.3.5_alpha`, add title and notes (e.g. from `docs/GITHUB_RELEASE_TEMPLATE_v0.3.5.md`), publish.

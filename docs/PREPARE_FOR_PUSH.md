# Prepare for Push to GitHub — v0.4.5-alpha

**Last updated:** 2026-02-03

Use this checklist before pushing to GitHub (main and/or release tag). For a summary of recent features (one card per app, details dropdown, Operation Chaotic Good, onboarding), see [RECENT_CHANGES.md](RECENT_CHANGES.md).

---

## 1. Pre-push checklist

| Step | Command / action | Status |
|------|------------------|--------|
| **Build** | `npm run build` | ✅ Must pass (TS + Vite) |
| **Rust** | `cd src-tauri && cargo check` | ✅ Must pass |
| **.gitignore** | `.cursor` and build artifacts ignored | ✅ Done |
| **No secrets** | No API keys, tokens, or `.env` committed | ⬜ Verify |
| **Version** | package.json, tauri.conf.json, Cargo.toml (monarch-gui + monarch-helper), PKGBUILD = 0.4.5-alpha | ✅ Synced |

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

**If commit and tag are already done** (e.g. by an agent), you only need to push:

```bash
git push origin main
git push origin v0.4.5-alpha
```

**Otherwise**, from repo root:

```bash
# 1. Stage all changes (respects .gitignore)
git add -A

# 2. Review what will be committed
git status

# 3. Commit (adjust message if needed)
git commit -m "Release v0.4.5-alpha: Version bump, Chaotic Good, unification"

# 4. Push main
git push origin main

# 5. (Optional) Create and push release tag (triggers CI build + draft release)
git tag -a v0.4.5-alpha -m "Release v0.4.5-alpha"
git push origin v0.4.5-alpha
```

If you use SSH for GitHub:

```bash
git remote set-url origin git@github.com:cpg716/monarch-store.git
git push origin main
git push origin v0.4.5-alpha
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

## 5. GitHub Release (container-built)

Pushing a tag (e.g. `v0.4.5-alpha`) triggers the **Release** workflow: it builds the app in Docker and creates a **draft** GitHub Release. The release body is **dynamic** — generated from `RELEASE_NOTES.md` for the pushed tag. After the workflow completes, open the draft release, verify artifacts, and publish. See [RELEASE_PUSH_STEPS.md](RELEASE_PUSH_STEPS.md).

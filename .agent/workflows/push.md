---
description: Prepare for Push to GitHub (v0.4.6-alpha)
---
# /push
**Goal:** Prepare and push changes to GitHub (main and/or release tag).
1. **Pre-flight Check:** Run `npm run build` and `cd src-tauri && cargo check` to ensure stability.
2. **Version Sync:** Verify `package.json`, `tauri.conf.json`, `Cargo.toml`, and `PKGBUILD` are synced to the target version.
3. **Commit:** Stage all changes with `git add -A` and commit with a descriptive message.
4. **Push:** Execute `git push origin main` and (if releasing) `git push origin v0.4.x-alpha`.
5. **Finalize:** For release tags, run `./scripts/release-finalize-pkgbuild.sh` to update the PKGBUILD.

# Release push steps — v0.4.0-alpha

Follow these steps to push the release to GitHub and finalize the PKGBUILD for the versioned tag.

**Before pushing:** See [PREPARE_FOR_PUSH.md](PREPARE_FOR_PUSH.md) — run `npm run build` and `cargo check`, ensure `.gitignore` is correct, then stage and commit.

## 1. Push main and tag (you need GitHub credentials)

From the repo root:

```bash
# Push the release commit
git push origin main

# Push the release tag (triggers GitHub Actions: Docker build + Release draft with AppImage/artifacts)
git push origin v0.4.0-alpha
```

If you use SSH for GitHub, ensure `origin` uses `git@github.com:cpg716/monarch-store.git`, or run:

```bash
git remote set-url origin git@github.com/cpg716/monarch-store.git
git push origin main
git push origin v0.4.0-alpha
```

## 2. Finalize PKGBUILD (tarball + checksums)

After the tag exists on GitHub, run:

```bash
chmod +x scripts/release-finalize-pkgbuild.sh
./scripts/release-finalize-pkgbuild.sh
```

This script will:

- Switch `PKGBUILD` source to the release tarball (`v0.4.0-alpha.tar.gz`)
- Run `updpkgsums` to fill `sha256sums`
- Regenerate `.SRCINFO` with `makepkg --printsrcinfo`
- Commit `PKGBUILD` and `.SRCINFO` and push to `main`

If the script does not push (e.g. credentials), run manually:

```bash
git push origin main
```

## 3. GitHub Release (container-built artifacts)

The **Release** workflow (on tag push) builds the app in Docker (`ghcr.io/cpg716/monarch-store-builder`) and uploads the resulting AppImage (and other Tauri bundle artifacts) to a **draft** GitHub Release. After the workflow completes, open the draft release, add notes (e.g. from `RELEASE_NOTES.md`), and publish. For Arch `.pkg.tar.zst`, use the PKGBUILD flow below or attach the built package to the release.

## Summary

| Step | Command / action |
|------|-------------------|
| Push main | `git push origin main` |
| Push tag | `git push origin v0.4.0-alpha` (triggers CI build + draft release) |
| Finalize PKGBUILD | `./scripts/release-finalize-pkgbuild.sh` (after tag exists) |
| GitHub Release | Publish the draft created by the workflow, or attach `.pkg.tar.zst` from local build |

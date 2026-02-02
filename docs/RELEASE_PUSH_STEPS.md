# Release push steps — v0.4.0-alpha

Follow these steps to push the release to GitHub and finalize the PKGBUILD for the versioned tag.

**Before pushing:** See [PREPARE_FOR_PUSH.md](PREPARE_FOR_PUSH.md) — run `npm run build` and `cargo check`, ensure `.gitignore` is correct, then stage and commit.

## 1. Push main and tag (you need GitHub credentials)

From the repo root:

```bash
# Push the release commit
git push origin main

# Push the release tag (creates the tarball on GitHub)
git push origin v0.3.6_alpha
```

If you use SSH for GitHub, ensure `origin` uses `git@github.com:cpg716/monarch-store.git`, or run:

```bash
git remote set-url origin git@github.com:cpg716/monarch-store.git
git push origin main
git push origin v0.3.6_alpha
```

## 2. Finalize PKGBUILD (tarball + checksums)

After the tag exists on GitHub, run:

```bash
chmod +x scripts/release-finalize-pkgbuild.sh
./scripts/release-finalize-pkgbuild.sh
```

This script will:

- Switch `PKGBUILD` source to the release tarball (`v0.3.6_alpha.tar.gz`)
- Run `updpkgsums` to fill `sha256sums`
- Regenerate `.SRCINFO` with `makepkg --printsrcinfo`
- Commit `PKGBUILD` and `.SRCINFO` and push to `main`

If the script does not push (e.g. credentials), run manually:

```bash
git push origin main
```

## 3. Create GitHub Release (optional)

On GitHub: **Releases** → **Draft a new release** → choose tag `v0.3.6_alpha`, add title and notes (e.g. from `docs/GITHUB_RELEASE_TEMPLATE_v0.3.5.md`), publish.

## Summary

| Step | Command / action |
|------|-------------------|
| Push main | `git push origin main` |
| Push tag | `git push origin v0.3.6_alpha` |
| Finalize PKGBUILD | `./scripts/release-finalize-pkgbuild.sh` |
| Optional: GitHub Release | Draft release from tag in GitHub UI |

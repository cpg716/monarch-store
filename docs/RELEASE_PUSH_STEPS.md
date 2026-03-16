# Monarch Store - Git Release Process

This document outlines the authoritative process for releasing a new version of Monarch Store.

## 🏗️ Architecture

The release process relies on two GitHub Actions workflows:
1. **Build Builder Image** (`build-builder.yml`): Builds the Docker container (`monarch-store-builder`) containing all system dependencies (`librsvg`, `libadwaita`, `rust`).
2. **Release** (`release.yml`): Runs *inside* the builder container to compile the app and generate artifacts (`.AppImage`, `.deb`, `.rpm`).

---

## 🚀 Release Steps

### 1. Pre-Flight Checks
Ensure your local `package.json` and Cargo.toml versions match the target release.
```bash
# Check version consistency (GTK release)
grep "version" package.json src-tauri/monarch-core/Cargo.toml src-tauri/monarch-gtk/Cargo.toml src-tauri/monarch-helper/Cargo.toml PKGBUILD
```

### 2. Update the Builder Image (If Dependencies Changed)
**CRITICAL**: If you modified `docker/Dockerfile` (e.g., added a system library like `librsvg`), you **MUST** wait for the builder image to rebuild before releasing.

1. Commit and push changes to `main`.
2. Go to [GitHub Actions](https://github.com/cpg716/monarch-store/actions).
3. Wait for **Build Builder Image** to complete successfully.

### 3. Push the Release Tag
The **Release** workflow is triggered by pushing a tag starting with `v*`.

```bash
# Tag the current commit (ensure it's clean and tested)
git tag v0.5.0-alpha

# Push to GitHub
git push origin v0.5.0-alpha
```

### 4. Verify & Publish
1. Go to the **Actions** tab and watch the **Release** workflow.
2. Once green, go to the **Releases** tab.
3. You will see a new **Draft Release**.
4. Verify the artifacts are attached:
    - `MonARCH_Store_..._amd64.AppImage` (Universal Linux App)
    - `monarch-store_..._amd64.deb` (Debian/Ubuntu)
    - `monarch-store-...-1.x86_64.rpm` (Fedora/OpenSUSE)
5. The release body is **dynamic**: the workflow extracts the changelog for the pushed tag from `RELEASE_NOTES.md`. If you want to tweak it, click **Edit**, adjust the notes, and click **Publish release**. For a full feature summary (one card per app, Chaotic Good, onboarding), see [RECENT_CHANGES.md](RECENT_CHANGES.md).

---

## 🆘 Troubleshooting

### "The Release workflow failed!"
Check the logs.
- **"Package ... not found"**: You are likely missing a dependency in the Dockerfile. Add it, push to main, wait for the builder to rebuild, then retry.
- **"Network error"**: Transient failure. Retry via the GitHub UI.

### "How do I retry a release?"
You have two options:

**Option A: The Clean Reset (Recommended)**
Delete the tag remote and local, then re-push.
```bash
git push --delete origin v0.5.0-alpha
git tag -d v0.5.0-alpha
git tag v0.5.0-alpha
git push origin v0.5.0-alpha
```

**Option B: Manual Trigger**
We have enabled `workflow_dispatch` on the release workflow.
1. Go to Actions -> Release.
2. Click **Run workflow**.
3. Select `main` branch.
4. **Note**: This will create artifacts labeled with the *branch name* or *short sha* unless you manually override, so standardizing on Tags (Option A) is preferred for final releases.

### "My AppImage has no icons!"
This usually means `librsvg` was missing in the builder.
1. Check `docker/Dockerfile` for `librsvg2-dev`.
2. If missing, add it.
3. Rebuild builder (Step 2).
4. Retry release (Step 3).

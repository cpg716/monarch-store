---
description: Execute a full GitHub Release process.
---
# /release
**Goal:** Successfully release a new version via GitHub Actions and Tags.
1. **Version Audit:** Use `grep "version" package.json src-tauri/monarch-gui/Cargo.toml` to ensure match.
2. **Tag:** Create a new tag (e.g., `git tag v0.4.x-alpha`).
3. **Push Tag:** Push the tag with `git push origin v0.4.x-alpha` to trigger the CI builder.
4. **Monitor Actions:** Watch the GitHub Actions "Release" workflow until it completes.
5. **Publish:** Verify artifacts (AppImage, deb, rpm) on the GitHub Releases page and publish the draft.

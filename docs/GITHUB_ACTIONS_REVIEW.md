# GitHub Actions Review

Review of `.github/workflows/` for monarch-store (v0.4.5-alpha).  
**Date:** 2026-02-03

---

## 1. Release workflow (`release.yml`)

**Purpose:** Build the Tauri app in Docker and create a draft GitHub Release with artifacts when a version tag is pushed (or on manual run).

### Triggers
- **`push: tags: - 'v*'`** — Runs on any tag like `v0.4.5-alpha`, `v1.0.0`, etc. ✅
- **`workflow_dispatch`** — Can be run manually from the Actions tab. ✅

### Job: `publish-tauri`
- **Runs on:** `ubuntu-22.04` (matrix with one entry; expandable later for other OSes).
- **Container:** `ghcr.io/cpg716/monarch-store-builder:latest` with `--user root`. ✅  
  All build deps (Node, Rust, libalpm, GTK, etc.) are expected in this image.

### Steps (in order)
1. **Checkout** — `actions/checkout@v4`. ✅
2. **Git safe directory** — Fixes `safe.directory` in the container so git works. ✅
3. **Setup Node** — `actions/setup-node@v4`, Node 20. ✅ (Matches Dockerfile LTS.)
4. **Rust** — `rustup default stable`. ✅ (Image already has Rust; this ensures stable.)
5. **System deps** — `apt-get install -y xdg-utils`. ✅ (Dockerfile has it; harmless redundancy.)
6. **npm install** — Frontend deps. ✅
7. **npm run build** — Frontend production build. ✅
8. **Build monarch-helper** — `cd src-tauri && cargo build --release -p monarch-helper`. ✅  
  Relies on container `PKG_CONFIG_PATH` for libalpm.
9. **Disable Tauri build hook** — `sed` replaces `npm run build` with `true` in `tauri.conf.json` so the Tauri step doesn’t re-run the frontend build. ✅
10. **Prepare release body** — Extracts the changelog section for the pushed tag from `RELEASE_NOTES.md` (awk) and writes it to `GITHUB_OUTPUT`; fallback one-liner if no section matches. ✅
11. **tauri-action** — Builds the app and creates/updates the release. ✅

### Tauri action config
- **projectPath:** `./src-tauri/monarch-gui`. ✅
- **tagName:** `${{ github.ref_name }}` — Uses the tag that triggered the run (e.g. `v0.4.5-alpha`). ✅
- **releaseName / releaseBody:** releaseName uses `github.ref_name`; **releaseBody is dynamic** — `${{ steps.release_body.outputs.body }}` (from step 10). ✅
- **releaseDraft: true** — Release is created as draft so you can review before publishing. ✅
- **prerelease: true** — Marks the release as pre-release. ✅
- **Secrets:** Uses `GITHUB_TOKEN`, optional `TAURI_SIGNING_*` for signed updates. ✅
- **APPIMAGE_EXTRACT_AND_RUN: 1** — Improves AppImage compatibility. ✅
- **args: --verbose** — Helps debug Tauri build. ✅

### Permissions
- **contents: write** — Needed to create the release and upload assets. ✅

### Possible improvements (optional)
- **CARGO_TARGET_DIR:** If you ever see “helper not found” or duplicate builds, set `CARGO_TARGET_DIR: ${{ github.workspace }}/src-tauri/target` in the env for the job so all Rust output stays under `src-tauri/target`.
- **Cache:** `actions/setup-node` with `cache: 'npm'` would cache `node_modules`; container already has a fixed setup so benefit is small. Optional.

**Verdict:** Release workflow is correct, well-ordered, and appropriate for tag-based releases and manual runs. No blocking issues.

---

## 2. Build Builder Image workflow (`build-builder.yml`)

**Purpose:** Build and push the Docker image used by the Release workflow to `ghcr.io/cpg716/monarch-store-builder`.

### Triggers
- **Path filters:** Runs when `docker/Dockerfile` or `.github/workflows/build-builder.yml` change. ✅
- **workflow_dispatch** — Can be run manually. ✅

### Job: `build-and-push`
- **Runs on:** `ubuntu-latest` (no container). ✅
- **Permissions:** `contents: read`, `packages: write` (for ghcr.io). ✅

### Steps
1. **Checkout** — Needed for Docker build context. ✅
2. **Log in to registry** — `docker/login-action@v3` for `ghcr.io` with `GITHUB_TOKEN`. ✅
3. **Metadata** — `docker/metadata-action@v5`: tags `latest` and `sha`. ✅
4. **Build and push** — `docker/build-push-action@v5`, context `.`, file `docker/Dockerfile`, push to ghcr.io. ✅

### Notes
- **No cache:** Build is from scratch every time. You could add `cache-from` / `cache-to` (e.g. `type=gha`) to speed up rebuilds; optional.
- **Context:** Context is repo root (`.`), so Dockerfile can `COPY` from repo if needed; current Dockerfile doesn’t rely on repo content. ✅
- After changing the Dockerfile (e.g. adding `ca-certificates`), either push those changes and let the path filter trigger this workflow, or run “Build Builder Image” manually so the Release workflow uses the new image.

**Verdict:** Build-builder workflow is correct and sufficient. Optional: add Docker layer cache (GHA cache) for faster rebuilds.

---

## 3. Summary

| Workflow           | Status   | Notes                                                                 |
|--------------------|----------|-----------------------------------------------------------------------|
| **Release**        | ✅ Good  | Tag-driven and manual; container + steps order correct; draft release. |
| **Build Builder**  | ✅ Good  | Path-triggered and manual; image build and push correct.             |

**Dependency:** Release workflow depends on `ghcr.io/cpg716/monarch-store-builder:latest`. Keep that image up to date (via Build Builder workflow) when you change `docker/Dockerfile` or want a clean rebuild.

**No other workflows** in `.github/workflows/`; no conflicting or redundant jobs.

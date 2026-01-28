#!/bin/bash
# Release Script for MonARCH Store v0.3.00-Alpha1

# 1. Stage Documentation & Version Bumps
git add ARCHITECTURE.md README.md RELEASE_NOTES.md package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json

# 2. Commit the "Universal Manager" Update
git commit -m "chore(release): v0.3.00-alpha.1 - Universal Arch Linux App Manager Rebrand & Distro-Aware Architecture"

# 3. Tag the Release (Triggers CI/CD Build)
git tag -a v0.3.00-alpha.1 -m "v0.3.00-Alpha1: The Universal Manager Update"

# 4. Push to GitHub
git push origin main --follow-tags

# Backend Commands Not Wired to Frontend

**Release:** v0.3.5-alpha

This document lists Tauri commands available in the backend but not currently used in the frontend TypeScript code.

## Commands Available in Backend (from `lib.rs`)

### ✅ Used in Frontend
- `search_aur` ✅
- `search_packages` ✅
- `get_packages_by_names` ✅
- `get_chaotic_package_info` ✅
- `get_chaotic_packages_batch` ✅
- `get_trending` ✅
- `get_package_variants` ✅
- `get_category_packages_paginated` ✅
- `install_package` ✅
- `uninstall_package` ✅
- `get_essentials_list` ✅
- `abort_installation` ✅
- `check_installed_status` ✅
- `perform_system_update` ✅
- `fetch_pkgbuild` ✅
- `get_installed_packages` ✅
- `check_for_updates` ✅
- `check_reboot_required` ✅
- `get_pacnew_warnings` ✅
- `get_orphans` ✅
- `remove_orphans` ✅
- `get_cache_size` ✅
- `get_orphans_with_size` ✅
- `set_parallel_downloads` ✅
- `rank_mirrors` ✅
- `force_refresh_databases` ✅
- `check_repo_sync_status` ✅
- `get_system_info` ✅
- `get_infra_stats` ✅
- `get_repo_counts` ✅
- `get_repo_states` ✅
- `is_aur_enabled` ✅
- `toggle_repo` ✅
- `toggle_repo_family` ✅
- `set_aur_enabled` ✅
- `is_one_click_enabled` ✅
- `set_one_click_enabled` ✅
- `is_advanced_mode` ✅
- `set_advanced_mode` ✅
- `check_security_policy` ✅
- `install_monarch_policy` ✅
- `optimize_system` ✅
- `get_all_installed_names` ✅
- `fix_keyring_issues` ✅
- `trigger_repo_sync` ✅
- `update_and_install_package` ✅
- `check_app_update` ✅ (used in Sidebar.tsx)
- `get_install_mode_command` ✅
- `is_telemetry_enabled` ✅
- `is_notifications_enabled` ✅
- `set_notifications_enabled` ✅
- `set_telemetry_enabled` ✅
- `is_sync_on_startup_enabled` ✅
- `set_sync_on_startup_enabled` ✅
- `check_and_clear_refresh_requested` ✅
- `get_package_icon` ✅
- `clear_cache` ✅
- `launch_app` ✅
- `track_event` ✅
- `get_metadata` ✅
- `get_metadata_batch` ✅
- `check_system_health` ✅
- `check_initialization_status` ✅
- `submit_review` ✅
- `get_local_reviews` ✅
- `get_app_rating` ✅
- `get_app_ratings_batch` ✅
- `get_app_reviews` ✅
- `repair_unlock_pacman` ✅
- `check_keyring_health` ✅
- `repair_emergency_sync` ✅
- `check_pacman_lock` ✅
- `initialize_system` ✅
- `bootstrap_system` ✅
- `enable_repos_batch` ✅
- `enable_repo` ✅
- `reset_pacman_conf` ✅
- `set_repo_priority` ✅
- `check_repo_status` ✅
- `set_one_click_control` ✅
- `fix_keyring_issues_alias` ✅
- `get_distro_context` ✅

### ❌ NOT Used in Frontend

1. **`get_local_reviews`** - Get locally stored reviews for a package
   - Status: Backend command exists (`commands::reviews::get_local_reviews`) but **never called from frontend**
   - Location: `src-tauri/monarch-gui/src/commands/reviews.rs:66`
   - Use case: Could display user's own submitted reviews in PackageDetails

2. **`submit_review`** - Submit a local review for a package
   - Status: Backend command exists (`commands::reviews::submit_review`) but **never called from frontend**
   - Location: `src-tauri/monarch-gui/src/commands/reviews.rs:30`
   - Use case: Review submission UI exists in PackageDetailsFresh.tsx but doesn't call this command
   - Note: PackageDetailsFresh.tsx has review UI but only calls `track_event`, not `submit_review`

3. **`check_keyring_health`** - Standalone keyring health check
   - Status: Backend command exists (`repair::check_keyring_health`) but **never called directly from frontend**
   - Location: `src-tauri/monarch-gui/src/repair.rs:42`
   - Note: Frontend uses `check_system_health` which may include keyring status, but dedicated check could be useful

4. **`initialize_system`** - System initialization command
   - Status: Backend command exists (`repair::initialize_system`) but **never called from frontend**
   - Location: `src-tauri/monarch-gui/src/repair.rs:320`
   - Note: Frontend uses `bootstrap_system` instead - may be redundant or serve different purpose

## Commands Referenced in Frontend But Missing from Backend

1. **`repair_reset_keyring`** - Referenced in InstallMonitor.tsx and SystemHealthSection.tsx
   - Status: **MISSING** - Not in lib.rs command list, but exists in permissions file
   - Location: Referenced in `src/components/InstallMonitor.tsx:457` and `src/components/SystemHealthSection.tsx:109`
   - Fix needed: Either add as Tauri command or use `fix_keyring_issues`/`fix_keyring_issues_alias` instead

2. **`apply_os_config`** - Referenced in OnboardingModal.tsx
   - Status: **MISSING** - Not a Tauri command, but exists as method in RepoManager
   - Location: Referenced in `src/components/OnboardingModal.tsx:206`
   - Fix needed: Expose `repo_manager::apply_os_config` as a Tauri command or remove frontend call

3. **`emit_sync_progress`** - Referenced in App.tsx
   - Status: **MISSING** - Not a Tauri command
   - Location: Referenced in `src/App.tsx:209,212`
   - Fix needed: Should use Tauri event emitter (`app.emit()`) instead of `invoke()`, or create the command

## Recommendations

### High Priority (Broken Functionality)

1. **`repair_reset_keyring`**: Add as Tauri command or replace frontend calls with `fix_keyring_issues_alias`
2. **`apply_os_config`**: Expose RepoManager method as Tauri command or remove frontend usage
3. **`emit_sync_progress`**: Replace `invoke()` calls with proper Tauri event emission (`app.emit()`)

### Medium Priority (Missing Features)

4. **`submit_review`** and **`get_local_reviews`**: Wire up review submission/retrieval in PackageDetailsFresh.tsx
   - Currently review UI exists but only tracks events, doesn't actually save reviews
   - Would enable users to save and view their own reviews

### Low Priority (Nice to Have)

5. **`check_keyring_health`**: Could be exposed as dedicated health check in Settings for diagnostics
6. **`initialize_system`**: Verify if needed or if `bootstrap_system` is sufficient

## Notes

- Most commands are well-integrated
- A few repair/health commands may benefit from better UI exposure
- Review system commands exist but may not be fully implemented in UI
- Some commands may be intentionally backend-only (internal use)

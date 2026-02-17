# Settings Page — Control Wiring

**Last updated:** 2026-02-03 (post–Settings Polish refactor)

This doc confirms that each Settings control correctly updates the parts of the app it should. Refactors (e.g. layout, navigation) must not break these connections.

---

## 1. General tab

| Control | Source | Where it’s used |
|--------|--------|------------------|
| **Theme (Light/Dark/System)** | `useTheme()` → `setThemeMode` | Global: `useTheme()` in App and elsewhere; CSS/theme context. |
| **One-Click Authentication (Reduce password prompts)** | `useAppStore()` → `setReducePasswordPrompts` | Persisted to `localStorage` key `monarch_reduce_password_prompts`. Read by: `App.tsx` (startup unlock, batch flow), `InstallMonitor.tsx`, `InstalledPage.tsx`, `UpdatesPage.tsx`, `SessionPasswordContext` (session password), `useSettings.ts` (AUR/chaotic toggles). |
| **Anonymous Telemetry** | `useSettings()` → `toggleTelemetry` → `useAppStore().setTelemetry` | Store updates + `invoke('set_telemetry_enabled', { enabled })`. Read by: onboarding, error service, Aptabase/track_event. |

---

## 2. Sources tab

| Control | Source | Where it’s used |
|--------|--------|------------------|
| **Repos (incl. Chaotic-AUR)** | `useSettings()` → `repos`, `toggleRepo`, `refresh` | `SourcesTab` uses `useSettings()`; backend `get_repo_states`, `set_repo_state`, `check_chaotic_status`, `prepare_chaotic_components`. Search and package lists respect enabled repos. |
| **Flatpak / AUR toggles** | `useSettings()` → `isFlatpakEnabled`, `toggleFlatpak`, `isAurEnabled`, `toggleAur` | Persisted (localStorage for Flatpak; backend `set_aur_enabled`). Search and install flows show/hide Flatpak and AUR. |

Sources tab is self-contained in `SourcesTab.tsx`; it uses the same hooks and invoke calls. No props are passed from `SettingsPage.tsx`; wiring is unchanged by the layout refactor.

---

## 3. Builder tab

| Control | Source | Where it’s used |
|--------|--------|------------------|
| **Verbose logs / Clean build / Parallel downloads** | `useAppStore()` → `verboseLogsEnabled`, `setVerboseLogsEnabled`, etc. | Persisted via store (localStorage). AUR build and helper flows read these. |

Builder tab is self-contained in `BuilderTab.tsx`; it uses `useAppStore()` and `invoke`. No props from `SettingsPage.tsx`; wiring unchanged.

---

## 4. Maintenance tab

| Control | Source | Where it’s used |
|--------|--------|------------------|
| **Repair Keyring** | `invoke('fix_keyring_issues')` + `onRepairComplete` | Backend repair; `onRepairComplete` from App → `refreshSystemHealth()`. |
| **Unlock Pacman** | `invoke('repair_unlock_pacman')` + `onRepairComplete` | Clears stale db.lck; same callback. |
| **Sync Databases** | `invoke('sync_system_databases')` | Backend refresh. |
| **System Cleanup** | `invoke('clear_cache')` | In-memory + backend cache clear. |
| **Advanced Repository Mode** | `useSettings()` → `advancedMode`, `toggleAdvancedMode` | Backend `is_advanced_mode` / set; distro safety locks. |

All handlers and `onRepairComplete` are still called from the refactored `renderContent()` (maintenance case). `ConfirmationModal` for Advanced Mode still uses `modalConfig` and `onConfirm`.

---

## 5. About tab

| Control | Source | Where it’s used |
|--------|--------|------------------|
| **Restart Onboarding Wizard** | `onRestartOnboarding` (from App) | App passes `() => setShowOnboarding(true)`; reopening onboarding. |

`onRestartOnboarding` is still passed into `SettingsPage` and used by the About section button.

---

## 6. Props from App.tsx

- `onRestartOnboarding={() => setShowOnboarding(true)}` — used in About.
- `onRepairComplete={async () => { await refreshSystemHealth(); }}` — used after Repair Keyring and Unlock Pacman.

Both are still accepted by `SettingsPage` and used in the refactored code.

---

## Verification checklist (post-refactor)

- [x] Theme: `setThemeMode` still called from General → Appearance.
- [x] Reduce password prompts: `setReducePasswordPrompts` still called from General → Security & Privacy.
- [x] Telemetry: `toggleTelemetry` still called from General → Security & Privacy.
- [x] Sources: `SourcesTab` still rendered with no props; uses `useSettings()` and invoke internally.
- [x] Builder: `BuilderTab` still rendered with no props; uses `useAppStore()` and invoke internally.
- [x] Maintenance: all repair handlers and `toggleAdvancedMode` / `ConfirmationModal` still in place.
- [x] About: `onRestartOnboarding` still passed and used.
- [x] Repair callbacks: `onRepairComplete` still called after keyring repair and unlock.

Settings will work correctly and continue to update the parts of the app they control; the refactor only changed layout, navigation, and content transitions, not these wirings.

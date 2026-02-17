# Updates Page — Review

**Date:** 2026-02-05  
**File:** `src/pages/UpdatesPage.tsx`

---

## 1. Purpose and data flow

- **Purpose:** Show pending updates (repo, AUR, Flatpak), grouped by source; run “Update All” (with optional critical-news gate); show progress, reboot/pacnew/service-restart notices, and post-update orphan cleanup.
- **Data:** `check_updates(include_aur, include_flatpak)` on mount and when AUR/Flatpak settings change; list is deduplicated by `name:source_type:id`. Update All calls `perform_system_update(password, include_aur, include_flatpak)` and listens for `update-complete`.

---

## 2. What works well

- **Settings alignment:** Uses `isAurEnabled` and `isFlatpakEnabled` from `useSettings()` for both the check and the perform call, so the list and “Update All” match the user’s Sources toggles.
- **Grouping:** Updates are grouped by source (System repos, AUR, Flatpak) with section headers and `RepoBadge` per row.
- **Critical news gate:** Update All can be gated by unread critical news (`CriticalNewsBlockerModal`); user must acknowledge or proceed.
- **Progress UX:** Four-step stepper (Sync DBs → Upgrade system → AUR → Flatpak), progress bar, status text, optional “Show Process Details” log panel, and auth hint after 5s.
- **Post-update:** Reboot required, .pacnew warnings, and pending service restarts are shown with clear CTAs; orphan cleanup is offered after a successful update.
- **Lock/busy handling:** Detects lock/busy errors and shows a “Fix It” button that calls `repair_unlock_pacman`.
- **UI:** Sticky header, semantic colors, dark mode, `AnimatePresence` for panels, responsive layout. Copy-command button for “Update in terminal.”

---

## 3. Fixes applied

1. **Password not passed to update:** The confirmation modal showed a password field when AUR updates were present, but `performUpdate` always passed `password: null`. It now passes the modal’s `password` (trimmed, or null if empty) and clears it after invoking.
2. **launch_app param consistency:** Reboot buttons used `pkg_name` in one place; all now use `pkgName` to match the Rust command and the rest of the app. `.catch(() => {})` added so failures don’t throw.
3. **refreshPendingUpdates after service restart:** After “Restart Now” for services, the store’s `refreshPendingUpdates()` was called with no args (defaulting to both AUR and Flatpak true). It now calls `refreshPendingUpdates(isAurEnabled, isFlatpakEnabled)` so the sidebar badge matches the user’s Sources settings.

---

## 4. Minor notes and limitations

- **Size estimate:** Subtitle uses `(updates.length * 1.5).toFixed(1) MB` as a placeholder. Real total download size would require backend support (e.g. sum of package sizes from repos/Flatpak).
- **Reboot button:** `launch_app({ pkgName: 'reboot' })` is best-effort (e.g. gtk-launch or a .desktop containing “reboot”). It may not exist on all systems; harmless if it fails.
- **Duplicate reboot/pacnew messaging:** Reboot and .pacnew are shown both in the “System Status Indicators” block (top) and again in the “Reboot & Pacnew Warnings” block (AnimatePresence). Consider consolidating to a single section to avoid repetition.
- **update-complete listener:** Effect depends on `setUpdating` and `setPacnewWarnings` in the dependency array; `checkForUpdates` is used inside the listener but not in the array (stable by reference). Fine as-is; if the listener were to close over changing `checkForUpdates`, a ref could be used.
- **ConfirmationModal password:** When `showPasswordInput` is true, the modal shows “Required for AUR builds & system updates.” The backend uses the password for sudo/makepkg during the AUR phase; now that we pass it, the flow is correct.

---

## 5. Summary table

| Area | Status | Notes |
|------|--------|--------|
| Check/perform with AUR/Flatpak settings | OK | Both use isAurEnabled, isFlatpakEnabled |
| Grouping by source | OK | Repo / AUR / Flatpak sections + RepoBadge |
| Update All + critical news gate | OK | Blocker modal then confirm |
| Password for AUR | Fixed | Modal password now passed to perform_system_update |
| Progress stepper and log | OK | 4 steps, progress %, expandable log |
| Reboot / pacnew / services / orphans | OK | Banners and CTAs; service restart now refreshes with settings |
| Lock/busy Fix It | OK | repair_unlock_pacman, then re-check |
| launch_app param | Fixed | pkgName consistently; reboot button non-throwing |
| refreshPendingUpdates after actions | Fixed | Service restart passes isAurEnabled, isFlatpakEnabled |

The Updates page is feature-complete and aligned with the update system (see `docs/UPDATE_SYSTEM_REVIEW.md`). The only behavioral fixes were passing the modal password, using `pkgName` for reboot, and passing AUR/Flatpak settings into `refreshPendingUpdates` after restarting services.

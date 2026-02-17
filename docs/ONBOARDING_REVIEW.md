# Onboarding System Review

**Date:** 2026-02-05  
**Scope:** First-run and repair-triggered onboarding flow, persistence, and integration with App startup and Settings.

---

## 1. Overview

The onboarding system guides new users through source selection (Flatpak, AUR, Chaotic-AUR), security preferences (reduce password prompts, telemetry), and appearance (theme, accent). It can be shown on first launch, when system defects are detected, or when the user chooses "Restart Onboarding Wizard" in Settings.

**Key files:**
- **Frontend:** `src/components/OnboardingModal.tsx` (steps, toggles, Chaotic-AUR sub-flow, finish)
- **App orchestration:** `src/App.tsx` (when to show, System Fix popup, completion handler)
- **Backend health:** `src-tauri/monarch-gui/src/repair.rs` (`check_initialization_status`)

---

## 2. Flow Summary

### 2.1 Startup decision (App.tsx)

1. **Unlock:** If `needs_startup_unlock` and reduce-password mode, ask for password and call `unlock_pacman_if_stale`; otherwise call without password (Polkit).
2. **Health:** `check_initialization_status()` returns `needs_policy`, `needs_keyring`, `needs_migration`, `needs_sync_db_repair`, `is_healthy`, `reasons`.
3. **Completion flags:**  
   - `isCompleted = localStorage.getItem('monarch_onboarding_v3')`  
   - `legacyCompleted = monarch_onboarding_v2_final || monarch_onboarding_completed`
4. **Grandma-proof branch:**
   - If **only** issue is sync DB repair (single reason mentioning sync/database): set `pendingDbRepair = true` → user sees one-step "Package databases need a quick fix" overlay (no full onboarding).
   - If **other** defects (policy, keyring, etc.): set `onboardingReason`, `showSystemFixPopup = true`, `redoOnboarding = true`.
5. **Show onboarding:** If `!isCompleted && !legacyCompleted` → `setShowOnboarding(true)`. If unhealthy and not only sync DB, we also set the popup and reason as above.

### 2.2 System Fix popup (before onboarding)

When `showSystemFixPopup && onboardingReason`:
- **ConfirmationModal** shows "System Setup Required" with `onboardingReason` (e.g. "MonARCH detected system defects: … MonARCH will attempt to fix them on launch. …").
- Both **Close** and **Continue to Setup** dismiss the popup and then `setShowOnboarding(true)`.
- There is no "skip and never show onboarding" when defects exist; skipping just closes the popup and still shows onboarding.

### 2.3 Onboarding modal

- **Condition:** `showOnboarding && !showSystemFixPopup`.
- **Props:** `onComplete={handleOnboardingComplete}`, `reason={onboardingReason}`.
- **Steps (distro-dependent):**
  - **With Chaotic-AUR support (e.g. Garuda, CachyOS):** welcome → sources → chaotic → security → theme → confirm.
  - **Without (e.g. Manjaro):** welcome → sources → security → theme → confirm (chaotic step omitted).
- **Finish (handleFinish):**
  - `invoke('set_aur_enabled', { enabled: isAurEnabled })`
  - `localStorage.setItem('flatpak-enabled', String(isFlatpakEnabled))`
  - `localStorage.setItem('monarch_onboarding_v4', 'true')`
  - `invoke('set_one_click_enabled', { enabled: oneClickEnabled, password: null })`
  - `setTelemetry(localTelemetry)`, `setReducePasswordPrompts(oneClickEnabled)`
  - Aptabase `onboarding_completed` event
  - 600 ms delay then `onComplete()`

### 2.4 Completion (App.tsx)

- **handleOnboardingComplete:**  
  - `localStorage.setItem('monarch_onboarding_v3', 'true')`  
  - `setShowOnboarding(false)`  
  - `refreshSystemHealth()` so the infrastructure banner updates.

---

## 3. Persistence and Versions

| Key | Set by | Read by |
|-----|--------|--------|
| `monarch_onboarding_v3` | App.tsx (handleOnboardingComplete) | App.tsx (startup: isCompleted) |
| `monarch_onboarding_v4` | OnboardingModal (handleFinish) | Nothing (legacy v3 is the gate) |
| `monarch_onboarding_v2_final` / `monarch_onboarding_completed` | (legacy) | App.tsx (legacyCompleted) |
| `flatpak-enabled` | OnboardingModal, useSettings | useSettings (initial state) |
| `monarch_reduce_password_prompts` | internal_store (setReducePasswordPrompts) | internal_store, SessionPasswordContext |

**Note:** Both v3 and v4 are set after a successful completion; only v3 (and legacy) are used to decide whether to show onboarding. Unifying to a single version key (e.g. v4) would simplify the story; low priority.

---

## 4. Integration with Settings

- **Sources:** Onboarding uses `useSettings()` for `isAurEnabled`, `toggleAur`, `isFlatpakEnabled`, `toggleFlatpak`. Same state as Settings → Sources; no duplication.
- **Reduce password prompts / Telemetry:** Stored in Zustand (`useAppStore`) and persisted (localStorage for reduce prompts; backend for one-click). Onboarding and Settings both read/write the same store.
- **Restart Onboarding:** Settings → "Restart Onboarding Wizard" calls `onRestartOnboarding` → `setShowOnboarding(true)`. No clearing of completion flags; after the user completes again, v3/v4 are set again. Correct.

---

## 5. Chaotic-AUR sub-flow

- **Step "chaotic":** User can click "Install Keys & Mirrors" → `requestSessionPassword()` → `invoke('prepare_chaotic_components', { password })` → on success, `setShowChaoticFinalModal(true)`.
- **Final step modal:** Shows snippet for `/etc/pacman.conf`; Copy and "Check Connection" (`force_refresh_databases` + `check_chaotic_status`). If `chaotic_in_alpm`, modal closes. User can skip and configure later in Settings → Sources.
- **Manjaro:** Chaotic step is omitted; sources step shows a disabled Chaotic-AUR row with "Incompatible with Manjaro Stable Branch."

### 5.1 Updates (2026-02-08)

- **Already enabled:** When the Chaotic step is shown, the app calls `check_chaotic_status`; if **native** (Garuda/CachyOS) or **chaotic_in_alpm** is true, the step shows "Chaotic-AUR is already enabled on your system" and a "Ready to use" state instead of the "Install Keys & Mirrors" button. After "Check Connection" in the Final Step modal, `chaoticAlreadyInAlpm` is set so the main step reflects success.
- **Sources step:** The Chaotic row (when not supported) is now driven by **capability** `!supportsChaotic` (not only `distro.id === 'manjaro'`), with text "Not available on this distro (incompatible with this system)."
- **Final Step modal copy:** Instructions updated for new Linux users: "Open /etc/pacman.conf in a text editor (e.g. sudo nano /etc/pacman.conf), add the two lines below at the end, save and exit, then click Check Connection." Same wording used in Settings → Sources Chaotic modal.
- **Security step:** Wording clarified: "Choose how you authorize installs and updates: one prompt per session (recommended for most users) or a system dialog every time (for advanced control)." Subtext: "One prompt per session (recommended). Turn off to use the system auth dialog every time."

---

## 6. Backend health (repair.rs)

`check_initialization_status()` drives both the System Fix popup and the decision to show onboarding when unhealthy:

- **Policy:** Polkit policy and helper binary present under `/usr/share/polkit-1/actions` and `/usr/lib/monarch-store/`.
- **Keyring:** `/etc/pacman.d/gnupg` exists.
- **Migration:** Not enforced (host-adaptive; always false).
- **Sync DB:** `check_sync_db_corrupt()` (with persistent and in-memory cache). If corrupt, adds reason about "Pacman sync databases are corrupt…".

`is_healthy` is false if any of these need attention. The frontend uses this plus `reasons` and the "only sync DB" heuristic to choose between the one-step DB repair overlay and the full onboarding + system-fix message.

---

## 7. Accessibility and UX

- **Modal:** `role="dialog"`, `aria-modal="true"`, `aria-labelledby="onboarding-title"`. Focus trap and Escape to close (calls `onComplete`).
- **Steps:** Progress dots; "Step X of Y" in footer; Back/Next; last step "Start Using MonARCH".
- **Chaotic final modal:** Same pattern; Copy and Check Connection; can be dismissed without completing Chaotic.

---

## 8. Recommendations

1. **Use `reason` on welcome step:** The modal receives `reason={onboardingReason}` but does not display it. When the user has just dismissed the System Fix popup, showing a short line on the welcome step (e.g. "We've detected some system issues we'll help you fix.") improves context. *Implemented.*
2. **Version key:** Consider using a single completion key (e.g. `monarch_onboarding_v4`) for both setting and reading, and deprecate v3 in a future cleanup. *Implemented: v4 is primary; v3/legacy read for backward compat; completion sets only v4.*
3. **Skip when defects:** If product wants a true "skip onboarding" when defects exist, the System Fix popup would need a path that sets a "user declined setup" flag and does not call `setShowOnboarding(true)` on skip. *Implemented: Skip sets `monarch_declined_system_setup` and does not show onboarding; next launch user sees main app with repair banner.*

---

## 9. Summary

- **Triggers:** First run (no v3/legacy), unhealthy system (with System Fix popup when not only sync DB), or user-triggered "Restart Onboarding Wizard."
- **Persistence:** v3 is the gate for "onboarding done"; v4 and flatpak/reduce prompts/telemetry are set in modal; Settings and onboarding share the same source and security state.
- **Repair paths:** One-step DB repair overlay vs full onboarding with system-fix message; both feed from `check_initialization_status()`.
- **Completion:** Modal sets backend and localStorage; App sets v3 and refreshes system health so the infra banner reflects the new state.

# Settings & Config UX Audit

**Last updated:** 2025-01-29 (v0.3.5-alpha.1)

**Role:** Principal UX (Distro-Aware system utilities)  
**Scope:** SettingsPage.tsx, repository management, sync and repair flows.

---

## 1. Repository Management & "Soft Disable" Logic

### 1.1 Identity Matrix (Hidden vs Active)

| Aspect | Current state | Assessment |
|--------|----------------|------------|
| **Copy** | "Toggling a source here **hides it from the Store** but keeps it active in the system. Your installed apps **continue to update safely**." | ✅ Clear and accurate. |
| **Visual state** | Enabled = blue toggle + "Primary" badge; Disabled = muted card + grey toggle. | ⚠️ "Hidden" is implied by muted style; no explicit "Hidden from Store / Active in system" label per repo. |
| **Recommendation** | Add a short state label per card: e.g. "Visible in Store" vs "Hidden from Store · Still receives updates". | Reduces confusion about what "off" means. |

### 1.2 State Conflict & Guardrails (Distro-Aware)

| Aspect | Current state | Assessment |
|--------|----------------|------------|
| **Hard block (e.g. Chaotic-AUR on Manjaro)** | Badge: "Blocked by {distro}". Toggle disabled, red styling. On click: `reportWarning` toast. | ✅ Non-intimidating; user cannot enable. |
| **Explanation** | Toast: "This repository is incompatible with {distro}." | ⚠️ No in-context explanation of *why* (glibc/kernel mismatch). PackageDetailsFresh has richer copy; Settings could link or repeat a one-liner. |
| **Recommendation** | Keep guardrails; add optional tooltip or expandable "Why is this blocked?" with 1–2 sentences (glibc/library mismatch, risk of breakage). | |

### 1.3 Sync Feedback

| Aspect | Current state | Assessment |
|--------|----------------|------------|
| **UI** | "Sync Now" → button shows "Syncing..." + spinner; progress bar animates. | ⚠️ Generic; no step-by-step (GPG, DB refresh, Chaotic-AUR fetch). |
| **Backend** | Emits `sync-progress` ("Syncing repositories...", "Updating {repo}...", "Fetching Chaotic-AUR metadata...", "Initialization complete."). | ✅ Events exist; only LoadingScreen listens. |
| **Recommendation** | On Settings, listen to `sync-progress` and show current step (e.g. "Updating core...", "Fetching Chaotic-AUR metadata...") so users see GPG/db progress instead of a generic spinner. | Implemented in this audit. |

---

## 2. Information Architecture & Grouping

### 2.1 Logical Clusters

| Section | Contents | Assessment |
|---------|----------|------------|
| **Repository Control** | Sync Now, repo counts, Auto Sync Interval. | ✅ Clear. |
| **Software Sources** | Repo cards (soft disable), AUR. | ✅ Grouped; "Soft disable" copy at top. |
| **Workflow & Interface** | Notifications, Initial Setup wizard. | ✅ Logical. |
| **Appearance** | Theme, Accent. | ✅ Logical. |
| **System Management** | One-Click Auth, Maintenance (Unlock, Keyring, Cache, Orphans). | ✅ Good; "Maintenance & Repair" is clear. |
| **Privacy & Data** | Telemetry. | ✅ Clear. |
| **Advanced Configuration** | God Mode / Distro-safety bypass. | ✅ Clearly separated (danger zone). |

**Verdict:** Smart Essentials / Hardware Optimization are not separate section titles; "System Management" and "Maintenance & Repair Tools" cover optimization and repair. Grouping is intuitive; no major re-org required.

### 2.2 Progressive Disclosure (Advanced)

| Aspect | Current state | Assessment |
|--------|----------------|------------|
| **Advanced Mode** | Confirmation modal before enabling: critical warning, glibc/Manjaro/Arch caveats. | ✅ High-risk; secondary confirmation present. |
| **Reset Keyring / Unlock** | Buttons in Maintenance grid; Keyring/Unlock trigger backend (and may prompt for password). | ✅ Not hidden; no extra modal (acceptable for repair actions). |
| **Manual path override** | Not present in Settings. | N/A; if added later, hide behind Advanced + confirmation. |

### 2.3 Adaptive Layouts

| Aspect | Current state | Assessment |
|--------|----------------|------------|
| **Cards** | `grid-cols-1 md:grid-cols-2` for repo cards; `md:grid-cols-3` for health; `xl:grid-cols-2` for Workflow/Appearance. | ✅ Responsive; avoids smushing on small windows. |
| **Density** | Padding and spacing consistent; no compact/comfortable toggle. | ✅ Adequate; optional density toggle is nice-to-have. |

---

## 3. Visual Feedback & Consistency

### 3.1 Toggle Micro-interactions

| Aspect | Current state | Assessment |
|--------|----------------|------------|
| **Repo / AUR / Notifications / One-Click / Telemetry** | CSS `transition-transform duration-300` on knob; immediate visual flip; backend persist async. | ⚠️ No explicit "Saved to config" feedback (e.g. checkmark or brief toast). Rollback on error exists. |
| **Recommendation** | Optional: very brief "Saved" state (e.g. check icon for 1s) or rely on existing success toasts where they exist. Low priority. | |

### 3.2 Contrast & Readability (Dark Mode)

| Aspect | Current state | Assessment |
|--------|----------------|------------|
| **Technical copy** | e.g. AVX-512 / CPU optimization in system info: `text-slate-500 dark:text-white/50`, `text-slate-600 dark:text-white/60`. | ✅ Sufficient contrast on glassmorphic panels. |
| **Danger zone** | Red text and borders with dark bg: `text-red-600 dark:text-red-400`, `bg-red-50 dark:bg-red-500/5`. | ✅ Readable. |

### 3.3 Empty & Error States (Corrupted DB)

| Aspect | Current state | Assessment |
|--------|----------------|------------|
| **Smart Repair CTA** | `SystemHealthSection` exposes `check_system_health` and shows Critical/Warning issues with action buttons (e.g. "Fix System Keys"). | ⚠️ **SystemHealthSection is not rendered anywhere** (Settings uses its own "Maintenance & Repair Tools" grid). So health-check–driven "Smart Repair" is not visible on Settings. |
| **Recommendation** | Either (a) render `SystemHealthSection` on Settings (e.g. above or below System Management), or (b) call `check_system_health` from Settings and show a compact banner + primary CTA when issues exist (e.g. "Keyring needs repair" → "Fix now"). | Ensures corrupted DB / keyring issues are visible and actionable. |

---

## 4. Accessibility & Friction

### 4.1 Custom Mirror / URL Inputs

| Aspect | Current state | Assessment |
|--------|----------------|------------|
| **Custom mirror / manual path** | No URL or path override fields on Settings. | N/A. If added later: real-time URL validation (format, HTTPS) before invoking privileged helper. |

### 4.2 Keyboard Flow

| Aspect | Current state | Assessment |
|--------|----------------|------------|
| **Settings sidebar** | Main nav is Sidebar; Tab order follows DOM. | ✅ Standard flow. |
| **Repo list** | Reorder (Up/Down) and Toggle are buttons; Tab/Enter/Space work. | ✅ Focusable. |
| **Modals** | `ConfirmationModal` and `RepoSetupModal` use `useFocusTrap` + `useEscapeKey`. | ✅ Trapped focus and Escape to close. |
| **Gaps** | Repo cards: reorder and toggle lack `aria-label`; Sync Now has no `aria-label`. | ⚠️ Fixed in this audit (ARIA labels added). |

### 4.3 ARIA Labeling

| Element | Current state | Change |
|---------|----------------|--------|
| Repo toggle (per repo) | None | Add `aria-label` e.g. "Show {name} in Store" / "Hide {name} from Store" (and "Blocked by distro" when locked). |
| Sync Now button | None | Add `aria-label="Sync repositories now"` and `aria-busy` when syncing. |
| Reorder Up/Down | None | Add `aria-label="Move {name} up"` / `"Move {name} down"`. |
| AUR toggle | None | Add `aria-label="Enable AUR"` / `"Disable AUR"`. |

---

## 5. Friction Map (Where Users Get Confused)

High-risk confusion points related to **Distro-Aware** logic and **Soft Disable**:

1. **"Why is this repo greyed out?"**  
   - **Location:** Software Sources repo cards.  
   - **Risk:** User thinks the repo is "broken" or "uninstalled."  
   - **Mitigation:** Explicit state label: "Hidden from Store · Still receives updates" and optional "Why is this blocked?" for locked repos.

2. **"I turned it off but my system still has it."**  
   - **Location:** After disabling a repo.  
   - **Risk:** Expectation that "off" = removed from system.  
   - **Mitigation:** Existing top copy is good; per-card "Hidden from Store" reinforces that it stays active for updates.

3. **"Why can’t I enable Chaotic-AUR?"**  
   - **Location:** Manjaro (or other blocked distros).  
   - **Risk:** Perceived as a bug or arbitrary restriction.  
   - **Mitigation:** Keep "Blocked by {distro}" badge; add short explainer (library mismatch, avoid breakage).

4. **"What is Sync doing?"**  
   - **Location:** Sync Now during long run.  
   - **Risk:** User thinks the app is stuck.  
   - **Mitigation:** Show current step (e.g. "Updating core...", "Fetching Chaotic-AUR metadata...") via `sync-progress` listener.

5. **"Something is wrong with the database/keyring."**  
   - **Location:** After failed install or corrupted state.  
   - **Risk:** User doesn’t know where to repair.  
   - **Mitigation:** Surface health check on Settings (banner or SystemHealthSection) with clear "Fix" CTA.

---

## 6. Tailwind UI Refactor: Repository Toggle / Card

**Goals:** Clear state identity (Visible vs Hidden vs Blocked), better semantics, ARIA, and sync progress.

### 6.1 State Clarity (Tailwind)

- **Enabled:**  
  - Card: `bg-white dark:bg-white/5`, border, "Primary" badge when first.  
  - Label: **"Visible in Store"** (small, high-contrast).  
  - Toggle: blue track, knob right.

- **Disabled (soft):**  
  - Card: muted `bg-slate-100 dark:bg-black/20`, lower opacity.  
  - Label: **"Hidden from Store · Still receives updates"** (small).  
  - Toggle: grey track, knob left.

- **Blocked (distro):**  
  - Card: red tint `border-red-500/20`, badge "Blocked by {distro}".  
  - Label: **"Incompatible with your system"** (optional).  
  - Toggle: disabled, red track; `aria-disabled="true"` and `aria-label` explaining blocked.

### 6.2 Component Structure (Inline or Extracted)

- Keep repo list in SettingsPage; optionally extract a `RepoCard` component for readability.
- Each card: reorder buttons (Up/Down) + name + description + **state label** + toggle.
- Toggle: `role="switch"`, `aria-checked={enabled}`, `aria-label` as above, `aria-disabled` when locked.

### 6.3 Sync Section

- Add local state: `syncProgressMessage: string | null`.
- Listen to `sync-progress` while `isSyncing` is true; set message.
- Show under "Sync Now" / in the Sync card: e.g. "Syncing... Updating core..." or "Fetching Chaotic-AUR metadata...".
- Use `aria-live="polite"` for the progress text for screen readers.

---

## 7. Implementation Checklist (This Audit)

- [x] Friction Map and audit doc (this file).
- [x] Repository card: state labels ("Visible in Store" / "Hidden from Store · Still receives updates") and Blocked copy.
- [x] Sync: listen to `sync-progress` on Settings and show current step.
- [x] ARIA: repo toggles, Sync button, reorder buttons, AUR toggle.
- [ ] Optional follow-up: Render SystemHealthSection on Settings or add a small health-check banner for "Smart Repair" CTA.
- [ ] Optional: "Why is this blocked?" expandable for Chaotic-AUR on Manjaro.

# Settings Page — Full Review & Redesign

**Date:** 2026-02-05  
**File:** `src/pages/SettingsPage.tsx`

---

## 1. Previous design (tabbed)

- **Layout:** Sticky header "Mission Control" + desktop left sidebar (6 tabs) + mobile horizontal scrollable tab bar. Content area showed one tab at a time with `AnimatePresence` transition.
- **Tabs:** General, Sources, Storage, Native AUR Engine, Maintenance, About.
- **Issues:** Felt like a separate mini-app with its own navigation; didn’t match the rest of the app (Explore, Updates, Installed are single-scroll pages with no inner tabs). Extra cognitive load (where am I, how do I get to X). Desktop sidebar + mobile tab bar duplicated navigation patterns.

---

## 2. New design (single scroll)

Settings is now **one scrollable page** with clear sections, aligned with Updates and Installed:

- **Sticky header:** Same pattern as other pages — "Mission Control" with icon, title, and subtitle; `bg-app-bg/95 backdrop-blur-3xl`, `border-b border-black/5 dark:border-white/5`.
- **No tabs:** All content is in one flow. Sections are stacked in a single column with consistent spacing (`space-y-12`).
- **Section structure:** Each area has a **SectionHeader** (icon + title + optional description) and `scroll-mt-24` for future anchor links. Optional `id` on sections (e.g. `#appearance`, `#sources`) for deep links.
- **Order:** Appearance → Security & Privacy → Sources → Disk & Cache → Native AUR Engine → Maintenance & Repair → About.
- **Visual consistency:** Same glassmorphism and borders as the rest of the app (`bg-app-card/50`, `border-app-border`, `rounded-2xl`). Toggle and repair cards unchanged; SourcesTab, StorageTab, BuilderTab are embedded as-is (they already provide their own cards/sections).

---

## 3. What each section contains

| Section | Content |
|--------|--------|
| **Appearance** | Theme: Light / Dark / System (card grid). |
| **Security & Privacy** | One-Click Authentication, Anonymous Telemetry (toggle cards). |
| **Sources** | Full `SourcesTab`: Host System, Package Sources (Chaotic, Flatpak, AUR). |
| **Disk & Cache** | Full `StorageTab`: cache stats, clean options. |
| **Native AUR Engine** | Full `BuilderTab`: workspace path, build logs, clean build, parallel downloads, clear build cache. |
| **Maintenance & Repair** | Repair Keyring, Unlock Pacman, Sync Databases, System Cleanup (action cards); Advanced Repository Mode (danger card). |
| **About** | Logo, version, badges, description, Restart Onboarding; Installation & Host Kernel cards; license footer. |

---

## 4. Technical notes

- **Tab components:** `SourcesTab`, `StorageTab`, `BuilderTab` are unchanged; they are rendered inline. No wrapper cards around them so we don’t double-wrap their internal sections.
- **Modals:** `ConfirmationModal` for Advanced Mode is unchanged. SourcesTab’s Chaotic “Final step” modal remains inside SourcesTab.
- **State:** Removed `activeTab` and `setActiveTab`. All repair/clear state (e.g. `isRefreshingKeyring`, `isCleaningCache`) is unchanged.
- **Accessibility:** Section headers use semantic `<header>` and `<h2>`; buttons and toggles keep existing behavior. Optional: add a “Jump to section” list or anchor links later using the section `id`s.

---

## 5. Summary

| Before | After |
|--------|--------|
| Tab bar (desktop sidebar + mobile horizontal) | No tabs; single scroll |
| One section visible at a time | All sections in one scroll |
| Different navigation pattern from rest of app | Matches Updates/Installed/Explore (sticky header + scroll) |
| Page title changed per tab | Single “Mission Control” header |

The new layout fits the app’s Liquid UI and glassmorphism, reduces navigation complexity, and makes every setting discoverable by scrolling. No new dependencies; same tab components and modals.

---

## 6. Updates (2026-02-08)

- **Chaotic-AUR (Sources):** The "Final step" modal copy was updated for new Linux users: open `/etc/pacman.conf` in a text editor (e.g. `sudo nano /etc/pacman.conf`), add the two lines at the end, save and exit, then click **Check Again**. Same wording as in Onboarding.
- **Auth:** All privileged actions (including Chaotic prepare, sync, refresh) use **one-click** from `RepoManager`: when "Reduce password prompts" is on, the app uses the branded password dialog and `sudo -S`; when off, Polkit (`pkexec`) is used. See `docs/RECENT_CHANGES.md` §8 and `docs/STARTUP_AND_PERMISSIONS_REVIEW.md`.

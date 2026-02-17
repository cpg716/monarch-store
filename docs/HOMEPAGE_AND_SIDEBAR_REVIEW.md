# Homepage and Sidebar — Full Review

**Date:** 2026-02-05  
**Files:** `src/pages/HomePage.tsx`, `src/components/Sidebar.tsx`; integration in `App.tsx`, `HeroSection`, `CategoryGrid`, `TrendingSection`, `MobileNav`.

---

## Part 1: Homepage (Explore)

### 1.1 Role and layout

- **Route:** Shown when `activeTab === 'explore'` and no search query. Rendered below `HeroSection` and the sticky search bar.
- **Sections (top to bottom):**
  1. **Recommended Essentials** — Section header + “See All →” + `TrendingSection` with `filterIds={essentials}`, `limit={7}`, `variant="scroll"`, `preloadedPackages={essentialsPackages}`, `hideHeader`. Essentials come from `useSmartEssentials(essentialsList)`; list and package data are cached at app startup in App (essentialsList, essentialsPackages) so returning to Explore doesn’t refetch.
  2. **Trending Applications** — Section header + “See All →” + `TrendingSection` with no filterIds, `limit={7}`, `variant="scroll"`. Fetches via `get_trending` when no preloadedPackages.
  3. **Browse by Category** — `CategoryGrid` with `onSelectCategory`; clicking a category or a popular pill calls `onSelectCategory(cat.id)` and switches to category view (App sets `setSelectedCategory` and shows CategoryView or similar).

### 1.2 Banners

- **Alpha notice:** Dismissible; state persisted to `localStorage` (`monarch_alpha_notice_dismissed`). Explains experimental alpha and advises care on production.
- **Offline banner:** Shown when `!isOnline` (from `useOnlineStatus()`) and not dismissed. Resets when back online. Explains cached data and that install/update may fail without connectivity.

### 1.3 Data flow

- **Essentials:** App’s `initializeStartup` calls `get_essentials_list`, shuffles the list, sets `essentialsList`; then `get_packages_by_names(names: list)` and sets `essentialsPackages`. HomePage receives both; `useSmartEssentials(essentialsList)` returns the same list (or from cache). First TrendingSection uses `filterIds={essentials}` and `preloadedPackages={essentialsPackages}` so the section can render without a second fetch; TrendingSection reorders preloaded packages by filterIds when both are provided.
- **Trending:** Second TrendingSection has no filterIds/preloadedPackages, so it fetches `get_trending` and shows up to 7 in scroll variant.

### 1.4 What works well

- **Liquid UI:** No fixed column counts in HomePage; `space-y-12`, responsive padding; semantic colors and dark mode (`text-slate-900 dark:text-white`, etc.).
- **Performance:** Essentials loaded once at startup; preloadedPackages avoids duplicate requests when opening See All (essentials) or returning to Explore.
- **Accessibility:** Dismiss buttons have `aria-label`; section structure is clear.
- **Consistency:** “See All” uses `text-accent`; section headers use icon + title + subtitle pattern.

### 1.5 Minor notes

- **CategoryGrid selected state:** `CategoryGrid` accepts optional `selectedCategoryId`; HomePage doesn’t pass it. If the app navigates back to Explore after selecting a category, the grid won’t highlight the last-selected category. Acceptable if selection is not persisted or is handled elsewhere (e.g. URL).
- **TrendingSection dependency:** The effect in TrendingSection uses `JSON.stringify(filterIds)` in the dependency array; that’s a simple way to stabilize deps when filterIds is an array. Works; could use a ref or deep comparison if needed later.

---

## Part 2: Sidebar

### 2.1 Role and layout

- **Placement:** Left side of main layout; width animates between 80px (collapsed) and 260px (expanded) via Framer Motion.
- **Sections:**
  1. **Logo** — Icon + “MonARCH” + tagline when expanded; icon only when collapsed.
  2. **Tabs** — Search, Explore, Installed, Favorites, Updates, News, Settings. Each has icon, label (when expanded), optional badge (Updates only: `pendingUpdates.total`).
  3. **Bottom** — Collapse/Expand toggle (ChevronLeft / ChevronRight).

### 2.2 Behavior

- **Expand state:** Initial value from `localStorage` (`monarch_sidebar_expanded`); saved on change. Auto-collapse when `window.innerWidth < 1024` (resize listener).
- **Active state:** Highlight via `bg-blue-600/10 text-blue-500`, `layoutId="activeTabGlow"` and `layoutId="activeTabStrip"` for shared layout animation, plus left strip and icon stroke/drop-shadow.
- **Collapsed tooltip:** On hover when collapsed, a floating tooltip shows label + desc to the right of the sidebar (`pointer-events-none` so it doesn’t block clicks).
- **Badge:** Only the Updates tab has `badge: pendingUpdates.total`. Red dot shown when `tab.badge != null && tab.badge > 0`; positioned absolute top-right of the button.

### 2.3 Fix applied

- **Typing:** Tabs are now typed as `SidebarTab[]` with optional `badge?: number`. Replaced `(tab as any).badge` with `tab.badge != null && tab.badge > 0` so the badge check is type-safe.

### 2.4 What works well

- **Responsive:** Collapse on small viewports; expanded state persisted so desktop users keep preference.
- **Visual feedback:** Active tab, hover, and badge are clear; motion is smooth.
- **Store integration:** `pendingUpdates` from Zustand; updated by `refreshPendingUpdates` (e.g. after updates or service restarts) and by background update checker.
- **Accessibility:** Buttons have `aria-label` (tab label or “Collapse/Expand sidebar”).

### 2.5 Minor notes

- **Badge position when collapsed:** Button is `justify-center` with icon only; badge at `top-3 right-3` (or `top-4 right-4` when expanded) sits at the edge of the pill. Works; ensure overflow isn’t clipped (parent has no overflow-hidden).
- **Theme tokens:** Active/hover use `blue-600/10`, `blue-500`; could be switched to app accent tokens (e.g. `app-accent`) for theme consistency if the design system defines them.

---

## Part 3: Integration (App, HeroSection, MobileNav)

### 3.1 App

- **Explore content:** When `activeTab === 'explore'` and no search, App renders HeroSection (sticky top), then search bar, then HomePage with `onSelectPackage`, `onSeeAll`, `onSelectCategory`, `onOpenSettings`, `essentialsList`, `essentialsPackages`. View-all and category flows are handled by App state (`viewAll`, `selectedCategory`).
- **Tab change:** `handleTabChange` clears selected package/category/view and, for Search tab, focuses the search input after a short delay.

### 3.2 HeroSection

- Shown only on Explore (no search). Distro-aware badge (CachyOS, Manjaro, Garuda, EndeavourOS, default Arch), logo, tagline. Loading skeleton when distro is loading. Complements the sidebar “MonARCH” branding.

### 3.3 MobileNav (removed)

- **Removed:** The bottom navigation bar (MobileNav) was removed. The app targets Arch Linux distros (desktop/laptop); the sidebar already collapses to icon-only at &lt;1024px, so a second navigation bar was redundant and consumed vertical space. Navigation is now sidebar-only at all breakpoints.

---

## Summary tables

### Homepage

| Area | Status | Notes |
|------|--------|--------|
| Essentials section | OK | Cached list + preloaded packages; See All uses same cache |
| Trending section | OK | Fetches get_trending; limit 7, scroll variant |
| Category grid | OK | Browse by category; popular pills and cards both call onSelectCategory |
| Alpha / offline banners | OK | Dismissible; alpha persisted; offline resets when online |
| Liquid UI / responsive | OK | No fixed grids; semantic colors; dark mode |
| selectedCategoryId | Optional | Not passed; category grid doesn’t show selection on return |

### Sidebar

| Area | Status | Notes |
|------|--------|--------|
| Expand/collapse | OK | Persisted; auto-collapse &lt;1024px |
| Tabs and navigation | OK | 7 tabs; active state and layout animation |
| Updates badge | OK | pendingUpdates.total; type-safe check |
| Collapsed tooltip | OK | Label + desc on hover |
| Typing | Fixed | SidebarTab[] with optional badge |
| Accessibility | OK | aria-labels on buttons |

---

Overall, the homepage and sidebar are consistent with the rest of the app (Liquid UI, app tokens where used, caching, and store-driven badge). Code changes: Sidebar tab typing and type-safe badge check; MobileNav removed (desktop-focused Arch Linux app — sidebar-only navigation at all breakpoints). Optional improvements: selectedCategoryId, theme tokens.

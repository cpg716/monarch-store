# News System Review

**Date:** 2026-02-05  
**Scope:** Operation Town Crier — distro-aware news aggregation, critical-news safety gate on Updates, and read-state persistence.

---

## 1. Overview

The news system fetches RSS/Atom feeds from the user’s distro (and Flathub), normalizes items, and marks as **critical** any title containing "manual intervention", "stable update", "security", "cve", or "vulnerability". Critical items are shown first and, on the **Updates** page, block "Update All" until the user acknowledges them (or has already marked them read in the News tab).

**Key files:**
- **Backend:** `src-tauri/monarch-gui/src/commands/news.rs` (feed specs, fetch, parse, categorize)
- **Frontend:** `src/components/NewsFeed.tsx` (list, expand, mark read, DOMPurify), `src/components/CriticalNewsBlockerModal.tsx` (safety gate), `src/pages/NewsPage.tsx`, `src/pages/UpdatesPage.tsx` (fetch news, blocker gate before Update All)

---

## 2. Backend (news.rs)

### 2.1 Data model

- **NewsCategory:** `Critical` | `System` | `Discovery` (serialized lowercase).
- **NewsItem:** `id`, `title`, `link`, `pub_date`, `source_label`, `is_critical`, `category`, `content` (optional).

### 2.2 Feed selection

- **Flathub** is always included: `https://flathub.org/api/v2/feed/new` (RSS).
- **Distro-specific** (by `distro_id`):
  - **arch:** Arch News feed.
  - **manjaro:** Manjaro forum announcements RSS.
  - **garuda:** Garuda forum announcements RSS.
  - **endeavouros:** EndeavourOS feed.
  - **cachyos:** CachyOS forum atom + Arch News.
  - **default:** Arch News.

### 2.3 Fetch and parse

- **fetch_one_feed:** HTTP GET with 10s timeout, User-Agent `MonARCH-Store/1.0`, then `feed_rs::parser::parse` on the response body. Failures (non-2xx, read error, parse error) log and return empty vec; no hard error.
- **Category:** Flathub → `Discovery`; others → `System`. Any item whose title matches `is_critical_title` is upgraded to `Critical`.
- **ID:** entry.id, or link, or `label:title_prefix`.
- **Sort:** Category order (Critical → System → Discovery), then by date descending (RFC2822).

### 2.4 Critical detection

- **is_critical_title(title):** true if title (lowercased) contains: `"manual intervention"`, `"stable update"`, `"security"`, `"cve"`, `"vulnerability"`.

### 2.5 Tauri command

- **fetch_news(State<DistroContext>):** Builds reqwest client (12s timeout), fetches all feed specs in parallel via `join_all`, concatenates, sorts, returns `Vec<NewsItem>`.

---

## 3. Frontend

### 3.1 NewsFeed.tsx

- **Storage:** Read state in `localStorage` under `monarch_read_news` (JSON array of IDs, capped at 500).
- **getReadNewsIds():** Exported; returns current read IDs from localStorage (used by UpdatesPage for the gate).
- **Fetch:** On mount, `invoke('fetch_news')`; loading and error states; "Try Again" on error.
- **Grouping:** Items grouped by `category` (critical, system, discovery); sections rendered in that order with distinct styling.
- **Cards:** Expand/collapse; mark read on click; "Read Full Article" opens link (opener or window.open). Content is sanitized with DOMPurify (allowed tags/attrs limited).
- **Props:** `limit`, `compact`, `onItemOpen` (optional callback when a item is opened).

### 3.2 NewsPage.tsx

- Full-page view: header "News & Announcements" and one `<NewsFeed />` (no limit).

### 3.3 CriticalNewsBlockerModal.tsx

- **When:** Shown when the user clicks "Update All" and there is at least one **unread critical** item (critical and id not in `getReadNewsIds()`).
- **Content:** List of critical item titles (links open in browser); checkbox "I have read these and understand the risks"; Cancel / Proceed with Update.
- **Proceed:** Only enabled when checkbox is checked; on Proceed, calls `onProceed()` then closes. The parent (UpdatesPage) passes an onProceed that marks those critical items as read via `markNewsItemsAsRead` then runs the update, so the blocker does not reappear for the same items.

### 3.4 UpdatesPage.tsx

- **On mount:** Fetches news via `fetch_news` and stores in `newsItems`.
- **handleUpdateAll:** Reads `getReadNewsIds()`, filters `newsItems` to unread critical; if any, sets `unreadCriticalItems` and opens `CriticalNewsBlockerModal`; otherwise opens the normal update confirmation.
- **Blocker onProceed:** Marks acknowledged critical items as read via `markNewsItemsAsRead(unreadCriticalItems.map(i => i.id))`, then calls `performUpdate()`.

---

## 4. Permissions

- **fetch_news** is in **app-commands-read** (read-only aggregation from public feeds).

---

## 5. Security and robustness

- **Content:** Feed HTML is sanitized with DOMPurify before being rendered (XSS mitigation).
- **Links:** Opened via Tauri opener or `window.open(..., 'noopener')`.
- **Network:** Backend uses timeout and does not fail the whole fetch if one feed fails; failed feeds contribute empty lists.
- **Read state:** Stored only in localStorage (no backend); cap at 500 IDs to avoid unbounded growth.

---

## 6. Recommendations and fix

1. **Mark critical as read on Proceed:** When the user acknowledges critical news in the blocker and clicks "Proceed with Update", mark those items as read so the gate does not show again for the same items. *Implemented: `markNewsItemsAsRead(ids)` exported from NewsFeed; UpdatesPage calls it in onProceed before performUpdate.*
2. **Flathub feed:** Backend uses `feed_rs` (RSS/Atom). Flathub’s `/api/v2/feed/new` is documented as RSS; if the API ever returns JSON only, this feed would parse as empty; worth a quick manual check or a comment in code.
3. **Optional:** Refresh news when the user focuses the Updates page (or News tab) so the gate uses up-to-date data without a full app reload.

---

## 7. Summary

| Area | Behavior |
|------|----------|
| **Source** | Distro-specific feeds + Flathub; selected by `DistroContext`. |
| **Critical** | Title contains manual intervention / stable update / security / cve / vulnerability. |
| **Read state** | localStorage `monarch_read_news` (IDs, max 500); NewsFeed marks read on open. |
| **Gate** | UpdatesPage blocks "Update All" if there are unread critical items; modal forces acknowledge; after fix, proceed marks those items read. |
| **UI** | News tab = full feed; Updates = fetch + gate; content sanitized (DOMPurify). |

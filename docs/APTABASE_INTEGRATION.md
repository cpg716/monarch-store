# Aptabase Integration — MonARCH Store

**Last updated:** 2025-01-31 (v0.3.5-alpha)

This document describes how MonARCH Store integrates with [Aptabase](https://aptabase.com) for privacy-respecting, opt-in analytics.

---

## 1. Overview

- **Purpose:** Anonymous usage analytics (app starts, search, installs, onboarding, errors) to improve the product.
- **Privacy:** Telemetry is **opt-in**. The user enables it in **Onboarding** (Privacy step) or **Settings → Privacy**. No events are sent unless telemetry is enabled.
- **Stack:** [tauri-plugin-aptabase](https://github.com/aptabase/tauri-plugin-aptabase) (bundled in `src-tauri/monarch-gui/plugins/tauri-plugin-aptabase`), app key `A-US-1496058535` (US region). Events are sent to `https://us.aptabase.com/api/v0/events`.
- **CSP:** `tauri.conf.json` allows `https://*.aptabase.com` in `connect-src`.

---

## 2. Architecture

```
Frontend (React)                    Backend (Rust)
     |                                    |
     |  invoke('track_event',             |
     |    { event, payload })             |
     |----------------------------------->|  commands::utils::track_event
     |                                    |       |
     |                                    |       v
     |                                    |  utils::track_event_safe
     |                                    |       | 1. RepoManager::is_telemetry_enabled()?
     |                                    |       | 2. Inject event_category, event_label
     |                                    |       | 3. app.track_event(event, enriched_payload)
     |                                    |       v
     |                                    |  Plugin (Aptabase) → enqueue → flush to API
```

- **Frontend** never calls the Aptabase plugin directly. All events go through the app command `track_event`, which runs in the GUI process and checks consent.
- **Backend gate:** `track_event_safe` in `utils.rs` checks `RepoManager::is_telemetry_enabled()` (persisted in `repos.json`). If disabled, the event is dropped. If enabled, the payload is enriched with `event_category` and `event_label`, then passed to the plugin.
- **Plugin** adds system props (OS, locale, app version, etc.) and sends events in batches to the Aptabase API.

---

## 3. Privacy and Consent

| Item | Behavior |
|------|----------|
| **Default** | Telemetry off (`telemetry_enabled: false` in new config). |
| **Persistence** | Stored in `~/.config/monarch-store/repos.json` as `telemetry_enabled`. |
| **Startup** | Frontend calls `checkTelemetry()` and awaits it so the UI reflects the backend value. |
| **Onboarding** | User can toggle “Anonymous usage stats” in the Privacy step; choice is persisted with `set_telemetry_enabled` before sending `onboarding_completed`. |
| **Settings** | Settings → Privacy shows the same toggle; changes are persisted and synced to the backend. |
| **Panic events** | Sent by the plugin’s panic hook directly (no consent check); only fires on app crash. |

---

## 4. Event Categories and Labels

Every event sent through `track_event_safe` (and the panic hook) includes:

- **`event_category`** — Used in the Aptabase dashboard to filter or segment (e.g. “Install”, “Search”, “Error”).
- **`event_label`** — Human-readable short name for the event (e.g. “Package installed”, “App started”).

These are injected in the backend so the dashboard can group events and show each type as its own “box” or segment.

| Event name | event_category | event_label |
|------------|----------------|-------------|
| `app_started` | lifecycle | App started |
| `search` | search | Search |
| `search_query` | search | Search |
| `onboarding_completed` | engagement | Onboarding completed |
| `review_submitted` | engagement | Review submitted |
| `install_package` | install | Package installed |
| `uninstall_package` | install | Package uninstalled |
| `error_reported` | error | Error reported |
| `panic` | error | App panic |
| (other) | other | other |

---

## 5. Event Catalog

### 5.1 Lifecycle

| Event | When | Payload (after enrichment) |
|-------|------|-----------------------------|
| **app_started** | Once per app launch (in `lib.rs` setup). | `event_category`, `event_label`. (System props added by plugin: OS, app version, etc.) |

### 5.2 Search

| Event | When | Payload (after enrichment) |
|-------|------|-----------------------------|
| **search** | After a search completes (frontend, `App.tsx`). | `query`, `result_count`, `query_length`, `has_results`, `event_category`, `event_label`. |
| **search_query** | When a search is triggered (backend, `commands/search.rs`). | `term`, `term_length`, `category`, `event_category`, `event_label`. |

### 5.3 Engagement

| Event | When | Payload (after enrichment) |
|-------|------|-----------------------------|
| **onboarding_completed** | When the user finishes onboarding (frontend, `OnboardingModal.tsx`). | `step_count`, `aur_enabled`, `telemetry_enabled`, `completed_at_step`, `event_category`, `event_label`. |
| **review_submitted** | When the user submits a review (frontend, `PackageDetailsFresh.tsx`). | `package`, `rating`, `rating_bucket` (1-2, 3, 4-5), `event_category`, `event_label`. |

### 5.4 Install / Uninstall

| Event | When | Payload (after enrichment) |
|-------|------|-----------------------------|
| **install_package** | After a package install succeeds (backend, `commands/package.rs`). | `pkg`, `source`, `success: true`, `event_category`, `event_label`. |
| **uninstall_package** | After a package uninstall succeeds (backend, `commands/package.rs`). | `pkg`, `success: true`, `event_category`, `event_label`. |

### 5.5 Error

| Event | When | Payload (after enrichment) |
|-------|------|-----------------------------|
| **error_reported** | When the app reports an error (frontend, `ErrorContext.tsx`). | `severity`, `title`, `description` (truncated), `kind`, `raw_preview` (truncated), `event_category`, `event_label`. |
| **panic** | On Rust panic (plugin panic hook). | `event_category`, `event_label`, `message`, `location`. |

---

## 6. Using the Aptabase Dashboard

- **Filter by category:** Use the property filter on **`event_category`** (e.g. `lifecycle`, `search`, `install`, `error`) so each “box” or view shows one category.
- **Segment / break down:** In charts or tables, break down by **`event_category`** or **`event_label`** so each event type appears as its own segment.
- **Custom dashboards:** If supported, create one panel per category (e.g. Search, Install, Errors) and filter each panel by `event_category`; use `event_label` and other props for labels and breakdowns.
- **Useful properties:**  
  Search: `query_length`, `has_results`.  
  Reviews: `rating_bucket`.  
  Install/Uninstall: `success`, `pkg`, `source`.  
  Errors: `severity`, `kind`.

---

## 7. Adding a New Event

1. **Frontend:** Call `invoke('track_event', { event: 'my_event', payload: { key: value } })`. Use an object for `payload` (or omit for minimal payload).
2. **Backend (Rust):** Call `crate::utils::track_event_safe(&app, "my_event", Some(serde_json::json!({ ... }))).await`.
3. **Category/label:** Add a branch in `event_category_and_label()` in `src-tauri/monarch-gui/src/utils.rs` so the new event gets the right `event_category` and `event_label` (otherwise it will be `other` / `other`).
4. **Permissions:** `track_event` is already allowed in `permissions/app-commands.toml`; no change needed for new event names.

---

## 8. File Reference

| File | Role |
|------|------|
| `src-tauri/monarch-gui/src/lib.rs` | Registers Aptabase plugin (app key, panic hook); sends `app_started` in setup. |
| `src-tauri/monarch-gui/src/utils.rs` | `event_category_and_label()`, `track_event_safe()` — consent check and payload enrichment. |
| `src-tauri/monarch-gui/src/commands/utils.rs` | Tauri command `track_event` — frontend entry point. |
| `src-tauri/monarch-gui/src/commands/package.rs` | `install_package`, `uninstall_package` — telemetry on success. |
| `src-tauri/monarch-gui/src/commands/search.rs` | `search_packages` — sends `search_query`. |
| `src-tauri/monarch-gui/src/repo_manager.rs` | `telemetry_enabled` state, `is_telemetry_enabled()`, `set_telemetry_enabled()`, persisted in `repos.json`. |
| `src/store/internal_store.ts` | Frontend: `telemetryEnabled`, `checkTelemetry()`, `setTelemetry()`. |
| `src/App.tsx` | Calls `checkTelemetry()` at startup; sends `search` after search. |
| `src/components/OnboardingModal.tsx` | Telemetry toggle; sends `onboarding_completed` on finish. |
| `src/pages/PackageDetailsFresh.tsx` | Sends `review_submitted`. |
| `src/context/ErrorContext.tsx` | Sends `error_reported`. |
| `src-tauri/monarch-gui/plugins/tauri-plugin-aptabase/` | Aptabase plugin (client, dispatcher, config). |
| `src-tauri/monarch-gui/tauri.conf.json` | CSP includes `https://*.aptabase.com`. |
| `src-tauri/monarch-gui/capabilities/default.json` | Includes `aptabase:default`. |

---

## 9. Verification

- **Consent:** With telemetry off, no events (except panic) are sent; `track_event_safe` returns without calling the plugin.
- **Enrichment:** Every event sent through `track_event_safe` has `event_category` and `event_label` in the payload.
- **Startup:** `checkTelemetry()` is awaited in `initializeStartup()` so the UI matches the backend preference.
- **Onboarding:** Finish flow persists telemetry with `setTelemetry(localToggle)` and sends `onboarding_completed` with `telemetry_enabled: localToggle`.

# MonARCH GTK Parity Matrix

**Active release gate**  
**Last updated:** 2026-03-09

This is the single active parity audit for MonARCH Store.

GTK is the current product frontend. Tauri/React is legacy/reference-only and is used here only as a historical comparison baseline when needed. The GTK package detail UI (hero, data bar, action row) is aligned with [Bazaar](https://github.com/kolunmi/bazaar) as the design reference.

Historical `docs/*_REVIEW.md` files are reference material, not release gates.

## Status values

- `done`
- `partial`
- `missing`
- `regressed`
- `docs-only stale`

## Iron Core contract parity

| Feature / behavior | Source doc | State | Fix area | Acceptance criteria |
| --- | --- | --- | --- | --- |
| Cards consume canonical backend package payloads only | `ARCHITECTURE.md`, `docs/PACKAGING_AND_METADATA_FLOW.md` | done | monarch-gtk | No GTK-side metadata parsing, merge logic, or icon guessing beyond presentation fallback |
| Details consume canonical backend detail payloads only | `ARCHITECTURE.md`, `docs/DEVELOPER.md` | done | monarch-gtk | Details uses `FullPackageDetails` and source-specific payload reloads from backend |
| Canonical app identity is shared across Home/Search/Categories/Details | `ARCHITECTURE.md`, `docs/UNIVERSAL_DATA_ENGINE.md` | done | monarch-core, monarch-gtk | Stable app appears once across surfaces with merged sources |
| Category results come from backend taxonomy and filtering | `USER_GUIDE.md`, `ARCHITECTURE.md` | done | monarch-core, monarch-gtk | Category buttons and category result pages use backend taxonomy and return expected apps |
| Detail source switching is variant-aware and reloads payload | `USER_GUIDE.md`, `ARCHITECTURE.md` | done | monarch-core, monarch-gtk | Source choice updates facts, actions, and source summary correctly |

## Home and discovery parity

| Feature / behavior | Source doc | State | Fix area | Acceptance criteria |
| --- | --- | --- | --- | --- |
| Featured lane is backend-fed and populated with user-facing apps | `USER_GUIDE.md` | done | monarch-core, monarch-gtk | Featured does not drift toward installed-only or technical packages |
| Essentials lane is curated correctly | `USER_GUIDE.md` | done | monarch-core | Essentials picks intended canonical apps, not fuzzy misses |
| Trending lane is curated correctly | `USER_GUIDE.md` | done | monarch-core | Trending returns real app listings, not technical packages |
| Home categories match backend snapshot order and labels | `USER_GUIDE.md`, `ARCHITECTURE.md` | done | monarch-core, monarch-gtk | Home categories match current backend taxonomy |
| Disabled sources disappear from discovery | `USER_GUIDE.md` | done | monarch-core | Source toggles affect Home/Search/Categories only |

## Search parity

| Feature / behavior | Source doc | State | Fix area | Acceptance criteria |
| --- | --- | --- | --- | --- |
| Search returns one canonical stable listing per app | `USER_GUIDE.md`, `docs/UNIVERSAL_DATA_ENGINE.md` | done | monarch-core | No duplicate stable app cards from repo/Flatpak/AUR variants |
| Search respects source toggles | `USER_GUIDE.md` | done | monarch-core | Disabled discovery sources are hidden from search |
| Search respects `show_system_apps` | `USER_GUIDE.md`, `TESTING.md` | done | monarch-core, monarch-gtk | System apps appear only when toggle is enabled |
| Search uses screenshot-style cards only | `ARCHITECTURE.md` | done | monarch-gtk | Search card composition differs from compact Home/category cards only in search |

## Card and detail parity

| Feature / behavior | Source doc | State | Fix area | Acceptance criteria |
| --- | --- | --- | --- | --- |
| Compact Flathub-style cards outside search | `ARCHITECTURE.md`, `USER_GUIDE.md` | done | monarch-gtk | Home/Categories/Favorites use compact cards consistently |
| Cards show only icon, title, short description, ordered source badges | `ARCHITECTURE.md` | done | monarch-gtk | No redundant installed/source text on cards |
| Source badges are branded and ordered by importance | `ARCHITECTURE.md` | done | monarch-gtk | Badge order reflects source priority and visual language is consistent |
| Details is the single action surface | `USER_GUIDE.md` | done | monarch-gtk | Install/open/remove/update state lives in details, not cards |
| Details surface screenshots, long description, links, and metadata cleanly | `USER_GUIDE.md` | done | monarch-gtk | Details layout matches product intent and uses backend data properly |
| Details icon/logo fidelity is professional | `USER_GUIDE.md`, `docs/PACKAGING_AND_METADATA_FLOW.md` | done | monarch-core, monarch-gtk | Real app logos win over symbolic placeholders; fallback is intentional |

## Onboarding, settings, news, updates parity

| Feature / behavior | Source doc | State | Fix area | Acceptance criteria |
| --- | --- | --- | --- | --- |
| Onboarding reflects current GTK product and source choices | `USER_GUIDE.md` | done | monarch-gtk | Flow is coherent and persisted state is correct |
| Source toggles behave as documented | `USER_GUIDE.md`, `AGENTS.md` | done | monarch-core, monarch-gtk | Discovery/search hide disabled sources while Installed/Updates remain truthful |
| `show_system_apps` exists and behaves correctly | `USER_GUIDE.md`, `TESTING.md` | done | monarch-core, monarch-gtk | Toggle changes discovery/search only |
| Updates surface remains source-truthful | `USER_GUIDE.md`, `TESTING.md` | done | monarch-core, monarch-gtk | Installed updates remain visible regardless of discovery toggle state |
| News/advisories state is reflected accurately | `ARCHITECTURE.md` | done | monarch-core, monarch-gtk | Critical and read-state behavior match current implementation |

## Documentation parity

| Feature / behavior | Source doc | State | Fix area | Acceptance criteria |
| --- | --- | --- | --- | --- |
| Root/public docs describe GTK as the active frontend | `README.md`, `ARCHITECTURE.md`, `USER_GUIDE.md`, `TESTING.md`, `CONTRIBUTING.md` | done | docs only | No public root doc describes Tauri as current |
| Engineering docs describe GTK as primary while preserving backend truth | `docs/DEVELOPER.md`, `docs/PACKAGING_AND_METADATA_FLOW.md`, `docs/UNIVERSAL_DATA_ENGINE.md`, `docs/STATE_OF_THE_UNION_ARCHITECTURE_REPORT.md` | done | docs only | Frontend references are GTK-first and Tauri is explicitly legacy/reference |
| Repo instructions match current GTK workflow | `AGENTS.md`, `.cursorrules` | done | docs only | Build/test/workflow sections align with GTK-first development |

## Release rule

GTK cannot be called feature-complete until every `partial`, `missing`, or `regressed` row is either:
- implemented and validated, or
- intentionally re-scoped and documented as a product change in this file

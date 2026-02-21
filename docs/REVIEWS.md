# Hybrid Review System (v0.4.7-alpha)

**Last updated:** 2026-02-14

MonARCH Store uses a unique **Hybrid Review System** to provide the best possible coverage of Linux applications.

## 1. Composition
We merge reviews from two sources into a single, unified list:

### A. ODRS (Open Desktop Ratings Service)
*   **Source:** Global Linux community (Gnome Software, KDE Discover, etc.).
*   **Coverage:** Excellent for major apps (Firefox, VLC, GIMP).
*   **Integration:** We use the `odrs.gnome.org` API (Read-Only).
*   **Identification:** Reviews display a <span style="color:#60a5fa">**Blue "ODRS" Badge**</span>.

### B. MonARCH Community (Supabase)
*   **Source:** MonARCH Store users.
*   **Coverage:** Fills the gaps for AUR packages, chaotic-aur binaries, and niche tools often missing from ODRS.
*   **Integration:** We use a private Supabase instance (Read/Write).
*   **Identification:** Reviews display a <span style="color:#c084fc">**Purple "MonARCH" Badge**</span>.

## 2. Currency Policy (The "365-Day Rule")
To ensure ratings reflect the *current* state of software, MonARCH enforces a strict time window:
*   **Reviews older than 365 days (1 year) are discarded.**
*   The "Star Rating" is recalculated on-the-fly based *only* on these valid reviews.
*   This prevents extensive legacy ratings (e.g. from 5 years ago) from skewing the score of a rolling-release application.

## 3. Metadata Intelligence
The system uses "Backend Metadata Intelligence" to find the correct App ID for reviews:
1.  **SQLite Registry Hydration**: During the primary search/listing pass, the backend automatically joins packages to their corresponding App ID in the SQLite registry.
2.  **Consensus Mapping**: If native metadata is sparse, the backend uses its integrated mapping logic (v0.4.6) to find the canonical ID (e.g. mapping `firefox` -> `org.mozilla.firefox`).
3.  **Result**: This allows the frontend to receive pre-mapped ratings and reviews without any client-side guesswork.

## 4. Submission
*   All user reviews submitted via the MonARCH client are sent to the **MonARCH (Supabase)** backend.
*   We do not currently write back to ODRS (due to authentication complexity).

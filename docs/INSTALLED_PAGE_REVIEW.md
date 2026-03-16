> Historical note: This review document describes a legacy or point-in-time parity snapshot. GTK current-state and release-gate status now live in `docs/GTK_TAURI_PARITY_MATRIX.md` and the GTK-first root docs.

# Installed Page — Review

**Date:** 2026-02-05  
**File:** `src/pages/InstalledPage.tsx`

---

## 1. Purpose and data source

- **Purpose:** List user-installed *applications* (native ALPM packages that have an icon or desktop entry), with filter, launch, uninstall, and navigation to package details.
- **Backend:** `get_installed_packages` (package.rs) builds the list from `alpm_read::get_installed_packages_native()`, then keeps only packages for which the metadata loader has an icon or app_id. So the list is **native (repo/AUR/Chaotic) apps only**; **Flatpak apps are not included**.

---

## 2. What works well

- **UI/UX:** Sticky header with title, package count, and total size; search filter; responsive layout (meta stats hidden on small screens); AnimatePresence for list items; semantic colors and dark mode.
- **Liquid UI:** Responsive (e.g. `hidden sm:flex`), no fixed column counts, `rounded-xl`, `min-w-0` for truncation, `backdrop-blur` on header.
- **Navigation:** Row click opens details via `get_packages_by_names` with `for_installed_lookup: true`, then fallback to search so the correct package (and source) is resolved even when sources are off.
- **Uninstall:** Confirmation modal (danger variant); protected packages are enforced on the backend; success removes the app from local state and shows a toast.
- **Launch:** Uses `launch_app(pkgName)` (gtk-launch / .desktop lookup); appropriate for native apps.
- **Loading and empty states:** Loader with "Loading library..."; empty state with icon and "No applications found" / "Try a different search term" when the filter yields no results.
- **Session password:** Respects `reducePasswordPrompts` and `requestSessionPassword()` for uninstall.

---

## 3. Fix applied

- **Confirm modal not closing after uninstall:** After a successful uninstall, the code now calls `setConfirmModal(null)` so the modal closes. State update for the list was changed to a functional update: `setApps((prev) => prev.filter(...))` so it uses the latest state.

---

## 4. Backend / data limitations (for awareness)

- **install_date:** Backend always sets `install_date: None` for the installed list. The UI shows "N/A" for date; if install date is desired, the backend would need to expose it from ALPM (e.g. `pkg.install_date()`).
- **repository/source:** `InstalledPackage` has `repository: Option<String>` but it is always `None`. The Installed page does not show "Official" / "AUR" / "Chaotic" per row. Adding repo/source would require the backend to determine origin (e.g. from localdb + sync DBs) and pass it through.
- **Flatpak not in list:** Only native (ALPM) apps are returned. Flatpak apps are not listed on the Installed page. Uninstall from this page is therefore native-only; Flatpak uninstall is available from the package details flow (InstallMonitor with source). If product goal is "all installed apps in one place," the backend would need to merge in Flatpak apps (e.g. from `flatpak list --app`) and the UI would need to pass `source` on uninstall.

---

## 5. Minor notes

- **Total size:** Backend sends size as e.g. `"12 MB"` (actually MiB from `installed_size / (1024*1024)`). Frontend sums the numeric part and labels "X MiB used." Only packages that are in the filtered "apps" list are counted, not every installed package.
- **Focus ring:** Search input uses `focus:ring-blue-500/50`. Could be switched to an app accent token (e.g. `focus:ring-app-accent`) for theme consistency if available.
- **Arch logo fallback:** Apps without an icon use `arch-logo.png` with reduced opacity/grayscale; consistent with the rest of the app.

---

## 6. Summary

| Area              | Status | Notes                                                |
|-------------------|--------|------------------------------------------------------|
| Layout / responsive | OK   | Sticky header, responsive meta, truncation          |
| Data source       | OK     | Native apps only (icon/app_id); no Flatpak          |
| Search filter     | OK     | Client-side by name and description                  |
| Uninstall         | OK     | Confirm modal, backend protection; modal now closes  |
| Launch            | OK     | gtk-launch / .desktop lookup                         |
| Navigate to details | OK   | get_packages_by_names + search fallback, installed lookup |
| Loading / empty   | OK     | Spinner and empty state with message                 |
| install_date/repo | Limitation | Backend does not provide; UI shows N/A / no source badge |

The Installed page is consistent with the current backend (native apps only) and with Liquid UI and app patterns. The only code change made was closing the confirmation modal after a successful uninstall and using a functional state update for the list.

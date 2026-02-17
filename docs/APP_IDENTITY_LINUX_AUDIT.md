# MonARCH Store — App Identity Audit (Linux Taskbar/Icon)

**Goal:** Ensure the app icon and taskbar/dock entry render correctly on all Linux DEs (GNOME, KDE, Wayland/X11) without manual Window Rules.

---

## 1. Synchronized Identifiers

| Item | Value | Location |
|------|--------|----------|
| **App identifier (reverse-DNS)** | `com.monarch.store` | `src-tauri/monarch-gui/tauri.conf.json` → `identifier` |
| **Product name (display)** | `MonARCH Store` | `tauri.conf.json` → `productName` |
| **Binary / desktop name** | `monarch-store` | `package.json` → `name`; PKGBUILD/desktop `Exec`/`Icon` |
| **GTK app_id / WM handshake** | `com.monarch.store` | Set by Tauri when `app.enableGTKAppId: true` (see below) |

- **identifier** is reverse-DNS and used as the GTK application ID on Linux when `enableGTKAppId` is true.
- **productName** is used for window title and human-readable name; it is not used as the window identity for taskbar matching.
- **Binary name** `monarch-store` is used in the .desktop `Exec` and `Icon`; the DE matches the *window* to the .desktop via `StartupWMClass` (see below).

---

## 2. Tauri Configuration (`src-tauri/monarch-gui/tauri.conf.json`)

- **`identifier`**: `"com.monarch.store"` — reverse-DNS format.
- **`productName`**: `"MonARCH Store"` — matches window title and branding.
- **`app.enableGTKAppId`**: `true` — **critical.** This makes the Tauri runtime pass `config.identifier` (`com.monarch.store`) to the Linux backend (Wry/Tao) as the GTK application ID. The window then identifies itself to the compositor as `com.monarch.store` (Wayland app_id / X11 WM_CLASS), which is the handshake for taskbar/dock icon association.
- **`app.windows`**: Single window with title `"MonARCH Store - Universal Arch Linux App Manager"` (consistent with productName).
- **`bundle.icon`**: Points to `icons/` under the crate (`icons/icon.png`, `icons/32x32.png`, …). The `src-tauri/monarch-gui/icons/` folder exists and contains these files; the bundler will pack them into .deb/AppImage so the installed app has the correct icon.

No `bundle.linux`-specific override is required for identity; the generated .desktop (when Tauri builds .deb/AppImage) will use the same identifier/productName. The **Pacman** install uses the repo’s `monarch-store.desktop` (see below).

---

## 3. Rust / Setup Hook (`src-tauri/monarch-gui/src/lib.rs`)

- No X11-only or Wayland-only hacks. Transparency/shadow handling is conditional on `WAYLAND_DISPLAY` for visual correctness only, not for identity.
- The window’s “App ID” is set by the **Tauri runtime** when `app.enableGTKAppId` is true: it passes `config.identifier` into the Linux event loop (`with_app_id(app_id)`). No extra Rust call is required to set the window’s app_id; a short comment in `lib.rs` documents that the handshake is driven by `tauri.conf.json` (identifier + enableGTKAppId).

---

## 4. Linux .desktop and StartupWMClass

- **Pacman / repo install:** `src-tauri/monarch-store.desktop` is installed as `monarch-store.desktop`.
  - **StartupWMClass=com.monarch.store** — added so the window manager matches the running window (GTK app_id / WM_CLASS `com.monarch.store`) to this .desktop file. Without it, some DEs may not group the window with the correct launcher icon.
  - **Exec=monarch-store**, **Icon=monarch-store**, **Name=MonARCH Store** — consistent with binary name and productName.

- **Tauri-built .deb/AppImage:** The Tauri bundler may generate its own .desktop; that generated file should use the same identifier/name from config. If a custom desktop template is used in the future, it must include `StartupWMClass=com.monarch.store`.

---

## 5. Icons

- **Config:** `bundle.icon` lists `icons/icon.png`, `icons/32x32.png`, `icons/64x64.png`, `icons/128x128.png`, `icons/128x128@2x.png`, `icons/icon.icns`, `icons/icon.ico` — all relative to `src-tauri/monarch-gui/`, and the `icons/` directory exists with these assets.
- **Desktop:** `Icon=monarch-store` — the DE will resolve this to the installed app icon (e.g. from the bundle or from the theme/icon path where the package installs it).

---

## 6. Security and Best Practices

- **No X11-only hacks:** Identity is set via the standard GTK application ID (config-driven). No `XA_WM_CLASS` or X11-specific overrides that could break on Wayland.
- **Wayland scaling:** No special X11 scaling or DPI hacks that would interfere with Wayland; the app uses normal Tauri/Wry behavior.
- **System tray (optional):** For tray icons, `libappindicator3-1` (or `libappindicator-gtk3` on Arch) is listed in the **Dockerfile** (`libappindicator3-dev`) for the CI/build environment. For **Debian/Ubuntu .deb** installs, if the app uses a system tray, add `libappindicator3-1` (or the appropriate package) to the bundler’s Linux deb dependencies if not already present. PKGBUILD does not require it for basic window/icon behavior.

---

## 7. Outcome Checklist

- [x] **identifier** = `com.monarch.store` (reverse-DNS) in `tauri.conf.json`.
- [x] **productName** = `MonARCH Store`; window title and branding consistent.
- [x] **app.enableGTKAppId** = `true` so the window reports `com.monarch.store` to the DE.
- [x] **StartupWMClass=com.monarch.store** in `monarch-store.desktop` for Pacman installs.
- [x] **bundle.icon** points to existing `icons/` in the crate.
- [x] No X11-only identity hacks; GTK app_id works on Wayland and X11.
- [x] **libappindicator** present in Dockerfile for tray support where needed.

After `tauri build`, the .deb or AppImage should show the MonARCH icon in the taskbar/dock on GNOME, KDE, and other Linux DEs (Wayland or X11) without manual Window Rules, as long as the installed .desktop file includes `StartupWMClass=com.monarch.store` and the app is built with `enableGTKAppId: true`.

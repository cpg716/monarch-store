use ashpd::desktop::settings::Settings;
use once_cell::sync::Lazy;
use std::sync::RwLock;

/// App-wide theme mode: "system" | "light" | "dark". Portal only applies when "system".
static APP_THEME_MODE: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new("system".to_string()));

/// Apply color scheme from saved or chosen theme mode. Call at startup and when user changes theme.
pub fn apply_theme_from_mode(mode: &str) {
    let style = adw::StyleManager::default();
    let scheme = match mode {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    style.set_color_scheme(scheme);
    if let Ok(mut g) = APP_THEME_MODE.write() {
        *g = mode.to_string();
    }
}

/// Current app theme mode (for portal to decide whether to apply system scheme).
fn app_theme_mode() -> String {
    APP_THEME_MODE
        .read()
        .map(|g| g.clone())
        .unwrap_or_else(|_| "system".to_string())
}

pub fn setup_css_and_portals(_app: &adw::Application) {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        "
        /* Bazaar-style: solid window background (libadwaita-like) */
        window,
        .background {
            background-color: @window_bg_color;
        }

        .monarch-page {
            padding: 6px;
        }

        .monarch-panel {
            background: mix(@window_bg_color, @card_bg_color, 0.78);
            border: 1px solid alpha(@shade_color, 0.08);
            border-radius: 22px;
            padding: 16px;
            box-shadow: 0 12px 32px alpha(black, 0.08);
        }

        /* Bazaar-style: section card and layout (home + details) */
        .card {
            border-radius: 24px;
            padding: 24px;
            background: mix(@window_bg_color, @card_bg_color, 0.85);
            border: 1px solid alpha(@shade_color, 0.08);
            box-shadow: 0 8px 24px alpha(black, 0.06);
        }

        .monarch-sp-section {
            border-radius: 24px;
            padding: 24px;
            background: linear-gradient(135deg, alpha(@accent_bg_color, 0.06), alpha(@card_bg_color, 0.92));
            border: 1px solid alpha(@shade_color, 0.08);
            box-shadow: 0 8px 20px alpha(black, 0.05);
        }

        .app-title {
            font-size: 1.5rem;
            font-weight: 800;
            letter-spacing: -0.02em;
        }

        /* Bazaar 1:1: context row of pills (no card bar) */
        .app-context-bar {
            border-radius: 0;
            padding: 0;
            background: transparent;
        }

        /* Bazaar-style data bar: distinct tiles (card-like surface, clear separation) */
        .app-context-bar .monarch-hero-stat-value-wrap {
            border-radius: 12px;
            padding: 8px 16px;
            min-height: 0;
            background: mix(@window_bg_color, @card_bg_color, 0.6);
            border: 1px solid alpha(@shade_color, 0.25);
            box-shadow: 0 1px 2px alpha(black, 0.06);
        }

        .monarch-shell-header {
            padding: 6px 8px;
        }

        .monarch-shell-titlebar {
            padding: 4px 10px;
        }

        .monarch-wordmark {
            font-size: 1.05rem;
            font-weight: 800;
            letter-spacing: 0.04em;
        }

        .monarch-source-pill {
            background: alpha(@accent_bg_color, 0.1);
            color: @accent_bg_color;
            border-radius: 999px;
            padding: 4px 12px;
            font-size: 0.8rem;
            font-weight: 800;
            border: 1px solid alpha(@accent_bg_color, 0.15);
        }

        .monarch-source-pill.is-arch {
            background: alpha(rgb(136, 209, 243), 0.12);
            color: rgb(136, 209, 243);
            border-color: alpha(rgb(136, 209, 243), 0.2);
        }

        .monarch-source-pill.is-flatpak {
            background: alpha(rgb(148, 194, 255), 0.12);
            color: rgb(148, 194, 255);
            border-color: alpha(rgb(148, 194, 255), 0.2);
        }

        .monarch-source-pill.is-aur {
            background: alpha(rgb(255, 190, 138), 0.12);
            color: rgb(255, 190, 138);
            border-color: alpha(rgb(255, 190, 138), 0.2);
        }

        .monarch-source-pill.is-chaotic {
            background: alpha(rgb(214, 193, 255), 0.18);
            color: rgb(214, 193, 255);
            border-color: alpha(rgb(214, 193, 255), 0.25);
        }

        .monarch-source-pill.is-cachyos {
            background: alpha(rgb(168, 255, 251), 0.14);
            color: rgb(168, 255, 251);
            border-color: alpha(rgb(168, 255, 251), 0.22);
        }

        .monarch-source-pill.is-manjaro {
            background: alpha(rgb(176, 255, 215), 0.14);
            color: rgb(176, 255, 215);
            border-color: alpha(rgb(176, 255, 215), 0.22);
        }

        .monarch-source-pill.is-garuda {
            background: alpha(rgb(255, 205, 186), 0.14);
            color: rgb(255, 205, 186);
            border-color: alpha(rgb(255, 205, 186), 0.22);
        }

        .monarch-source-pill.is-endeavouros {
            background: alpha(rgb(222, 211, 255), 0.14);
            color: rgb(222, 211, 255);
            border-color: alpha(rgb(222, 211, 255), 0.22);
        }

        .monarch-card-source-pill {
            padding: 3px 10px;
            font-size: 0.7rem;
            font-weight: 800;
            letter-spacing: 0.03em;
            border-radius: 999px;
            border: 1px solid;
        }



        .monarch-header-tabs {
            padding: 4px;
            border-radius: 9999px;
            background: alpha(@headerbar_fg_color, 0.05);
            box-shadow: inset 0 0 0 1px alpha(@shade_color, 0.08);
        }

        .monarch-header-tab {
            min-height: 38px;
            border-radius: 9999px;
            padding: 0 16px;
            font-weight: 760;
        }

        .monarch-header-tab.is-active {
            background: alpha(@accent_bg_color, 0.16);
            color: @accent_bg_color;
            box-shadow: inset 0 0 0 1px alpha(@accent_bg_color, 0.18);
        }

        .monarch-header-more {
            min-width: 38px;
            min-height: 38px;
            border-radius: 999px;
            background: alpha(@headerbar_fg_color, 0.05);
            box-shadow: inset 0 0 0 1px alpha(@shade_color, 0.08);
        }

        .monarch-header-menu-item {
            border-radius: 14px;
            min-height: 36px;
            padding: 0 12px;
        }

        /* Bazaar browser-banner style: 25px radius, gradient */
        .monarch-hero {
            background:
                linear-gradient(135deg, alpha(@accent_bg_color, 0.18), alpha(@accent_bg_color, 0.06)),
                linear-gradient(to bottom right, alpha(@card_bg_color, 0.92), alpha(@card_bg_color, 0.82));
            border-radius: 25px;
            border: 1px solid alpha(@accent_bg_color, 0.15);
            padding: 24px;
            box-shadow: 0 12px 28px alpha(black, 0.08);
        }

        .monarch-onboarding-shell {
            background: mix(@window_bg_color, @card_bg_color, 0.72);
            border: 1px solid alpha(@shade_color, 0.08);
            border-radius: 28px;
            padding: 24px;
        }

        .monarch-onboarding-stage {
            background: alpha(@card_bg_color, 0.98);
            border-radius: 24px;
            padding: 20px;
            box-shadow: inset 0 0 0 1px alpha(@shade_color, 0.06);
        }

        .monarch-onboarding-icon {
            min-width: 72px;
            min-height: 72px;
            border-radius: 22px;
            padding: 14px;
            background: alpha(@accent_bg_color, 0.12);
            color: @accent_bg_color;
        }

        .monarch-onboarding-icon image {
            -gtk-icon-size: 42px;
        }

        .monarch-onboarding-actions button {
            min-width: 168px;
        }

        .monarch-hero-title {
            font-size: 1.8rem;
            font-weight: 800;
            letter-spacing: -0.02em;
        }

        .monarch-hero-copy {
            opacity: 0.8;
            font-size: 1rem;
        }

        .monarch-chip {
            background: alpha(@accent_bg_color, 0.10);
            color: @accent_bg_color;
            border-radius: 999px;
            padding: 6px 12px;
            font-size: 0.85rem;
            font-weight: 700;
        }

        .monarch-toolbar-card {
            background: alpha(@card_bg_color, 0.92);
            border: 1px solid alpha(@shade_color, 0.07);
            border-radius: 20px;
            padding: 14px;
        }

        /* Bazaar .search-box: 9999px radius, card-bg, focus outline */
        .monarch-home-search,
        .monarch-search-2026,
        .monarch-bazaar-search {
            min-height: 48px;
            border-radius: 9999px;
            padding: 8px 12px;
            background-color: @card_bg_color;
            box-shadow: inset 0 0 0 1px alpha(@shade_color, 0.12);
            font-size: 1rem;
            outline: 0 solid transparent;
            outline-offset: 6px;
        }
        .monarch-bazaar-search:focus-within {
            outline-width: 2px;
            outline-offset: 0;
            outline-color: alpha(@accent_bg_color, 0.5);
        }

        .monarch-bazaar-search-layout {
            margin-bottom: 6px;
        }

        /* spacing is a widget property, not valid in GTK CSS; use margin on children if needed */
        .monarch-bazaar-filter-pills button {
            border-radius: 9999px;
            padding: 6px 14px;
            font-size: 0.9rem;
        }

        .monarch-search-result-count {
            font-size: 0.88rem;
            font-weight: 700;
            opacity: 0.78;
        }

        .monarch-home-stat {
            min-width: 0;
            padding: 12px 14px;
            border-radius: 18px;
        }

        .monarch-package-row {
            padding: 12px;
            border-radius: 18px;
            background:
                linear-gradient(180deg, alpha(@accent_bg_color, 0.08), alpha(black, 0.04)),
                alpha(black, 0.14);
            box-shadow: inset 0 0 0 1px alpha(white, 0.04);
            transition: 180ms ease;
        }

        .monarch-package-row:hover {
            background:
                linear-gradient(180deg, alpha(@accent_bg_color, 0.12), alpha(black, 0.06)),
                alpha(black, 0.18);
            box-shadow:
                inset 0 0 0 1px alpha(@accent_bg_color, 0.18),
                0 8px 20px alpha(black, 0.14);
        }

        .monarch-card-icon-wrap {
            min-width: 64px;
            min-height: 64px;
            border-radius: 20px;
            padding: 10px;
            background: alpha(black, 0.16);
            box-shadow: inset 0 0 0 1px alpha(white, 0.04);
        }

        .monarch-card-icon-wrap picture {
            min-width: 44px;
            min-height: 44px;
        }

        .monarch-card-title {
            font-size: 1.24rem;
            font-weight: 800;
            letter-spacing: -0.01em;
        }

        .monarch-card-description {
            opacity: 0.9;
            font-size: 0.98rem;
        }

        .monarch-card-support {
            opacity: 0.72;
            font-size: 0.9rem;
        }

        .monarch-card-side {
            min-width: 178px;
        }

        /* Bazaar .search-grid: window bg, child margin 6px, radius 12px */
        .monarch-search-grid {
            background-color: @window_bg_color;
        }
        .monarch-search-grid flowboxchild {
            margin: 6px;
            border-radius: 12px;
            padding: 0;
            transition: background-color 200ms;
        }
        /* Bazaar .search-pill for filter chips */
        .monarch-search-pill {
            font-weight: 450;
            border-radius: 9999px;
            padding: 6px 14px;
        }

        /* Bazaar search-grid child: 12px radius, subtle hover */
        .monarch-store-card {
            background: @card_bg_color;
            border-radius: 12px;
            padding: 0;
            border: 1px solid alpha(@shade_color, 0.08);
            box-shadow: 0 1px 3px alpha(black, 0.06);
            transition: background-color 200ms ease;
        }

        .monarch-store-card:hover {
            background: alpha(@accent_bg_color, 0.08);
            transition: background-color 200ms ease;
        }

        .monarch-store-card-compact {
            min-width: 300px;
            margin: 0;
        }

        .monarch-source-combo-row {
            font-weight: 800;
            border-radius: 12px;
            padding: 4px 0;
        }

        .monarch-source-combo-row list {
            border-radius: 12px;
        }

        /* GtkDropDown popover: standard list (Bazaar 1:1). No pill per row — integrated list. */
        dropdown popover.background {
            background-color: transparent;
        }

        dropdown popover.background contents {
            border-radius: 12px;
            box-shadow: 0 2px 12px alpha(black, 0.15);
            background-color: @window_bg_color;
            border: 1px solid alpha(@window_fg_color, 0.08);
            padding: 6px 0;
        }

        dropdown.monarch-source-dropdown popover.background contents {
            min-width: 200px;
        }

        dropdown popover.background contents listview {
            background: transparent;
            padding: 0;
        }

        dropdown popover.background contents row,
        dropdown popover.background contents listview row,
        dropdown popover.background contents listitem {
            border: none;
            outline: none;
            box-shadow: none;
            border-radius: 6px;
            margin: 2px 6px;
            padding: 0;
            background-color: transparent;
            min-height: 0;
        }

        dropdown popover.background contents row:focus,
        dropdown popover.background contents row:focus-visible,
        dropdown popover.background contents row:selected,
        dropdown popover.background contents row:hover,
        dropdown popover.background contents listitem:focus,
        dropdown popover.background contents listitem:selected,
        dropdown popover.background contents listitem:hover {
            border: none;
            outline: none;
            box-shadow: none;
            background-color: alpha(@window_fg_color, 0.08);
        }

        .monarch-installed-source-notice {
            padding: 16px 20px;
            border-radius: 12px;
            border: 1px solid alpha(@success_bg_color, 0.4);
            background-color: alpha(@success_bg_color, 0.15);
        }

        .monarch-installed-source-notice .title-4 {
            color: @success_fg_color;
            font-weight: bold;
        }

        .monarch-installed-source-notice .dim-label {
            color: alpha(@success_fg_color, 0.9);
        }

        .monarch-store-card-screenshot {
            min-width: 336px;
            min-height: 200px;
        }

        .monarch-card-banner {
            border-top-left-radius: 24px;
            border-top-right-radius: 24px;
            background:
                linear-gradient(135deg, alpha(@accent_bg_color, 0.20), alpha(@accent_bg_color, 0.04)),
                alpha(@window_bg_color, 0.08);
        }

        .monarch-card-app-icon {
            min-width: 80px;
            min-height: 80px;
        }

        .monarch-source-badge-icon {
            min-width: 16px;
            min-height: 16px;
        }

        .monarch-store-card-title {
            font-size: 1.05rem;
            font-weight: 850;
            letter-spacing: -0.02em;
        }

        .monarch-store-card-subtitle {
            opacity: 0.62;
            font-size: 0.78rem;
            font-weight: 700;
        }

        .monarch-store-card-description {
            opacity: 0.86;
            font-size: 0.9rem;
            line-height: 1.3;
        }

        .monarch-store-card-logo-chip {
            padding: 2px;
            border-radius: 6px;
            background: alpha(black, 0.42);
            border: 1px solid alpha(white, 0.08);
            box-shadow: 0 2px 8px alpha(black, 0.1);
            min-width: 24px;
            min-height: 24px;
        }

        .monarch-store-card-logo-chip picture,
        .monarch-store-card-logo-chip .monarch-source-badge-icon,
        .monarch-store-card-logo-chip .monarch-source-badge-logo-only {
            min-width: 20px;
            min-height: 20px;
        }

        .monarch-store-card-logo-label {
            font-size: 0.56rem;
            font-weight: 850;
            letter-spacing: 0.04em;
            text-transform: uppercase;
            opacity: 0.92;
        }

        .monarch-card-badges {
            min-height: 18px;
        }

        .monarch-store-card-logo-chip.is-arch,
        .monarch-source-preview-badge.is-arch {
            color: rgb(136, 209, 243);
        }

        .monarch-store-card-logo-chip.is-flatpak,
        .monarch-source-preview-badge.is-flatpak {
            color: rgb(148, 194, 255);
        }

        .monarch-store-card-logo-chip.is-aur,
        .monarch-source-preview-badge.is-aur {
            color: rgb(255, 190, 138);
        }

        .monarch-store-card-logo-chip.is-chaotic,
        .monarch-source-preview-badge.is-chaotic {
            color: rgb(214, 193, 255);
        }

        .monarch-store-card-logo-chip.is-cachyos,
        .monarch-source-preview-badge.is-cachyos {
            color: rgb(168, 255, 251);
        }

        .monarch-store-card-logo-chip.is-manjaro,
        .monarch-source-preview-badge.is-manjaro {
            color: rgb(176, 255, 215);
        }

        .monarch-store-card-logo-chip.is-garuda,
        .monarch-source-preview-badge.is-garuda {
            color: rgb(255, 205, 186);
        }

        .monarch-store-card-logo-chip.is-endeavouros,
        .monarch-source-preview-badge.is-endeavouros {
            color: rgb(222, 211, 255);
        }

        .monarch-card-action {
            min-width: 72px;
            min-height: 40px;
            border-radius: 9999px;
            padding: 0 14px;
        }

        .monarch-settings-tile {
            border-radius: 18px;
            min-height: 128px;
            background:
                linear-gradient(180deg, alpha(@accent_bg_color, 0.08), alpha(@card_bg_color, 0.06)),
                alpha(@card_bg_color, 0.92);
            border: 1px solid alpha(@accent_bg_color, 0.10);
        }

        .monarch-card-list {
            padding-top: 6px;
            padding-bottom: 6px;
        }

        /* Bazaar-style see-more-of-section link */
        button.monarch-see-more {
            padding: 4px 0;
            margin-top: 4px;
        }
        button.monarch-see-more:hover {
            color: @accent_bg_color;
        }

        .monarch-save-button {
            border-radius: 999px;
            min-width: 30px;
            min-height: 30px;
            padding: 0;
        }

        .monarch-source-pill {
            margin-top: 6px;
            margin-right: 6px;
            padding: 4px 10px;
            border-radius: 999px;
            background: alpha(@accent_bg_color, 0.18);
            color: mix(@accent_bg_color, white, 0.16);
            font-size: 0.66rem;
            font-weight: 850;
            letter-spacing: 0.04em;
        }

        .monarch-source-pill.selected {
            background: alpha(@accent_bg_color, 0.35);
            border: 1px solid alpha(@accent_bg_color, 0.4);
        }

        .monarch-sidebar {
            padding: 2px;
            border-radius: 24px;
            background:
                linear-gradient(180deg, alpha(@card_bg_color, 0.98), alpha(black, 0.12)),
                alpha(@window_bg_color, 0.97);
        }

        .monarch-sidebar row {
            min-height: 44px;
        }

        .monarch-source-pill {
            border-radius: 999px;
            padding: 4px 12px;
            background: alpha(@accent_bg_color, 0.1);
            font-weight: 800;
            color: @accent_bg_color;
            border: 1px solid alpha(@accent_bg_color, 0.2);
        }

        .monarch-sidebar-brand {
            min-width: 44px;
            min-height: 44px;
            border-radius: 14px;
            padding: 4px;
            background:
                linear-gradient(180deg, alpha(@accent_bg_color, 0.14), alpha(@accent_bg_color, 0.06)),
                alpha(black, 0.12);
        }

        .monarch-sidebar.is-collapsed {
            padding-left: 2px;
            padding-right: 2px;
        }

        .monarch-sidebar-brand.is-collapsed {
            margin-left: 0;
            margin-right: 0;
        }

        .monarch-sidebar list {
            background: transparent;
        }

        .monarch-sidebar-section {
            background: transparent;
            box-shadow: none;
        }

        .monarch-sidebar-section row {
            background: transparent;
            border-radius: 18px;
        }

        .monarch-sidebar-row {
            border-radius: 16px;
            padding: 0;
            min-height: 46px;
        }

        .monarch-sidebar-label {
            font-size: 0.9rem;
            font-weight: 700;
        }

        .monarch-sidebar-section row:selected .monarch-sidebar-row {
            background:
                linear-gradient(180deg, alpha(@accent_bg_color, 0.28), alpha(@accent_bg_color, 0.14));
            box-shadow:
                inset 0 0 0 1px alpha(@accent_bg_color, 0.20),
                0 8px 18px alpha(@accent_bg_color, 0.08);
        }

        .monarch-sidebar-section row:selected image {
            color: @accent_bg_color;
        }

        .monarch-sidebar.is-collapsed .monarch-sidebar-section {
            border-radius: 14px;
        }

        .monarch-sidebar.is-collapsed .monarch-sidebar-row {
            min-height: 40px;
            padding-left: 0;
            padding-right: 0;
        }

        .monarch-sidebar-collapse {
            min-width: 40px;
            min-height: 40px;
            border-radius: 999px;
            background: alpha(@accent_bg_color, 0.08);
            border: 1px solid alpha(@accent_bg_color, 0.10);
        }

        .monarch-sidebar-health {
            border-radius: 14px;
            padding: 8px 10px;
            background: alpha(@accent_bg_color, 0.06);
            box-shadow: inset 0 0 0 1px alpha(@accent_bg_color, 0.10);
        }

        .monarch-sidebar-health-title {
            font-size: 0.82rem;
            font-weight: 800;
            letter-spacing: 0.03em;
            opacity: 0.9;
        }

        .monarch-sidebar-health-copy {
            font-size: 0.76rem;
            opacity: 0.66;
        }

        .monarch-detail-actionbar {
            border-radius: 18px;
            padding: 12px 14px;
            background:
                linear-gradient(180deg, alpha(@accent_bg_color, 0.08), alpha(@card_bg_color, 0.04)),
                alpha(black, 0.12);
            box-shadow: inset 0 0 0 1px alpha(@accent_bg_color, 0.08);
        }

        .monarch-source-choice {
            border-radius: 12px;
            padding: 6px 14px;
            background: alpha(@window_fg_color, 0.05);
            box-shadow: inset 0 0 0 1px alpha(@window_fg_color, 0.1);
            transition: 120ms ease;
        }

        .monarch-source-choice:hover {
            background: alpha(@window_fg_color, 0.09);
            box-shadow: inset 0 0 0 1px alpha(@window_fg_color, 0.15);
        }

        /* Bazaar-style: rating badge with accent color */
        .monarch-hero-rating-badge {
            border-radius: 9999px;
            padding: 6px 12px;
            background: alpha(@accent_bg_color, 0.15);
            color: @accent_fg_color;
            font-weight: 800;
        }

        .monarch-hero-rating-star {
            color: @accent_bg_color;
        }

        .monarch-hero-rating-value {
            font-weight: 800;
            font-size: 1.15rem;
            color: @accent_fg_color;
        }

        /* Prominent rating under name/maintainer (reviews aggregate) */
        .monarch-hero-rating-focus {
            font-size: 1.5rem;
            font-weight: 800;
            letter-spacing: 0.02em;
            color: @accent_bg_color;
        }

        .monarch-source-choice-compact {
            min-width: 244px;
            padding: 10px 12px;
        }

        .monarch-source-inline-controls {
            background: transparent;
            border-radius: 14px;
            padding: 0;
        }

        .monarch-source-preview-badge {
            border-radius: 999px;
            padding: 4px 10px;
            border: 1px solid alpha(@shade_color, 0.10);
            background: alpha(@headerbar_fg_color, 0.05);
            box-shadow: inset 0 0 0 1px alpha(white, 0.02);
        }

        .monarch-source-preview-badge picture {
            min-width: 14px;
            min-height: 14px;
        }

        .monarch-source-preview-badge-label {
            font-size: 0.72rem;
            font-weight: 850;
            letter-spacing: 0.04em;
        }

        /* Source in hero: just the dropdown, no pill (user: remove pill around source) */
        .monarch-hero-source-dropdown {
            min-height: 0;
            min-width: 100px;
            padding: 4px 8px;
            background: transparent;
            border: none;
            box-shadow: none;
        }
        .monarch-hero-source-dropdown .monarch-source-pill,
        .monarch-hero-source-dropdown .monarch-card-source-pill {
            background: transparent;
            border: none;
            padding: 0;
            min-height: 0;
        }
        .monarch-hero-source-dropdown .monarch-source-pill-label,
        .monarch-hero-source-dropdown .dim-label {
            color: @window_fg_color;
            font-weight: 500;
        }
        .monarch-source-dropdown {
            min-height: 0;
            padding: 4px 8px;
            background: transparent;
            box-shadow: none;
        }

        .monarch-security-card {
            background: rgba(91, 56, 7, 0.36);
            border: 1px solid rgba(243, 168, 28, 0.32);
            border-radius: 22px;
            padding: 18px;
        }

        .monarch-copy-action {
            min-height: 54px;
            border-radius: 16px;
        }

        .monarch-skeleton-row {
            background: alpha(@window_fg_color, 0.015);
        }

        .monarch-skeleton-block {
            background:
                linear-gradient(
                    90deg,
                    alpha(@shade_color, 0.08),
                    alpha(@accent_bg_color, 0.14),
                    alpha(@shade_color, 0.08)
                );
            border-radius: 999px;
            transition: 260ms ease;
        }

        .monarch-skeleton-icon {
            border-radius: 14px;
        }

        .monarch-skeleton-panel.monarch-skeleton-bright .monarch-skeleton-block {
            background:
                linear-gradient(
                    90deg,
                    alpha(@shade_color, 0.10),
                    alpha(@accent_bg_color, 0.22),
                    alpha(@shade_color, 0.10)
                );
        }

        .monarch-meta {
            opacity: 0.7;
            font-size: 0.9rem;
        }

        .monarch-source-pill {
            background: alpha(@accent_bg_color, 0.12);
            border-radius: 999px;
            padding: 3px 9px;
            font-size: 0.68rem;
            font-weight: 800;
            letter-spacing: 0.04em;
        }

        .monarch-version-pill {
            opacity: 0.82;
            font-size: 0.94rem;
            font-weight: 700;
        }

        .monarch-back-button {
            border-radius: 999px;
            padding: 6px 12px;
            background: alpha(@accent_bg_color, 0.08);
            border: 1px solid alpha(@accent_bg_color, 0.10);
        }

        .monarch-detail-header {
            padding: 18px;
        }

        .monarch-bazaar-hero {
            background: transparent;
        }

        .monarch-bazaar-hero-row {
            min-height: 0;
        }

        /* Bazaar .verified: blue checkmark */
        .monarch-detail-verified {
            color: #3584e4;
        }
        /* Bazaar .support: pink Support button */
        .monarch-detail-support {
            border-radius: 9999px;
            padding: 6px 14px;
            background: alpha(#f06292, 0.25);
            color: #f06292;
        }
        /* Bazaar .favorite pill */
        .monarch-detail-favorite-pill {
            border-radius: 9999px;
            padding: 6px 14px;
            min-width: 32px;
            min-height: 32px;
        }
        /* Bazaar .context-tile for metadata pills */
        .monarch-context-tile {
            box-shadow: none;
            padding: 4px;
            background-color: transparent;
        }
        button.monarch-context-tile {
            background-color: transparent;
        }
        .monarch-context-tile-text {
            font-size: 10pt;
            font-weight: 500;
        }
        /* Bazaar-style primary CTA pill in hero */
        .monarch-detail-install-pill {
            border-radius: 9999px;
            padding: 6px 18px;
        }
        .monarch-detail-install-pill.suggested-action {
            padding: 8px 20px;
        }

        .monarch-detail-developer {
            font-weight: 700;
            opacity: 0.78;
        }

        /* Bazaar 1:1: no big logo behind hero — keep hidden (opacity + min size 0; no width/height in GTK theme) */
        .monarch-detail-backdrop {
            opacity: 0;
            min-width: 0;
            min-height: 0;
        }

        /* Bazaar 1:1: large circular app icon in hero (Bazaar has much larger logo) */
        .monarch-detail-icon-wrap {
            min-width: 112px;
            min-height: 112px;
            border-radius: 9999px;
            padding: 0;
            background: transparent;
            box-shadow: none;
        }

        .monarch-detail-icon-wrap picture {
            min-width: 112px;
            min-height: 112px;
            border-radius: 9999px;
        }

        .monarch-bazaar-stats-bar {
            margin-top: 8px;
            margin-bottom: 8px;
        }

        /* Bazaar: value in tile, label under tile; distinct tile (card-like) */
        .monarch-hero-stat {
            min-width: 0;
            background: transparent;
            box-shadow: none;
            padding: 0;
        }
        .monarch-hero-stat-value-wrap {
            border-radius: 12px;
            padding: 8px 16px;
            background: mix(@window_bg_color, @card_bg_color, 0.6);
            border: 1px solid alpha(@shade_color, 0.25);
            box-shadow: 0 1px 2px alpha(black, 0.06);
        }
        .monarch-hero-stat .monarch-hero-stat-value,
        .monarch-hero-stat-value-wrap .monarch-context-tile-text {
            font-size: 11pt;
            font-weight: 600;
        }
        .monarch-hero-stat .monarch-hero-stat-title {
            font-size: 9pt;
            font-weight: 500;
            opacity: 0.78;
            margin-top: 3px;
        }

        .monarch-hero-actions {
            border-radius: 16px;
            padding: 0;
            background: transparent;
            box-shadow: none;
        }

        /* Bazaar .screenshot: 20px radius */
        .monarch-detail-shot {
            border-radius: 20px;
            padding: 10px;
            min-height: 252px;
            background:
                linear-gradient(180deg, alpha(@accent_bg_color, 0.06), alpha(@card_bg_color, 0.02)),
                alpha(@shade_color, 0.10);
            box-shadow: inset 0 0 0 1px alpha(@shade_color, 0.08);
        }

        .monarch-inline-note {
            opacity: 0.62;
            font-size: 0.78rem;
        }

        .monarch-source-select-row {
            min-height: 42px;
        }

        /* Bazaar-style: source + actions in one row, context-tile look */
        .monarch-detail-source-actions-row {
            border-radius: 14px;
            padding: 4px 0;
            background: transparent;
        }
        .monarch-detail-source-row {
            border-radius: 12px;
            padding: 6px 10px;
            min-height: 40px;
            background: alpha(@window_fg_color, 0.06);
            box-shadow: inset 0 0 0 1px alpha(@shade_color, 0.08);
        }

        .monarch-hero-stat-title {
            opacity: 0.7;
            font-size: 9pt;
            font-weight: 500;
        }

        .monarch-hero-stat-value {
            font-size: 0.94rem;
            font-weight: 760;
        }

        .monarch-category-pill {
            min-height: 48px;
            min-width: 156px;
            padding-left: 20px;
            padding-right: 20px;
            font-weight: 800;
        }

        .monarch-card-grid {
            padding-top: 4px;
            padding-bottom: 4px;
        }

        .monarch-featured-slide {
            min-height: 178px;
        }

        .monarch-featured-panel {
            border-radius: 28px;
            padding: 18px;
            background:
                linear-gradient(135deg, alpha(@accent_bg_color, 0.12), transparent 42%),
                alpha(@card_bg_color, 0.98);
            box-shadow: inset 0 0 0 1px alpha(@shade_color, 0.08);
        }

        .monarch-filter-chip {
            min-height: 36px;
            border-radius: 9999px;
            padding: 0 14px;
            background: alpha(@accent_bg_color, 0.08);
            box-shadow: inset 0 0 0 1px alpha(@shade_color, 0.10);
        }

        .monarch-filter-chip.is-active,
        togglebutton.monarch-filter-chip:checked {
            background: alpha(@accent_bg_color, 0.18);
            color: @accent_bg_color;
            box-shadow: inset 0 0 0 1px alpha(@accent_bg_color, 0.18);
        }

        .monarch-search-header,
        .monarch-search-filters,
        .monarch-search-blank,
        .monarch-discover-hero {
            border-radius: 25px;
        }

        .monarch-category-tile {
            min-width: 160px;
            min-height: 84px;
            border-radius: 24px;
            padding: 0 18px;
            background:
                linear-gradient(135deg, alpha(@accent_bg_color, 0.16), alpha(@accent_bg_color, 0.04)),
                alpha(@card_bg_color, 0.96);
            box-shadow: inset 0 0 0 1px alpha(@shade_color, 0.08);
            font-weight: 800;
            color: white;
            text-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
        }
        /* Bazaar-style category gradients (from style.css) */
        .monarch-category-tile.monarch-category-trending {
            background: linear-gradient(135deg, #99c1f1, #3584e4);
        }
        .monarch-category-tile.monarch-category-popular {
            background: linear-gradient(135deg, #f7ef74, #ffbf6f);
            color: rgba(0, 0, 0, 0.8);
        }
        .monarch-category-tile.monarch-category-audiovideo {
            background: linear-gradient(135deg, #ffcd3c 0%, #ff6b35 100%);
        }
        .monarch-category-tile.monarch-category-game {
            background: linear-gradient(135deg, #f9e2a7 0%, #eb5ec3 50%, #6d53e0 100%);
            color: #393484;
        }
        .monarch-category-tile.monarch-category-network {
            background: linear-gradient(135deg, #ff6b35, #ed333b);
        }
        .monarch-category-tile.monarch-category-education {
            background: linear-gradient(135deg, #2ec27e 30%, #27a66c 100%);
        }

        /* Bazaar .installed-list-view > *: margin 5px 4.5px, padding 0 */
        .monarch-installed-list-view row,
        .monarch-library-list row {
            padding: 0;
            margin: 5px 4.5px;
            min-height: 0;
            border-radius: 12px;
            transition: background-color 200ms;
        }

        .monarch-library-row {
            background: alpha(@card_bg_color, 0.92);
            box-shadow: 0 0 0 1px rgba(0,0,6,0.03), 0 1px 3px 1px rgba(0,0,6,0.07), 0 2px 6px 2px rgba(0,0,6,0.03);
        }

        /* Bazaar .update-card for Pending Updates strip */
        .monarch-update-card {
            border-radius: 12px;
            box-shadow: 0 0 0 1px rgba(0,0,6,0.03), 0 1px 3px 1px rgba(0,0,6,0.07), 0 2px 6px 2px rgba(0,0,6,0.03);
            padding: 12px 16px;
            background: alpha(@card_bg_color, 0.8);
        }

        .monarch-pending-updates-row {
            padding: 0;
        }

        .monarch-bazaar-update-all {
            border-radius: 9999px;
            padding: 6px 16px;
        }

        .monarch-library-action {
            border-radius: 9999px;
            padding: 6px;
        }
        .monarch-library-favorite.is-favorite {
            color: @accent_bg_color;
        }
        .monarch-library-remove:hover {
            color: @error_color;
        }

        .monarch-review-row {
            background: alpha(@card_bg_color, 0.68);
            border-radius: 20px;
            padding: 16px 18px;
            box-shadow: inset 0 0 0 1px alpha(@shade_color, 0.06);
        }

        .monarch-review-user {
            font-size: 1.02rem;
            font-weight: 700;
        }

        .monarch-review-rating {
            background: alpha(@accent_bg_color, 0.10);
            color: @accent_bg_color;
            border-radius: 999px;
            padding: 4px 10px;
            font-size: 0.86rem;
            font-weight: 800;
        }

        .monarch-review-meta {
            opacity: 0.62;
            font-size: 0.84rem;
        }

        .monarch-review-body {
            opacity: 0.9;
            font-size: 0.96rem;
        }

        .monarch-favorite-button {
            border-radius: 999px;
            min-width: 34px;
            min-height: 34px;
            padding: 0;
            color: alpha(@window_fg_color, 0.65);
            transition: 180ms ease;
        }

        .monarch-favorite-button:hover {
            background: alpha(@accent_bg_color, 0.10);
            color: @accent_bg_color;
        }

        .monarch-favorite-button.is-favorite {
            color: mix(@accent_bg_color, #f5b400, 0.55);
            background: alpha(@accent_bg_color, 0.12);
        }

        .monarch-soft-list row {
            border-radius: 18px;
            margin-bottom: 8px;
        }

        .monarch-progress-card {
            background: alpha(@card_bg_color, 0.94);
            border-radius: 24px;
            border: 1px solid alpha(@shade_color, 0.08);
            padding: 18px;
        }

        .monarch-log-view text {
            font-family: Monospace;
            font-size: 0.88rem;
        }

        .monarch-progress-card .success-step {
            color: @success_color;
            font-weight: bold;
        }
        .monarch-progress-card .current-step {
            color: @accent_bg_color;
            font-weight: bold;
        }
        .monarch-progress-card .dim-label {
            color: alpha(@window_fg_color, 0.5);
        }

        .monarch-source-switch-notice {
            background: alpha(@accent_bg_color, 0.12);
            border: 1px solid alpha(@accent_bg_color, 0.25);
            border-radius: 12px;
            padding: 12px 14px;
        }
        .installed-source-row { color: @success_color; }
        .available-after-uninstall-row { color: alpha(@window_fg_color, 0.6); }

        .monarch-empty {
            opacity: 0.82;
        }
        ",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    #[cfg(target_os = "linux")]
    {
        let main_context = glib::MainContext::default();
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };

            let portal_state = runtime.block_on(async {
                let settings = Settings::new().await.ok()?;
                let scheme = read_color_scheme(&settings).await.ok();
                let accent_css = read_accent_css(&settings).await;
                Some((scheme, accent_css))
            });

            if let Some((scheme, accent_css)) = portal_state {
                main_context.invoke(move || {
                    // Only apply system scheme when user has chosen "Follow System"
                    if app_theme_mode() == "system" {
                        if let Some(scheme) = scheme {
                            let style = adw::StyleManager::default();
                            let color_scheme = match scheme {
                                1 => adw::ColorScheme::ForceDark,
                                2 => adw::ColorScheme::ForceLight,
                                _ => adw::ColorScheme::Default,
                            };
                            style.set_color_scheme(color_scheme);
                        }
                    }

                    if let Some(css) = accent_css {
                        let provider = gtk::CssProvider::new();
                        provider.load_from_data(&css);
                        if let Some(display) = gtk::gdk::Display::default() {
                            gtk::style_context_add_provider_for_display(
                                &display,
                                &provider,
                                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
                            );
                        }
                    }
                });
            }
        });
    }
}

async fn read_color_scheme(settings: &Settings<'_>) -> Result<u8, ashpd::Error> {
    if let Ok(value) = settings
        .read::<u32>("org.freedesktop.appearance", "color-scheme")
        .await
    {
        return Ok(value as u8);
    }

    settings
        .read::<u8>("org.freedesktop.appearance", "color-scheme")
        .await
}

async fn read_accent_css(settings: &Settings<'_>) -> Option<String> {
    let rgb = if let Ok(value) = settings
        .read::<(f64, f64, f64)>("org.freedesktop.appearance", "accent-color")
        .await
    {
        Some(value)
    } else if let Ok(value) = settings
        .read::<Vec<f64>>("org.freedesktop.appearance", "accent-color")
        .await
    {
        if value.len() >= 3 {
            Some((value[0], value[1], value[2]))
        } else {
            None
        }
    } else {
        None
    }?;

    let to_u8 = |component: f64| -> u8 { (component.clamp(0.0, 1.0) * 255.0).round() as u8 };
    Some(format!(
        ".monarch-package-row {{ background-image: linear-gradient(to right, alpha(#{:02x}{:02x}{:02x}, 0.03), transparent); }}",
        to_u8(rgb.0),
        to_u8(rgb.1),
        to_u8(rgb.2)
    ))
}

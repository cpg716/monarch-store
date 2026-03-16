use crate::context::AppContext;
use crate::theme::portal::apply_theme_from_mode;
use crate::ui::auth::{ensure_session_auth, parent_window_for};
use adw::prelude::*;
use monarch_core::models::{ChaoticSupport, GtkSettings, SettingsView, StartupStatus};

pub fn build_settings_page(context: AppContext) -> gtk::Widget {
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .css_classes(vec!["monarch-page".to_string()])
        .build();

    let hero = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-hero".to_string()])
        .build();
    hero.append(
        &gtk::Label::builder()
            .label("Mission Control")
            .xalign(0.0)
            .css_classes(vec!["monarch-hero-title".to_string()])
            .build(),
    );
    hero.append(
        &gtk::Label::builder()
            .label("Configure package sources, security behavior, and repairs in one place.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["monarch-hero-copy".to_string()])
            .build(),
    );

    let mission_grid = gtk::Grid::builder()
        .column_spacing(12)
        .row_spacing(12)
        .hexpand(true)
        .build();
    let readiness_value = gtk::Label::builder()
        .label("Checking…")
        .xalign(0.0)
        .css_classes(vec!["title-3".to_string()])
        .build();
    let readiness_copy = gtk::Label::builder()
        .label("Startup readiness, helper access, and repair pressure.")
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let sources_value = gtk::Label::builder()
        .label("Checking…")
        .xalign(0.0)
        .css_classes(vec!["title-3".to_string()])
        .build();
    let sources_copy = gtk::Label::builder()
        .label("Which ecosystems are active on this machine.")
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let workflow_value = gtk::Label::builder()
        .label("Checking…")
        .xalign(0.0)
        .css_classes(vec!["title-3".to_string()])
        .build();
    let workflow_copy = gtk::Label::builder()
        .label("How MonARCH will ask for auth and sync state.")
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    mission_grid.attach(
        &build_status_tile("Readiness", &readiness_value, &readiness_copy),
        0,
        0,
        1,
        1,
    );
    mission_grid.attach(
        &build_status_tile("Sources", &sources_value, &sources_copy),
        1,
        0,
        1,
        1,
    );
    mission_grid.attach(
        &build_status_tile("Workflow", &workflow_value, &workflow_copy),
        2,
        0,
        1,
        1,
    );

    let health_group = adw::PreferencesGroup::builder().title("System Health").build();
    let health_summary_row = adw::ActionRow::builder()
        .title("Host readiness")
        .subtitle("Checking startup health…")
        .build();
    let helper_health_row = adw::ActionRow::builder()
        .title("Privileged helper")
        .subtitle("Checking helper availability…")
        .build();
    let source_health_row = adw::ActionRow::builder()
        .title("Source policy")
        .subtitle("Checking distro source capability…")
        .build();
    let snapshot_health_row = adw::ActionRow::builder()
        .title("Snapshot protection")
        .subtitle("Checking Timeshift or Snapper availability…")
        .build();
    let warning_health_row = adw::ActionRow::builder()
        .title("Update warnings")
        .subtitle("Checking reboot, pacnew, and service restart state…")
        .build();
    health_group.add(&health_summary_row);
    health_group.add(&helper_health_row);
    health_group.add(&source_health_row);
    health_group.add(&snapshot_health_row);
    health_group.add(&warning_health_row);

    let source_group = adw::PreferencesGroup::builder().title("Sources").build();
    let aur_row = adw::SwitchRow::builder()
        .title("Enable AUR discovery")
        .subtitle("Show Arch User Repository packages in Discovery. AUR builds remain user-space and never run as root.")
        .build();
    let flatpak_row = adw::SwitchRow::builder()
        .title("Enable Flatpak discovery")
        .subtitle("Show Flatpak apps alongside repo and AUR results so desktop software is easier to compare.")
        .build();
    let chaotic_row = adw::SwitchRow::builder()
        .title("Enable Chaotic-AUR discovery")
        .subtitle("Show Chaotic-AUR binaries when the distro allows them. MonARCH will not force unsupported host repo combinations.")
        .build();
    let system_apps_row = adw::SwitchRow::builder()
        .title("Show system apps in search")
        .subtitle("Include lower-level system-facing apps and utilities in Search. Installed and Updates already remain visible regardless of discovery source toggles.")
        .build();
    source_group.add(&aur_row);
    source_group.add(&flatpak_row);
    source_group.add(&chaotic_row);
    source_group.add(&system_apps_row);

    let workflow_group = adw::PreferencesGroup::builder().title("Workflow and Auth").build();
    let one_click_row = adw::SwitchRow::builder()
        .title("Enable one-click auth")
        .subtitle("Prefer the branded MonARCH credential flow for helper actions instead of repetitive system prompts.")
        .build();
    let reduce_prompts_row = adw::SwitchRow::builder()
        .title("Reduce password prompts")
        .subtitle("Reuse the current session credential when MonARCH performs repeated privileged actions.")
        .build();
    let sync_row = adw::SwitchRow::builder()
        .title("Sync on startup")
        .subtitle("Automatically refresh package databases when MonARCH launches. Disable this if you prefer manual control.")
        .build();
    let verbose_row = adw::SwitchRow::builder()
        .title("Verbose transaction logs")
        .subtitle("Keep more operation detail visible in the GTK monitor when installs, removals, or updates are running.")
        .build();
    workflow_group.add(&one_click_row);
    workflow_group.add(&reduce_prompts_row);
    workflow_group.add(&sync_row);
    workflow_group.add(&verbose_row);

    let builder_group = adw::PreferencesGroup::builder().title("Builder").build();
    let housekeeping_row = adw::SwitchRow::builder()
        .title("Automatic housekeeping")
        .subtitle("Allow MonARCH to clean up transient state and cached operation residue after successful workflows.")
        .build();
    let clean_build_row = adw::SwitchRow::builder()
        .title("Flush AUR build cache after success")
        .subtitle("Remove built AUR artifacts after a successful install so the cache stays smaller.")
        .build();
    let parallel_row = adw::ActionRow::builder()
        .title("Parallel downloads/build jobs")
        .subtitle("Current value will be persisted for later backend parity.")
        .build();
    let parallel_adjustment = gtk::Adjustment::new(3.0, 1.0, 10.0, 1.0, 1.0, 0.0);
    let parallel_spin = gtk::SpinButton::builder()
        .adjustment(&parallel_adjustment)
        .numeric(true)
        .width_chars(3)
        .build();
    parallel_row.add_suffix(&parallel_spin);
    builder_group.add(&housekeeping_row);
    builder_group.add(&clean_build_row);
    builder_group.add(&parallel_row);

    let appearance_group = adw::PreferencesGroup::builder().title("Appearance").build();
    appearance_group.add(
        &adw::ActionRow::builder()
            .title("Desktop theme and accent")
            .subtitle("MonARCH follows xdg-desktop-portal for color-scheme and accent on KDE, GNOME, and Hyprland.")
            .build(),
    );
    let theme_model = gtk::StringList::new(&["Follow System", "Light", "Dark"]);
    let theme_dropdown = gtk::DropDown::builder()
        .model(&theme_model)
        .build();
    let theme_row = adw::ActionRow::builder()
        .title("Theme")
        .subtitle("Use system appearance or pin light or dark mode.")
        .activatable(false)
        .build();
    theme_row.add_suffix(&theme_dropdown);
    appearance_group.add(&theme_row);

    let privacy_group = adw::PreferencesGroup::builder().title("Privacy").build();
    let telemetry_row = adw::SwitchRow::builder()
        .title("Telemetry")
        .subtitle("Controls whether MonARCH stores optional frontend behavior signals. Package operations are unaffected.")
        .build();
    privacy_group.add(&telemetry_row);

    let maintenance_group = adw::PreferencesGroup::builder().title("Repairs and Maintenance").build();
    let cache_state_row = adw::ActionRow::builder()
        .title("Package cache")
        .subtitle("Checking pacman cache size…")
        .activatable(true)
        .build();
    let orphan_state_row = adw::ActionRow::builder()
        .title("Orphan packages")
        .subtitle("Checking orphaned dependency state…")
        .activatable(true)
        .build();
    let mirror_tool_row = adw::ActionRow::builder()
        .title("Mirror ranking tool")
        .subtitle("Checking which ranking tool is available…")
        .activatable(false)
        .build();
    let clear_cache_row = adw::ActionRow::builder()
        .title("Clear pacman package cache")
        .subtitle("Runs the helper-backed cache cleanup path.")
        .activatable(true)
        .build();
    let refresh_db_row = adw::ActionRow::builder()
        .title("Force refresh package databases")
        .subtitle("Refreshes pacman sync databases through the helper path.")
        .activatable(true)
        .build();
    let keyring_row = adw::ActionRow::builder()
        .title("Refresh security keyrings")
        .subtitle("Repairs common package-signing failures before retrying.")
        .activatable(true)
        .build();
    let clear_metadata_row = adw::ActionRow::builder()
        .title("Clear metadata caches")
        .subtitle("Drops local MonARCH metadata caches so discovery can rebuild them.")
        .activatable(true)
        .build();
    let clear_build_row = adw::ActionRow::builder()
        .title("Clear AUR build cache")
        .subtitle("Removes cached AUR sources and build artifacts.")
        .activatable(true)
        .build();
    let chaotic_prepare_row = adw::ActionRow::builder()
        .title("Prepare Chaotic-AUR components")
        .subtitle("Installs the Chaotic keyring and mirrorlist through the helper.")
        .activatable(true)
        .build();
    let restart_onboarding_row = adw::ActionRow::builder()
        .title("Restart onboarding")
        .subtitle("Return to the first-run flow on the next launch.")
        .activatable(true)
        .build();
    let unlock_row = adw::ActionRow::builder()
        .title("Repair stale pacman lock")
        .subtitle("Use this when a previous transaction left db.lck behind.")
        .activatable(true)
        .build();
    let reload_row = adw::ActionRow::builder()
        .title("Refresh GTK discovery views")
        .subtitle("Marks catalog-backed pages dirty and reloads them in place.")
        .activatable(true)
        .build();
    let rank_mirrors_row = adw::ActionRow::builder()
        .title("Rank pacman mirrors")
        .subtitle("Uses the best available mirror tool for this distro.")
        .activatable(true)
        .build();
    let remove_orphans_row = adw::ActionRow::builder()
        .title("Remove orphan packages")
        .subtitle("Cleans orphaned dependencies through the preserved helper path.")
        .activatable(true)
        .build();
    maintenance_group.add(&clear_cache_row);
    maintenance_group.add(&cache_state_row);
    maintenance_group.add(&orphan_state_row);
    maintenance_group.add(&mirror_tool_row);
    maintenance_group.add(&refresh_db_row);
    maintenance_group.add(&keyring_row);
    maintenance_group.add(&clear_metadata_row);
    maintenance_group.add(&clear_build_row);
    maintenance_group.add(&chaotic_prepare_row);
    maintenance_group.add(&rank_mirrors_row);
    maintenance_group.add(&remove_orphans_row);
    maintenance_group.add(&unlock_row);
    maintenance_group.add(&reload_row);
    maintenance_group.add(&restart_onboarding_row);

    let status_label = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();

    page.append(&hero);
    page.append(&mission_grid);
    page.append(&wrap_settings_group(
        "System Readiness",
        "What MonARCH sees about this machine before it installs or updates software.",
        &health_group,
    ));
    page.append(&wrap_settings_group(
        "Package Sources",
        "Control which ecosystems appear in discovery and how source policy is explained.",
        &source_group,
    ));
    page.append(&wrap_settings_group(
        "Workflow and Authorization",
        "Choose between guided one-click behavior and more explicit power-user prompts.",
        &workflow_group,
    ));
    page.append(&wrap_settings_group(
        "Behavior and Interface",
        "Appearance, concurrency, and local frontend preferences.",
        &appearance_group,
    ));
    page.append(&wrap_settings_group(
        "Builder and Privacy",
        "AUR cleanup, concurrency, and local privacy preferences.",
        &builder_group,
    ));
    page.append(&wrap_settings_group(
        "Privacy",
        "Optional frontend-only signals and local preference storage.",
        &privacy_group,
    ));
    page.append(&wrap_settings_group(
        "Repairs and Maintenance",
        "Cache, keyring, lock, and database actions through the preserved helper path.",
        &maintenance_group,
    ));
    page.append(&status_label);
    page.set_vexpand(false);
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&page)
        .build();
    scrolled.set_kinetic_scrolling(true);

    let rows = vec![
        (aur_row.clone(), "aur_enabled"),
        (flatpak_row.clone(), "flatpak_enabled"),
        (chaotic_row.clone(), "chaotic_enabled"),
        (system_apps_row.clone(), "show_system_apps"),
        (one_click_row.clone(), "one_click_enabled"),
        (reduce_prompts_row.clone(), "reduce_password_prompts"),
        (housekeeping_row.clone(), "automatic_housekeeping"),
        (sync_row.clone(), "sync_on_startup"),
        (verbose_row.clone(), "verbose_logs"),
        (telemetry_row.clone(), "telemetry_enabled"),
        (clean_build_row.clone(), "clean_build"),
    ];

    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let context = context.clone();
        async move {
            let _ = sender.send(context.fetch_settings_view().await);
        }
    });

    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || match receiver.try_recv() {
        Ok(Ok(view)) => {
            theme_dropdown.set_selected(match view.settings.theme_mode.as_str() {
                "light" => 1,
                "dark" => 2,
                _ => 0,
            });
            let context_theme = context.clone();
            theme_dropdown.connect_selected_notify(move |dropdown| {
                let mode = match dropdown.selected() {
                    1 => "light",
                    2 => "dark",
                    _ => "system",
                };
                apply_theme_from_mode(mode);
                let _ = context_theme.settings.update(|s| s.theme_mode = mode.to_string());
            });
            apply_settings_rows(&rows, &parallel_spin, view.settings.clone());
            if let Some(startup_status) = view.startup.as_ref() {
                apply_startup_status_rows(&chaotic_row, startup_status);
                apply_health_rows(
                    &health_summary_row,
                    &helper_health_row,
                    &source_health_row,
                    startup_status,
                );
                apply_mission_tiles(
                    &readiness_value,
                    &readiness_copy,
                    &sources_value,
                    &sources_copy,
                    &workflow_value,
                    &workflow_copy,
                    &view,
                );
            }
            apply_settings_view_rows(
                &snapshot_health_row,
                &warning_health_row,
                &cache_state_row,
                &orphan_state_row,
                &mirror_tool_row,
                &view,
            );
            wire_settings_rows(&rows, &parallel_spin, context.clone(), &status_label);
            wire_maintenance_rows(
                context.clone(),
                &cache_state_row,
                &orphan_state_row,
                &clear_cache_row,
                &refresh_db_row,
                &keyring_row,
                &clear_metadata_row,
                &clear_build_row,
                &chaotic_prepare_row,
                &rank_mirrors_row,
                &remove_orphans_row,
                &unlock_row,
                &reload_row,
                &restart_onboarding_row,
                &status_label,
            );
            glib::ControlFlow::Break
        }
        Ok(Err(error)) => {
            status_label.set_label(&error);
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
    });

    scrolled.upcast()
}

fn wrap_settings_group(
    title: &str,
    description: &str,
    group: &adw::PreferencesGroup,
) -> gtk::Box {
    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    panel.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(vec!["title-4".to_string()])
            .build(),
    );
    panel.append(
        &gtk::Label::builder()
            .label(description)
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["dim-label".to_string()])
            .build(),
    );
    panel.append(group);
    panel
}

fn build_status_tile(title: &str, value: &gtk::Label, copy: &gtk::Label) -> gtk::Box {
    let tile = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .hexpand(true)
        .css_classes(vec!["monarch-panel".to_string(), "monarch-settings-tile".to_string()])
        .build();
    tile.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(vec!["title-5".to_string()])
            .build(),
    );
    tile.append(value);
    tile.append(copy);
    tile
}

fn apply_mission_tiles(
    readiness_value: &gtk::Label,
    readiness_copy: &gtk::Label,
    sources_value: &gtk::Label,
    sources_copy: &gtk::Label,
    workflow_value: &gtk::Label,
    workflow_copy: &gtk::Label,
    view: &SettingsView,
) {
    if let Some(startup) = view.startup.as_ref() {
        let ready = startup.missing_required_bins.is_empty()
            && startup.helper_available
            && !startup.stale_pacman_lock
            && startup.keyring_ready
            && startup.sync_db_healthy;
        readiness_value.set_label(if ready { "Ready" } else { "Needs Attention" });
        let mut issues = Vec::new();
        if startup.stale_pacman_lock {
            issues.push("stale lock");
        }
        if !startup.helper_available {
            issues.push("helper");
        }
        if !startup.keyring_ready {
            issues.push("keyring");
        }
        if !startup.sync_db_healthy {
            issues.push("sync db");
        }
        if !startup.missing_required_bins.is_empty() {
            issues.push("missing tools");
        }
        let readiness_detail = if issues.is_empty() {
            "Startup checks are clear and package workflows can run normally.".to_string()
        } else {
            format!("Blocking areas: {}.", issues.join(", "))
        };
        readiness_copy.set_label(&readiness_detail);

        let mut enabled = vec!["Official".to_string()];
        if view.settings.aur_enabled {
            enabled.push("AUR".to_string());
        }
        if view.settings.flatpak_enabled {
            enabled.push("Flatpak".to_string());
        }
        if startup.distro.chaotic_support != ChaoticSupport::Blocked
            && (view.settings.chaotic_enabled || startup.distro.chaotic_configured)
        {
            enabled.push("Chaotic".to_string());
        }
        sources_value.set_label(&enabled.join(" • "));
        sources_copy.set_label(match startup.distro.chaotic_support {
            ChaoticSupport::Blocked => {
                "Host policy blocks Chaotic-AUR; MonARCH keeps discovery aligned with that."
            }
            ChaoticSupport::Native => {
                "This distro ships Chaotic natively; MonARCH follows host repo configuration."
            }
            ChaoticSupport::Allowed => {
                "Source visibility is controlled here and respected across discovery and installs."
            }
        });
    }

    let workflow_mode = if view.settings.one_click_enabled {
        "One-Click"
    } else if view.settings.reduce_password_prompts {
        "Reduced Prompts"
    } else {
        "System Auth"
    };
    workflow_value.set_label(workflow_mode);
    workflow_copy.set_label(&format!(
        "{} sync on startup • {} logs",
        if view.settings.sync_on_startup {
            "Automatic"
        } else {
            "Manual"
        },
        if view.settings.verbose_logs {
            "verbose"
        } else {
            "concise"
        }
    ));
}

fn apply_health_rows(
    health_summary_row: &adw::ActionRow,
    helper_health_row: &adw::ActionRow,
    source_health_row: &adw::ActionRow,
    startup_status: &StartupStatus,
) {
    let readiness = if startup_status.missing_required_bins.is_empty()
        && startup_status.helper_available
        && !startup_status.stale_pacman_lock
        && startup_status.keyring_ready
        && startup_status.sync_db_healthy
    {
        "Ready. Discovery and transactions can run normally.".to_string()
    } else {
        let mut issues = Vec::new();
        if !startup_status.helper_available {
            issues.push("helper unavailable");
        }
        if startup_status.stale_pacman_lock {
            issues.push("stale pacman lock");
        }
        if !startup_status.keyring_ready {
            issues.push("keyring needs repair");
        }
        if !startup_status.sync_db_healthy {
            issues.push("sync databases need refresh");
        }
        if !startup_status.missing_required_bins.is_empty() {
            issues.push("missing required tools");
        }
        format!("Attention needed: {}.", issues.join(", "))
    };
    health_summary_row.set_subtitle(&readiness);
    helper_health_row.set_subtitle(if startup_status.helper_available {
        "Available and ready for privileged package actions."
    } else {
        "Not available. Installs, updates, and repairs will fail until the helper path is restored."
    });
    source_health_row.set_subtitle(match startup_status.distro.chaotic_support {
        ChaoticSupport::Blocked => {
            "Chaotic-AUR is blocked on this distro. Official, AUR, and Flatpak behavior still follows your configured settings."
        }
        ChaoticSupport::Native => {
            "Chaotic-AUR is native on this distro and expected to come from host pacman.conf."
        }
        ChaoticSupport::Allowed => {
            if startup_status.distro.chaotic_configured {
                "Official, AUR, Flatpak, and optional Chaotic-AUR can be shown here."
            } else {
                "Chaotic-AUR is allowed here but still needs host preparation before it should be exposed."
            }
        }
    });
}

fn apply_settings_rows(
    rows: &[(adw::SwitchRow, &str)],
    parallel_spin: &gtk::SpinButton,
    settings: GtkSettings,
) {
    for (row, key) in rows {
        row.set_active(match *key {
            "aur_enabled" => settings.aur_enabled,
            "flatpak_enabled" => settings.flatpak_enabled,
            "chaotic_enabled" => settings.chaotic_enabled,
            "show_system_apps" => settings.show_system_apps,
            "one_click_enabled" => settings.one_click_enabled,
            "reduce_password_prompts" => settings.reduce_password_prompts,
            "automatic_housekeeping" => settings.automatic_housekeeping,
            "sync_on_startup" => settings.sync_on_startup,
            "verbose_logs" => settings.verbose_logs,
            "telemetry_enabled" => settings.telemetry_enabled,
            "clean_build" => settings.clean_build,
            _ => false,
        });
    }
    parallel_spin.set_value(settings.parallel_downloads as f64);
}

fn apply_startup_status_rows(chaotic_row: &adw::SwitchRow, startup_status: &StartupStatus) {
    match startup_status.distro.chaotic_support {
        ChaoticSupport::Blocked => {
            chaotic_row.set_sensitive(false);
            chaotic_row.set_active(false);
            chaotic_row.set_subtitle("Blocked on this distro to avoid incompatible partial-upgrade paths.");
        }
        ChaoticSupport::Native => {
            chaotic_row.set_sensitive(false);
            chaotic_row.set_active(true);
            chaotic_row.set_subtitle("Native on this distro. Chaotic-AUR is expected to come from host pacman.conf.");
        }
        ChaoticSupport::Allowed => {
            chaotic_row.set_sensitive(true);
            if startup_status.distro.chaotic_configured {
                chaotic_row.set_subtitle("Configured on this host. Discovery visibility can be toggled here.");
            } else {
                chaotic_row.set_subtitle("Allowed on this distro. Prepare components in Maintenance if the host is not configured yet.");
            }
        }
    }
}

fn apply_settings_view_rows(
    snapshot_health_row: &adw::ActionRow,
    warning_health_row: &adw::ActionRow,
    cache_state_row: &adw::ActionRow,
    orphan_state_row: &adw::ActionRow,
    mirror_tool_row: &adw::ActionRow,
    view: &SettingsView,
) {
    if let Some(snapshot) = view.snapshot_status.as_ref() {
        snapshot_health_row.set_subtitle(&snapshot.message);
    }

    if let Some(warnings) = view.update_warnings.as_ref() {
        let mut items = Vec::new();
        if warnings.reboot_required {
            items.push("reboot required".to_string());
        }
        if !warnings.pacnew_warnings.is_empty() {
            items.push(format!("{} pacnew files", warnings.pacnew_warnings.len()));
        }
        if !warnings.restart_required_services.is_empty() {
            items.push(format!(
                "{} services require restart",
                warnings.restart_required_services.len()
            ));
        }
        if !warnings.critical_advisories.is_empty() {
            items.push(format!(
                "{} critical advisories",
                warnings.critical_advisories.len()
            ));
        }
        let subtitle = if items.is_empty() {
            "No active reboot, pacnew, or service restart warnings.".to_string()
        } else {
            items.join(" • ")
        };
        warning_health_row.set_subtitle(&subtitle);
    }

    if let Some(cache) = view.cache.as_ref() {
        cache_state_row.set_subtitle(&format!("Current pacman cache size: {}", cache.human_readable));
    }

    if let Some(orphans) = view.orphans.as_ref() {
        let subtitle = if orphans.orphans.is_empty() {
            "No orphan packages detected.".to_string()
        } else {
            format!(
                "{} orphan packages consuming {}.",
                orphans.orphans.len(),
                orphans.human_readable
            )
        };
        orphan_state_row.set_subtitle(&subtitle);
    }

    mirror_tool_row.set_subtitle(
        &view
            .mirror_rank_tool
            .clone()
            .unwrap_or_else(|| "No mirror ranking tool detected.".to_string()),
    );
}

fn wire_settings_rows(
    rows: &[(adw::SwitchRow, &str)],
    parallel_spin: &gtk::SpinButton,
    context: AppContext,
    status_label: &gtk::Label,
) {
    for (row, key) in rows {
        let key = (*key).to_string();
        let context = context.clone();
        let status_label = status_label.clone();
        row.connect_active_notify(move |row| {
            let value = row.is_active();
            let prompt_result = if value && matches!(key.as_str(), "one_click_enabled" | "reduce_password_prompts") {
                ensure_session_auth(&context, parent_window_for(row).as_ref(), true)
            } else if value && key == "flatpak_enabled" {
                ensure_session_auth(&context, parent_window_for(row).as_ref(), false)
            } else {
                Ok(())
            };
            if let Err(error) = prompt_result {
                status_label.set_label(&error);
                row.set_active(!value);
                return;
            }
            let key_for_task = key.clone();
            let status_label_for_result = status_label.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            context.runtime.spawn({
                let settings = context.settings.clone();
                let catalog = context.catalog.clone();
                async move {
                    let result = settings.update(|state| match key_for_task.as_str() {
                        "aur_enabled" => state.aur_enabled = value,
                        "flatpak_enabled" => state.flatpak_enabled = value,
                        "chaotic_enabled" => state.chaotic_enabled = value,
                        "show_system_apps" => state.show_system_apps = value,
                        "one_click_enabled" => state.one_click_enabled = value,
                        "reduce_password_prompts" => state.reduce_password_prompts = value,
                        "automatic_housekeeping" => state.automatic_housekeeping = value,
                        "sync_on_startup" => state.sync_on_startup = value,
                        "verbose_logs" => state.verbose_logs = value,
                        "telemetry_enabled" => state.telemetry_enabled = value,
                        "clean_build" => state.clean_build = value,
                        _ => {}
                    });
                    let result = match (key_for_task.as_str(), value, result) {
                        ("one_click_enabled", _, Ok(_)) => {
                            let policy_result = catalog.install_monarch_policy().await;
                            if !value {
                                let _ = catalog.set_session_password(None);
                            }
                            policy_result.map(|message| {
                                if value {
                                    format!("{message} Branded one-click auth is now active.")
                                } else {
                                    format!("{message} MonARCH will use explicit system authentication again.")
                                }
                            })
                        }
                        ("reduce_password_prompts", _, Ok(_)) => {
                            if !value {
                                match catalog.settings().load() {
                                    Ok(settings) => {
                                        if !settings.one_click_enabled {
                                            let _ = catalog.set_session_password(None);
                                        }
                                    }
                                    Err(error) => {
                                        let _ = sender.send(error);
                                        return;
                                    }
                                }
                            }
                            Ok(if value {
                                "Reduced-prompt mode enabled. MonARCH can reuse the current session credential.".to_string()
                            } else {
                                "Reduced-prompt mode disabled.".to_string()
                            })
                        }
                        ("flatpak_enabled", true, Ok(_)) => catalog
                            .prepare_flatpak()
                            .await
                            .map(|message| format!("{message} Discovery will now include Flatpak apps.")),
                        ("chaotic_enabled", true, Ok(_)) => Ok(
                            "Chaotic-AUR discovery enabled. If the host is not configured yet, run 'Prepare Chaotic-AUR components' below and then refresh discovery."
                                .to_string(),
                        ),
                        ("chaotic_enabled", false, Ok(_)) => {
                            Ok("Chaotic-AUR discovery disabled.".to_string())
                        }
                        ("show_system_apps", true, Ok(_)) => {
                            Ok("System apps will now appear in Search when they match the query.".to_string())
                        }
                        ("show_system_apps", false, Ok(_)) => {
                            Ok("System apps are hidden from Search again.".to_string())
                        }
                        (_, _, Ok(_)) => Ok("Settings updated.".to_string()),
                        (_, _, Err(error)) => Err(error),
                    };
                    let _ = sender.send(result.unwrap_or_else(|error| error));
                }
            });
            let context_for_result = context.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                match receiver.try_recv() {
                    Ok(message) => {
                        status_label_for_result.set_label(&message);
                        context_for_result.mark_catalog_dirty();
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                }
            });
        });
    }

    let context_for_spin = context.clone();
    let status_for_spin = status_label.clone();
    parallel_spin.connect_value_changed(move |spin| {
        let value = spin.value() as u32;
        let status_for_spin = status_for_spin.clone();
        let context_for_toast = context_for_spin.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        context_for_spin.runtime.spawn({
            let settings = context_for_spin.settings.clone();
            async move {
                let _ = sender.send(
                    settings
                        .update(|state| state.parallel_downloads = value)
                        .map(|_| format!("Parallel job preference saved ({value})."))
                        .unwrap_or_else(|error| error),
                );
            }
        });
        glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
            match receiver.try_recv() {
                Ok(message) => {
                    status_for_spin.set_label(&message);
                    context_for_toast.show_toast(&message);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
    });
}

#[allow(clippy::too_many_arguments)] // TODO: group into MaintenanceRowsRefs struct
fn wire_maintenance_rows(
    context: AppContext,
    cache_state_row: &adw::ActionRow,
    orphan_state_row: &adw::ActionRow,
    clear_cache_row: &adw::ActionRow,
    refresh_db_row: &adw::ActionRow,
    keyring_row: &adw::ActionRow,
    clear_metadata_row: &adw::ActionRow,
    clear_build_row: &adw::ActionRow,
    chaotic_prepare_row: &adw::ActionRow,
    rank_mirrors_row: &adw::ActionRow,
    remove_orphans_row: &adw::ActionRow,
    unlock_row: &adw::ActionRow,
    reload_row: &adw::ActionRow,
    restart_onboarding_row: &adw::ActionRow,
    status_label: &gtk::Label,
) {
    clear_cache_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        let clear_cache_row = clear_cache_row.clone();
        move |_| {
            if let Err(error) = ensure_session_auth(&context, parent_window_for(&clear_cache_row).as_ref(), false) {
                status_label.set_label(&error);
                return;
            }
            status_label.set_label("Clearing pacman cache through monarch-helper...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog.clear_pacman_cache().await
            });
        }
    });

    cache_state_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        move |_| {
            status_label.set_label("Refreshing cache statistics...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog
                    .load_cache_size()
                    .await
                    .map(|cache| format!("Current pacman cache size: {}", cache.human_readable))
            });
        }
    });

    orphan_state_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        move |_| {
            status_label.set_label("Refreshing orphan package state...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog.load_orphans().await.map(|orphans| {
                    if orphans.orphans.is_empty() {
                        "No orphan packages detected.".to_string()
                    } else {
                        format!(
                            "{} orphan packages are consuming {}.",
                            orphans.orphans.len(),
                            orphans.human_readable
                        )
                    }
                })
            });
        }
    });

    refresh_db_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        let refresh_db_row = refresh_db_row.clone();
        move |_| {
            if let Err(error) = ensure_session_auth(&context, parent_window_for(&refresh_db_row).as_ref(), false) {
                status_label.set_label(&error);
                return;
            }
            status_label.set_label("Refreshing pacman sync databases...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog.force_refresh_databases().await
            });
        }
    });

    keyring_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        let keyring_row = keyring_row.clone();
        move |_| {
            if let Err(error) = ensure_session_auth(&context, parent_window_for(&keyring_row).as_ref(), false) {
                status_label.set_label(&error);
                return;
            }
            status_label.set_label("Refreshing security keyrings...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog.refresh_keyring().await
            });
        }
    });

    clear_metadata_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        move |_| {
            status_label.set_label("Clearing metadata caches...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog.clear_metadata_caches().await
            });
        }
    });

    clear_build_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        move |_| {
            status_label.set_label("Clearing AUR build cache...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog.clear_build_cache().await
            });
        }
    });

    chaotic_prepare_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        let chaotic_prepare_row = chaotic_prepare_row.clone();
        move |_| {
            if let Err(error) = ensure_session_auth(&context, parent_window_for(&chaotic_prepare_row).as_ref(), false) {
                status_label.set_label(&error);
                return;
            }
            status_label.set_label("Preparing Chaotic-AUR keyring and mirrorlist...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog.prepare_chaotic_components().await
            });
        }
    });

    rank_mirrors_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        let rank_mirrors_row = rank_mirrors_row.clone();
        move |_| {
            if let Err(error) =
                ensure_session_auth(&context, parent_window_for(&rank_mirrors_row).as_ref(), false)
            {
                status_label.set_label(&error);
                return;
            }
            status_label.set_label("Ranking pacman mirrors...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog.rank_mirrors().await
            });
        }
    });

    remove_orphans_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        let remove_orphans_row = remove_orphans_row.clone();
        move |_| {
            if let Err(error) = ensure_session_auth(
                &context,
                parent_window_for(&remove_orphans_row).as_ref(),
                false,
            ) {
                status_label.set_label(&error);
                return;
            }
            status_label.set_label("Removing orphan packages...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog.remove_orphans().await
            });
        }
    });

    unlock_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        let unlock_row = unlock_row.clone();
        move |_| {
            if let Err(error) = ensure_session_auth(&context, parent_window_for(&unlock_row).as_ref(), false) {
                status_label.set_label(&error);
                return;
            }
            status_label.set_label("Repairing pacman lock through monarch-helper...");
            run_catalog_action(context.clone(), status_label.clone(), |catalog| async move {
                catalog.repair_unlock_pacman().await
            });
        }
    });

    reload_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        move |_| {
            context.mark_catalog_dirty();
            status_label.set_label("Discovery-backed GTK pages are refreshing.");
        }
    });

    restart_onboarding_row.connect_activated({
        let context = context.clone();
        let status_label = status_label.clone();
        move |_| {
            let (sender, receiver) = std::sync::mpsc::channel();
            context.runtime.spawn({
                let settings = context.settings.clone();
                async move {
                    let _ = sender.send(settings.set_onboarding_completed(false));
                }
            });
            let status_label_for_result = status_label.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                match receiver.try_recv() {
                    Ok(Ok(_)) => {
                        status_label_for_result
                            .set_label("Onboarding will be shown again on the next launch.");
                        glib::ControlFlow::Break
                    }
                    Ok(Err(error)) => {
                        status_label_for_result.set_label(&error);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                }
            });
        }
    });
}

fn run_catalog_action<F, Fut>(context: AppContext, status_label: gtk::Label, action: F)
where
    F: FnOnce(std::sync::Arc<monarch_core::catalog::CatalogService>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<String, String>> + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let catalog = context.catalog.clone();
        async move {
            let _ = sender.send(action(catalog).await);
        }
    });
    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
        match receiver.try_recv() {
            Ok(Ok(message)) => {
                status_label.set_label(&message);
                context.mark_catalog_dirty();
                glib::ControlFlow::Break
            }
            Ok(Err(error)) => {
                status_label.set_label(&error);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        }
    });
}

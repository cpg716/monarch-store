use crate::context::AppContext;
use crate::telemetry;
use crate::theme::portal::apply_theme_from_mode;
use crate::ui::auth::ensure_session_auth;
use crate::ui::media;
use crate::ui::pages::discovery::DiscoveryPage;
use crate::ui::pages::favorites::build_favorites_page;
use crate::ui::pages::home::build_home_page;
use crate::ui::pages::installed::build_installed_page;
use crate::ui::pages::news::build_news_page;
use crate::ui::pages::settings::build_settings_page;
use crate::ui::pages::updates::build_updates_page;
use adw::prelude::*;
use monarch_core::models::ChaoticSupport;
use std::rc::Rc;

pub fn build_ui(app: &adw::Application, context: AppContext) {
    /* Ensure placeholder texture is initialized on main thread before any Picture is realized */
    let _ = media::placeholder_texture();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("MonARCH Store")
        .default_width(1240)
        .default_height(820)
        .build();

    let root_stack = gtk::Stack::new();

    let loading = build_loading_screen();
    root_stack.add_named(&loading, Some("loading"));

    let navigation = adw::NavigationView::new();
    let (shell, titlebar, shortcut_refs) = build_main_shell(&context, &navigation);
    navigation.add(&shell);
    root_stack.add_named(&navigation, Some("shell"));

    let key_controller = gtk::EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    key_controller.connect_key_pressed({
        let shortcut_refs = shortcut_refs.clone();
        move |_, keyval, _, state| {
            let is_ctrl_k = keyval == gtk::gdk::Key::k && state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            let is_slash = keyval == gtk::gdk::Key::slash || keyval == gtk::gdk::Key::KP_Divide;
            if is_ctrl_k || is_slash {
                let _ = shortcut_refs.navigation.pop_to_page(&shortcut_refs.page);
                shortcut_refs.view_stack.set_visible_child_name("search");
                let _ = shortcut_refs.context.settings.set_active_tab("search");
                sync_primary_header_buttons(&shortcut_refs.primary_buttons, Some("search"));
                shortcut_refs.search_entry.grab_focus();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    let onboarding = build_onboarding_page(context.clone(), &root_stack, window.upcast_ref());
    root_stack.add_named(&onboarding, Some("onboarding"));
    window.add_controller(key_controller);

    root_stack.set_visible_child_name("loading");
    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&root_stack));

    let (toast_tx, toast_rx) = std::sync::mpsc::channel();
    context.set_toast_sender(toast_tx);
    let toast_overlay_for_source = toast_overlay.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(150), move || {
        while let Ok(msg) = toast_rx.try_recv() {
            let toast = adw::Toast::new(&msg);
            toast_overlay_for_source.add_toast(toast);
        }
        glib::ControlFlow::Continue
    });

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&titlebar);
    toolbar.set_content(Some(&toast_overlay));
    window.set_content(Some(&toolbar));
    window.present();

    let (sender, receiver) = std::sync::mpsc::channel();
    let context_for_load = context.clone();
    context.runtime.spawn(async move {
        let _ = sender.send((context_for_load.fetch_startup_status().await, context_for_load.settings.load()));
    });

    let root_stack_for_load = root_stack.clone();
    let context_for_unlock = context.clone();
    let window_for_unlock = window.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
        match receiver.try_recv() {
            Ok((Ok(status), Ok(settings))) => {
                apply_theme_from_mode(&settings.theme_mode);
                let telemetry_settings = context_for_unlock.settings.clone();
                context_for_unlock.runtime.spawn(async move {
                    telemetry::track_event_async(&telemetry_settings, "app_started", None).await;
                });
                let warnings_require_attention = status
                    .warnings
                    .iter()
                    .any(|warning| !warning.to_lowercase().contains("stale pacman database lock"));
                if status.stale_pacman_lock && status.onboarding_completed {
                    if settings.one_click_enabled || settings.reduce_password_prompts {
                        let _ = ensure_session_auth(&context_for_unlock, Some(window_for_unlock.upcast_ref()), false);
                    }
                    let (unlock_sender, unlock_receiver) = std::sync::mpsc::channel();
                    let context_for_unlock_task = context_for_unlock.clone();
                    context_for_unlock.runtime.spawn(async move {
                        let _ = unlock_sender.send(context_for_unlock_task.catalog.repair_unlock_pacman().await);
                    });
                    let root_stack_for_unlock = root_stack_for_load.clone();
                    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                        match unlock_receiver.try_recv() {
                            Ok(Ok(_)) => {
                                if !status.onboarding_completed || status.registry_empty || warnings_require_attention {
                                    root_stack_for_unlock.set_visible_child_name("onboarding");
                                } else {
                                    root_stack_for_unlock.set_visible_child_name("shell");
                                }
                                glib::ControlFlow::Break
                            }
                            Ok(Err(_)) => {
                                root_stack_for_unlock.set_visible_child_name("onboarding");
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                        }
                    });
                    glib::ControlFlow::Break
                } else if !status.onboarding_completed || status.registry_empty || warnings_require_attention {
                    root_stack_for_load.set_visible_child_name("onboarding");
                    glib::ControlFlow::Break
                } else {
                    root_stack_for_load.set_visible_child_name("shell");
                    glib::ControlFlow::Break
                }
            }
            Ok((Err(_), _)) | Ok((_, Err(_))) => {
                root_stack_for_load.set_visible_child_name("onboarding");
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        }
    });
}

#[derive(Clone)]
struct ShellShortcutRefs {
    navigation: adw::NavigationView,
    view_stack: gtk::Stack,
    search_entry: gtk::SearchEntry,
    primary_buttons: Rc<std::cell::RefCell<Vec<(String, gtk::Button)>>>,
    page: adw::NavigationPage,
    context: AppContext,
}

fn build_main_shell(
    context: &AppContext,
    navigation: &adw::NavigationView,
) -> (adw::NavigationPage, adw::HeaderBar, ShellShortcutRefs) {
    let shell_settings = context.settings.load().unwrap_or_default();
    apply_theme_from_mode(&shell_settings.theme_mode);
    let view_stack = gtk::Stack::new();
    view_stack.set_vexpand(true);
    view_stack.set_hexpand(true);

    let search_page = DiscoveryPage::new(context.clone(), navigation);
    let discover_page = build_home_page(context.clone(), navigation, view_stack.clone());
    let favorites_page = build_favorites_page(context.clone(), navigation);
    let installed_page = build_installed_page(context.clone(), navigation, view_stack.clone());
    let updates_page = build_updates_page(context.clone(), navigation);
    let news_page = build_news_page(context.clone());
    let settings_page = build_settings_page(context.clone());

    view_stack.add_named(&discover_page, Some("discover"));
    view_stack.add_named(&installed_page, Some("library"));
    view_stack.add_named(&search_page.root, Some("search"));
    view_stack.add_named(&updates_page, Some("updates"));
    view_stack.add_named(&favorites_page, Some("favorites"));
    view_stack.add_named(&news_page, Some("news"));
    view_stack.add_named(&settings_page, Some("settings"));

    let page = adw::NavigationPage::builder()
        .title("MonARCH Store")
        .child(&view_stack)
        .build();

    let primary_buttons = Rc::new(std::cell::RefCell::new(Vec::<(String, gtk::Button)>::new()));
    let nav_tabs = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .css_classes(vec!["monarch-header-tabs".to_string()])
        .build();
    for (name, label, icon_name) in [
        ("discover", "Discover", "view-grid-symbolic"),
        ("library", "Library", "folder-download-symbolic"),
        ("search", "Search", "system-search-symbolic"),
    ] {
        let button = gtk::Button::builder()
            .label(label)
            .icon_name(icon_name)
            .css_classes(vec!["flat".to_string(), "monarch-header-tab".to_string()])
            .build();
        button.connect_clicked({
            let view_stack = view_stack.clone();
            let context = context.clone();
            let primary_buttons = primary_buttons.clone();
            let navigation = navigation.clone();
            let page = page.clone();
            let tab_name = name.to_string();
            move |_| {
                let _ = navigation.pop_to_page(&page);
                view_stack.set_visible_child_name(&tab_name);
                let _ = context.settings.set_active_tab(&tab_name);
                sync_primary_header_buttons(&primary_buttons, Some(tab_name.as_str()));
            }
        });
        primary_buttons
            .borrow_mut()
            .push((name.to_string(), button.clone()));
        nav_tabs.append(&button);
    }

    let more_menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .css_classes(vec!["flat".to_string(), "monarch-header-more".to_string()])
        .build();
    let more_menu_content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    for (name, label) in [
        ("updates", "Updates"),
        ("favorites", "Favorites"),
        ("news", "News"),
        ("settings", "Settings"),
    ] {
        let button = gtk::Button::builder()
            .label(label)
            .halign(gtk::Align::Fill)
            .css_classes(vec!["flat".to_string(), "monarch-header-menu-item".to_string()])
            .build();
        button.connect_clicked({
            let view_stack = view_stack.clone();
            let context = context.clone();
            let primary_buttons = primary_buttons.clone();
            let menu_button = more_menu_button.clone();
            let navigation = navigation.clone();
            let page = page.clone();
            let tab_name = name.to_string();
            move |_| {
                let _ = navigation.pop_to_page(&page);
                view_stack.set_visible_child_name(&tab_name);
                let _ = context.settings.set_active_tab(&tab_name);
                sync_primary_header_buttons(&primary_buttons, None);
                menu_button.popdown();
            }
        });
        more_menu_content.append(&button);
    }
    more_menu_button.set_popover(Some(
        &gtk::Popover::builder().child(&more_menu_content).build(),
    ));

    let titlebar = build_window_titlebar(navigation, &page, &nav_tabs, &more_menu_button);

    let initial_tab = normalize_shell_tab(&shell_settings.active_tab);
    view_stack.set_visible_child_name(&initial_tab);
    if matches!(initial_tab.as_str(), "discover" | "library" | "search") {
        sync_primary_header_buttons(&primary_buttons, Some(initial_tab.as_str()));
    } else {
        sync_primary_header_buttons(&primary_buttons, None);
    }

    let shortcut_refs = ShellShortcutRefs {
        navigation: navigation.clone(),
        view_stack: view_stack.clone(),
        search_entry: search_page.search_entry.clone(),
        primary_buttons: primary_buttons.clone(),
        page: page.clone(),
        context: context.clone(),
    };

    (page, titlebar, shortcut_refs)
}

fn build_loading_screen() -> gtk::Box {
    let shell = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .vexpand(true)
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Center)
        .css_classes(vec!["monarch-page".to_string()])
        .build();
    // Only set_filename for raster formats; SVG can leave Picture with a null paintable (gtk_scaler_new assertion).
    let logo_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../src/assets/arch-logo.svg"
    );
    let logo = gtk::Picture::new();
    logo.set_paintable(Some(media::placeholder_texture()));
    let use_file = std::path::Path::new(logo_path).exists()
        && !(logo_path.ends_with(".svg") || logo_path.ends_with(".SVG"));
    if use_file {
        logo.set_filename(Some(logo_path));
    }
    logo.set_width_request(86);
    logo.set_height_request(86);
    logo.set_can_shrink(true);
    let logo_wrap = gtk::Box::builder()
        .halign(gtk::Align::Center)
        .css_classes(vec!["monarch-sidebar-brand".to_string()])
        .build();
    logo_wrap.append(&logo);
    let title = gtk::Label::builder()
        .label("Preparing MonARCH")
        .css_classes(vec!["monarch-hero-title".to_string()])
        .build();
    let description = gtk::Label::builder()
        .label("Hydrating the catalog, checking startup health, and preparing the store.")
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(vec!["monarch-hero-copy".to_string()])
        .build();
    let badges = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Center)
        .build();
    for label in ["Managed", "Host-Adaptive", "Iron Core", "AUR", "Flatpak"] {
        badges.append(
            &gtk::Label::builder()
                .label(label)
                .css_classes(vec!["monarch-store-card-badge".to_string()])
                .build(),
        );
    }
    let progress = gtk::ProgressBar::builder().pulse_step(0.18).build();
    progress.pulse();
    let progress_for_pulse = progress.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(140), move || {
        progress_for_pulse.pulse();
        glib::ControlFlow::Continue
    });
    shell.append(&logo_wrap);
    shell.append(&title);
    shell.append(&description);
    shell.append(&badges);
    shell.append(&progress);
    shell
}

fn sync_primary_header_buttons(
    primary_buttons: &Rc<std::cell::RefCell<Vec<(String, gtk::Button)>>>,
    active: Option<&str>,
) {
    for (name, button) in primary_buttons.borrow().iter() {
        if active.is_some_and(|active| active == name) {
            button.add_css_class("is-active");
        } else {
            button.remove_css_class("is-active");
        }
    }
}

fn normalize_shell_tab(tab: &str) -> String {
    match tab {
        "home" | "discovery" => "discover".to_string(),
        "" => "discover".to_string(),
        other => other.to_string(),
    }
}

fn build_window_titlebar(
    navigation: &adw::NavigationView,
    shell_page: &adw::NavigationPage,
    nav_tabs: &gtk::Box,
    more_menu_button: &gtk::MenuButton,
) -> adw::HeaderBar {
    let back_button = gtk::Button::builder()
        .icon_name("go-previous-symbolic")
        .css_classes(vec!["flat".to_string(), "monarch-header-back".to_string()])
        .visible(false)
        .build();
    back_button.connect_clicked({
        let navigation = navigation.clone();
        move |_| {
            let _ = navigation.pop();
        }
    });

    let brand = gtk::Label::builder()
        .label("MonARCH Store")
        .css_classes(vec!["monarch-wordmark".to_string()])
        .build();
    let subtitle = gtk::Label::builder()
        .label("Universal Arch Linux app manager")
        .css_classes(vec!["monarch-title-copy".to_string()])
        .build();
    let brand_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(14)
        .css_classes(vec!["monarch-shell-header".to_string()])
        .build();
    brand_box.append(&brand);
    brand_box.append(&subtitle);
    let title_shell = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(16)
        .css_classes(vec!["monarch-shell-titlebar".to_string()])
        .build();
    title_shell.append(nav_tabs);

    let titlebar = adw::HeaderBar::builder()
        .title_widget(&title_shell)
        .show_start_title_buttons(true)
        .show_end_title_buttons(true)
        .build();
    titlebar.pack_start(&back_button);
    titlebar.pack_start(&brand_box);
    titlebar.pack_end(more_menu_button);

    let sync_back_button = {
        let navigation = navigation.clone();
        let shell_page = shell_page.clone();
        let back_button = back_button.clone();
        move || {
            let visible = navigation
                .visible_page()
                .map(|page| page != shell_page)
                .unwrap_or(false);
            back_button.set_visible(visible);
        }
    };
    sync_back_button();
    navigation.connect_pushed({
        let sync_back_button = sync_back_button.clone();
        move |_| {
            sync_back_button();
        }
    });
    navigation.connect_popped({
        let sync_back_button = sync_back_button.clone();
        move |_, _| {
            sync_back_button();
        }
    });
    navigation.connect_replaced(move |_| {
        sync_back_button();
    });

    titlebar
}

fn build_onboarding_page(
    context: AppContext,
    root_stack: &gtk::Stack,
    window: &gtk::Window,
) -> gtk::Widget {
    let window = window.clone();
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .css_classes(vec!["monarch-page".to_string()])
        .build();

    let hero = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .halign(gtk::Align::Center)
        .build();
    let hero_icon = gtk::Box::builder()
        .halign(gtk::Align::Center)
        .css_classes(vec!["monarch-onboarding-icon".to_string()])
        .build();
    hero_icon.append(
        &gtk::Image::builder()
            .icon_name("system-software-install-symbolic")
            .build(),
    );
    hero.append(&hero_icon);
    hero.append(
        &gtk::Label::builder()
            .label("Welcome to MonARCH Store")
            .wrap(true)
            .justify(gtk::Justification::Center)
            .css_classes(vec!["monarch-hero-title".to_string()])
            .build(),
    );
    hero.append(
        &gtk::Label::builder()
            .label("Run through the first-use checklist so discovery, updates, and privileged actions behave safely on this machine.")
            .wrap(true)
            .justify(gtk::Justification::Center)
            .css_classes(vec!["monarch-hero-copy".to_string()])
            .build(),
    );

    let current_settings = context.settings.load().unwrap_or_default();
    let stack = gtk::Stack::new();
    let step_names = std::rc::Rc::new(std::cell::RefCell::new(vec![
        "welcome",
        "flatpak",
        "aur",
        "chaotic",
        "security",
        "theme",
        "confirm",
    ]));
    let step = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let settings_state = std::rc::Rc::new(std::cell::RefCell::new(current_settings.clone()));

    let startup_warning = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let missing_bins = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let health_detail = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let repair_button = gtk::Button::builder()
        .label("Repair Pacman Lock")
        .css_classes(vec!["destructive-action".to_string()])
        .halign(gtk::Align::Start)
        .sensitive(false)
        .build();
    let keyring_button = gtk::Button::builder()
        .label("Refresh Keyrings")
        .halign(gtk::Align::Start)
        .sensitive(false)
        .build();
    let refresh_db_button = gtk::Button::builder()
        .label("Refresh Databases")
        .halign(gtk::Align::Start)
        .sensitive(false)
        .build();
    let repair_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .build();
    repair_row.append(&repair_button);
    repair_row.append(&keyring_button);
    repair_row.append(&refresh_db_button);
    let status_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    status_box.append(&startup_warning);
    status_box.append(&missing_bins);
    status_box.append(&health_detail);
    status_box.append(&repair_row);

    let system_apps = adw::SwitchRow::builder()
        .title("Show system apps in search")
        .subtitle("Includes lower-level tools, runtimes, drivers, and other system-facing packages in Search. Installed apps still appear in Library and Updates regardless of this setting.")
        .active(current_settings.show_system_apps)
        .build();
    let search_group = adw::PreferencesGroup::builder()
        .title("Search visibility")
        .build();
    search_group.add(&system_apps);
    status_box.append(&search_group);
    stack.add_named(&status_box, Some("welcome"));

    let aur = adw::SwitchRow::builder()
        .title("Enable AUR discovery")
        .subtitle("Shows user-maintained packages from the Arch User Repository. AUR packages build locally before MonARCH asks the helper to install them.")
        .active(current_settings.aur_enabled)
        .build();
    let flatpak = adw::SwitchRow::builder()
        .title("Enable Flatpak discovery")
        .subtitle("Shows sandboxed desktop apps from Flatpak remotes like Flathub. These installs stay outside pacman.")
        .active(current_settings.flatpak_enabled)
        .build();
    let chaotic = adw::SwitchRow::builder()
        .title("Enable Chaotic-AUR discovery")
        .subtitle("Shows Chaotic-AUR binaries when this distro allows them. MonARCH never forces unsupported Chaotic setups.")
        .active(current_settings.chaotic_enabled)
        .build();
    let one_click = adw::SwitchRow::builder()
        .title("Enable one-click auth")
        .subtitle("Use the branded MonARCH password flow for helper actions instead of a fresh Polkit prompt each time.")
        .active(current_settings.one_click_enabled)
        .build();
    let reduce_prompts = adw::SwitchRow::builder()
        .title("Reduce password prompts")
        .subtitle("Reuses the current session credential when possible so installs, repairs, and updates feel less repetitive.")
        .active(current_settings.reduce_password_prompts)
        .build();
    let chaotic_copy = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let flatpak_intro = gtk::Label::builder()
        .label("Flatpak gives MonARCH a universal software lane for desktop apps that may not belong in pacman. It is especially useful for proprietary, complex, or cross-distro desktop software.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let flatpak_group = adw::PreferencesGroup::builder().title("Flatpak Support").build();
    flatpak_group.add(&flatpak);
    let flatpak_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    flatpak_page.append(&flatpak_intro);
    flatpak_page.append(&flatpak_group);
    stack.add_named(&flatpak_page, Some("flatpak"));

    let aur_intro = gtk::Label::builder()
        .label("The Arch User Repository exposes community-maintained build scripts. MonARCH keeps this safe by building in user space and only handing finished packages to the helper.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let aur_group = adw::PreferencesGroup::builder().title("AUR Support").build();
    aur_group.add(&aur);
    let aur_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    aur_page.append(&aur_intro);
    aur_page.append(&aur_group);
    stack.add_named(&aur_page, Some("aur"));

    let chaotic_intro = gtk::Label::builder()
        .label("Chaotic-AUR can provide pre-built binaries for some AUR packages. MonARCH only offers it when the host distro supports that configuration safely.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let chaotic_group = adw::PreferencesGroup::builder().title("Chaotic-AUR").build();
    chaotic_group.add(&chaotic);
    let chaotic_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    chaotic_page.append(&chaotic_intro);
    chaotic_page.append(&chaotic_group);
    chaotic_page.append(&chaotic_copy);
    stack.add_named(&chaotic_page, Some("chaotic"));

    let security_intro = gtk::Label::builder()
        .label("MonARCH can guide new users with branded one-click authorization while still letting experienced users stay on standard Polkit prompts. Your choice here only affects how auth is presented.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let telemetry_row = adw::SwitchRow::builder()
        .title("Anonymous usage stats")
        .subtitle("Optional analytics to improve the store. No personal data; can be changed later in Settings.")
        .active(current_settings.telemetry_enabled)
        .build();
    let security_group = adw::PreferencesGroup::builder().title("Authorization and privacy").build();
    security_group.add(&one_click);
    security_group.add(&reduce_prompts);
    security_group.add(&telemetry_row);
    let security_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    security_page.append(&security_intro);
    security_page.append(&security_group);
    stack.add_named(&security_page, Some("security"));

    let theme_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    let theme_choice_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    let theme_label = gtk::Label::builder()
        .label("Theme mode")
        .xalign(0.0)
        .css_classes(vec!["title-5".to_string()])
        .build();
    let theme_model = gtk::StringList::new(&["Follow System", "Light", "Dark"]);
    let theme_dropdown = gtk::DropDown::builder()
        .model(&theme_model)
        .selected(match current_settings.theme_mode.as_str() {
            "light" => 1,
            "dark" => 2,
            _ => 0,
        })
        .build();
    let theme_caption = gtk::Label::builder()
        .label("Choose whether MonARCH should follow the desktop appearance or pin a lighter or darker store surface from first launch.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    theme_page.append(
        &gtk::Label::builder()
            .label("Appearance")
            .xalign(0.0)
            .css_classes(vec!["title-4".to_string()])
            .build(),
    );
    theme_page.append(
        &gtk::Label::builder()
            .label("GTK MonARCH follows the host appearance through portals so KDE, GNOME, and Hyprland users get a native-feeling storefront without losing MonARCH branding.")
            .wrap(true)
            .xalign(0.0)
            .css_classes(vec!["dim-label".to_string()])
            .build(),
    );
    theme_choice_row.append(&theme_label);
    theme_choice_row.append(
        &gtk::Box::builder()
            .hexpand(true)
            .build(),
    );
    theme_choice_row.append(&theme_dropdown);
    theme_page.append(&theme_choice_row);
    theme_page.append(&theme_caption);
    theme_page.append(
        &gtk::Label::builder()
            .label("You can fine-tune appearance, accent behavior, and sidebar behavior later in Mission Control.")
            .wrap(true)
            .xalign(0.0)
            .css_classes(vec!["monarch-inline-note".to_string()])
            .build(),
    );
    stack.add_named(&theme_page, Some("theme"));

    let confirm_copy = gtk::Label::builder()
        .label("MonARCH will follow your desktop appearance, keep the host package sources intact, and route repo transactions through the preserved Iron Core helper path. You can change every one of these choices later in Settings, but this checklist makes the first launch safer and easier to understand.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let confirm_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    confirm_page.append(&confirm_copy);
    stack.add_named(&confirm_page, Some("confirm"));
    stack.set_visible_child_name("welcome");

    let previous = gtk::Button::builder().label("Back").sensitive(false).build();
    let next = gtk::Button::builder()
        .label("Next")
        .css_classes(vec!["suggested-action".to_string()])
        .build();
    let buttons = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .halign(gtk::Align::End)
        .css_classes(vec!["monarch-onboarding-actions".to_string()])
        .build();
    buttons.append(&previous);
    buttons.append(&next);

    let root_stack_for_next = root_stack.clone();
    next.connect_clicked({
        let step = step.clone();
        let step_names = step_names.clone();
        let stack = stack.clone();
        let settings_state = settings_state.clone();
        let context = context.clone();
        let aur = aur.clone();
        let flatpak = flatpak.clone();
        let chaotic = chaotic.clone();
        let system_apps = system_apps.clone();
        let one_click = one_click.clone();
        let reduce_prompts = reduce_prompts.clone();
        let telemetry_row = telemetry_row.clone();
        let theme_dropdown = theme_dropdown.clone();
        let previous = previous.clone();
        let next = next.clone();
        let window = window.clone();
        move |_| {
            let current = step.get();
            let steps = step_names.borrow();
            if current + 1 < steps.len() - 1 {
                step.set(current + 1);
                stack.set_visible_child_name(steps[current + 1]);
                previous.set_sensitive(true);
                return;
            }
            if current + 1 == steps.len() - 1 {
                let mut settings = settings_state.borrow_mut();
                settings.aur_enabled = aur.is_active();
                settings.flatpak_enabled = flatpak.is_active();
                settings.chaotic_enabled = chaotic.is_active();
                settings.show_system_apps = system_apps.is_active();
                settings.one_click_enabled = one_click.is_active();
                settings.reduce_password_prompts = reduce_prompts.is_active();
                settings.telemetry_enabled = telemetry_row.is_active();
                settings.theme_mode = match theme_dropdown.selected() {
                    1 => "light".to_string(),
                    2 => "dark".to_string(),
                    _ => "system".to_string(),
                };
                step.set(steps.len() - 1);
                stack.set_visible_child_name("confirm");
                next.set_label("Open the Store");
                return;
            }

            let final_settings = settings_state.borrow().clone();
            if final_settings.one_click_enabled || final_settings.reduce_password_prompts {
                let _ = ensure_session_auth(&context, Some(&window), false);
            }
            apply_theme_from_mode(&final_settings.theme_mode);
            let root_stack_for_finish = root_stack_for_next.clone();
            context.runtime.spawn({
                let settings = context.settings.clone();
                let catalog = context.catalog.clone();
                let final_settings = final_settings.clone();
                async move {
                    let was_first_time = settings.load().map(|s| !s.onboarding_completed).unwrap_or(true);
                    let _ = settings.update(|state| *state = final_settings.clone());
                    let _ = settings.set_onboarding_completed(true);
                    if final_settings.one_click_enabled {
                        let _ = catalog.install_monarch_policy().await;
                    }
                    if final_settings.flatpak_enabled {
                        let _ = catalog.prepare_flatpak().await;
                    }
                    telemetry::track_event_async(
                        &settings,
                        "onboarding_completed",
                        Some(serde_json::json!({
                            "step_count": 7,
                            "aur_enabled": final_settings.aur_enabled,
                            "flatpak_enabled": final_settings.flatpak_enabled,
                            "chaotic_enabled": final_settings.chaotic_enabled,
                            "telemetry_enabled": final_settings.telemetry_enabled,
                        })),
                    )
                    .await;
                    if was_first_time {
                        telemetry::track_event_async(&settings, "store_installed", None).await;
                    }
                }
            });
            root_stack_for_finish.set_visible_child_name("shell");
        }
    });
    previous.connect_clicked({
        let step = step.clone();
        let step_names = step_names.clone();
        let stack = stack.clone();
        let previous = previous.clone();
        let next = next.clone();
        move |_| {
            let current = step.get();
            if current == 0 {
                return;
            }
            let steps = step_names.borrow();
            let previous_index = current.saturating_sub(1);
            step.set(previous_index);
            stack.set_visible_child_name(steps[previous_index]);
            previous.set_sensitive(previous_index > 0);
            if previous_index < steps.len().saturating_sub(1) {
                next.set_label("Next");
            }
        }
    });

    repair_button.connect_clicked({
        let context = context.clone();
        let startup_warning = startup_warning.clone();
        let repair_button = repair_button.clone();
        let window = window.clone();
        move |_| {
            let _ = ensure_session_auth(&context, Some(&window), false);
            repair_button.set_sensitive(false);
            startup_warning.set_label("Repairing stale pacman lock through monarch-helper...");
            let (sender, receiver) = std::sync::mpsc::channel();
            context.runtime.spawn({
                let catalog = context.catalog.clone();
                async move {
                    let _ = sender.send(catalog.repair_unlock_pacman().await);
                }
            });
            let startup_warning_for_result = startup_warning.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                match receiver.try_recv() {
                    Ok(Ok(_)) => {
                        startup_warning_for_result.set_label("Pacman lock repaired. You can continue setup.");
                        glib::ControlFlow::Break
                    }
                    Ok(Err(error)) => {
                        startup_warning_for_result.set_label(&error);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                }
            });
        }
    });

    keyring_button.connect_clicked({
        let context = context.clone();
        let startup_warning = startup_warning.clone();
        let keyring_button = keyring_button.clone();
        let window = window.clone();
        move |_| {
            let _ = ensure_session_auth(&context, Some(&window), false);
            keyring_button.set_sensitive(false);
            startup_warning.set_label("Refreshing system keyrings through monarch-helper...");
            let (sender, receiver) = std::sync::mpsc::channel();
            context.runtime.spawn({
                let catalog = context.catalog.clone();
                async move {
                    let _ = sender.send(catalog.refresh_keyring().await);
                }
            });
            let startup_warning_for_result = startup_warning.clone();
            let keyring_button_for_result = keyring_button.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                match receiver.try_recv() {
                    Ok(Ok(message)) => {
                        startup_warning_for_result.set_label(&message);
                        keyring_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Ok(Err(error)) => {
                        startup_warning_for_result.set_label(&error);
                        keyring_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                }
            });
        }
    });

    refresh_db_button.connect_clicked({
        let context = context.clone();
        let startup_warning = startup_warning.clone();
        let refresh_db_button = refresh_db_button.clone();
        let window = window.clone();
        move |_| {
            let _ = ensure_session_auth(&context, Some(&window), false);
            refresh_db_button.set_sensitive(false);
            startup_warning.set_label("Refreshing pacman databases through monarch-helper...");
            let (sender, receiver) = std::sync::mpsc::channel();
            context.runtime.spawn({
                let catalog = context.catalog.clone();
                async move {
                    let _ = sender.send(catalog.force_refresh_databases().await);
                }
            });
            let startup_warning_for_result = startup_warning.clone();
            let refresh_db_button_for_result = refresh_db_button.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                match receiver.try_recv() {
                    Ok(Ok(message)) => {
                        startup_warning_for_result.set_label(&message);
                        refresh_db_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Ok(Err(error)) => {
                        startup_warning_for_result.set_label(&error);
                        refresh_db_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                }
            });
        }
    });

    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let context = context.clone();
        async move {
            let _ = sender.send((context.fetch_startup_status().await, context.settings.load()));
        }
    });
    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
        match receiver.try_recv() {
            Ok((Ok(status), Ok(settings))) => {
                *settings_state.borrow_mut() = settings.clone();
                aur.set_active(settings.aur_enabled);
                flatpak.set_active(settings.flatpak_enabled);
                chaotic.set_active(settings.chaotic_enabled);
                system_apps.set_active(settings.show_system_apps);
                one_click.set_active(settings.one_click_enabled);
                reduce_prompts.set_active(settings.reduce_password_prompts);
                match status.distro.chaotic_support {
                    ChaoticSupport::Blocked => {
                        chaotic.set_sensitive(false);
                        chaotic.set_active(false);
                        chaotic_copy.set_label("Chaotic-AUR is blocked on this distro. MonARCH will keep it disabled.");
                        step_names.borrow_mut().retain(|name| *name != "chaotic");
                    }
                    ChaoticSupport::Native => {
                        chaotic.set_sensitive(false);
                        chaotic.set_active(true);
                        chaotic_copy.set_label("Chaotic-AUR is native on this distro and should come from the host configuration.");
                        step_names.borrow_mut().retain(|name| *name != "chaotic");
                    }
                    ChaoticSupport::Allowed => {
                        chaotic.set_sensitive(true);
                        chaotic_copy.set_label(if status.distro.chaotic_configured {
                            "Chaotic-AUR is already configured on this host. Leave discovery enabled if you want those packages visible."
                        } else {
                            "Chaotic-AUR is allowed here. If you enable it now, use Settings -> Maintenance -> Prepare Chaotic-AUR components after onboarding if the host is not configured yet."
                        });
                    }
                }
                let startup_message = if status.warnings.is_empty() {
                    "Startup checks passed. MonARCH can prepare a safe first-use setup before opening the full store.".to_string()
                } else {
                    status.warnings.join("  ")
                };
                let missing_bins_message = if status.missing_required_bins.is_empty() {
                    "Required tools detected: git for AUR workflows, checkupdates for repo update checks, and pkexec for privileged helper actions.".to_string()
                } else {
                    format!(
                        "Missing required tools: {}",
                        status.missing_required_bins.join(", ")
                    )
                };
                let health_lines = [
                    (!status.policy_installed, "Security policy is missing and privileged auth may fail."),
                    (!status.keyring_ready, "System keyrings need initialization or refresh."),
                    (!status.sync_db_healthy, "Sync databases look corrupt; refresh them before updating."),
                ]
                .into_iter()
                .filter_map(|(show, line)| show.then_some(line))
                .collect::<Vec<_>>();
                startup_warning.set_label(&startup_message);
                missing_bins.set_label(&missing_bins_message);
                let health_summary = if health_lines.is_empty() {
                    "Startup health checks are clear. Continue to source and authorization setup.".to_string()
                } else {
                    health_lines.join("  ")
                };
                health_detail.set_label(&health_summary);
                repair_button.set_sensitive(status.stale_pacman_lock);
                keyring_button.set_sensitive(!status.keyring_ready);
                refresh_db_button.set_sensitive(!status.sync_db_healthy);
                glib::ControlFlow::Break
            }
            Ok((Err(error), _)) => {
                startup_warning.set_label(&error);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            _ => glib::ControlFlow::Break,
        }
    });

    let shell = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .css_classes(vec!["monarch-onboarding-shell".to_string()])
        .build();
    let stage = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(vec!["monarch-onboarding-stage".to_string()])
        .build();
    stage.append(&stack);
    shell.append(&hero);
    shell.append(&stage);
    shell.append(&buttons);

    let clamp = adw::Clamp::builder()
        .maximum_size(980)
        .child(&shell)
        .build();
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();
    scrolled.set_kinetic_scrolling(true);

    page.append(&scrolled);
    page.upcast()
}

use crate::context::AppContext;
use crate::ui::components::package_card::{
    bind_compact_package_card_widget, build_compact_package_card_widget, enabled_source_legend,
    source_label_to_pill_css_class,
};
use crate::ui::pages::discovery::build_skeleton_panel;
use crate::ui::pages::package_detail::build_package_detail_page;
use adw::prelude::*;
use monarch_core::models::{DistroProfile, HomeSnapshot, Package, SearchOptions};
use std::rc::Rc;

pub fn build_home_page(
    context: AppContext,
    navigation: &adw::NavigationView,
    view_stack: gtk::Stack,
) -> gtk::Widget {
    let navigation = navigation.clone();
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();
    container.set_vexpand(false);

    let status_summary = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["monarch-hero-copy".to_string()])
        .build();
    let distro_tile = build_hero_tile("Host", "Detecting distro…");
    let source_pill_row = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(false)
        .row_spacing(6)
        .column_spacing(8)
        .min_children_per_line(1)
        .max_children_per_line(20)
        .build();
    source_pill_row.add_css_class("monarch-hero-source-key");
    let sources_tile = build_hero_tile_with_content("Sources", &source_pill_row);
    let readiness_tile = build_hero_tile("Status", "Preparing store…");
    let hero = build_hero(
        &status_summary,
        &distro_tile.0,
        &sources_tile,
        &readiness_tile.0,
    );

    // Bazaar-style: FlowBox grids per section (no horizontal strips).
    let featured = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(false)
        .row_spacing(14)
        .column_spacing(14)
        .min_children_per_line(2)
        .max_children_per_line(5)
        .build();
    let essentials = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(false)
        .row_spacing(14)
        .column_spacing(14)
        .min_children_per_line(2)
        .max_children_per_line(5)
        .build();
    let trending = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(false)
        .row_spacing(14)
        .column_spacing(14)
        .min_children_per_line(2)
        .max_children_per_line(5)
        .build();
    let categories = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .row_spacing(10)
        .column_spacing(10)
        .min_children_per_line(3)
        .max_children_per_line(6)
        .build();

    let icon_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Both);
    let title_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Vertical);
    let desc_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Vertical);
    let card_root_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Both);

    // Bazaar-style content box: margin 30 start/end, 5 top, 50 bottom, spacing 15.
    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(15)
        .margin_start(30)
        .margin_end(30)
        .margin_top(5)
        .margin_bottom(50)
        .build();
    content_box.set_vexpand(false);
    content_box.set_size_request(400, -1);

    content_box.append(&build_flowbox_section(
        "Featured Picks",
        Some("Backend-fed recommendations for polished, user-facing software across the enabled sources."),
        &featured,
        true,
        &view_stack,
        "Featured Picks",
    ));
    content_box.append(&build_flowbox_section(
        "Recommended Essentials",
        Some("Curated software to get started quickly, with native Arch and distro-aware sources staying first."),
        &essentials,
        true,
        &view_stack,
        "Recommended Essentials",
    ));
    content_box.append(&build_flowbox_section(
        "Trending Applications",
        Some("Popular applications people are actively exploring right now."),
        &trending,
        true,
        &view_stack,
        "Trending Applications",
    ));
    content_box.append(&build_category_section(&categories));

    let loading = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    loading.append(&build_skeleton_panel(6));
    let error = adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title("Discover is unavailable")
        .description("The storefront snapshot could not be built.")
        .css_classes(vec!["monarch-empty".to_string()])
        .build();

    let sections = HomeSections {
        featured,
        essentials,
        trending,
        categories,
    };

    let stack = gtk::Stack::new();
    stack.set_vexpand(false);
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&content_box, Some("content"));
    stack.add_named(&error, Some("error"));
    stack.set_visible_child_name("loading");

    load_home_snapshot(
        context.clone(),
        navigation.clone(),
        sections.clone(),
        stack.clone(),
        status_summary.clone(),
        distro_tile.1.clone(),
        readiness_tile.1.clone(),
        source_pill_row.clone(),
        icon_group.clone(),
        title_group.clone(),
        desc_group.clone(),
        card_root_group.clone(),
    );

    let last_refresh_epoch = Rc::new(std::cell::Cell::new(context.refresh_epoch()));
    let last_refresh_epoch_for_timeout = last_refresh_epoch.clone();
    let context_for_timeout = context.clone();
    let navigation_for_timeout = navigation.clone();
    let sections_for_timeout = sections.clone();
    let stack_for_timeout = stack.clone();
    let status_summary_for_timeout = status_summary.clone();
    let distro_label_for_timeout = distro_tile.1.clone();
    let readiness_label_for_timeout = readiness_tile.1.clone();
    let source_pill_row_for_timeout = source_pill_row.clone();
    let icon_group_for_timeout = icon_group.clone();
    let title_group_for_timeout = title_group.clone();
    let desc_group_for_timeout = desc_group.clone();
    let card_root_group_for_timeout = card_root_group.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(500), move || {
        let current = context_for_timeout.refresh_epoch();
        if current != last_refresh_epoch_for_timeout.get() {
            last_refresh_epoch_for_timeout.set(current);
            load_home_snapshot(
                context_for_timeout.clone(),
                navigation_for_timeout.clone(),
                sections_for_timeout.clone(),
                stack_for_timeout.clone(),
                status_summary_for_timeout.clone(),
                distro_label_for_timeout.clone(),
                readiness_label_for_timeout.clone(),
                source_pill_row_for_timeout.clone(),
                icon_group_for_timeout.clone(),
                title_group_for_timeout.clone(),
                desc_group_for_timeout.clone(),
                card_root_group_for_timeout.clone(),
            );
        }
        glib::ControlFlow::Continue
    });

    container.set_vexpand(false);
    container.append(&hero);
    container.append(&stack);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&container)
        .build();
    scrolled.set_kinetic_scrolling(true);
    scrolled.set_propagate_natural_height(true);
    scrolled.set_propagate_natural_width(true);
    scrolled.upcast()
}

#[derive(Clone)]
struct HomeSections {
    featured: gtk::FlowBox,
    essentials: gtk::FlowBox,
    trending: gtk::FlowBox,
    categories: gtk::FlowBox,
}

fn build_hero(
    status_summary: &gtk::Label,
    distro_tile: &gtk::Box,
    sources_tile: &gtk::Box,
    readiness_tile: &gtk::Box,
) -> gtk::Box {
    // Bazaar-style: card/section look with padding.
    let hero = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(30)
        .margin_end(30)
        .margin_top(16)
        .margin_bottom(8)
        .css_classes(vec![
            "monarch-hero".to_string(),
            "monarch-discover-hero".to_string(),
            "card".to_string(),
            "monarch-sp-section".to_string(),
        ])
        .build();
    hero.append(
        &gtk::Label::builder()
            .label("Software for your Arch-based system")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["monarch-hero-title".to_string()])
            .build(),
    );
    hero.append(
        &gtk::Label::builder()
            .label("Discover trusted apps, compare sources, and install without touching the terminal.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["monarch-hero-copy".to_string()])
            .build(),
    );

    let status_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    status_row.append(distro_tile);
    status_row.append(sources_tile);
    status_row.append(readiness_tile);
    hero.append(&status_row);
    hero.append(status_summary);
    hero
}

/// Bazaar-style section: title (title-1) + optional description + FlowBox + optional "See more of [section]" button.
fn build_flowbox_section(
    title: &str,
    description: Option<&str>,
    flowbox: &gtk::FlowBox,
    show_more: bool,
    view_stack: &gtk::Stack,
    section_name: &str,
) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_start(3)
        .margin_end(3)
        .margin_bottom(12)
        .build();
    section.set_vexpand(false);
    let title_label = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(vec!["title-1".to_string()])
        .build();
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    section.append(&title_label);
    if let Some(desc) = description {
        section.append(
            &gtk::Label::builder()
                .label(desc)
                .xalign(0.0)
                .wrap(true)
                .css_classes(vec!["dim-label".to_string()])
                .build(),
        );
    }
    section.append(flowbox);
    if show_more {
        let see_more_label = format!("See more of {}", section_name);
        let view_stack = view_stack.clone();
        let see_more_btn = gtk::Button::builder()
            .label(&see_more_label)
            .halign(gtk::Align::Start)
            .css_classes(vec!["flat".to_string(), "dim-label".to_string(), "monarch-see-more".to_string()])
            .build();
        see_more_btn.connect_clicked(move |_| {
            view_stack.set_visible_child_name("search");
        });
        section.append(&see_more_btn);
    }
    section
}

fn build_category_section(categories: &gtk::FlowBox) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_start(3)
        .margin_end(3)
        .margin_bottom(12)
        .build();
    section.set_vexpand(false);
    section.append(
        &gtk::Label::builder()
            .label("Browse Categories")
            .xalign(0.0)
            .css_classes(vec!["title-1".to_string()])
            .build(),
    );
    section.append(
        &gtk::Label::builder()
            .label("Jump into a category without leaving the main storefront flow.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["dim-label".to_string()])
            .build(),
    );
    section.append(categories);
    section
}

fn build_hero_tile(title: &str, value: &str) -> (gtk::Box, gtk::Label) {
    let tile = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .css_classes(vec![
            "monarch-toolbar-card".to_string(),
            "monarch-home-stat".to_string(),
        ])
        .hexpand(true)
        .build();
    tile.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(vec!["monarch-meta".to_string()])
            .build(),
    );
    let value_label = gtk::Label::builder()
        .label(value)
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["title-5".to_string()])
        .build();
    tile.append(&value_label);
    (tile, value_label)
}

fn build_hero_tile_with_content(title: &str, content: &impl IsA<gtk::Widget>) -> gtk::Box {
    let tile = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .css_classes(vec![
            "monarch-toolbar-card".to_string(),
            "monarch-home-stat".to_string(),
        ])
        .hexpand(true)
        .build();
    tile.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(vec!["monarch-meta".to_string()])
            .build(),
    );
    tile.append(content.upcast_ref());
    tile
}

#[allow(clippy::too_many_arguments)] // TODO: group into HomeLoadRefs struct
fn load_home_snapshot(
    context: AppContext,
    navigation: adw::NavigationView,
    sections: HomeSections,
    stack: gtk::Stack,
    status_summary: gtk::Label,
    distro_tile: gtk::Label,
    readiness_tile: gtk::Label,
    source_pill_row: gtk::FlowBox,
    icon_group: gtk::SizeGroup,
    title_group: gtk::SizeGroup,
    desc_group: gtk::SizeGroup,
    card_root_group: gtk::SizeGroup,
) {
    stack.set_visible_child_name("loading");
    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let context = context.clone();
        async move {
            let snapshot = context.fetch_home_snapshot().await;
            let startup = context.fetch_startup_status().await;
            let settings = context.settings.load();
            let _ = sender.send((snapshot, startup, settings));
        }
    });

    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || match receiver
        .try_recv()
    {
        Ok((Ok(snapshot), Ok(startup), Ok(settings))) => {
            render_home_snapshot(
                &sections,
                &navigation,
                &context,
                snapshot,
                &icon_group,
                &title_group,
                &desc_group,
                &card_root_group,
            );
            status_summary.set_label(
                "MonARCH Store keeps host-aware repositories and helper-backed installs aligned with Arch Linux safety rules.",
            );
            distro_tile.set_label(&startup.distro.pretty_name);
            readiness_tile.set_label(if startup.stale_pacman_lock {
                "Lock needs attention"
            } else {
                "Ready"
            });
            populate_source_key_row(&source_pill_row, &context, &settings, Some(&startup.distro));
            stack.set_visible_child_name("content");
            if let Some(content) = stack.visible_child() {
                content.queue_resize();
            }
            glib::ControlFlow::Break
        }
        Ok(_) => {
            stack.set_visible_child_name("error");
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
    });
}

fn render_home_snapshot(
    sections: &HomeSections,
    navigation: &adw::NavigationView,
    context: &AppContext,
    snapshot: HomeSnapshot,
    icon_group: &gtk::SizeGroup,
    title_group: &gtk::SizeGroup,
    desc_group: &gtk::SizeGroup,
    card_root_group: &gtk::SizeGroup,
) {
    clear_flowbox(&sections.featured);
    clear_flowbox(&sections.essentials);
    clear_flowbox(&sections.trending);
    clear_flowbox(&sections.categories);

    let featured = if snapshot.featured.is_empty() {
        snapshot.popular.clone()
    } else {
        snapshot.featured.clone()
    };
    let essentials = if snapshot.popular.is_empty() {
        snapshot.featured.clone()
    } else {
        snapshot.popular.clone()
    };
    let trending = if snapshot.trending.is_empty() {
        snapshot.updated.clone()
    } else {
        snapshot.trending.clone()
    };

    for package in &featured {
        sections.featured.insert(
            &build_package_card(
                navigation,
                context.clone(),
                package,
                Some(icon_group),
                Some(title_group),
                Some(desc_group),
                Some(card_root_group),
            ),
            -1,
        );
    }
    for package in &essentials {
        sections.essentials.insert(
            &build_package_card(
                navigation,
                context.clone(),
                package,
                Some(icon_group),
                Some(title_group),
                Some(desc_group),
                Some(card_root_group),
            ),
            -1,
        );
    }
    for package in &trending {
        sections.trending.insert(
            &build_package_card(
                navigation,
                context.clone(),
                package,
                Some(icon_group),
                Some(title_group),
                Some(desc_group),
                Some(card_root_group),
            ),
            -1,
        );
    }

    const BAZAAR_GRADIENT_CLASSES: &[&str] = &[
        "monarch-category-trending",
        "monarch-category-popular",
        "monarch-category-audiovideo",
        "monarch-category-game",
        "monarch-category-network",
        "monarch-category-education",
    ];
    for (i, category_label) in snapshot.categories.iter().enumerate() {
        let gradient_class = BAZAAR_GRADIENT_CLASSES[i % BAZAAR_GRADIENT_CLASSES.len()];
        let button = gtk::Button::builder()
            .label(category_label)
            .css_classes(vec![
                "flat".to_string(),
                "monarch-category-tile".to_string(),
                gradient_class.to_string(),
            ])
            .build();
        button.connect_clicked({
            let context = context.clone();
            let navigation = navigation.clone();
            let category = category_label.clone();
            let category_label = category_label.clone();
            move |_| {
                navigation.push(&build_category_results_page(
                    context.clone(),
                    navigation.clone(),
                    &category,
                    &category_label,
                ));
            }
        });
        sections.categories.insert(&button, -1);
    }
}


fn build_package_card(
    navigation: &adw::NavigationView,
    context: AppContext,
    package: &Package,
    icon_group: Option<&gtk::SizeGroup>,
    title_group: Option<&gtk::SizeGroup>,
    desc_group: Option<&gtk::SizeGroup>,
    card_root_group: Option<&gtk::SizeGroup>,
) -> gtk::Widget {
    let card = build_compact_package_card_widget(context.clone());
    bind_compact_package_card_widget(
        &card,
        package,
        &context,
        icon_group,
        title_group,
        desc_group,
        card_root_group,
    );

    let gesture = gtk::GestureClick::new();
    gesture.connect_released({
        let context = context.clone();
        let navigation = navigation.clone();
        let package = package.clone();
        move |_, _, _, _| {
            navigation.push(&build_package_detail_page(
                context.clone(),
                &navigation,
                &package,
            ));
        }
    });
    card.add_controller(gesture);
    card.upcast()
}

fn build_category_results_page(
    context: AppContext,
    navigation: adw::NavigationView,
    category: &str,
    category_label: &str,
) -> adw::NavigationPage {
    let listbox = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .row_spacing(14)
        .column_spacing(14)
        .min_children_per_line(3)
        .max_children_per_line(3)
        .build();
    let loading = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    loading.append(&build_skeleton_panel(6));
    let stack = gtk::Stack::new();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(
        &gtk::ScrolledWindow::builder()
            .child(&listbox)
            .vexpand(true)
            .css_classes(vec!["monarch-panel".to_string()])
            .build(),
        Some("list"),
    );
    stack.add_named(
        &adw::StatusPage::builder()
            .icon_name("view-list-symbolic")
            .title(format!("No packages in {category}"))
            .description("Try another category from Discover.")
            .build(),
        Some("empty"),
    );
    stack.set_visible_child_name("loading");

    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    container.append(
        &gtk::Label::builder()
            .label(category_label)
            .xalign(0.0)
            .css_classes(vec!["monarch-hero-title".to_string()])
            .build(),
    );
    container.append(
        &gtk::Label::builder()
            .label("Packages from the backend storefront snapshot filtered by category.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["monarch-hero-copy".to_string()])
            .build(),
    );
    container.append(&stack);

    let category_name = category.to_string();
    let icon_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Both);
    let title_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Vertical);
    let desc_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Vertical);
    let card_root_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Both);
    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let catalog = context.catalog.clone();
        let settings = context.settings.clone();
        let category_name_for_default = category_name.clone();
        async move {
            let options = settings
                .load()
                .map(|state| SearchOptions {
                    flatpak_enabled: Some(state.flatpak_enabled),
                    aur_enabled: Some(state.aur_enabled),
                    chaotic_enabled: Some(state.chaotic_enabled),
                    show_system_apps: Some(state.show_system_apps),
                    source_filter: None,
                    category_filter: Some(category_name.clone()),
                    installed_only: Some(false),
                    sort_mode: Some(monarch_core::models::SearchSortMode::Name),
                    for_installed_lookup: Some(false),
                })
                .unwrap_or_else(|_| SearchOptions {
                    category_filter: Some(category_name_for_default),
                    sort_mode: Some(monarch_core::models::SearchSortMode::Name),
                    ..Default::default()
                });
            let _ = sender.send(
                catalog
                    .load_category_packages(category_name.clone(), options, 240)
                    .await,
            );
        }
    });

    let context_for_result = context.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || match receiver
        .try_recv()
    {
        Ok(Ok(packages)) if packages.is_empty() => {
            stack.set_visible_child_name("empty");
            glib::ControlFlow::Break
        }
        Ok(Ok(packages)) => {
            for package in packages {
                listbox.insert(
                    &build_package_card(
                        &navigation,
                        context_for_result.clone(),
                        &package,
                        Some(&icon_group),
                        Some(&title_group),
                        Some(&desc_group),
                        Some(&card_root_group),
                    ),
                    -1,
                );
            }
            stack.set_visible_child_name("list");
            glib::ControlFlow::Break
        }
        Ok(Err(_)) => {
            stack.set_visible_child_name("empty");
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
    });

    adw::NavigationPage::builder()
        .title(category)
        .child(&container)
        .build()
}

fn populate_source_key_row(
    flow: &gtk::FlowBox,
    _context: &AppContext,
    settings: &monarch_core::models::GtkSettings,
    host_distro: Option<&DistroProfile>,
) {
    clear_flowbox(flow);
    let host_id = host_distro.map(|d| d.id.as_str());
    let legend = enabled_source_legend(settings, host_id);
    for (label, _logo_path) in legend {
        let pill_label = gtk::Label::builder()
            .label(&label)
            .css_classes(vec!["monarch-source-pill-label".to_string()])
            .build();
        let pill = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .css_classes(vec![
                "monarch-source-pill".to_string(),
                "monarch-card-source-pill".to_string(),
                source_label_to_pill_css_class(&label).to_string(),
            ])
            .build();
        pill.append(&pill_label);
        flow.insert(&pill, -1);
    }
}

#[allow(dead_code)]
fn active_sources_copy(settings: &monarch_core::models::GtkSettings) -> String {
    let mut labels = vec!["Arch".to_string()];
    if settings.chaotic_enabled {
        labels.push("Chaotic-AUR".to_string());
    }
    if settings.flatpak_enabled {
        labels.push("Flatpak".to_string());
    }
    if settings.aur_enabled {
        labels.push("AUR".to_string());
    }
    labels.join(" • ")
}

fn clear_flowbox(flowbox: &gtk::FlowBox) {
    while let Some(child) = flowbox.first_child() {
        flowbox.remove(&child);
    }
}

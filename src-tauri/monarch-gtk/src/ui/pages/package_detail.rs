use crate::context::AppContext;
use crate::ui::auth::{ensure_session_auth, parent_window_for};
use crate::ui::components::operation_dialog::{present_operation_dialog, OperationDialogOptions};
use crate::ui::components::package_card::{
    sort_available_sources_by_priority, source_display_label_and_icon_path,
    source_label_to_pill_css_class,
};
use crate::ui::media::{arch_logo_fallback, set_picture_source};
use crate::ui::models::source_list_item::SourceListItem;
use adw::prelude::*;
use monarch_core::models::{
    GtkSettings, Package, PackagePresentation, PackageReview, PackageSource, PackageVariant,
};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

fn source_identity_key(source: &PackageSource) -> String {
    format!(
        "{}:{}:{}",
        source.source_type,
        source.id,
        source.package_name.as_deref().unwrap_or_default()
    )
}

fn source_matches_identity_key(source: &PackageSource, key: &str) -> bool {
    !key.is_empty() && source_identity_key(source) == key
}

fn build_source_list_store(sources: &[PackageSource]) -> gio::ListStore {
    let store = gio::ListStore::new::<SourceListItem>();
    update_source_list_store(&store, sources);
    store
}

fn update_source_list_store(store: &gio::ListStore, sources: &[PackageSource]) {
    store.remove_all();
    let items: Vec<SourceListItem> = sources
        .iter()
        .map(|s| {
            let (label, icon_path) = source_display_label_and_icon_path(s);
            SourceListItem::new(&icon_path, &label, &s.version)
        })
        .collect();
    store.extend_from_slice(&items);
}

/// Popup list: plain list rows (Bazaar/GTK4 default). No pill per row — just label so dropdown looks part of the app.
fn build_source_list_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, list_item| {
        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);
        row_box.set_margin_top(4);
        row_box.set_margin_bottom(4);
        let source_lbl = gtk::Label::new(None);
        source_lbl.set_xalign(0.0);
        source_lbl.add_css_class("monarch-dropdown-list-source");
        let version_lbl = gtk::Label::new(None);
        version_lbl.set_xalign(0.0);
        version_lbl.add_css_class("dim-label");
        version_lbl.add_css_class("monarch-dropdown-list-version");
        row_box.append(&source_lbl);
        row_box.append(&version_lbl);
        list_item.set_child(Some(&row_box));
    });
    factory.connect_bind(move |_, list_item| {
        let Some(row_box) = list_item.child().and_downcast::<gtk::Box>() else {
            return;
        };
        let Some(obj) = list_item.item().and_downcast::<SourceListItem>() else {
            return;
        };
        let title = obj.title();
        let subtitle = obj.subtitle();
        if let Some(source_lbl) = row_box.first_child().and_downcast::<gtk::Label>() {
            source_lbl.set_label(&title);
        }
        if let Some(version_lbl) = row_box.last_child().and_downcast::<gtk::Label>() {
            version_lbl.set_label(&subtitle);
        }
    });
    factory
}

fn build_source_row_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, list_item| {
        let pill_label = gtk::Label::new(None);
        pill_label.add_css_class("monarch-source-pill-label");
        let pill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        pill.add_css_class("monarch-source-pill");
        pill.add_css_class("monarch-card-source-pill");
        pill.append(&pill_label);
        let version_lbl = gtk::Label::new(None);
        version_lbl.add_css_class("dim-label");
        version_lbl.set_margin_start(8);
        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row_box.set_halign(gtk::Align::Start);
        row_box.append(&pill);
        row_box.append(&version_lbl);
        list_item.set_child(Some(&row_box));
    });
    factory.connect_bind(move |_, list_item| {
        let Some(row_box) = list_item.child().and_downcast::<gtk::Box>() else {
            return;
        };
        let Some(obj) = list_item.item().and_downcast::<SourceListItem>() else {
            return;
        };
        let title = obj.title();
        let subtitle = obj.subtitle();
        if let Some(pill) = row_box.first_child().and_downcast::<gtk::Box>() {
            pill.remove_css_class("is-arch");
            pill.remove_css_class("is-aur");
            pill.remove_css_class("is-chaotic");
            pill.remove_css_class("is-flatpak");
            pill.remove_css_class("is-cachyos");
            pill.remove_css_class("is-garuda");
            pill.remove_css_class("is-endeavouros");
            pill.remove_css_class("is-manjaro");
            pill.add_css_class(source_label_to_pill_css_class(&title));
            if let Some(lbl) = pill.first_child().and_downcast::<gtk::Label>() {
                lbl.set_label(&title);
            }
        }
        if let Some(version_lbl) = row_box.last_child().and_downcast::<gtk::Label>() {
            version_lbl.set_label(&subtitle);
        }
    });
    factory
}

pub fn build_package_detail_page(
    context: AppContext,
    _navigation: &adw::NavigationView,
    initial_package: &Package,
) -> adw::NavigationPage {
    let mut available_sources = initial_package
        .available_sources
        .clone()
        .filter(|sources| !sources.is_empty())
        .unwrap_or_else(|| vec![initial_package.source.clone()]);
    sort_available_sources_by_priority(&mut available_sources);
    let available_sources = Rc::new(RefCell::new(available_sources));
    let selected_source = Rc::new(RefCell::new(
        available_sources
            .borrow()
            .first()
            .cloned()
            .unwrap_or_else(|| initial_package.source.clone()),
    ));
    let current_package = Rc::new(RefCell::new(initial_package.clone()));
    let available_variants = Rc::new(RefCell::new(Vec::<PackageVariant>::new()));
    let current_presentation = Rc::new(RefCell::new(None::<PackagePresentation>));
    let current_app_rating = Rc::new(RefCell::new(None::<f64>));
    let source_list_store = Rc::new(RefCell::new(build_source_list_store(
        &available_sources.borrow(),
    )));
    let source_selection =
        gtk::SingleSelection::new(Some(source_list_store.borrow().clone()));
    source_selection.set_autoselect(true);
    let source_selection_programmatic = Rc::new(RefCell::new(false));
    let source_list_factory = build_source_list_factory();
    let source_row_factory = build_source_row_factory();
    let hero_badges = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    let hero_icon = gtk::Picture::builder()
        .width_request(112)
        .height_request(112)
        .can_shrink(true)
        .build();
    hero_icon.set_paintable(Some(crate::ui::media::placeholder_texture()));
    let title_label = gtk::Label::builder()
        .label(initial_package.effective_title())
        .xalign(0.0)
        .wrap(true)
        .lines(2)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .css_classes(vec![
            "title-1".to_string(),
            "app-title".to_string(),
            "monarch-hero-title".to_string(),
        ])
        .build();
    hero_icon.set_keep_aspect_ratio(true);
    hero_icon.add_css_class("monarch-app-icon-hero");
    let developer_label = gtk::Label::builder()
        .label(hero_identity_copy(
            initial_package,
            None,
            &initial_package.source,
        ))
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec![
            "monarch-meta".to_string(),
            "monarch-detail-developer".to_string(),
        ])
        .build();
    let verified_icon = gtk::Image::builder()
        .icon_name("emblem-ok-symbolic")
        .pixel_size(16)
        .css_classes(vec!["monarch-detail-verified".to_string()])
        .build();
    let developer_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    developer_row.append(&developer_label);
    developer_row.append(&verified_icon);

    let summary_label = gtk::Label::builder()
        .label(summary_text(initial_package))
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let (rating_score, _) = if let Some(rating) = initial_package.rating.as_ref() {
        if let Some(score) = rating.score {
            (normalize_rating(score), rating.total)
        } else {
            (0.0, 0)
        }
    } else {
        (0.0, 0)
    };

    /* Rating: always visible under Title and Maintainer (Bazaar-style: number + star). Click scrolls to Reviews. */
    let hero_rating_label = gtk::Label::builder()
        .label(if rating_score > 0.0 {
            format!("{rating_score:.1} ★")
        } else {
            "— ★".to_string()
        })
        .xalign(0.0)
        .css_classes(vec![
            "monarch-hero-rating-value".to_string(),
            "monarch-hero-rating-focus".to_string(),
        ])
        .build();
    hero_rating_label.set_visible(true);
    hero_rating_label.set_tooltip_text(Some("Go to Reviews"));

    let title_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    title_row.append(&title_label);

    /* Bazaar hero: title, developer, rating number under */
    let title_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .hexpand(true)
        .build();
    title_box.append(&title_row);
    title_box.append(&developer_row);
    title_box.append(&hero_rating_label);

    let source_drop_down = gtk::DropDown::builder()
        .model(&source_selection)
        .factory(&source_row_factory)
        .list_factory(&source_list_factory)
        .selected(0)
        .build();
    source_drop_down.add_css_class("monarch-source-dropdown");
    let _source_selector_row = adw::ActionRow::builder()
        .title("Source")
        .valign(gtk::Align::Center)
        .css_classes(vec!["monarch-source-combo-row".to_string(), "monarch-detail-source-row".to_string()])
        .build();
    /* Source dropdown lives in stats_bar as Bazaar-style context tile */

    // source_summary_label is intentionally NOT appended here —
    // the source_row in source_toolbar already shows source name + version.
    let source_summary_label = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false) // kept for refresh_source_preview compat; hidden
        .css_classes(vec!["monarch-meta".to_string()])
        .build();
    /* Bazaar-style data bar: distinct tiles, value in tile / label under */
    let stats_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .homogeneous(false)
        .hexpand(true)
        .css_classes(vec![
            "app-context-bar".to_string(),
            "monarch-bazaar-stats-bar".to_string(),
        ])
        .build();
    let hero_size = build_hero_stat(
        "Size",
        &format_optional_size(
            initial_package
                .download_size_bytes
                .or(initial_package.download_size),
        ),
    );
    let hero_version = build_hero_stat("Version", &initial_package.version);
    let hero_maintainer = build_hero_stat(
        "Maintainer",
        initial_package
            .maintainer
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Unknown"),
    );
    let hero_license = build_hero_stat(
        "License",
        &initial_package
            .license
            .as_ref()
            .filter(|value| !value.is_empty())
            .map(|value| value.join(", "))
            .unwrap_or_else(|| "Unknown".to_string()),
    );
    let initial_trust = source_trust_label_for_variant(
        initial_package
            .available_sources
            .as_ref()
            .and_then(|s| s.first())
            .unwrap_or(&initial_package.source),
        None,
    );
    let hero_source_trust = build_hero_stat("Source", &initial_trust);
    stats_bar.append(&hero_size);
    stats_bar.append(&hero_version);
    stats_bar.append(&hero_maintainer);
    stats_bar.append(&hero_license);
    stats_bar.append(&hero_source_trust);

    /* Bazaar-style data bar has no Source; source selector lives in hero with Install/Launch/Uninstall (distro-aware). */
    source_drop_down.add_css_class("monarch-hero-source-dropdown");

    /* Bazaar hero actions: Support, Favorite, Source (dropdown), Install/Launch/Uninstall */
    let support_button = gtk::Button::builder()
        .label("Support")
        .icon_name("favorite-symbolic")
        .css_classes(vec!["monarch-detail-support".to_string()])
        .halign(gtk::Align::Start)
        .build();
    let support_url = initial_package.url.clone();
    support_button.set_visible(support_url.as_ref().map_or(false, |u| !u.is_empty()));
    support_button.connect_clicked({
        let support_url = support_url.clone();
        move |_| {
            if let Some(ref u) = support_url {
                let _ = gio::AppInfo::launch_default_for_uri(u, None::<&gio::AppLaunchContext>);
            }
        }
    });

    let install_button = gtk::Button::builder()
        .label("Install")
        .css_classes(vec![
            "suggested-action".to_string(),
            "monarch-card-action".to_string(),
            "monarch-detail-install-pill".to_string(),
        ])
        .halign(gtk::Align::Start)
        .build();
    let launch_button = gtk::Button::builder()
        .label("Launch")
        .css_classes(vec![
            "suggested-action".to_string(),
            "monarch-launch-button".to_string(),
            "monarch-detail-install-pill".to_string(),
        ])
        .halign(gtk::Align::Start)
        .visible(initial_package.installed)
        .build();
    let uninstall_button = gtk::Button::builder()
        .label("Uninstall")
        .css_classes(vec![
            "destructive-action".to_string(),
            "monarch-card-action".to_string(),
            "monarch-detail-install-pill".to_string(),
        ])
        .halign(gtk::Align::Start)
        .visible(initial_package.installed)
        .build();
    let favorite_button = gtk::Button::builder()
        .icon_name("non-starred-symbolic")
        .has_frame(false)
        .halign(gtk::Align::Start)
        .css_classes(vec![
            "monarch-save-button".to_string(),
            "monarch-favorite-button".to_string(),
            "monarch-detail-favorite-pill".to_string(),
        ])
        .build();
    let action_status = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let source_toolbar = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .hexpand(true)
        .build();
    let hero_action_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .halign(gtk::Align::Start)
        .hexpand(true)
        .css_classes(vec!["monarch-hero-actions".to_string()])
        .build();
    hero_action_row.append(&support_button);
    hero_action_row.append(&favorite_button);
    hero_action_row.append(&source_drop_down);
    hero_action_row.append(&install_button);
    hero_action_row.append(&launch_button);
    hero_action_row.append(&uninstall_button);

    // Match Tauri PackageDetailsFresh: when installed show read-only "Installed from X" box;
    // when not installed show source dropdown + optional hint.
    let installed_title_label = gtk::Label::builder()
        .label("Installed Package")
        .xalign(0.0)
        .css_classes(vec!["title-4".to_string()])
        .build();
    let installed_notice_label = gtk::Label::builder()
        .label("Installed apps stay on their current source. Compare other sources below, then uninstall first if you want to switch.")
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let installed_notice_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .hexpand(true)
        .css_classes(vec!["monarch-installed-source-notice".to_string()])
        .build();
    installed_notice_box.append(&installed_title_label);
    installed_notice_box.append(&installed_notice_label);
    installed_notice_box.set_visible(initial_package.installed);

    let source_hint_label = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let source_selector_wrapper = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .hexpand(true)
        .build();
    source_selector_wrapper.append(&source_hint_label);
    source_selector_wrapper.set_visible(!initial_package.installed);

    // Uninstalled app: recommendation status, repo info, and source-specific warnings (e.g. Chaotic-AUR).
    let uninstalled_info_label = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .css_classes(vec!["body".to_string(), "dim-label".to_string()])
        .build();
    let uninstalled_info_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(12)
        .build();
    let uninstalled_info_icon = gtk::Image::builder()
        .icon_name("dialog-information-symbolic")
        .pixel_size(20)
        .build();
    uninstalled_info_box.append(&uninstalled_info_icon);
    uninstalled_info_box.append(&uninstalled_info_label);
    uninstalled_info_box.add_css_class("monarch-source-switch-notice");
    uninstalled_info_box.set_visible(!initial_package.installed);

    let source_slot = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .hexpand(true)
        .build();
    source_slot.append(&installed_notice_box);
    source_slot.append(&source_selector_wrapper);
    source_slot.append(&uninstalled_info_box);

    let source_and_actions_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(18)
        .hexpand(true)
        .css_classes(vec!["monarch-detail-source-actions-row".to_string()])
        .build();
    source_and_actions_row.append(&source_slot);
    // hero_action_row is appended only to header_top (Bazaar hero row). Appending it here too would
    // trigger gtk_box_append assertion (widget can have only one parent).

    // Tauri-style info box: when installed, show backend source_switch_notice (uninstall-first, Flatpak vs repo).
    let source_switch_notice_label = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .css_classes(vec!["body".to_string(), "dim-label".to_string()])
        .build();
    let source_switch_notice_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(12)
        .build();
    let info_icon = gtk::Image::builder()
        .icon_name("dialog-information-symbolic")
        .pixel_size(20)
        .build();
    source_switch_notice_box.append(&info_icon);
    source_switch_notice_box.append(&source_switch_notice_label);
    source_switch_notice_box.add_css_class("monarch-source-switch-notice");
    source_switch_notice_box.set_visible(initial_package.installed);

    // When installed and multiple sources: list each as "Installed source" or "Available after uninstall".
    let installed_variants_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .build();
    let installed_variants_section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(12)
        .build();
    let section_title = gtk::Label::builder()
        .label("Sources")
        .xalign(0.0)
        .css_classes(vec!["title-4".to_string()])
        .build();
    installed_variants_section.append(&section_title);
    installed_variants_section.append(&installed_variants_list);
    installed_variants_section.set_visible(initial_package.installed && initial_package.available_sources.as_ref().map(|s| s.len() > 1).unwrap_or(false));

    let source_switch_notice_ref: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    source_toolbar.append(&source_and_actions_row);
    source_toolbar.append(&source_switch_notice_box);
    source_toolbar.append(&installed_variants_section);
    source_toolbar.append(&action_status);

    // Clone for connect_selected_notify so originals remain for install/uninstall closures.
    let installed_notice_box_for_selector = installed_notice_box.clone();
    let installed_title_label_for_selector = installed_title_label.clone();
    let installed_notice_label_for_selector = installed_notice_label.clone();
    let source_selector_wrapper_for_selector = source_selector_wrapper.clone();
    let source_hint_label_for_selector = source_hint_label.clone();
    let uninstalled_info_box_for_selector = uninstalled_info_box.clone();
    let uninstalled_info_label_for_selector = uninstalled_info_label.clone();

    // Don't append source_summary_label — source_row shows source info instead

    let source_group = adw::PreferencesGroup::builder()
        .title("Technical Details")
        .build();
    let version_row = build_action_row("Version", &initial_package.version);
    let name_row = build_action_row("Package Name", &initial_package.name);
    let installed_row = build_action_row(
        "Installed",
        if initial_package.installed {
            "Yes"
        } else {
            "No"
        },
    );
    let maintainer_row = build_action_row(
        "Maintainer",
        initial_package
            .maintainer
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Not published"),
    );
    let license_row = build_action_row(
        "License",
        &initial_package
            .license
            .as_ref()
            .filter(|value| !value.is_empty())
            .map(|value| value.join(", "))
            .unwrap_or_else(|| "Unknown".to_string()),
    );
    let download_size_row = build_action_row(
        "Download Size",
        &format_optional_size(
            initial_package
                .download_size_bytes
                .or(initial_package.download_size),
        ),
    );
    let installed_size_row = build_action_row(
        "Installed Size",
        &format_optional_size(
            initial_package
                .installed_size_bytes
                .or(initial_package.installed_size),
        ),
    );
    source_group.add(&version_row);
    source_group.add(&name_row);
    // Only add installed row, maintainer, license, size rows with actual values
    if initial_package.installed {
        source_group.add(&installed_row);
    }
    if initial_package
        .maintainer
        .as_deref()
        .is_some_and(|v| !v.trim().is_empty())
    {
        source_group.add(&maintainer_row);
    }
    if initial_package
        .license
        .as_ref()
        .is_some_and(|v| !v.is_empty())
    {
        source_group.add(&license_row);
    }
    source_group.add(&download_size_row);
    source_group.add(&installed_size_row);

    let alternatives_group = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    alternatives_group.append(
        &gtk::Label::builder()
            .label("Available Sources")
            .xalign(0.0)
            .css_classes(vec!["title-4".to_string()])
            .build(),
    );
    // Detail view sub-components setup
    let short_description = gtk::Label::builder()
        .label(&initial_package.description)
        .wrap(true)
        .xalign(0.0)
        .justify(gtk::Justification::Left)
        .css_classes(vec!["body".to_string()])
        .build();
    let description = gtk::Label::builder()
        .label(detail_text(initial_package))
        .wrap(true)
        .xalign(0.0)
        .justify(gtk::Justification::Left)
        .selectable(true)
        .css_classes(vec!["body".to_string()])
        .build();

    let body_group = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    body_group.append(
        &gtk::Label::builder()
            .label("Overview")
            .xalign(0.0)
            .css_classes(vec!["title-4".to_string()])
            .build(),
    );
    body_group.append(
        &gtk::Label::builder()
            .label("What this app does and why you might want it.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["dim-label".to_string()])
            .build(),
    );
    body_group.append(&short_description);
    body_group.append(&description);

    let screenshots_group = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    screenshots_group.append(
        &gtk::Label::builder()
            .label("Screenshots")
            .xalign(0.0)
            .css_classes(vec!["title-4".to_string()])
            .build(),
    );
    screenshots_group.append(
        &gtk::Label::builder()
            .label("Preview the app before installing. MonARCH uses the first-party screenshots bundled with the merged metadata payload whenever they are available.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["dim-label".to_string()])
            .build(),
    );
    screenshots_group.set_visible(false); // hidden until screenshots actually load
    let screenshots_carousel = adw::Carousel::new();
    screenshots_carousel.set_allow_mouse_drag(true);
    screenshots_carousel.set_allow_scroll_wheel(true);
    screenshots_carousel.set_hexpand(true);
    screenshots_carousel.set_vexpand(false);
    let screenshots_indicator = adw::CarouselIndicatorDots::new();
    screenshots_indicator.set_carousel(Some(&screenshots_carousel));
    screenshots_group.append(&screenshots_carousel);
    screenshots_group.append(&screenshots_indicator);

    let links_group = adw::PreferencesGroup::builder().title("Links").build();
    let homepage_row = build_action_row(
        "Project Page",
        initial_package
            .url
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(""),
    );
    homepage_row.set_visible(
        initial_package
            .url
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty()),
    );
    let app_id_row = build_action_row(
        "App ID",
        initial_package
            .app_id
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(""),
    );
    app_id_row.set_visible(
        initial_package
            .app_id
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty()),
    );
    let launch_target_row = build_action_row(
        "Launch Target",
        initial_package
            .launch_target
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(""),
    );
    launch_target_row.set_visible(
        initial_package
            .launch_target
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty()),
    );
    let open_homepage_button = gtk::Button::builder()
        .label("Open Project Website")
        .halign(gtk::Align::Start)
        .css_classes(vec!["monarch-copy-action".to_string()])
        .sensitive(
            initial_package
                .url
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        )
        .visible(
            initial_package
                .url
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        )
        .build();
    links_group.add(&homepage_row);
    links_group.add(&app_id_row);
    links_group.add(&launch_target_row);
    links_group.add(&open_homepage_button);

    let reviews_summary = gtk::Label::builder()
        .label("Loading review summary…")
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let reviews_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(vec![
            "boxed-list".to_string(),
            "monarch-soft-list".to_string(),
        ])
        .build();

    let security_title = gtk::Label::builder()
        .label("Security")
        .xalign(0.0)
        .css_classes(vec!["title-4".to_string()])
        .build();
    let security_notice = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-security-card".to_string()])
        .build();
    let security_label = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["title-5".to_string()])
        .build();
    let security_copy = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["body".to_string()])
        .build();
    security_notice.append(&security_label);
    security_notice.append(&security_copy);
    let security_panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    security_panel.append(&security_title);
    security_panel.append(&security_notice);

    let copy_package_button = gtk::Button::builder()
        .label("Copy package identifier")
        .halign(gtk::Align::Fill)
        .css_classes(vec!["monarch-copy-action".to_string()])
        .build();
    let copy_source_button = gtk::Button::builder()
        .label("Copy source tuple")
        .halign(gtk::Align::Fill)
        .css_classes(vec!["monarch-copy-action".to_string()])
        .build();
    let copy_install_button = gtk::Button::builder()
        .label("Copy install command")
        .halign(gtk::Align::Fill)
        .css_classes(vec!["monarch-copy-action".to_string()])
        .build();

    let loading = adw::StatusPage::builder()
        .icon_name("package-x-generic-symbolic")
        .title("Refreshing package details")
        .description("Loading richer metadata from the shared catalog.")
        .build();

    // Bazaar-style main_box: spacing 20, margin-bottom 15.
    let detail_content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(20)
        .margin_bottom(15)
        .build();
    detail_content.set_vexpand(false);
    let icon_wrap = gtk::Box::builder()
        .valign(gtk::Align::Start)
        .width_request(92)
        .height_request(92)
        .css_classes(vec!["monarch-detail-icon-wrap".to_string()])
        .build();
    icon_wrap.append(&hero_icon);
    /* Bazaar-style hero: no heavy panel, transparent header */
    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(20)
        .margin_start(25)
        .margin_end(25)
        .margin_top(15)
        .css_classes(vec!["monarch-detail-header".to_string(), "monarch-bazaar-hero".to_string()])
        .build();
    let header_main = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(20)
        .hexpand(true)
        .build();
    let header_content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(3)
        .hexpand(true)
        .build();
    /* Bazaar hero row: [Icon] [Title + Developer + Rating number] [Support? Favorite Source Install] */
    let header_top = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(24)
        .css_classes(vec!["monarch-bazaar-hero-row".to_string()])
        .build();
    header_top.append(&icon_wrap);
    header_content.append(&title_box);
    header_top.append(&header_content);
    hero_action_row.set_halign(gtk::Align::End);
    hero_action_row.set_valign(gtk::Align::Start);
    header_top.append(&hero_action_row);
    header_main.append(&header_top);
    header_main.append(&stats_bar);
    header_main.append(&source_toolbar);
    header_box.append(&header_main);
    let reviews_panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    let reviews_title_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    reviews_title_row.append(
        &gtk::Label::builder()
            .label("Reviews")
            .xalign(0.0)
            .css_classes(vec!["title-4".to_string()])
            .hexpand(true)
            .build(),
    );
    let write_review_button = gtk::Button::builder()
        .label("Write a review")
        .css_classes(vec!["pill".to_string(), "suggested-action".to_string()])
        .build();
    reviews_title_row.append(&write_review_button);
    reviews_panel.append(&reviews_title_row);
    reviews_panel.append(&reviews_summary);
    reviews_panel.append(&reviews_list);

    let detail_sections = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(15)
        .margin_start(25)
        .margin_end(25)
        .margin_bottom(20)
        .margin_top(10)
        .build();

    detail_sections.append(&body_group);
    detail_sections.append(&reviews_panel);
    detail_sections.append(&alternatives_group);
    detail_sections.append(&security_panel);
    detail_sections.append(&links_group);
    detail_sections.append(&source_group);

    // Bazaar-style: top Clamp (max 910) for header + context_bar + install; then screenshots; then bottom Clamp for description etc.
    let top_clamp = adw::Clamp::builder()
        .maximum_size(910)
        .tightening_threshold(576)
        .child(&header_box)
        .build();
    top_clamp.set_vexpand(false);

    let bottom_clamp = adw::Clamp::builder()
        .maximum_size(910)
        .tightening_threshold(576)
        .child(&detail_sections)
        .build();
    bottom_clamp.set_vexpand(false);

    detail_content.append(&top_clamp);
    detail_content.append(&screenshots_group);
    detail_content.append(&bottom_clamp);

    /* Backdrop kept for reload/apply_package_icon API but not shown (Bazaar 1:1: no big logo behind hero). */
    let detail_backdrop = gtk::Picture::new();
    detail_backdrop.set_paintable(Some(crate::ui::media::placeholder_texture()));
    detail_backdrop.set_width_request(320);
    detail_backdrop.set_height_request(200);
    detail_backdrop.set_halign(gtk::Align::Center);
    detail_backdrop.set_valign(gtk::Align::Start);
    detail_backdrop.set_can_target(false);
    detail_backdrop.set_can_shrink(true);
    detail_backdrop.set_keep_aspect_ratio(true);
    detail_backdrop.add_css_class("monarch-detail-backdrop");
    set_picture_source(
        &detail_backdrop,
        context.runtime.clone(),
        Some(arch_logo_fallback()),
        None,
    );

    // Bazaar-style: no big logo behind hero; content only.
    let detail_overlay = gtk::Overlay::new();
    detail_overlay.set_vexpand(false);
    detail_overlay.set_child(Some(&detail_content));

    let detail_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&detail_overlay)
        .build();
    detail_scrolled.set_kinetic_scrolling(true);

    // Click on hero rating scrolls to Reviews section
    {
        let scrolled = detail_scrolled.clone();
        let target = reviews_panel.clone();
        let content = detail_overlay.clone();
        let gesture = gtk::GestureClick::new();
        gesture.connect_pressed(move |_, _n_press, _x, _y| {
            let adj = scrolled.vadjustment();
            if let Some(matrix) = target.compute_transform(&content) {
                let pt = gtk::graphene::Point::new(0.0f32, 0.0f32);
                let out = matrix.transform_point(&pt);
                let y = f64::from(out.y());
                let value = (y - 24.0).max(0.0).min(adj.upper() - adj.page_size());
                adj.set_value(value);
            }
        });
        hero_rating_label.add_controller(gesture);
    }

    let stack = gtk::Stack::new();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&detail_scrolled, Some("detail"));
    stack.set_visible_child_name("detail");

    let page = adw::NavigationPage::builder()
        .title(initial_package.effective_title())
        .child(&stack)
        .build();
    wire_copy_action(
        &copy_package_button,
        "Copy package identifier",
        "Package identifier copied",
    );
    wire_copy_action(
        &copy_source_button,
        "Copy source tuple",
        "Source tuple copied",
    );
    wire_copy_action(
        &copy_install_button,
        "Copy install command",
        "Install command copied",
    );
    set_copy_action_value(&copy_package_button, &initial_package.canonical_id);
    set_copy_action_value(
        &copy_source_button,
        &format!(
            "{}:{}",
            initial_package.source.source_type, initial_package.source.id
        ),
    );
    set_copy_action_value(
        &copy_install_button,
        &suggested_install_command_for_source(initial_package, &initial_package.source),
    );

    let canonical_id = initial_package.canonical_id.clone();
    let installed_state = Rc::new(std::cell::Cell::new(initial_package.installed));
    let installed_source_id = Rc::new(RefCell::new(source_identity_key(&initial_package.source)));
    let current_review_target = Rc::new(RefCell::new(review_target_id(initial_package)));
    {
        let current_review_target_btn = current_review_target.clone();
        let current_app_rating_btn = current_app_rating.clone();
        let reviews_panel_btn = reviews_panel.clone();
        let context_btn = context.clone();
        let reviews_list_btn = reviews_list.clone();
        let reviews_summary_btn = reviews_summary.clone();
        let hero_rating_label_btn = hero_rating_label.clone();
        write_review_button.connect_clicked(move |_| {
            let app_id = current_review_target_btn.borrow().clone();
            present_write_review_dialog(
                parent_window_for(&reviews_panel_btn).as_ref(),
                app_id,
                &context_btn,
                &reviews_list_btn,
                &reviews_summary_btn,
                &hero_rating_label_btn,
                current_app_rating_btn.as_ref(),
            );
        });
    }
    let _hero_icon_for_source_reload = hero_icon.clone();
    let _detail_backdrop_for_reload = detail_backdrop.clone();
    let favorite_state = Rc::new(std::cell::Cell::new(
        context
            .favorites
            .contains(&initial_package.canonical_id)
            .unwrap_or(false),
    ));
    update_favorite_button(&favorite_button, favorite_state.get());
    let installed_row_for_action = installed_row.clone();
    let source_list_store_for_selector = source_list_store.clone();
    let installed_state_for_selector = installed_state.clone();
    let _source_drop_down_for_reload = source_drop_down.clone();
    let source_selection_programmatic_for_notify = source_selection_programmatic.clone();
    let _page_for_reload = page.clone();
    let title_label_for_reload = title_label.clone();
    let title_label_for_initial = title_label_for_reload.clone();
    let _hero_badges_for_reload = hero_badges.clone();
    let source_switch_notice_ref_for_load = source_switch_notice_ref.clone();
    let source_switch_notice_box_for_load = source_switch_notice_box.clone();
    let source_switch_notice_label_for_load = source_switch_notice_label.clone();
    let installed_variants_section_for_load = installed_variants_section.clone();
    let installed_variants_list_for_load = installed_variants_list.clone();
    let uninstalled_info_box_for_load = uninstalled_info_box.clone();
    let uninstalled_info_label_for_load = uninstalled_info_label.clone();
    let source_switch_notice_ref_for_initial = source_switch_notice_ref_for_load.clone();
    let source_switch_notice_box_for_initial = source_switch_notice_box_for_load.clone();
    let source_switch_notice_label_for_initial = source_switch_notice_label_for_load.clone();
    let installed_variants_section_for_initial = installed_variants_section_for_load.clone();
    let installed_variants_list_for_initial = installed_variants_list_for_load.clone();
    let uninstalled_info_box_for_initial = uninstalled_info_box_for_load.clone();
    let uninstalled_info_label_for_initial = uninstalled_info_label_for_load.clone();
    let source_switch_notice_ref_for_load_detail = source_switch_notice_ref_for_load.clone();
    let source_switch_notice_box_for_load_detail = source_switch_notice_box_for_load.clone();
    let source_switch_notice_label_for_load_detail = source_switch_notice_label_for_load.clone();
    let installed_variants_section_for_load_detail = installed_variants_section_for_load.clone();
    let installed_variants_list_for_load_detail = installed_variants_list_for_load.clone();
    let uninstalled_info_box_for_load_detail = uninstalled_info_box_for_load.clone();
    let uninstalled_info_label_for_load_detail = uninstalled_info_label_for_load.clone();
    let _hero_rating_label_for_source_result = hero_rating_label.clone();
    let current_app_rating_for_connect = current_app_rating.clone();
    source_drop_down.connect_selected_notify({
        let context = context.clone();
        let source_selection_programmatic = source_selection_programmatic_for_notify.clone();
        let selected_source = selected_source.clone();
        let available_sources = available_sources.clone();
        let _source_list_store_for_selector = source_list_store_for_selector.clone();
        let installed_state_for_selector = installed_state_for_selector.clone();
        let action_status = action_status.clone();
        let installed_source_id = installed_source_id.clone();
        let current_package = current_package.clone();
        let available_variants = available_variants.clone();
        let current_presentation = current_presentation.clone();
        let developer_label = developer_label.clone();
        let summary_label = summary_label.clone();
        let source_summary_label = source_summary_label.clone();
        let hero_version = hero_version.clone();
        let hero_size = hero_size.clone();
        let hero_maintainer = hero_maintainer.clone();
        let hero_license = hero_license.clone();
        let hero_source_trust = hero_source_trust.clone();
        let version_row = version_row.clone();
        let maintainer_row = maintainer_row.clone();
        let license_row = license_row.clone();
        let download_size_row = download_size_row.clone();
        let installed_size_row = installed_size_row.clone();
        let short_description = short_description.clone();
        let description = description.clone();
        let screenshots_carousel = screenshots_carousel.clone();
        let screenshots_group = screenshots_group.clone();
        let security_label = security_label.clone();
        let security_copy = security_copy.clone();
        let copy_source_button = copy_source_button.clone();
        let copy_install_button = copy_install_button.clone();
        let _canonical_id = canonical_id.clone();
        let install_button = install_button.clone();
        let launch_button = launch_button.clone();
        let uninstall_button = uninstall_button.clone();
        let installed_notice_box_for_immediate = installed_notice_box_for_selector.clone();
        let installed_title_label_for_immediate = installed_title_label_for_selector.clone();
        let installed_notice_label_for_immediate = installed_notice_label_for_selector.clone();
        let source_selector_wrapper_for_immediate = source_selector_wrapper_for_selector.clone();
        let source_hint_label_for_immediate = source_hint_label_for_selector.clone();
        let source_switch_notice_box_for_immediate = source_switch_notice_box_for_load.clone();
        let source_switch_notice_label_for_immediate = source_switch_notice_label_for_load.clone();
        let source_switch_notice_ref_for_immediate = source_switch_notice_ref_for_load.clone();
        let installed_variants_section_for_immediate = installed_variants_section_for_load.clone();
        let installed_variants_list_for_immediate = installed_variants_list_for_load.clone();
        let uninstalled_info_box_for_immediate = uninstalled_info_box_for_selector.clone();
        let uninstalled_info_label_for_immediate = uninstalled_info_label_for_selector.clone();
        let hero_source_trust_for_immediate = hero_source_trust.clone();
        let hero_rating_label_for_selector = hero_rating_label.clone();
        let current_app_rating_for_selector = current_app_rating_for_connect.clone();
        move |dropdown| {
            if *source_selection_programmatic.borrow() {
                source_selection_programmatic.replace(false);
                return;
            }
            let sources = available_sources.borrow();
            let len = sources.len();
            drop(sources);
            let selected = dropdown.selected() as usize;
            if selected >= len {
                return;
            }
            let source = match available_sources.borrow().get(selected).cloned() {
                Some(s) => s,
                None => return,
            };
            if same_source_identity(&selected_source.borrow(), &source) {
                return;
            }
            selected_source.replace(source.clone());
            // Update data bar, source info box, tech details, and screenshots immediately from already-loaded variants.
            let package = current_package.borrow().clone();
            sync_selected_source_ui(
                &context,
                &package,
                &available_variants.borrow(),
                current_presentation.borrow().as_ref(),
                &source,
                installed_state_for_selector.get(),
                &installed_source_id.borrow(),
                &installed_notice_box_for_immediate,
                &installed_title_label_for_immediate,
                &installed_notice_label_for_immediate,
                &source_selector_wrapper_for_immediate,
                &source_hint_label_for_immediate,
                &developer_label,
                &summary_label,
                &source_summary_label,
                &hero_version,
                &hero_size,
                &hero_maintainer,
                &hero_license,
                &hero_source_trust_for_immediate,
                &version_row,
                &maintainer_row,
                &license_row,
                &download_size_row,
                &installed_size_row,
                &short_description,
                &description,
                &screenshots_carousel,
                &screenshots_group,
                &security_label,
                &security_copy,
                &copy_source_button,
                &copy_install_button,
                &install_button,
                &launch_button,
                &uninstall_button,
                &action_status,
                &source_switch_notice_box_for_immediate,
                &source_switch_notice_label_for_immediate,
                &*source_switch_notice_ref_for_immediate,
                &installed_variants_section_for_immediate,
                &installed_variants_list_for_immediate,
                &uninstalled_info_box_for_immediate,
                &uninstalled_info_label_for_immediate,
                &hero_rating_label_for_selector,
                current_app_rating_for_selector.borrow().clone(),
            );
            update_action_button_for_source(&context, &install_button, &action_status, installed_state_for_selector.get(), &source);
        }
    });
    sync_link_action(&open_homepage_button, initial_package.url.as_deref());
    open_homepage_button.connect_clicked(|button| {
        let Some(uri) = button.tooltip_text() else {
            return;
        };
        let _ = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>);
    });
    update_action_button_for_source(
        &context,
        &install_button,
        &action_status,
        initial_package.installed,
        &selected_source.borrow(),
    );
    sync_hero_action_buttons(
        &install_button,
        &launch_button,
        &uninstall_button,
        initial_package.installed,
    );
    favorite_button.connect_clicked({
        let context = context.clone();
        let favorite_button = favorite_button.clone();
        let favorite_state = favorite_state.clone();
        move |_| {
            let canonical_id = favorite_button.widget_name().to_string();
            if canonical_id.trim().is_empty() {
                return;
            }

            let next_state = !favorite_state.get();
            favorite_state.set(next_state);
            update_favorite_button(&favorite_button, next_state);

            let (sender, receiver) = std::sync::mpsc::channel();
            context.runtime.spawn({
                let favorites = context.favorites.clone();
                let canonical_id = canonical_id.clone();
                async move {
                    let _ = sender.send(favorites.toggle(&canonical_id));
                }
            });

            let context_for_result = context.clone();
            let favorite_button_for_result = favorite_button.clone();
            let favorite_state_for_result = favorite_state.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                match receiver.try_recv() {
                    Ok(Ok(_)) => {
                        context_for_result.mark_catalog_dirty();
                        glib::ControlFlow::Break
                    }
                    Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        let reverted = !favorite_state_for_result.get();
                        favorite_state_for_result.set(reverted);
                        update_favorite_button(&favorite_button_for_result, reverted);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                }
            });
        }
    });
    launch_button.connect_clicked({
        let action_status = action_status.clone();
        let selected_source = selected_source.clone();
        let current_package = current_package.clone();
        move |_| match launch_selected_package(&current_package.borrow(), &selected_source.borrow())
        {
            Ok(()) => action_status.set_label("App launched."),
            Err(error) => action_status.set_label(&error),
        }
    });
    install_button.connect_clicked({
        let context = context.clone();
        let install_button = install_button.clone();
        let launch_button = launch_button.clone();
        let uninstall_button = uninstall_button.clone();
        let action_status = action_status.clone();
        let installed_state = installed_state.clone();
        let installed_row = installed_row_for_action.clone();
        let selected_source = selected_source.clone();
        let installed_source_id = installed_source_id.clone();
        let current_package = current_package.clone();
        let installed_notice_box = installed_notice_box.clone();
        let installed_title_label = installed_title_label.clone();
        let installed_notice_label = installed_notice_label.clone();
        let source_selector_wrapper = source_selector_wrapper.clone();
        let source_hint_label = source_hint_label.clone();
        move |_| {
            let chosen_source = selected_source.borrow().clone();
            let package = current_package.borrow().clone();
            if let Err(error) =
                ensure_session_auth(&context, parent_window_for(&install_button).as_ref(), false)
            {
                action_status.set_label(&error);
                return;
            }
            install_button.set_sensitive(false);
            action_status.set_label(&pending_action_copy(false, &chosen_source));

            let (sender, receiver) = std::sync::mpsc::channel();
            let package_title = package.effective_title();
            context.runtime.spawn({
                let catalog = context.catalog.clone();
                let package = package.clone();
                let chosen_source = chosen_source.clone();
                async move {
                    let result = catalog
                        .install_package_for_source_stream(package, chosen_source)
                        .await;
                    let _ = sender.send(result);
                }
            });

            let install_button_for_result = install_button.clone();
            let action_status_for_result = action_status.clone();
            let installed_state_for_result = installed_state.clone();
            let context_for_dialog = context.clone();
            let installed_row_for_timeout = installed_row.clone();
            let selected_source_for_timeout = selected_source.clone();
            let installed_source_id_for_timeout = installed_source_id.clone();
            let launch_button_for_timeout = launch_button.clone();
            let uninstall_button_for_timeout = uninstall_button.clone();
            let installed_notice_box_for_timeout = installed_notice_box.clone();
            let installed_title_label_for_timeout = installed_title_label.clone();
            let installed_notice_label_for_timeout = installed_notice_label.clone();
            let source_selector_wrapper_for_timeout = source_selector_wrapper.clone();
            let source_hint_label_for_timeout = source_hint_label.clone();
            let package_for_launch = package.clone();
            let source_for_launch = chosen_source.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                match receiver.try_recv() {
                    Ok(Ok(stream)) => {
                        let dialog_title = format!("Installing {}", package_title);
                        let initial_status = "Installing package through monarch-helper...";
                        let pkg = package_for_launch.clone();
                        let src = source_for_launch.clone();
                        let pkg_name_for_telemetry = package_for_launch.name.clone();
                        let source_label_for_telemetry = source_for_launch.label.clone();
                        let options = OperationDialogOptions {
                            is_uninstall: false,
                            success_display_name: Some(package_title.clone()),
                            on_launch: Some(Box::new(move || {
                                let _ = launch_selected_package(&pkg, &src);
                            })),
                        };
                        present_operation_dialog(
                            context_for_dialog.clone(),
                            &dialog_title,
                            initial_status,
                            stream,
                            {
                                let install_button_for_result = install_button_for_result.clone();
                                let action_status_for_result = action_status_for_result.clone();
                                let installed_state_for_result = installed_state_for_result.clone();
                                let installed_row_for_result = installed_row_for_timeout.clone();
                                let context_for_finish = context_for_dialog.clone();
                                let selected_source_for_result =
                                    selected_source_for_timeout.clone();
                                let installed_source_id_for_result =
                                    installed_source_id_for_timeout.clone();
                                let launch_button_for_result = launch_button_for_timeout.clone();
                                let uninstall_button_for_result =
                                    uninstall_button_for_timeout.clone();
                                let installed_notice_box_for_result =
                                    installed_notice_box_for_timeout.clone();
                                let installed_title_label_for_result =
                                    installed_title_label_for_timeout.clone();
                                let installed_notice_label_for_result =
                                    installed_notice_label_for_timeout.clone();
                                let source_selector_wrapper_for_result =
                                    source_selector_wrapper_for_timeout.clone();
                                let source_hint_label_for_result =
                                    source_hint_label_for_timeout.clone();
                                let settings_for_install_telemetry =
                                    context_for_dialog.settings.clone();
                                let runtime_for_install_telemetry =
                                    context_for_dialog.runtime.clone();
                                move |result| {
                                    match result {
                                        Ok(()) => {
                                            let next_installed = true;
                                            let source = selected_source_for_result.borrow().clone();
                                            installed_state_for_result.set(next_installed);
                                            installed_source_id_for_result.replace(
                                                source_identity_key(&source),
                                            );
                                            installed_row_for_result.set_subtitle(
                                                if next_installed { "Yes" } else { "No" },
                                            );
                                            installed_notice_box_for_result.set_visible(true);
                                            source_selector_wrapper_for_result.set_visible(false);
                                            installed_title_label_for_result
                                                .set_label(&format!("Installed from {}", source.label));
                                            installed_notice_label_for_result.set_label(
                                                "Installed apps stay on their current source. Compare other sources below, then uninstall first if you want to switch.",
                                            );
                                            source_hint_label_for_result.set_visible(false);
                                            context_for_finish.mark_catalog_dirty();
                                            action_status_for_result.set_label(
                                                "Package installed. Refreshing package views...",
                                            );
                                            update_action_button_for_source(
                                                &context_for_finish,
                                                &install_button_for_result,
                                                &action_status_for_result,
                                                next_installed,
                                                &source,
                                            );
                                            sync_hero_action_buttons(
                                                &install_button_for_result,
                                                &launch_button_for_result,
                                                &uninstall_button_for_result,
                                                next_installed,
                                            );
                                            runtime_for_install_telemetry.spawn({
                                                let settings = settings_for_install_telemetry.clone();
                                                let pkg = pkg_name_for_telemetry.clone();
                                                let src = source_label_for_telemetry.clone();
                                                async move {
                                                    crate::telemetry::track_event_async(
                                                        &settings,
                                                        "install_package",
                                                        Some(serde_json::json!({
                                                            "pkg": pkg,
                                                            "source": src,
                                                            "success": true,
                                                        })),
                                                    )
                                                    .await;
                                                }
                                            });
                                        }
                                        Err(error) => {
                                            action_status_for_result.set_label(&error);
                                        }
                                    }
                                    install_button_for_result.set_sensitive(true);
                                }
                            },
                            options,
                        );
                        glib::ControlFlow::Break
                    }
                    Ok(Err(error)) => {
                        action_status_for_result.set_label(&error);
                        install_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        action_status_for_result.set_label("Helper request was interrupted.");
                        install_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                }
            });
        }
    });

    uninstall_button.connect_clicked({
        let context = context.clone();
        let uninstall_button = uninstall_button.clone();
        let launch_button = launch_button.clone();
        let install_button = install_button.clone();
        let action_status = action_status.clone();
        let installed_state = installed_state.clone();
        let installed_row = installed_row_for_action.clone();
        let selected_source = selected_source.clone();
        let installed_source_id = installed_source_id.clone();
        let current_package = current_package.clone();
        let installed_notice_box = installed_notice_box.clone();
        let source_selector_wrapper = source_selector_wrapper.clone();
        let source_hint_label = source_hint_label.clone();
        move |_| {
            let chosen_source = selected_source.borrow().clone();
            let package = current_package.borrow().clone();
            if let Err(error) = ensure_session_auth(
                &context,
                parent_window_for(&uninstall_button).as_ref(),
                false,
            ) {
                action_status.set_label(&error);
                return;
            }
            uninstall_button.set_sensitive(false);
            action_status.set_label(&pending_action_copy(true, &chosen_source));

            let (sender, receiver) = std::sync::mpsc::channel();
            context.runtime.spawn({
                let catalog = context.catalog.clone();
                let package = package.clone();
                let chosen_source = chosen_source.clone();
                async move {
                    let result = catalog
                        .remove_package_for_source_stream(package, chosen_source)
                        .await;
                    let _ = sender.send(result);
                }
            });

            let uninstall_button_for_result = uninstall_button.clone();
            let action_status_for_result = action_status.clone();
            let installed_state_for_result = installed_state.clone();
            let context_for_dialog = context.clone();
            let installed_row_for_timeout = installed_row.clone();
            let selected_source_for_timeout = selected_source.clone();
            let installed_source_id_for_timeout = installed_source_id.clone();
            let install_button_for_timeout = install_button.clone();
            let launch_button_for_timeout = launch_button.clone();
            let installed_notice_box_for_timeout = installed_notice_box.clone();
            let source_selector_wrapper_for_timeout = source_selector_wrapper.clone();
            let source_hint_label_for_timeout = source_hint_label.clone();
            let package_title_for_timeout = package.effective_title();
            let pkg_name_for_uninstall_telemetry = package.name.clone();
            let source_label_for_uninstall_telemetry = chosen_source.label.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                match receiver.try_recv() {
                    Ok(Ok(stream)) => {
                        let uninstall_options = OperationDialogOptions {
                            is_uninstall: true,
                            success_display_name: Some(package_title_for_timeout.clone()),
                            on_launch: None,
                        };
                        let pkg_for_uninstall_telemetry = pkg_name_for_uninstall_telemetry.clone();
                        let src_for_uninstall_telemetry = source_label_for_uninstall_telemetry.clone();
                        present_operation_dialog(
                            context_for_dialog.clone(),
                            &format!("Removing {}", package_title_for_timeout),
                            "Removing package through monarch-helper...",
                            stream,
                            {
                                let uninstall_button_for_result =
                                    uninstall_button_for_result.clone();
                                let action_status_for_result = action_status_for_result.clone();
                                let installed_state_for_result = installed_state_for_result.clone();
                                let installed_row_for_result = installed_row_for_timeout.clone();
                                let context_for_finish = context_for_dialog.clone();
                                let selected_source_for_result =
                                    selected_source_for_timeout.clone();
                                let installed_source_id_for_result =
                                    installed_source_id_for_timeout.clone();
                                let install_button_for_result = install_button_for_timeout.clone();
                                let launch_button_for_result = launch_button_for_timeout.clone();
                                let installed_notice_box_for_result =
                                    installed_notice_box_for_timeout.clone();
                                let source_selector_wrapper_for_result =
                                    source_selector_wrapper_for_timeout.clone();
                                let source_hint_label_for_result =
                                    source_hint_label_for_timeout.clone();
                                let settings_for_uninstall_telemetry =
                                    context_for_dialog.settings.clone();
                                let runtime_for_uninstall_telemetry =
                                    context_for_dialog.runtime.clone();
                                move |result| {
                                    match result {
                                        Ok(()) => {
                                            let next_installed = false;
                                            installed_state_for_result.set(next_installed);
                                            installed_source_id_for_result.replace(String::new());
                                            installed_row_for_result.set_subtitle("No");
                                            installed_notice_box_for_result.set_visible(false);
                                            source_selector_wrapper_for_result.set_visible(true);
                                            source_hint_label_for_result.set_visible(false);
                                            context_for_finish.mark_catalog_dirty();
                                            action_status_for_result.set_label(
                                                "Package removed. Refreshing package views...",
                                            );
                                            update_action_button_for_source(
                                                &context_for_finish,
                                                &install_button_for_result,
                                                &action_status_for_result,
                                                next_installed,
                                                &selected_source_for_result.borrow(),
                                            );
                                            sync_hero_action_buttons(
                                                &install_button_for_result,
                                                &launch_button_for_result,
                                                &uninstall_button_for_result,
                                                next_installed,
                                            );
                                            runtime_for_uninstall_telemetry.spawn({
                                                let settings = settings_for_uninstall_telemetry.clone();
                                                let pkg = pkg_for_uninstall_telemetry.clone();
                                                let src = src_for_uninstall_telemetry.clone();
                                                async move {
                                                    crate::telemetry::track_event_async(
                                                        &settings,
                                                        "uninstall_package",
                                                        Some(serde_json::json!({
                                                            "pkg": pkg,
                                                            "source": src,
                                                            "success": true,
                                                        })),
                                                    )
                                                    .await;
                                                }
                                            });
                                        }
                                        Err(error) => {
                                            action_status_for_result.set_label(&error);
                                        }
                                    }
                                    uninstall_button_for_result.set_sensitive(true);
                                }
                            },
                            uninstall_options,
                        );
                        glib::ControlFlow::Break
                    }
                    Ok(Err(error)) => {
                        action_status_for_result.set_label(&error);
                        uninstall_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        action_status_for_result.set_label("Helper request was interrupted.");
                        uninstall_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                }
            });
        }
    });

    if !canonical_id.trim().is_empty() {
        favorite_button.set_widget_name(&canonical_id);
        favorite_button.set_sensitive(true);
        let (sender, receiver) = std::sync::mpsc::channel();
        let stack_for_result = stack.clone();
        let page_for_result = page.clone();
        let hero_icon_for_result = hero_icon.clone();
        let detail_backdrop_for_result = detail_backdrop.clone();
        let initial_package_for_result = initial_package.clone();
        let hero_badges_for_result = hero_badges.clone();
        let title_for_result = title_label.clone();
        let developer_for_result = developer_label.clone();
        let summary_for_result = summary_label.clone();
        let hero_rating_label_for_result = hero_rating_label.clone();
        let source_summary_for_result = source_summary_label.clone();

        let installed_source_id_for_result = installed_source_id.clone();
        let hero_version_for_result = hero_version.clone();
        let hero_size_for_result = hero_size.clone();
        let hero_maintainer_for_result = hero_maintainer.clone();
        let hero_license_for_result = hero_license.clone();
        let hero_source_trust_for_result = hero_source_trust.clone();
        let version_row_for_result = version_row.clone();
        let name_row_for_result = name_row.clone();
        let installed_row_for_result = installed_row.clone();
        let maintainer_row_for_result = maintainer_row.clone();
        let license_row_for_result = license_row.clone();
        let download_size_row_for_result = download_size_row.clone();
        let installed_size_row_for_result = installed_size_row.clone();
        let short_description_for_result = short_description.clone();
        let description_for_result = description.clone();
        let screenshots_carousel_for_result = screenshots_carousel.clone();
        let screenshots_group_for_result = screenshots_group.clone();
        let homepage_row_for_result = homepage_row.clone();
        let open_homepage_button_for_result = open_homepage_button.clone();
        let app_id_row_for_result = app_id_row.clone();
        let launch_target_row_for_result = launch_target_row.clone();
        let reviews_summary_for_result = reviews_summary.clone();
        let reviews_list_for_result = reviews_list.clone();
        let current_review_target_for_result = current_review_target.clone();
        let current_presentation_for_result = current_presentation.clone();
        let security_label_for_result = security_label.clone();
        let security_copy_for_result = security_copy.clone();
        let copy_package_button_for_result = copy_package_button.clone();
        let copy_source_button_for_result = copy_source_button.clone();
        let copy_install_button_for_result = copy_install_button.clone();
        let install_button_for_result = install_button.clone();
        let launch_button_for_result = launch_button.clone();
        let uninstall_button_for_result = uninstall_button.clone();
        let installed_state_for_result = installed_state.clone();
        let selected_source_for_result = selected_source.clone();
        let source_selection_programmatic_for_initial = source_selection_programmatic.clone();
        let _source_list_store_for_result = source_list_store.clone();
        let source_drop_down_for_result = source_drop_down.clone();
        let installed_notice_box_for_result = installed_notice_box.clone();
        let installed_title_label_for_result = installed_title_label.clone();
        let installed_notice_label_for_result = installed_notice_label.clone();
        let source_selector_wrapper_for_result = source_selector_wrapper.clone();
        let source_hint_label_for_result = source_hint_label.clone();
        let action_status_for_result = action_status.clone();
        let current_package_for_result = current_package.clone();
        let available_variants_for_result = available_variants.clone();
        let available_sources_for_result = available_sources.clone();
        let current_app_rating_for_result = current_app_rating.clone();
        let source_switch_notice_ref_for_result = source_switch_notice_ref_for_load_detail.clone();
        let source_switch_notice_box_for_result = source_switch_notice_box_for_load_detail.clone();
        let source_switch_notice_label_for_result = source_switch_notice_label_for_load_detail.clone();
        let installed_variants_section_for_result = installed_variants_section_for_load_detail.clone();
        let installed_variants_list_for_result = installed_variants_list_for_load_detail.clone();
        let uninstalled_info_box_for_result = uninstalled_info_box_for_load_detail.clone();
        let uninstalled_info_label_for_result = uninstalled_info_label_for_load_detail.clone();

        stack.set_visible_child_name("loading");
        context.runtime.spawn({
            let catalog = context.catalog.clone();
            let preferred_source = initial_package.source.clone();
            async move {
                let details = catalog
                    .load_full_package_details(canonical_id, Some(preferred_source))
                    .await;
                let reviews = if let Ok(Some(details)) = &details {
                    let review_target = details
                        .presentation
                        .as_ref()
                        .and_then(|presentation| presentation.app_id.clone())
                        .or_else(|| details.package.as_ref().map(review_target_id))
                        .unwrap_or_default();
                    if review_target.trim().is_empty() {
                        Vec::new()
                    } else {
                        catalog
                            .load_package_reviews(review_target)
                            .await
                            .unwrap_or_default()
                    }
                } else {
                    Vec::new()
                };
                let _ = sender.send((details, reviews));
            }
        });

        let context_for_result = context.clone();
        glib::source::timeout_add_local(
            std::time::Duration::from_millis(30),
            move || match receiver.try_recv() {
                Ok((Ok(Some(details)), reviews)) => {
                    let package = details
                        .package
                        .clone()
                        .unwrap_or_else(|| initial_package_for_result.clone());
                    current_package_for_result.replace(package.clone());
                    current_presentation_for_result.replace(details.presentation.clone());
                    current_review_target_for_result.replace(
                        details
                            .presentation
                            .as_ref()
                            .and_then(|presentation| presentation.app_id.clone())
                            .unwrap_or_else(|| review_target_id(&package)),
                    );
                    available_variants_for_result.replace(details.all_variants.clone());
                    source_switch_notice_ref_for_result.replace(
                        details.source_switch_notice.clone().unwrap_or_default(),
                    );
                    apply_package_icon(
                        &hero_icon_for_result,
                        &package,
                        details.presentation.as_ref(),
                        &context_for_result,
                    );
                    apply_package_icon(
                        &detail_backdrop_for_result,
                        &package,
                        details.presentation.as_ref(),
                        &context_for_result,
                    );
                    apply_detail_badges(&hero_badges_for_result, &package);
                    title_for_result.set_label(&package.effective_title());
                    developer_for_result.set_label(&hero_identity_copy(
                        &package,
                        details.presentation.as_ref(),
                        &package.source,
                    ));
                    summary_for_result.set_label(&summary_text(&package));

                    // App-level rating: prefer combined reviews (ODRS + Supabase) average; fall back to package.rating (ODRS only).
                    let (rating_score, _) = if !reviews.is_empty() {
                        let rated = reviews
                            .iter()
                            .filter_map(|r| r.rating.map(|x| normalize_rating(x as f64)))
                            .collect::<Vec<_>>();
                        if !rated.is_empty() {
                            (
                                rated.iter().sum::<f64>() / rated.len() as f64,
                                reviews.len() as u32,
                            )
                        } else {
                            (0.0, 0)
                        }
                    } else {
                        (0.0, 0)
                    };
                    let (rating_score, _) = if rating_score > 0.0 {
                        (rating_score, 0)
                    } else if let Some(rating) = package.rating.as_ref() {
                        if let Some(score) = rating.score {
                            let s = normalize_rating(score);
                            if s > 0.0 {
                                (s, rating.total)
                            } else {
                                (0.0, 0)
                            }
                        } else {
                            (0.0, 0)
                        }
                    } else {
                        (0.0, 0)
                    };
                    let rating_label = if rating_score > 0.0 {
                        format!("{rating_score:.1} ★")
                    } else {
                        "— ★".to_string()
                    };
                    if rating_score > 0.0 {
                        current_app_rating_for_result.replace(Some(rating_score));
                    }
                    hero_rating_label_for_result.set_label(&rating_label);
                    hero_rating_label_for_result.set_visible(true);

                    apply_security_summary(
                        &security_label_for_result,
                        &security_copy_for_result,
                        &details,
                        &package,
                    );
                    set_hero_stat_value(&hero_version_for_result, &package.source.version);
                    set_hero_stat_value(
                        &hero_size_for_result,
                        &format_optional_size(
                            package.download_size_bytes.or(package.download_size),
                        ),
                    );
                    set_hero_stat_value(
                        &hero_maintainer_for_result,
                        package
                            .maintainer
                            .as_deref()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or("Unknown"),
                    );
                    set_hero_stat_value(
                        &hero_license_for_result,
                        &package
                            .license
                            .as_ref()
                            .filter(|value| !value.is_empty())
                            .map(|value| value.join(", "))
                            .unwrap_or_else(|| "Unknown".to_string()),
                    );
                    version_row_for_result.set_subtitle(&package.source.version);
                    name_row_for_result.set_subtitle(&package.name);
                    installed_row_for_result.set_subtitle(if package.installed {
                        "Yes"
                    } else {
                        "No"
                    });
                    maintainer_row_for_result.set_subtitle(
                        package
                            .maintainer
                            .as_deref()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or("Not published"),
                    );
                    license_row_for_result.set_subtitle(
                        &package
                            .license
                            .as_ref()
                            .filter(|value| !value.is_empty())
                            .map(|value| value.join(", "))
                            .unwrap_or_else(|| "Unknown".to_string()),
                    );
                    download_size_row_for_result.set_subtitle(&format_optional_size(
                        package.download_size_bytes.or(package.download_size),
                    ));
                    installed_size_row_for_result.set_subtitle(&format_optional_size(
                        package.installed_size_bytes.or(package.installed_size),
                    ));
                    installed_state_for_result.set(package.installed);
                    installed_source_id_for_result.replace(source_identity_key(&package.source));
                    let is_favorite = context_for_result
                        .favorites
                        .contains(&package.canonical_id)
                        .unwrap_or(false);
                    favorite_state.set(is_favorite);
                    favorite_button.set_widget_name(&package.canonical_id);
                    update_favorite_button(&favorite_button, is_favorite);
                    let refreshed_sources = package
                        .available_sources
                        .clone()
                        .filter(|sources| !sources.is_empty())
                        .unwrap_or_else(|| vec![package.source.clone()]);
                    available_sources_for_result.replace(refreshed_sources.clone());
                    // Preserve user's current source selection if they already changed the dropdown
                    // (e.g. to Flatpak) before this initial load completed — otherwise we'd overwrite
                    // and "jump back" to the default source.
                    let current_selection = selected_source_for_result.borrow().clone();
                    let (selected_index, _source_to_show) = refreshed_sources
                        .iter()
                        .position(|s| source_matches_variant(&current_selection, s))
                        .and_then(|idx| refreshed_sources.get(idx).map(|s| (idx, s.clone())))
                        .unwrap_or_else(|| {
                            let preferred_source = details
                                .selected_default_source
                                .as_ref()
                                .cloned()
                                .unwrap_or_else(|| package.source.clone());
                            let idx = refreshed_sources
                                .iter()
                                .position(|s| source_matches_variant(&preferred_source, s))
                                .unwrap_or(0);
                            let src = refreshed_sources.get(idx).cloned().unwrap_or_else(|| package.source.clone());
                            (idx, src)
                        });

                    // Defer dropdown list update to next main-loop tick to avoid gtk_box_append
                    // assertion (replacing the model triggers internal list rebuild). Update the
                    // existing store in place instead of set_model so the list view never re-parents rows.
                    let store = _source_list_store_for_result.borrow().clone();
                    let refreshed_sources_for_timeout = refreshed_sources.clone();
                    let drop_down = source_drop_down_for_result.clone();
                    let idx = selected_index as u32;
                    source_selection_programmatic_for_initial.replace(true);
                    glib::source::timeout_add_local_once(std::time::Duration::from_millis(0), move || {
                        update_source_list_store(&store, &refreshed_sources_for_timeout);
                        drop_down.set_selected(idx);
                    });

                    if let Some(source) = refreshed_sources.get(selected_index).cloned() {
                        selected_source_for_result.replace(source.clone());
                        sync_selected_source_ui(
                            &context_for_result,
                            &package,
                            &available_variants_for_result.borrow(),
                            current_presentation_for_result.borrow().as_ref(),
                            &source,
                            package.installed,
                            &installed_source_id_for_result.borrow(),
                            &installed_notice_box_for_result,
                            &installed_title_label_for_result,
                            &installed_notice_label_for_result,
                            &source_selector_wrapper_for_result,
                            &source_hint_label_for_result,
                            &developer_for_result,
                            &summary_for_result,
                            &source_summary_for_result,
                            &hero_version_for_result,
                            &hero_size_for_result,
                            &hero_maintainer_for_result,
                            &hero_license_for_result,
                            &hero_source_trust_for_result,
                            &version_row_for_result,
                            &maintainer_row_for_result,
                            &license_row_for_result,
                            &download_size_row_for_result,
                            &installed_size_row_for_result,
                            &short_description_for_result,
                            &description_for_result,
                            &screenshots_carousel_for_result,
                            &screenshots_group_for_result,
                            &security_label_for_result,
                            &security_copy_for_result,
                            &copy_source_button_for_result,
                            &copy_install_button_for_result,
                            &install_button_for_result,
                            &launch_button_for_result,
                            &uninstall_button_for_result,
                            &action_status_for_result,
                            &source_switch_notice_box_for_result,
                            &source_switch_notice_label_for_result,
                            &*source_switch_notice_ref_for_result,
                            &installed_variants_section_for_result,
                            &installed_variants_list_for_result,
                            &uninstalled_info_box_for_result,
                            &uninstalled_info_label_for_result,
                            &hero_rating_label_for_result,
                            current_app_rating_for_result.borrow().clone(),
                        );
                    }
                    if package.rating.is_none() && !reviews.is_empty() {
                        let rated: Vec<f64> = reviews
                            .iter()
                            .filter_map(|r| r.rating.map(|x| normalize_rating(x as f64)))
                            .collect();
                        if !rated.is_empty() {
                            let avg = rated.iter().sum::<f64>() / rated.len() as f64;
                            hero_rating_label_for_result.set_label(&format!("{avg:.1} ★"));
                            hero_rating_label_for_result.set_visible(true);
                        }
                    }
                    render_reviews(
                        &reviews_list_for_result,
                        &reviews_summary_for_result,
                        &reviews,
                    );
                    homepage_row_for_result.set_subtitle(
                        package
                            .url
                            .as_deref()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or("No homepage published"),
                    );
                    sync_link_action(&open_homepage_button_for_result, package.url.as_deref());
                    app_id_row_for_result.set_subtitle(
                        package
                            .app_id
                            .as_deref()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or("No app ID published"),
                    );
                    launch_target_row_for_result.set_subtitle(
                        package
                            .launch_target
                            .as_deref()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or("No launch target published"),
                    );
                    set_copy_action_value(&copy_package_button_for_result, &package.canonical_id);
                    title_label_for_initial.set_label(&package.effective_title());
                    page_for_result.set_title(&package.effective_title());
                    stack_for_result.set_visible_child_name("detail");
                    glib::ControlFlow::Break
                }
                Ok((Ok(None), _)) | Ok((Err(_), _)) => {
                    stack_for_result.set_visible_child_name("detail");
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    stack_for_result.set_visible_child_name("detail");
                    glib::ControlFlow::Break
                }
            },
        );
    }

    apply_package_icon(&hero_icon, initial_package, None, &context);
    apply_package_icon(&detail_backdrop, initial_package, None, &context);
    apply_detail_badges(&hero_badges, initial_package);
    sync_selected_source_ui(
        &context,
        initial_package,
        &[],
        None,
        &selected_source.borrow(),
        initial_package.installed,
        &installed_source_id.borrow(),
        &installed_notice_box,
        &installed_title_label,
        &installed_notice_label,
        &source_selector_wrapper,
        &source_hint_label,
        &developer_label,
        &summary_label,
        &source_summary_label,
        &hero_version,
        &hero_size,
        &hero_maintainer,
        &hero_license,
        &hero_source_trust,
        &version_row,
        &maintainer_row,
        &license_row,
        &download_size_row,
        &installed_size_row,
        &short_description,
        &description,
        &screenshots_carousel,
        &screenshots_group,
        &security_label,
        &security_copy,
        &copy_source_button,
        &copy_install_button,
        &install_button,
        &launch_button,
        &uninstall_button,
        &action_status,
        &source_switch_notice_box_for_initial,
        &source_switch_notice_label_for_initial,
        &*source_switch_notice_ref_for_initial,
        &installed_variants_section_for_initial,
        &installed_variants_list_for_initial,
        &uninstalled_info_box_for_initial,
        &uninstalled_info_label_for_initial,
        &hero_rating_label,
        current_app_rating.borrow().clone(),
    );
    page
}

fn update_favorite_button(button: &gtk::Button, is_favorite: bool) {
    button.set_icon_name(if is_favorite {
        "starred-symbolic"
    } else {
        "non-starred-symbolic"
    });
    button.set_tooltip_text(Some(if is_favorite {
        "Remove from favorites"
    } else {
        "Add to favorites"
    }));
    if is_favorite {
        button.add_css_class("is-favorite");
    } else {
        button.remove_css_class("is-favorite");
    }
}

fn update_action_button_for_source(
    context: &AppContext,
    action_button: &gtk::Button,
    action_status: &gtk::Label,
    installed: bool,
    source: &PackageSource,
) {
    if installed {
        action_button.set_label("Install");
        action_status.set_label("");
        action_status.set_visible(false);
        return;
    }

    let settings = context.settings.load().unwrap_or_default();
    let can_install = source.source_type == "repo"
        || (source.source_type == "aur" && settings.aur_enabled)
        || (source.source_type == "flatpak" && settings.flatpak_enabled);
    if can_install {
        action_button.set_sensitive(true);
        action_button.set_label("Install");
        action_status.set_label("");
        action_status.set_visible(false);
    } else {
        action_button.set_sensitive(false);
        action_button.set_label("Configure Source in Settings");
        action_status.set_label(&unsupported_source_copy(source, &settings));
        action_status.set_visible(true);
    }
}

#[allow(clippy::too_many_arguments)] // TODO: refactor to SyncSourceUiParams struct (32 args)
fn sync_selected_source_ui(
    context: &AppContext,
    package: &Package,
    variants: &[PackageVariant],
    presentation: Option<&PackagePresentation>,
    source: &PackageSource,
    installed: bool,
    installed_source_id: &str,
    installed_notice_box: &gtk::Box,
    installed_title_label: &gtk::Label,
    installed_notice_label: &gtk::Label,
    source_selector_wrapper: &gtk::Box,
    source_hint_label: &gtk::Label,
    developer_label: &gtk::Label,
    summary_label: &gtk::Label,
    source_summary_label: &gtk::Label,
    hero_version: &gtk::Box,
    hero_size: &gtk::Box,
    hero_maintainer: &gtk::Box,
    hero_license: &gtk::Box,
    hero_source_trust: &gtk::Box,
    version_row: &adw::ActionRow,
    maintainer_row: &adw::ActionRow,
    license_row: &adw::ActionRow,
    download_size_row: &adw::ActionRow,
    installed_size_row: &adw::ActionRow,
    short_description: &gtk::Label,
    description: &gtk::Label,
    screenshots_carousel: &adw::Carousel,
    screenshots_group: &gtk::Box,
    security_label: &gtk::Label,
    security_copy: &gtk::Label,
    copy_source_button: &gtk::Button,
    copy_install_button: &gtk::Button,
    install_button: &gtk::Button,
    launch_button: &gtk::Button,
    uninstall_button: &gtk::Button,
    action_status: &gtk::Label,
    source_switch_notice_box: &gtk::Box,
    source_switch_notice_label: &gtk::Label,
    source_switch_notice_ref: &std::cell::RefCell<String>,
    installed_variants_section: &gtk::Box,
    installed_variants_list: &gtk::ListBox,
    uninstalled_info_box: &gtk::Box,
    uninstalled_info_label: &gtk::Label,
    hero_rating_label: &gtk::Label,
    app_rating_override: Option<f64>,
) {
    let rating_label = app_rating_override
        .filter(|&s| s > 0.0)
        .or_else(|| package.rating.as_ref().and_then(|r| r.score).map(normalize_rating))
        .filter(|&s| s > 0.0)
        .map(|s| format!("{s:.1} ★"))
        .unwrap_or_else(|| "— ★".to_string());
    hero_rating_label.set_label(&rating_label);
    hero_rating_label.set_visible(true);

    let selected_installed = installed && source_matches_identity_key(source, installed_source_id);

    // When installed show "Installed from X" box; always show source selector if multiple sources so user can preview other sources' data.
    installed_notice_box.set_visible(installed);
    let multiple_sources = package.available_sources.as_ref().map_or(false, |s| s.len() > 1);
    source_selector_wrapper.set_visible(multiple_sources);
    uninstalled_info_box.set_visible(!installed);
    if installed {
        let installed_label = if selected_installed {
            source.label.clone()
        } else {
            package
                .available_sources
                .as_ref()
                .and_then(|sources| {
                    sources
                        .iter()
                        .find(|candidate| source_matches_identity_key(candidate, installed_source_id))
                })
                .map(|s| s.label.as_str())
                .unwrap_or(package.source.label.as_str())
                .to_string()
            };
        installed_title_label.set_label(&format!("Installed from {}", installed_label));
        installed_notice_label.set_label(
            "Installed apps stay on their current source. Compare other sources below, then uninstall first if you want to switch.",
        );
        let notice = source_switch_notice_ref.borrow();
        if notice.is_empty() {
            source_switch_notice_label.set_label("To switch to another source, uninstall the current one first.");
        } else {
            source_switch_notice_label.set_label(notice.as_str());
        }
        source_switch_notice_box.set_visible(true);
        if let Some(sources) = package.available_sources.as_ref() {
            if sources.len() > 1 {
                while let Some(child) = installed_variants_list.first_child() {
                    installed_variants_list.remove(&child);
                }
                for src in sources.iter() {
                    let is_installed_source = source_matches_identity_key(src, installed_source_id);
                    let row = adw::ActionRow::builder()
                        .title(&src.label)
                        .subtitle(if is_installed_source {
                            "Installed source"
                        } else {
                            "Available after uninstall"
                        })
                        .build();
                    row.add_css_class(if is_installed_source {
                        "installed-source-row"
                    } else {
                        "available-after-uninstall-row"
                    });
                    installed_variants_list.append(&row);
                }
                installed_variants_section.set_visible(true);
            } else {
                installed_variants_section.set_visible(false);
            }
        } else {
            installed_variants_section.set_visible(false);
        }
    } else {
        source_switch_notice_box.set_visible(false);
        installed_variants_section.set_visible(false);
        if let Some(variants_count) = package.available_sources.as_ref().map(|s| s.len()) {
            if variants_count > 1 {
                source_hint_label.set_label(&format!("{} source options available", variants_count));
                source_hint_label.set_visible(true);
            } else {
                source_hint_label.set_visible(false);
            }
        } else {
            source_hint_label.set_visible(false);
        }
    }

    developer_label.set_label(&hero_identity_copy(package, presentation, source));
    let source_summary_text = selected_source_summary(
        package,
        source,
        selected_installed,
        installed_source_id,
    );
    source_summary_label.set_label(&source_summary_text);

    let variant = variants
        .iter()
        .find(|v| source_matches_variant(source, &v.source));
    let short_copy = variant
        .and_then(|variant| variant.description.as_deref())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            presentation
                .and_then(|presentation| presentation.short_description.as_deref())
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or(&package.description);
    let long_copy = presentation
        .and_then(|presentation| presentation.long_description.as_deref())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            package
                .long_description
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| variant.and_then(|variant| variant.description.as_deref()))
        .unwrap_or(&package.description);
    let screenshots = variant
        .and_then(|variant| variant.screenshots.as_deref())
        .filter(|shots| !shots.is_empty())
        .or_else(|| {
            presentation
                .map(|presentation| presentation.screenshots.as_slice())
                .filter(|shots| !shots.is_empty())
        })
        .or(package.screenshots.as_deref())
        .unwrap_or(&[]);
    let version = variant
        .map(|variant| variant.version.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&source.version);
    // When no variant matches (e.g. variants not loaded yet or identity mismatch), show only
    // what we know for the selected source (version) and "Unknown" for the rest so we never
    // display another source's data in the bar.
    let maintainer = variant
        .and_then(|v| v.maintainer.as_deref())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Unknown");
    let license = variant
        .and_then(|v| v.license.as_ref())
        .filter(|value| !value.is_empty())
        .map(|value| value.join(", "))
        .unwrap_or_else(|| "Unknown".to_string());
    let download_size = variant.and_then(|v| v.download_size);
    let installed_size = variant.and_then(|v| v.installed_size);
    let size_copy = format_optional_size(download_size.or(installed_size));

    summary_label.set_label(&summary_sentence(short_copy));
    set_hero_stat_value(hero_version, version);
    set_hero_stat_value(hero_size, &size_copy);
    set_hero_stat_value(hero_maintainer, maintainer);
    set_hero_stat_value(hero_license, &license);
    let source_trust_label = source_trust_label_for_variant(source, variant.and_then(|v| v.security.as_ref()));
    set_hero_stat_value(hero_source_trust, &source_trust_label);

    if !installed {
        let mut parts: Vec<String> = Vec::new();
        if let Some(v) = variant {
            if let Some(sec) = &v.security {
                parts.push(format!("{} {}", sec.verification_note, sec.user_action_note));
            }
        }
        parts.push(source_summary_text.clone());
        if source.id.to_lowercase().contains("chaotic") {
            parts.push(
                "Chaotic-AUR is a community repository. Check your distro's documentation and the repository before adding.".to_string(),
            );
        }
        uninstalled_info_label.set_label(&parts.join("\n\n"));
    }

    version_row.set_subtitle(version);
    maintainer_row.set_subtitle(maintainer);
    license_row.set_subtitle(&license);
    download_size_row.set_subtitle(&format_optional_size(download_size));
    installed_size_row.set_subtitle(&format_optional_size(installed_size));
    let short_clean = strip_html_description(short_copy);
    let long_clean = strip_html_description(long_copy);
    short_description.set_label(&short_clean);
    description.set_label(&long_clean);
    let short_normalized = short_clean.trim().to_string();
    let long_normalized = long_clean.trim().to_string();
    let same_copy = short_normalized.eq_ignore_ascii_case(&long_normalized);
    short_description.set_visible(!short_normalized.is_empty());
    description.set_visible(!long_normalized.is_empty() && !same_copy);
    render_screenshots(
        context,
        screenshots_group,
        screenshots_carousel,
        screenshots,
    );

    if let Some(security) = variant.and_then(|variant| variant.security.as_ref()) {
        security_label.set_label(&format!("{} access", security.system_access));
        security_copy.set_label(&format!(
            "{} {}",
            security.verification_note, security.user_action_note
        ));
    } else if source.source_type == "flatpak" {
        security_label.set_label("Sandboxed package");
        security_copy.set_label(package.security_summary.as_deref().unwrap_or(
            "Flatpak variants use scoped permissions and keep app access containerized.",
        ));
    } else {
        security_label.set_label("Native package");
        security_copy.set_label(
            package
                .security_summary
                .as_deref()
                .unwrap_or("Native packages have full system access. Review the maintainer and source before installing."),
        );
    }
    set_copy_action_value(
        copy_source_button,
        &format!("{}:{}", source.source_type, source.id),
    );
    set_copy_action_value(
        copy_install_button,
        &suggested_install_command_for_source(package, source),
    );

    update_action_button_for_source(
        context,
        install_button,
        action_status,
        selected_installed,
        source,
    );
    if installed && !selected_installed {
        action_status.set_label(&format!(
            "Installed from another source. Select {} to launch or remove the current install.",
            package.source.label
        ));
        action_status.set_visible(true);
    }
    sync_hero_action_buttons(
        install_button,
        launch_button,
        uninstall_button,
        selected_installed,
    );
}

fn sync_hero_action_buttons(
    install_button: &gtk::Button,
    launch_button: &gtk::Button,
    uninstall_button: &gtk::Button,
    installed: bool,
) {
    install_button.set_visible(!installed);
    install_button.set_sensitive(!installed);
    launch_button.set_visible(installed);
    launch_button.set_sensitive(installed);
    uninstall_button.set_visible(installed);
    uninstall_button.set_sensitive(installed);
}

pub(crate) fn default_package_source(package: &Package) -> PackageSource {
    package
        .available_sources
        .as_ref()
        .and_then(|sources| sources.first().cloned())
        .unwrap_or_else(|| package.source.clone())
}

pub(crate) fn launch_selected_package(
    package: &Package,
    source: &PackageSource,
) -> Result<(), String> {
    let package_name = package.name.trim();
    if package_name.is_empty() {
        return Err("Package name is empty".to_string());
    }

    let is_flatpak = source.source_type == "flatpak"
        || package
            .app_id
            .as_ref()
            .map(|value| value.contains('.'))
            .unwrap_or(false)
        || package
            .launch_target
            .as_ref()
            .map(|value| value.contains('.'))
            .unwrap_or(false)
        || package_name.contains('.');

    if is_flatpak {
        let app_id = package
            .launch_target
            .clone()
            .or(package.app_id.clone())
            .unwrap_or_else(|| package_name.to_string());
        return Command::new("flatpak")
            .args(["run", &app_id])
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Failed to launch Flatpak app '{}': {}", app_id, error));
    }

    if let Some(target) = package.launch_target.clone().or(package.app_id.clone()) {
        if let Ok(()) = try_spawn_desktop(&target) {
            return Ok(());
        }
    }

    if let Some(desktop_entry) = resolve_desktop_entry(package_name) {
        if let Ok(()) = try_spawn_desktop(&desktop_entry) {
            return Ok(());
        }
    }

    Command::new(package_name)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to launch '{}': {}", package_name, error))
}

fn try_spawn_desktop(target: &str) -> Result<(), String> {
    Command::new("gtk-launch")
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to launch desktop entry '{}': {}", target, error))
}

fn resolve_desktop_entry(pkg_name: &str) -> Option<String> {
    let search_paths = [
        "/usr/share/applications".to_string(),
        "/usr/local/share/applications".to_string(),
        format!(
            "{}/.local/share/applications",
            std::env::var("HOME").unwrap_or_default()
        ),
    ];

    for path in search_paths {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".desktop")
                    && (name == format!("{}.desktop", pkg_name) || name.contains(pkg_name))
                {
                    return Some(name.trim_end_matches(".desktop").to_string());
                }
            }
        }
    }

    None
}

fn unsupported_source_copy(source: &PackageSource, settings: &GtkSettings) -> String {
    match source.source_type.as_str() {
        "aur" if !settings.aur_enabled => {
            "AUR discovery is turned off. Enable it in Settings before installing from this source."
                .to_string()
        }
        "aur" => {
            "AUR discovery is enabled. This source builds locally with makepkg and will prompt for privileges only when dependencies or final install require it."
                .to_string()
        }
        "flatpak" if !settings.flatpak_enabled => {
            "Flatpak discovery is turned off. Enable it in Settings before installing from this source."
                .to_string()
        }
        "flatpak" => {
            "Flatpak discovery is enabled. If Flatpak itself is missing, MonARCH will install the runtime tool before the app."
                .to_string()
        }
        _ if source.id == "chaotic-aur" => {
            "Chaotic-AUR packages depend on the host repo configuration. If the host is not ready yet, use Settings -> Maintenance -> Prepare Chaotic-AUR components, then refresh discovery."
                .to_string()
        }
        _ => format!("This host cannot install packages from {} through the current execution path.", source.label),
    }
}

fn pending_action_copy(installed: bool, source: &PackageSource) -> String {
    if installed {
        match source.source_type.as_str() {
            "flatpak" => "Removing Flatpak app...".to_string(),
            _ => "Authorizing package removal...".to_string(),
        }
    } else {
        match source.source_type.as_str() {
            "aur" => "Preparing AUR build...".to_string(),
            "flatpak" => "Preparing Flatpak install...".to_string(),
            _ => "Authorizing package installation...".to_string(),
        }
    }
}

fn build_action_row(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(escape_markup(title))
        .subtitle(escape_markup(subtitle))
        .build()
}

/// Short, neutral label for the data bar "Source" tile (Bazaar-style). Explanatory, not negative; we support all sources.
fn source_trust_label_for_variant(
    source: &PackageSource,
    security: Option<&monarch_core::models::PackageSecuritySummary>,
) -> String {
    let tier = security
        .map(|s| s.trust_tier.as_str())
        .unwrap_or_else(|| {
            let id = source.id.to_lowercase();
            match source.source_type.as_str() {
                "flatpak" => "sandboxed",
                "aur" => "community_build",
                "repo" if id.contains("chaotic") => "third_party_repo",
                "repo"
                    if id.contains("cachyos")
                        || id.contains("manjaro")
                        || id.contains("garuda")
                        || id.contains("endeavour") =>
                {
                    "distro_native"
                }
                _ => "official",
            }
        });
    match tier {
        "sandboxed" => "Sandboxed",
        "official" => "Official repo",
        "distro_native" => "Distro repo",
        "community_build" => "Community build",
        "third_party_repo" => "Third-party repo",
        _ => "Sandboxed",
    }
    .to_string()
}

/// Bazaar-style: value inside pill, label under pill (title under pill - not inside).
fn build_hero_stat(title: &str, value: &str) -> gtk::Box {
    let value_label = gtk::Label::builder()
        .label(value)
        .xalign(0.0)
        .wrap(false)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(vec![
            "monarch-hero-stat-value".to_string(),
            "monarch-context-tile-text".to_string(),
        ])
        .build();
    let value_wrap = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(vec!["monarch-hero-stat-value-wrap".to_string()])
        .build();
    value_wrap.append(&value_label);
    let stat = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .css_classes(vec!["monarch-hero-stat".to_string()])
        .build();
    stat.append(&value_wrap);
    stat.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(vec!["monarch-hero-stat-title".to_string()])
            .build(),
    );
    stat
}

fn set_hero_stat_value(stat: &gtk::Box, value: &str) {
    if let Some(wrap) = stat.first_child().and_downcast::<gtk::Box>() {
        if let Some(label) = wrap.first_child().and_downcast::<gtk::Label>() {
            label.set_label(value);
        }
    }
}

fn same_source_identity(left: &PackageSource, right: &PackageSource) -> bool {
    left.source_type == right.source_type
        && left.id.eq_ignore_ascii_case(&right.id)
        && left.package_name == right.package_name
}

/// Matches a variant's source to the selected source. Type and id (case-insensitive) must match;
/// package_name must match when both are Some. When either is None we still match so dropdown
/// selection works when backend omits package_name on one side.
fn source_matches_variant(selected: &PackageSource, variant_source: &PackageSource) -> bool {
    if variant_source.source_type != selected.source_type
        || !variant_source.id.eq_ignore_ascii_case(&selected.id)
    {
        return false;
    }
    match (&variant_source.package_name, &selected.package_name) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

fn wire_copy_action(button: &gtk::Button, default_label: &str, success_message: &str) {
    let default_label = default_label.to_string();
    let success_message = success_message.to_string();
    button.set_label(&default_label);
    button.connect_clicked(move |button| {
        let Some(value) = button.tooltip_text() else {
            return;
        };
        if let Some(display) = gtk::gdk::Display::default() {
            display.clipboard().set_text(&value);
        }
        button.set_label(&success_message);
        let button_for_reset = button.clone();
        let default_label = default_label.clone();
        glib::source::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
            button_for_reset.set_label(&default_label);
        });
    });
}

fn sync_link_action(button: &gtk::Button, value: Option<&str>) {
    let normalized = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    button.set_sensitive(normalized.is_some());
    button.set_tooltip_text(normalized.as_deref());
    if normalized.is_some() {
        button.set_label("Open Project Website");
    } else {
        button.set_label("Project Website Unavailable");
    }
}

fn set_copy_action_value(button: &gtk::Button, value: &str) {
    button.set_tooltip_text(Some(value));
}

fn apply_security_summary(
    title: &gtk::Label,
    body: &gtk::Label,
    details: &monarch_core::models::FullPackageDetails,
    package: &Package,
) {
    if let Some(security) = details.security.as_ref() {
        title.set_label(&format!("{} access", security.system_access));
        body.set_label(&format!(
            "{} {}",
            security.verification_note, security.user_action_note
        ));
        return;
    }

    let is_flatpak = package.source.source_type == "flatpak";
    title.set_label(if is_flatpak {
        "Sandboxed package"
    } else {
        "Native package"
    });
    body.set_label(
        package
            .security_summary
            .as_deref()
            .unwrap_or(if is_flatpak {
                "Flatpak variants use scoped permissions and keep app access containerized."
            } else {
                "Native packages have full system access. Review the maintainer and source before installing."
            }),
    );
}

fn suggested_install_command_for_source(package: &Package, source: &PackageSource) -> String {
    let package_name = source
        .package_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&package.name);
    match source.source_type.as_str() {
        "flatpak" => format!("flatpak install {}", package_name),
        "aur" => format!("yay -S {}", package_name),
        _ => format!("sudo pacman -S {}", package_name),
    }
}

fn hero_identity_copy(
    package: &Package,
    presentation: Option<&PackagePresentation>,
    source: &PackageSource,
) -> String {
    let developer = presentation
        .and_then(|presentation| presentation.developer_name.as_deref())
        .or(package.maintainer.as_deref())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Community maintained");
    format!("{}  •  {}", developer, source.label)
}

fn detail_text(package: &Package) -> &str {
    package
        .long_description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&package.description)
}

fn summary_text(package: &Package) -> String {
    summary_sentence(&package.description)
}

fn summary_sentence(value: &str) -> String {
    value
        .split('.')
        .next()
        .map(|segment| segment.trim().to_string())
        .filter(|segment| !segment.is_empty())
        .unwrap_or_else(|| value.to_string())
}

/// Strip HTML/XML tags from AppStream description text, convert structural tags
/// to readable punctuation, and keep only the first English-dominant paragraph block.
fn strip_html_description(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }

    // Convert block-level tags to newlines before stripping
    let replaced = text
        .replace("<p>", "")
        .replace("</p>", "\n\n")
        .replace("<li>", "• ")
        .replace("</li>", "\n")
        .replace("<ul>", "")
        .replace("</ul>", "\n")
        .replace("<ol>", "")
        .replace("</ol>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");

    // Strip remaining XML/HTML tags
    let mut stripped = String::with_capacity(replaced.len());
    let mut in_tag = false;
    for ch in replaced.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => stripped.push(ch),
            _ => {}
        }
    }

    // Decode common HTML entities
    let decoded = stripped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    // Split into paragraph blocks and keep only English-dominant ones.
    // AppStream concatenates all locales; we heuristically identify English by
    // checking that > 70% of word chars are ASCII.
    let paragraphs: Vec<&str> = decoded
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();

    let english_paragraphs: Vec<&str> = paragraphs
        .iter()
        .copied()
        .filter(|p| {
            let total_word_chars = p.chars().filter(|c| c.is_alphabetic()).count();
            if total_word_chars == 0 {
                return false;
            }
            let ascii_chars = p.chars().filter(|c| c.is_ascii_alphabetic()).count();
            ascii_chars * 10 >= total_word_chars * 7 // ≥70% ASCII alphabetic
        })
        .collect();

    // Use English paragraphs if we found any, otherwise fall back to everything.
    let result_paragraphs = if english_paragraphs.is_empty() {
        &paragraphs
    } else {
        &english_paragraphs
    };

    // Show only the first paragraph to avoid spamming multiple languages or repeated text.
    let first = result_paragraphs
        .first()
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    first.trim().to_string()
}

fn selected_source_summary(
    package: &Package,
    source: &PackageSource,
    selected_installed: bool,
    installed_source_id: &str,
) -> String {
    if selected_installed {
        format!("Installed from {}", source.label)
    } else if package.installed && !installed_source_id.is_empty() {
        let installed_label = package
            .available_sources
            .as_ref()
            .and_then(|sources| {
                sources
                    .iter()
                    .find(|candidate| source_matches_identity_key(candidate, installed_source_id))
            })
            .map(|source| source.label.as_str())
            .unwrap_or(package.source.label.as_str());
        format!("Installed from {installed_label}. Remove it first to switch sources.")
    } else if package
        .available_sources
        .as_ref()
        .map(|items| items.len())
        .unwrap_or(1)
        > 1
    {
        let total_sources = package
            .available_sources
            .as_ref()
            .map(|items| items.len())
            .unwrap_or(1);
        format!("{total_sources} source options available")
    } else {
        format!("Available from {}", source.label)
    }
}

fn apply_detail_badges(container: &gtk::Box, package: &Package) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    for badge in detail_badges(package) {
        container.append(
            &gtk::Label::builder()
                .label(badge)
                .css_classes(vec!["monarch-chip".to_string()])
                .build(),
        );
    }
}

fn detail_badges(package: &Package) -> Vec<String> {
    let mut badges = vec![package.source.label.clone()];
    badges.push(if package.installed {
        "Installed".to_string()
    } else {
        "Available".to_string()
    });
    if package.is_optimized.unwrap_or(false) {
        badges.push("Optimized".to_string());
    }
    badges
}

fn is_valid_screenshot_url(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty()
        && (t.starts_with("http://") || t.starts_with("https://") || t.starts_with("data:"))
}

fn render_screenshots(
    context: &AppContext,
    group: &gtk::Box,
    carousel: &adw::Carousel,
    shots: &[String],
) {
    while carousel.n_pages() > 0 {
        let child = carousel.nth_page(0);
        carousel.remove(&child);
    }
    let shots: Vec<&str> = shots.iter().map(String::as_str).filter(|s| is_valid_screenshot_url(s)).collect();
    if shots.is_empty() {
        group.set_visible(false);
        return;
    }
    group.set_visible(true);

    for shot in shots {
        let frame = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .hexpand(true)
            .css_classes(vec![
                "monarch-toolbar-card".to_string(),
                "monarch-detail-shot".to_string(),
            ])
            .build();
        let picture = gtk::Picture::builder()
            .width_request(780)
            .height_request(430)
            .can_shrink(true)
            .build();
        picture.set_paintable(Some(crate::ui::media::placeholder_texture()));
        set_picture_source(&picture, context.runtime.clone(), Some(shot.to_string()), None);
        frame.append(&picture);
        carousel.append(&frame);
    }
}

fn render_reviews(list: &gtk::ListBox, summary: &gtk::Label, reviews: &[PackageReview]) {
    // Move focus to the list (or its parent) before removing rows so no row has focus
    // when removed (avoids gtk_list_box_row_grab_focus assertion 'box != NULL').
    if list.can_focus() {
        let _ = list.grab_focus();
    } else if let Some(parent) = list.parent() {
        let _ = parent.grab_focus();
    }
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    if reviews.is_empty() {
        summary.set_label("No community reviews have been published for this app yet.");
        return;
    }

    let rated = reviews
        .iter()
        .filter_map(|review| review.rating.map(|rating| normalize_rating(rating as f64)))
        .collect::<Vec<_>>();
    if rated.is_empty() {
        summary.set_label(&format!("{} reviews available.", reviews.len()));
    } else {
        let average = rated.iter().sum::<f64>() / rated.len() as f64;
        summary.set_label(&format!(
            "{average:.1}★ average across {} reviews.",
            reviews.len()
        ));
    }

    for review in reviews.iter().take(8) {
        let row = gtk::ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .css_classes(vec!["monarch-review-row".to_string()])
            .build();
        let header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();
        let title_block = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .hexpand(true)
            .build();
        title_block.append(
            &gtk::Label::builder()
                .label(
                    review
                        .user_display
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "Anonymous reviewer".to_string()),
                )
                .xalign(0.0)
                .css_classes(vec!["monarch-review-user".to_string()])
                .build(),
        );
        let meta_copy = review
            .locale
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                review
                    .version
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or("Community review");
        title_block.append(
            &gtk::Label::builder()
                .label(meta_copy)
                .xalign(0.0)
                .css_classes(vec!["monarch-review-meta".to_string()])
                .build(),
        );
        header.append(&title_block);
        header.append(
            &gtk::Label::builder()
                .label(
                    review
                        .rating
                        .map(|value| format!("{:.1}★", normalize_rating(value as f64)))
                        .unwrap_or_else(|| "No rating".to_string()),
                )
                .halign(gtk::Align::End)
                .valign(gtk::Align::Start)
                .css_classes(vec!["monarch-review-rating".to_string()])
                .build(),
        );
        container.append(&header);

        if let Some(summary_text) = review
            .summary
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            container.append(
                &gtk::Label::builder()
                    .label(&summary_text)
                    .xalign(0.0)
                    .wrap(true)
                    .css_classes(vec!["monarch-meta".to_string()])
                    .build(),
            );
        }

        container.append(
            &gtk::Label::builder()
                .label(
                    review
                        .description
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .or_else(|| review.summary.clone())
                        .unwrap_or_else(|| "No written review provided.".to_string()),
                )
                .xalign(0.0)
                .wrap(true)
                .justify(gtk::Justification::Left)
                .css_classes(vec!["monarch-review-body".to_string()])
                .build(),
        );
        row.set_child(Some(&container));
        list.append(&row);
    }
}

/// Present "Write a review" dialog. On submit, calls catalog.submit_review (Supabase) then refreshes
/// reviews_list, reviews_summary, hero_rating_label, and current_app_rating.
fn present_write_review_dialog(
    parent: Option<&gtk::Window>,
    app_id: String,
    context: &AppContext,
    reviews_list: &gtk::ListBox,
    reviews_summary: &gtk::Label,
    hero_rating_label: &gtk::Label,
    current_app_rating: &std::cell::RefCell<Option<f64>>,
) {
    if app_id.trim().is_empty() {
        context.show_toast("Cannot add review: no app ID for this package.");
        return;
    }
    let dialog = gtk::Dialog::builder()
        .title("Write a review")
        .modal(true)
        .default_width(400)
        .build();
    if let Some(p) = parent {
        dialog.set_transient_for(Some(p));
    }
    dialog.add_button("Cancel", gtk::ResponseType::Cancel);
    dialog.add_button("Submit", gtk::ResponseType::Accept);
    dialog.set_default_response(gtk::ResponseType::Accept);

    let content = dialog.content_area();
    content.set_spacing(12);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let rating_adj = gtk::Adjustment::new(3.0, 1.0, 5.0, 1.0, 1.0, 0.0);
    let rating_spin = gtk::SpinButton::builder()
        .adjustment(&rating_adj)
        .numeric(true)
        .climb_rate(1.0)
        .digits(0)
        .build();
    let rating_row = adw::ActionRow::builder()
        .title("Rating")
        .activatable_widget(&rating_spin)
        .build();
    rating_row.add_suffix(&rating_spin);
    content.append(&rating_row);

    content.append(
        &gtk::Label::builder()
            .label("Comment")
            .xalign(0.0)
            .css_classes(vec!["title-5".to_string()])
            .halign(gtk::Align::Start)
            .build(),
    );
    let comment_buf = gtk::TextBuffer::new(None);
    let comment_view = gtk::TextView::builder()
        .buffer(&comment_buf)
        .wrap_mode(gtk::WrapMode::WordChar)
        .accepts_tab(false)
        .build();
    let comment_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_height(80)
        .child(&comment_view)
        .build();
    content.append(&comment_scroll);

    let name_row = adw::EntryRow::builder()
        .title("Display name (optional)")
        .build();
    content.append(&name_row);

    let catalog = context.catalog.clone();
    let runtime = context.runtime.clone();
    let context_for_toast = context.clone();
    let reviews_list = reviews_list.clone();
    let reviews_summary = reviews_summary.clone();
    let hero_rating_label = hero_rating_label.clone();
    let current_app_rating_cell = std::rc::Rc::new(std::cell::RefCell::new(current_app_rating.borrow().clone()));
    let app_id_for_submit = app_id.clone();
    dialog.connect_response(move |dialog, response| {
        if response != gtk::ResponseType::Accept {
            dialog.close();
            return;
        }
        let rating = rating_spin.value_as_int().clamp(1, 5) as u32;
        let (start, end) = comment_buf.bounds();
        let comment = comment_buf.text(&start, &end, false).to_string();
        let comment = comment.trim().to_string();
        if comment.is_empty() {
            return;
        }
        let user_display = name_row.text().trim().to_string();
        let user_display = if user_display.is_empty() {
            "MonARCH user".to_string()
        } else {
            user_display
        };
        dialog.close();
        let (tx, rx) = std::sync::mpsc::channel::<(Result<monarch_core::models::LocalReview, String>, Vec<PackageReview>)>();
        let catalog = catalog.clone();
        let runtime = runtime.clone();
        let app_id = app_id_for_submit.clone();
        let reviews_list = reviews_list.clone();
        let reviews_summary = reviews_summary.clone();
        let hero_rating_label = hero_rating_label.clone();
        let current_app_rating = current_app_rating_cell.clone();
        let settings_for_telemetry = context_for_toast.settings.clone();
        runtime.spawn(async move {
            let res: Result<monarch_core::models::LocalReview, String> = catalog
                .submit_review(&app_id, rating, "", comment, &user_display)
                .await;
            if res.is_ok() {
                crate::telemetry::track_event_async(
                    &settings_for_telemetry,
                    "review_submitted",
                    Some(serde_json::json!({
                        "package_name": app_id,
                        "rating": rating,
                        "source": "supabase"
                    })),
                )
                .await;
            }
            let reviews = if res.is_ok() {
                catalog.load_package_reviews(&app_id).await.unwrap_or_default()
            } else {
                Vec::new()
            };
            let _ = tx.send((res, reviews));
        });
        let context_poll = context_for_toast.clone();
        glib::source::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if let Ok((res, reviews)) = rx.try_recv() {
                match res {
                    Ok(_) => {
                        render_reviews(&reviews_list, &reviews_summary, &reviews);
                        let rated: Vec<f64> = reviews
                            .iter()
                            .filter_map(|r| r.rating.map(|x| normalize_rating(x as f64)))
                            .collect();
                        if !rated.is_empty() {
                            let avg = rated.iter().sum::<f64>() / rated.len() as f64;
                            hero_rating_label.set_label(&format!("{avg:.1} ★"));
                            hero_rating_label.set_visible(true);
                            current_app_rating.replace(Some(avg));
                        }
                        context_poll.show_toast("Review published.");
                    }
                    Err(e) => {
                        context_poll.show_toast(&format!("Could not publish review: {}", e));
                    }
                }
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    });

    dialog.present();
}

fn review_target_id(package: &Package) -> String {
    package
        .app_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| package.canonical_id.clone())
}

fn apply_package_icon(
    icon: &gtk::Picture,
    package: &Package,
    presentation: Option<&PackagePresentation>,
    context: &AppContext,
) {
    let preferred_icon = presentation
        .and_then(|presentation| presentation.icon.clone())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| package.icon.clone());
    set_picture_source(
        icon,
        context.runtime.clone(),
        preferred_icon,
        Some(arch_logo_fallback()),
    );
}

fn normalize_rating(value: f64) -> f64 {
    if value > 5.0 {
        value / 20.0
    } else {
        value
    }
}

fn escape_markup(text: &str) -> String {
    glib::markup_escape_text(text).to_string()
}

fn format_optional_size(size: Option<u64>) -> String {
    size.map(glib::format_size)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

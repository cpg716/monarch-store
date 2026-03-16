use crate::context::AppContext;
use crate::ui::components::package_card::{
    bind_compact_package_card_widget, build_compact_package_card_widget,
};
use crate::ui::controllers::catalog_controller::{CatalogController, CatalogMode};
use crate::ui::pages::package_detail::build_package_detail_page;
use adw::prelude::*;
use monarch_core::models::{HomeSnapshot, Package};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Shared refs for discovery search UI (stack, empty state, results flow, size groups).
#[derive(Clone)]
struct DiscoverySearchRefs {
    controller: CatalogController,
    stack: gtk::Stack,
    empty: adw::StatusPage,
    error_description: gtk::Label,
    results_flow: gtk::FlowBox,
    result_count_label: gtk::Label,
    results_heading: gtk::Box,
    results_heading_title: gtk::Label,
    results_heading_count: gtk::Label,
    navigation: adw::NavigationView,
    icon_group: gtk::SizeGroup,
    title_group: gtk::SizeGroup,
    desc_group: gtk::SizeGroup,
    card_root_group: gtk::SizeGroup,
}

type SourceFilterButtonList = Rc<RefCell<Vec<(Option<String>, gtk::Button)>>>;

pub struct DiscoveryPage {
    pub root: gtk::ScrolledWindow,
    pub search_entry: gtk::SearchEntry,
}

impl DiscoveryPage {
    pub fn new(context: AppContext, navigation: &adw::NavigationView) -> Self {
        let navigation = navigation.clone();
        let controller = CatalogController::new(context, CatalogMode::Discovery);
        let context = controller.context().clone();
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .css_classes(vec!["monarch-page".to_string()])
            .build();

        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text("Search Apps, Games, Software")
            .hexpand(true)
            .css_classes(vec![
                "monarch-home-search".to_string(),
                "monarch-search-2026".to_string(),
                "monarch-bazaar-search".to_string(),
            ])
            .build();

        let source_filters = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        let category_filters = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(false)
            .row_spacing(8)
            .column_spacing(8)
            .build();
        let installed_only = gtk::ToggleButton::builder()
            .label("Installed")
            .css_classes(vec![
                "monarch-filter-chip".to_string(),
                "monarch-search-pill".to_string(),
            ])
            .build();

        let result_count_label = gtk::Label::builder()
            .label("")
            .xalign(1.0)
            .css_classes(vec!["dim-label".to_string(), "monarch-search-result-count".to_string()])
            .build();
        result_count_label.set_visible(false);

        let source_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        source_row.append(&source_filters);
        source_row.append(&installed_only);
        let source_row_spacer = gtk::Box::builder().hexpand(true).build();
        source_row.append(&source_row_spacer);
        source_row.append(&result_count_label);

        /* Bazaar layout: search box then horizontal filter pills (search-pill style) */
        let filters = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .css_classes(vec![
                "monarch-search-filters".to_string(),
                "monarch-bazaar-search-layout".to_string(),
            ])
            .build();
        filters.append(&search_entry);
        source_row.add_css_class("monarch-bazaar-filter-pills");
        filters.append(&source_row);

        /* Bazaar-style results heading: "Search results" / "N Applications" when list visible (separate label to avoid double-parent) */
        let results_heading = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();
        let results_heading_title = gtk::Label::builder()
            .label("Search results")
            .xalign(0.0)
            .css_classes(vec!["title-3".to_string()])
            .build();
        let results_heading_count = gtk::Label::builder()
            .label("")
            .xalign(1.0)
            .css_classes(vec!["dim-label".to_string(), "monarch-search-result-count".to_string()])
            .build();
        results_heading.append(&results_heading_title);
        results_heading.append(&gtk::Box::builder().hexpand(true).build());
        results_heading.append(&results_heading_count);
        results_heading.set_visible(false);

        let suggestions_strip = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        let categories_strip = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(false)
            .row_spacing(10)
            .column_spacing(10)
            .build();
        let blank = build_blank_state(&suggestions_strip, &categories_strip);

        let results_flow = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .row_spacing(6)
            .column_spacing(6)
            .min_children_per_line(3)
            .max_children_per_line(3)
            .build();
        results_flow.add_css_class("monarch-search-grid");

        let icon_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Both);
        let title_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Vertical);
        let desc_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Vertical);
        let card_root_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Both);

        let bin = adw::BreakpointBin::new();
        let breakpoint = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("(max-width: 1040px)")
                .expect("Invalid breakpoint condition"),
        );
        breakpoint.add_setter(
            &results_flow,
            "min-children-per-line",
            Some(&2u32.to_value()),
        );
        breakpoint.add_setter(
            &results_flow,
            "max-children-per-line",
            Some(&2u32.to_value()),
        );
        bin.add_breakpoint(breakpoint);
        bin.set_child(Some(&results_flow));
        bin.set_vexpand(false);
        /* Satisfy AdwBreakpointBin minimum size so it does not warn */
        bin.set_width_request(1);
        bin.set_height_request(1);

        let results_scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&bin)
            .css_classes(vec!["monarch-discovery-grid".to_string()])
            .build();
        results_scroller.set_kinetic_scrolling(true);

        let loading = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .css_classes(vec!["monarch-panel".to_string()])
            .build();
        loading.append(&build_skeleton_panel(8));
        let empty = adw::StatusPage::builder()
            .icon_name("view-list-symbolic")
            .title("No packages matched")
            .description("Try another query or broaden the selected source/category filter.")
            .css_classes(vec!["monarch-empty".to_string()])
            .build();
        let error = adw::StatusPage::builder()
            .icon_name("dialog-error-symbolic")
            .title("Search is unavailable")
            .description("The package registry was unavailable.")
            .css_classes(vec!["monarch-empty".to_string()])
            .build();
        let error_description = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .selectable(true)
            .css_classes(vec!["dim-label".to_string()])
            .build();
        error.set_child(Some(&error_description));

        let stack = gtk::Stack::new();
        stack.add_named(&blank, Some("blank"));
        stack.add_named(&loading, Some("loading"));
        stack.add_named(&results_scroller, Some("list"));
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&error, Some("error"));
        stack.set_visible_child_name("blank");

        let request_generation = Rc::new(Cell::new(0u64));
        let debounce_generation = Rc::new(Cell::new(0u64));
        let active_category = Rc::new(RefCell::new(None::<String>));
        let category_buttons = Rc::new(RefCell::new(Vec::<(String, gtk::Button)>::new()));

        let search_refs = DiscoverySearchRefs {
            controller: controller.clone(),
            stack: stack.clone(),
            empty: empty.clone(),
            error_description: error_description.clone(),
            results_flow: results_flow.clone(),
            result_count_label: result_count_label.clone(),
            results_heading: results_heading.clone(),
            results_heading_title: results_heading_title.clone(),
            results_heading_count: results_heading_count.clone(),
            navigation: navigation.clone(),
            icon_group: icon_group.clone(),
            title_group: title_group.clone(),
            desc_group: desc_group.clone(),
            card_root_group: card_root_group.clone(),
        };

        let source_buttons = build_source_filter_buttons(&search_refs, &source_filters, request_generation.clone());
        sync_source_filter_buttons(&source_buttons, None);

        search_entry.connect_search_changed({
            let search_refs = search_refs.clone();
            let request_generation = request_generation.clone();
            let debounce_generation = debounce_generation.clone();
            move |entry| {
                let generation = debounce_generation.get().wrapping_add(1);
                debounce_generation.set(generation);

                let text = entry.text().to_string();
                let timer = {
                    let search_refs = search_refs.clone();
                    let request_generation = request_generation.clone();
                    let debounce_generation = debounce_generation.clone();
                    move || {
                        if debounce_generation.get() != generation {
                            return glib::ControlFlow::Break;
                        }
                        trigger_search(&search_refs, request_generation.clone(), text.clone());
                        glib::ControlFlow::Break
                    }
                };
                let _ = glib::timeout_add_local(std::time::Duration::from_millis(180), timer);
            }
        });

        search_entry.connect_activate({
            let search_refs = search_refs.clone();
            let request_generation = request_generation.clone();
            move |entry| {
                let text = entry.text().to_string();
                trigger_search(&search_refs, request_generation.clone(), text);
            }
        });

        installed_only.connect_toggled({
            let search_refs = search_refs.clone();
            let request_generation = request_generation.clone();
            move |button| {
                search_refs.controller.set_installed_only(button.is_active());
                let query = search_refs.controller.search_query();
                if should_show_blank(&search_refs.controller, &query) {
                    search_refs.stack.set_visible_child_name("blank");
                } else {
                    trigger_search(&search_refs, request_generation.clone(), query);
                }
            }
        });

        populate_landing_state(
            context.clone(),
            &suggestions_strip,
            &categories_strip,
            &category_filters,
            &search_entry,
            search_refs.clone(),
            request_generation.clone(),
            active_category.clone(),
            category_buttons.clone(),
        );

        let last_refresh_epoch = Rc::new(Cell::new(context.refresh_epoch()));
        let last_refresh_epoch_for_timeout = last_refresh_epoch.clone();
        let search_refs_for_timeout = search_refs.clone();
        let request_generation_for_timeout = request_generation.clone();
        let context_for_timeout = context.clone();
        glib::source::timeout_add_local(std::time::Duration::from_millis(500), move || {
            let current = context_for_timeout.refresh_epoch();
            if current != last_refresh_epoch_for_timeout.get() {
                last_refresh_epoch_for_timeout.set(current);
                let query = search_refs_for_timeout.controller.search_query();
                if should_show_blank(&search_refs_for_timeout.controller, &query) {
                    search_refs_for_timeout.stack.set_visible_child_name("blank");
                } else {
                    trigger_search(&search_refs_for_timeout, request_generation_for_timeout.clone(), query);
                }
            }
            glib::ControlFlow::Continue
        });

        content.set_vexpand(false);
        content.append(&filters);
        content.append(&results_heading);
        content.append(&stack);
        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&content)
            .build();
        root.set_kinetic_scrolling(true);

        Self {
            root,
            search_entry,
        }
    }
}

fn build_blank_state(suggestions_strip: &gtk::Box, categories_strip: &gtk::FlowBox) -> gtk::Box {
    let blank = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .css_classes(vec![
            "monarch-panel".to_string(),
            "monarch-search-blank".to_string(),
        ])
        .build();
    blank.append(
        &gtk::Label::builder()
            .label("Search apps, package names, or tasks")
            .xalign(0.0)
            .css_classes(vec!["title-3".to_string()])
            .build(),
    );
    blank.append(
        &gtk::Label::builder()
            .label("Start with a search, or jump into a category.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["dim-label".to_string()])
            .build(),
    );
    blank.append(suggestions_strip);
    blank.append(
        &gtk::Label::builder()
            .label("Browse Categories")
            .xalign(0.0)
            .css_classes(vec!["title-4".to_string()])
            .build(),
    );
    blank.append(categories_strip);
    blank
}

fn build_source_filter_buttons(
    refs: &DiscoverySearchRefs,
    container: &gtk::Box,
    request_generation: Rc<Cell<u64>>,
) -> SourceFilterButtonList {
    let buttons = Rc::new(RefCell::new(Vec::<(Option<String>, gtk::Button)>::new()));

    for (id, label) in [
        (None, "All"),
        (Some("repo".to_string()), "Official"),
        (Some("aur".to_string()), "AUR"),
        (Some("flatpak".to_string()), "Flatpak"),
    ] {
        let button = gtk::Button::builder()
            .label(label)
            .css_classes(vec![
                "flat".to_string(),
                "monarch-filter-pill".to_string(),
                "monarch-search-pill".to_string(),
            ])
            .build();

        button.connect_clicked({
            let refs = refs.clone();
            let request_generation = request_generation.clone();
            let target = id.clone();
            move |_| {
                refs.controller.set_source_filter(target.clone());
                let query = refs.controller.search_query();
                if should_show_blank(&refs.controller, &query) {
                    refs.stack.set_visible_child_name("blank");
                } else {
                    trigger_search(&refs, request_generation.clone(), query);
                }
            }
        });
        buttons.borrow_mut().push((id, button.clone()));
        container.append(&button);
    }
    buttons
}

fn sync_source_filter_buttons(buttons: &SourceFilterButtonList, active: Option<&str>) {
    for (id, button) in buttons.borrow().iter() {
        if id.as_deref() == active || (active.is_none() && id.is_none()) {
            button.add_css_class("is-active");
        } else {
            button.remove_css_class("is-active");
        }
    }
}

fn sync_category_filter_buttons(
    buttons: &Rc<RefCell<Vec<(String, gtk::Button)>>>,
    active: Option<&str>,
) {
    for (category, button) in buttons.borrow().iter() {
        if active.is_some_and(|active| active.eq_ignore_ascii_case(category)) {
            button.add_css_class("is-active");
        } else {
            button.remove_css_class("is-active");
        }
    }
}

#[allow(clippy::too_many_arguments)] // TODO: group landing UI refs into struct
fn populate_landing_state(
    context: AppContext,
    suggestions_strip: &gtk::Box,
    categories_strip: &gtk::FlowBox,
    category_filters: &gtk::FlowBox,
    search_entry: &gtk::SearchEntry,
    search_refs: DiscoverySearchRefs,
    request_generation: Rc<Cell<u64>>,
    active_category: Rc<RefCell<Option<String>>>,
    category_buttons: Rc<RefCell<Vec<(String, gtk::Button)>>>,
) {
    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let context = context.clone();
        async move {
            let _ = sender.send(context.fetch_home_snapshot().await);
        }
    });

    let suggestions_strip = suggestions_strip.clone();
    let categories_strip = categories_strip.clone();
    let category_filters = category_filters.clone();
    let search_entry = search_entry.clone();
    let search_refs = search_refs.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || match receiver
        .try_recv()
    {
        Ok(Ok(snapshot)) => {
            render_landing_snapshot(
                snapshot,
                &suggestions_strip,
                &categories_strip,
                &category_filters,
                &search_entry,
                &search_refs,
                request_generation.clone(),
                active_category.clone(),
                category_buttons.clone(),
            );
            glib::ControlFlow::Break
        }
        Ok(Err(_)) => glib::ControlFlow::Break,
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
    });
}

#[allow(clippy::too_many_arguments)] // TODO: group landing UI refs into struct
/// Same Bazaar-style gradient classes as HOME so Search category tiles and pills match.
const BAZAAR_GRADIENT_CLASSES: &[&str] = &[
    "monarch-category-trending",
    "monarch-category-popular",
    "monarch-category-audiovideo",
    "monarch-category-game",
    "monarch-category-network",
    "monarch-category-education",
];

fn render_landing_snapshot(
    snapshot: HomeSnapshot,
    suggestions_strip: &gtk::Box,
    categories_strip: &gtk::FlowBox,
    category_filters: &gtk::FlowBox,
    search_entry: &gtk::SearchEntry,
    search_refs: &DiscoverySearchRefs,
    request_generation: Rc<Cell<u64>>,
    active_category: Rc<RefCell<Option<String>>>,
    category_buttons: Rc<RefCell<Vec<(String, gtk::Button)>>>,
) {
    while let Some(child) = suggestions_strip.first_child() {
        suggestions_strip.remove(&child);
    }
    clear_flowbox(categories_strip);
    clear_flowbox(category_filters);
    category_buttons.borrow_mut().clear();

    for suggestion in snapshot.suggested_searches {
        let button = gtk::Button::builder()
            .label(&suggestion)
            .css_classes(vec!["flat".to_string(), "monarch-filter-chip".to_string()])
            .build();
        button.connect_clicked({
            let search_entry = search_entry.clone();
            move |_| {
                search_entry.set_text(&suggestion);
                search_entry.grab_focus();
            }
        });
        suggestions_strip.append(&button);
    }

    for (i, category) in snapshot.categories.iter().enumerate() {
        let gradient_class = BAZAAR_GRADIENT_CLASSES[i % BAZAAR_GRADIENT_CLASSES.len()];
        let filter_button = gtk::Button::builder()
            .label(category)
            .css_classes(vec![
                "flat".to_string(),
                "monarch-filter-chip".to_string(),
                "monarch-category-tile".to_string(),
                gradient_class.to_string(),
            ])
            .build();
        filter_button.connect_clicked({
            let search_refs = search_refs.clone();
            let request_generation = request_generation.clone();
            let active_category = active_category.clone();
            let category_buttons = category_buttons.clone();
            let category_name = category.clone();
            move |_| {
                let next = if active_category.borrow().as_deref() == Some(category_name.as_str()) {
                    None
                } else {
                    Some(category_name.clone())
                };
                active_category.replace(next.clone());
                search_refs.controller.set_category_filter(next.clone());
                sync_category_filter_buttons(&category_buttons, next.as_deref());
                let query = search_refs.controller.search_query();
                if should_show_blank(&search_refs.controller, &query) {
                    search_refs.stack.set_visible_child_name("blank");
                } else {
                    trigger_search(&search_refs, request_generation.clone(), query);
                }
            }
        });
        category_buttons
            .borrow_mut()
            .push((category.clone(), filter_button.clone()));
        category_filters.insert(&filter_button, -1);

        let tile = gtk::Button::builder()
            .label(category)
            .css_classes(vec![
                "flat".to_string(),
                "monarch-category-tile".to_string(),
                gradient_class.to_string(),
            ])
            .build();
        tile.connect_clicked({
            let search_refs = search_refs.clone();
            let request_generation = request_generation.clone();
            let active_category = active_category.clone();
            let category_buttons = category_buttons.clone();
            let category_name = category.clone();
            move |_| {
                active_category.replace(Some(category_name.clone()));
                search_refs.controller.set_category_filter(Some(category_name.clone()));
                sync_category_filter_buttons(&category_buttons, Some(category_name.as_str()));
                trigger_search(&search_refs, request_generation.clone(), String::new());
            }
        });
        categories_strip.insert(&tile, -1);
    }
    sync_category_filter_buttons(&category_buttons, active_category.borrow().as_deref());
}

fn should_show_blank(controller: &CatalogController, query: &str) -> bool {
    query.trim().is_empty()
        && controller.source_filter().is_none()
        && controller.category_filter().is_none()
        && !controller.installed_only()
}

pub(crate) fn build_skeleton_panel(rows: usize) -> gtk::Box {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-skeleton-panel".to_string()])
        .build();
    for _ in 0..rows {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(14)
            .css_classes(vec![
                "monarch-package-row".to_string(),
                "monarch-skeleton-row".to_string(),
            ])
            .build();
        let icon = gtk::Box::builder()
            .width_request(40)
            .height_request(40)
            .css_classes(vec![
                "monarch-skeleton-block".to_string(),
                "monarch-skeleton-icon".to_string(),
            ])
            .build();
        let lines = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .hexpand(true)
            .build();
        let line1 = gtk::Box::builder()
            .height_request(16)
            .width_request(260)
            .css_classes(vec!["monarch-skeleton-block".to_string()])
            .build();
        let line2 = gtk::Box::builder()
            .height_request(12)
            .width_request(420)
            .css_classes(vec!["monarch-skeleton-block".to_string()])
            .build();
        lines.append(&line1);
        lines.append(&line2);
        row.append(&icon);
        row.append(&lines);
        container.append(&row);
    }
    animate_skeleton_panel(&container);
    container
}

fn animate_skeleton_panel(container: &gtk::Box) {
    let panel = container.downgrade();
    let bright = Rc::new(Cell::new(false));
    let bright_for_timeout = bright.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(700), move || {
        let Some(panel) = panel.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let next = !bright_for_timeout.get();
        bright_for_timeout.set(next);
        if next {
            panel.add_css_class("monarch-skeleton-bright");
        } else {
            panel.remove_css_class("monarch-skeleton-bright");
        }
        glib::ControlFlow::Continue
    });
}

fn trigger_search(
    refs: &DiscoverySearchRefs,
    request_generation: Rc<Cell<u64>>,
    query: String,
) {
    let generation = request_generation.get().wrapping_add(1);
    request_generation.set(generation);
    refs.stack.set_visible_child_name("loading");

    let refs_for_result = refs.clone();
    let icon_group = refs.icon_group.clone();
    let title_group = refs.title_group.clone();
    let desc_group = refs.desc_group.clone();
    let card_root_group = refs.card_root_group.clone();
    refs.controller.search_async(query, move |result| {
        if request_generation.get() != generation {
            return;
        }

        match result {
            Ok(packages) if packages.is_empty() => {
                clear_flowbox(&refs_for_result.results_flow);
                refs_for_result.result_count_label.set_visible(false);
                refs_for_result.results_heading.set_visible(false);
                apply_empty_state(
                    &refs_for_result.empty,
                    &refs_for_result.controller.search_query(),
                    refs_for_result.controller.source_filter().as_deref(),
                );
                refs_for_result.stack.set_visible_child_name("empty");
            }
            Ok(packages) => {
                refs_for_result.controller.replace_packages(packages.clone());
                let count = packages.len();
                refs_for_result.results_heading_title.set_label("Search results");
                let count_text = format!(
                    "{} {}",
                    count,
                    if count == 1 { "application" } else { "applications" }
                );
                refs_for_result.result_count_label.set_label(&count_text);
                refs_for_result.results_heading_count.set_label(&count_text);
                refs_for_result.result_count_label.set_visible(true);
                refs_for_result.results_heading_count.set_visible(true);
                refs_for_result.results_heading.set_visible(true);
                render_discovery_grid(
                    &refs_for_result.results_flow,
                    &refs_for_result.navigation,
                    &refs_for_result.controller,
                    &packages,
                    &icon_group,
                    &title_group,
                    &desc_group,
                    &card_root_group,
                );
                refs_for_result.stack.set_visible_child_name("list");
            }
            Err(error) => {
                clear_flowbox(&refs_for_result.results_flow);
                refs_for_result.result_count_label.set_visible(false);
                refs_for_result.results_heading.set_visible(false);
                refs_for_result.error_description.set_label(&error);
                refs_for_result.stack.set_visible_child_name("error");
            }
        }
    });
}

#[allow(clippy::too_many_arguments)] // size groups + flowbox/nav/controller/packages
fn render_discovery_grid(
    flowbox: &gtk::FlowBox,
    navigation: &adw::NavigationView,
    controller: &CatalogController,
    packages: &[Package],
    icon_group: &gtk::SizeGroup,
    title_group: &gtk::SizeGroup,
    desc_group: &gtk::SizeGroup,
    card_root_group: &gtk::SizeGroup,
) {
    clear_flowbox(flowbox);
    for package in packages {
        let card = build_discovery_card(
            navigation,
            controller.context(),
            package,
            Some(icon_group),
            Some(title_group),
            Some(desc_group),
            Some(card_root_group),
        );
        flowbox.insert(&card, -1);
    }
}

fn clear_flowbox(flowbox: &gtk::FlowBox) {
    while let Some(child) = flowbox.first_child() {
        flowbox.remove(&child);
    }
}

#[allow(clippy::too_many_arguments)] // size groups + nav/context/package
fn build_discovery_card(
    navigation: &adw::NavigationView,
    context: &AppContext,
    package: &Package,
    icon_group: Option<&gtk::SizeGroup>,
    title_group: Option<&gtk::SizeGroup>,
    desc_group: Option<&gtk::SizeGroup>,
    card_root_group: Option<&gtk::SizeGroup>,
) -> gtk::Widget {
    let card = build_compact_package_card_widget(context.clone());
    bind_compact_package_card_widget(&card, package, context, icon_group, title_group, desc_group, card_root_group);
    card.set_halign(gtk::Align::Start);
    card.set_valign(gtk::Align::Start);

    let click = gtk::GestureClick::new();
    click.connect_released({
        let navigation = navigation.clone();
        let context = context.clone();
        let package = package.clone();
        move |_, _, _, _| {
            navigation.push(&build_package_detail_page(
                context.clone(),
                &navigation,
                &package,
            ));
        }
    });
    card.add_controller(click);
    card.upcast()
}

fn apply_empty_state(empty: &adw::StatusPage, query: &str, source_filter: Option<&str>) {
    let normalized_query = query.trim();
    let source_label = match source_filter.unwrap_or("all") {
        "native" => "native repository packages",
        "repo" => "repository packages",
        "aur" => "AUR packages",
        "flatpak" => "Flatpak apps",
        "chaotic" | "chaotic-aur" => "Chaotic-AUR packages",
        _ => "apps",
    };
    let dynamic_title;
    let title = if normalized_query.is_empty() {
        "No packages matched this filter"
    } else {
        dynamic_title = format!("No results for “{normalized_query}”");
        dynamic_title.as_str()
    };
    empty.set_title(title);
    empty.set_description(Some(&format!(
        "MonARCH could not find {source_label} matching the current search."
    )));
}

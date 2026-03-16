use crate::context::AppContext;
use crate::ui::auth::{ensure_session_auth, parent_window_for};
use crate::ui::components::operation_dialog::{present_operation_dialog, OperationDialogOptions};
use crate::ui::pages::discovery::build_skeleton_panel;
use crate::ui::pages::package_detail::{build_package_detail_page, default_package_source};
use adw::prelude::*;
use monarch_core::models::Package;
use std::cell::Cell;
use std::rc::Rc;

pub fn build_installed_page(
    context: AppContext,
    navigation: &adw::NavigationView,
    view_stack: gtk::Stack,
) -> gtk::Widget {
    let navigation = navigation.clone();
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
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
            .label("Library")
            .xalign(0.0)
            .css_classes(vec!["monarch-hero-title".to_string()])
            .build(),
    );
    hero.append(
        &gtk::Label::builder()
            .label("Installed software across native packages and Flatpak, with fast open and remove actions.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["monarch-hero-copy".to_string()])
            .build(),
    );

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Search installed apps")
        .css_classes(vec!["monarch-home-search".to_string(), "monarch-bazaar-search".to_string()])
        .build();

    /* Bazaar-style Pending Updates strip */
    let pending_count_label = gtk::Label::builder()
        .label("0 Available Updates")
        .xalign(0.0)
        .css_classes(vec!["title-5".to_string()])
        .build();
    let update_all_button = gtk::Button::builder()
        .label("Update All")
        .css_classes(vec!["suggested-action".to_string(), "monarch-bazaar-update-all".to_string()])
        .build();
    update_all_button.connect_clicked({
        let view_stack = view_stack.clone();
        move |_| {
            view_stack.set_visible_child_name("updates");
        }
    });
    let pending_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .css_classes(vec!["monarch-pending-updates-row".to_string()])
        .build();
    pending_row.append(&pending_count_label);
    pending_row.append(
        &gtk::Box::builder()
            .hexpand(true)
            .build(),
    );
    pending_row.append(&update_all_button);

    /* Bazaar .update-card: rounded strip for Pending Updates */
    let pending_section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .css_classes(vec!["monarch-update-card".to_string()])
        .build();
    pending_section.append(
        &gtk::Label::builder()
            .label("Pending Updates")
            .xalign(0.0)
            .css_classes(vec!["title-4".to_string()])
            .build(),
    );
    pending_section.append(&pending_row);

    let installed_heading = gtk::Label::builder()
        .label("Installed Apps")
        .xalign(0.0)
        .css_classes(vec!["title-4".to_string()])
        .build();

    let listbox = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(vec![
            "boxed-list".to_string(),
            "monarch-soft-list".to_string(),
            "monarch-library-list".to_string(),
            "monarch-installed-list-view".to_string(),
        ])
        .build();
    listbox.set_vexpand(false);
    let scroller = gtk::ScrolledWindow::builder()
        .child(&listbox)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    scroller.set_kinetic_scrolling(true);

    let loading = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    loading.append(&build_skeleton_panel(6));
    let empty = adw::StatusPage::builder()
        .icon_name("package-x-generic-symbolic")
        .title("No installed packages in catalog")
        .description("Installed packages will appear here once the registry is hydrated.")
        .build();
    let error = adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title("Installed packages failed to load")
        .description("The local registry could not be read.")
        .build();

    let stack = gtk::Stack::new();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&empty, Some("empty"));
    stack.add_named(&error, Some("error"));
    stack.add_named(&scroller, Some("list"));
    stack.set_visible_child_name("loading");

    page.append(&hero);
    page.append(&search_entry);
    page.append(&pending_section);
    page.append(&installed_heading);
    page.append(&stack);

    /* Load pending update count (Bazaar-style strip); send count to main thread via channel */
    {
        let catalog = context.catalog.clone();
        let pending_count_label = pending_count_label.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel::<usize>(1);
        context.runtime.spawn(async move {
            let count = match catalog.load_updates().await {
                Ok(snap) => snap.items.len(),
                Err(_) => 0,
            };
            let _ = tx.send(count);
        });
        let pending_count_label_for_timeout = pending_count_label.clone();
        glib::source::timeout_add_local(std::time::Duration::from_millis(150), move || {
            match rx.try_recv() {
                Ok(count) => {
                    let text = format!("{} Available Update{}", count, if count == 1 { "" } else { "s" });
                    pending_count_label_for_timeout.set_label(&text);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
    }

    let packages = Rc::new(std::cell::RefCell::new(Vec::<Package>::new()));
    search_entry.connect_search_changed({
        let packages = packages.clone();
        let listbox = listbox.clone();
        let navigation = navigation.clone();
        let context = context.clone();
        move |entry| {
            render_installed_list(
                &listbox,
                &navigation,
                &context,
                &packages.borrow(),
                entry.text().as_str(),
            );
        }
    });

    load_installed_packages(
        context.clone(),
        navigation.clone(),
        &listbox,
        &stack,
        &packages,
        "",
        true,
    );

    let last_refresh_epoch = Rc::new(Cell::new(context.refresh_epoch()));
    let last_refresh_epoch_for_timeout = last_refresh_epoch.clone();
    let context_for_timeout = context.clone();
    let navigation_for_timeout = navigation.clone();
    let listbox_for_timeout = listbox.clone();
    let stack_for_timeout = stack.clone();
    let packages_for_timeout = packages.clone();
    let search_entry_for_timeout = search_entry.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(500), move || {
        let current = context_for_timeout.refresh_epoch();
        if current != last_refresh_epoch_for_timeout.get() {
            last_refresh_epoch_for_timeout.set(current);
            load_installed_packages(
                context_for_timeout.clone(),
                navigation_for_timeout.clone(),
                &listbox_for_timeout,
                &stack_for_timeout,
                &packages_for_timeout,
                search_entry_for_timeout.text().as_str(),
                false,
            );
        }
        glib::ControlFlow::Continue
    });

    page.upcast()
}

fn load_installed_packages(
    context: AppContext,
    navigation: adw::NavigationView,
    listbox: &gtk::ListBox,
    stack: &gtk::Stack,
    packages: &Rc<std::cell::RefCell<Vec<Package>>>,
    query: &str,
    show_loading: bool,
) {
    if show_loading {
        stack.set_visible_child_name("loading");
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let catalog = context.catalog.clone();
        async move {
            let _ = sender.send(catalog.load_installed().await);
        }
    });

    let listbox_for_result = listbox.clone();
    let stack_for_result = stack.clone();
    let packages_for_result = packages.clone();
    let query = query.to_string();
    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
        match receiver.try_recv() {
            Ok(Ok(items)) if items.is_empty() => {
                packages_for_result.borrow_mut().clear();
                clear_listbox(&listbox_for_result);
                stack_for_result.set_visible_child_name("empty");
                glib::ControlFlow::Break
            }
            Ok(Ok(items)) => {
                packages_for_result.replace(items);
                render_installed_list(
                    &listbox_for_result,
                    &navigation,
                    &context,
                    &packages_for_result.borrow(),
                    &query,
                );
                stack_for_result.set_visible_child_name("list");
                glib::ControlFlow::Break
            }
            Ok(Err(_)) => {
                stack_for_result.set_visible_child_name("error");
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        }
    });
}

fn render_installed_list(
    listbox: &gtk::ListBox,
    navigation: &adw::NavigationView,
    context: &AppContext,
    packages: &[Package],
    query: &str,
) {
    clear_listbox(listbox);
    let query = query.trim().to_lowercase();
    for package in packages.iter().filter(|package| {
        if query.is_empty() {
            return true;
        }
        let haystack = format!(
            "{} {} {} {}",
            package.effective_title(),
            package.name,
            package.description,
            package.source.label
        )
        .to_lowercase();
        haystack.contains(&query)
    }) {
        listbox.append(&build_library_row(navigation, context, package));
    }
}

fn build_library_row(
    navigation: &adw::NavigationView,
    context: &AppContext,
    package: &Package,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("monarch-library-row");

    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(14)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .build();
    let icon = gtk::Picture::builder()
        .width_request(52)
        .height_request(52)
        .can_shrink(true)
        .build();
    icon.set_paintable(Some(crate::ui::media::placeholder_texture()));
    crate::ui::media::set_picture_source(
        &icon,
        context.runtime.clone(),
        package.icon.clone(),
        None,
    );
    let icon_wrap = gtk::Box::builder()
        .css_classes(vec!["monarch-store-card-icon".to_string()])
        .build();
    icon_wrap.append(&icon);

    let copy = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .hexpand(true)
        .build();
    copy.append(
        &gtk::Label::builder()
            .label(package.effective_title())
            .xalign(0.0)
            .css_classes(vec!["title-5".to_string()])
            .build(),
    );
    let version_size_label = format!("{} • {}", package.version, package.source.label);
    copy.append(
        &gtk::Label::builder()
            .label(&version_size_label)
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["dim-label".to_string()])
            .build(),
    );

    /* Bazaar-style row actions: detail arrow, trash (remove), heart (favorite) */
    let actions = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .css_classes(vec!["monarch-library-row-actions".to_string()])
        .build();
    let detail_button = gtk::Button::builder()
        .icon_name("go-next-symbolic")
        .has_frame(false)
        .tooltip_text("View details")
        .css_classes(vec!["monarch-library-action".to_string()])
        .build();
    detail_button.connect_clicked({
        let navigation = navigation.clone();
        let context = context.clone();
        let package = package.clone();
        move |_| {
            navigation.push(&build_package_detail_page(context.clone(), &navigation, &package));
        }
    });
    let remove_button = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .has_frame(false)
        .tooltip_text("Uninstall")
        .css_classes(vec!["monarch-library-action".to_string(), "monarch-library-remove".to_string()])
        .build();
    remove_button.connect_clicked({
        let context = context.clone();
        let remove_button = remove_button.clone();
        let package = package.clone();
        move |_| {
            handle_remove_action(context.clone(), &remove_button, package.clone());
        }
    });
    let is_favorite = context
        .favorites
        .contains(&package.canonical_id)
        .unwrap_or(false);
    let favorite_button = gtk::Button::builder()
        .icon_name(if is_favorite {
            "starred-symbolic"
        } else {
            "non-starred-symbolic"
        })
        .has_frame(false)
        .tooltip_text(if is_favorite {
            "Remove from favorites"
        } else {
            "Add to favorites"
        })
        .css_classes(vec!["monarch-library-action".to_string(), "monarch-library-favorite".to_string()])
        .build();
    if is_favorite {
        favorite_button.add_css_class("is-favorite");
    }
    favorite_button.connect_clicked({
        let context = context.clone();
        let favorite_button = favorite_button.clone();
        let package = package.clone();
        move |_| {
            let _ = context.favorites.toggle(&package.canonical_id);
            let is_fav = context.favorites.contains(&package.canonical_id).unwrap_or(false);
            favorite_button.set_icon_name(if is_fav { "starred-symbolic" } else { "non-starred-symbolic" });
            favorite_button.set_tooltip_text(Some(if is_fav { "Remove from favorites" } else { "Add to favorites" }));
            if is_fav {
                favorite_button.add_css_class("is-favorite");
            } else {
                favorite_button.remove_css_class("is-favorite");
            }
        }
    });
    actions.append(&detail_button);
    actions.append(&remove_button);
    actions.append(&favorite_button);

    container.append(&icon_wrap);
    container.append(&copy);
    container.append(&actions);
    row.set_child(Some(&container));

    let click = gtk::GestureClick::new();
    click.connect_released({
        let navigation = navigation.clone();
        let context = context.clone();
        let package = package.clone();
        move |_, _, _, _| {
            navigation.push(&build_package_detail_page(context.clone(), &navigation, &package));
        }
    });
    row.add_controller(click);
    row
}

fn handle_remove_action(context: AppContext, button: &gtk::Button, package: Package) {
    let source = default_package_source(&package);
    if let Err(error) = ensure_session_auth(&context, parent_window_for(button).as_ref(), false) {
        button.set_tooltip_text(Some(&error));
        return;
    }

    button.set_sensitive(false);
    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let catalog = context.catalog.clone();
        let package = package.clone();
        let source = source.clone();
        async move {
            let _ = sender.send(catalog.remove_package_for_source_stream(package, source).await);
        }
    });

    let context_for_dialog = context.clone();
    let button_for_result = button.clone();
    let package_title = package.effective_title();
    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
        match receiver.try_recv() {
            Ok(Ok(stream)) => {
                let options = OperationDialogOptions {
                    is_uninstall: true,
                    success_display_name: Some(package_title.clone()),
                    on_launch: None,
                };
                present_operation_dialog(
                    context_for_dialog.clone(),
                    &format!("Removing {package_title}"),
                    "Removing package through monarch-helper...",
                    stream,
                    {
                        let button_for_result = button_for_result.clone();
                        let context_for_finish = context_for_dialog.clone();
                        move |result| {
                            if result.is_ok() {
                                context_for_finish.mark_catalog_dirty();
                            } else if let Err(error) = result {
                                button_for_result.set_tooltip_text(Some(&error));
                            }
                            button_for_result.set_sensitive(true);
                        }
                    },
                    options,
                );
                glib::ControlFlow::Break
            }
            Ok(Err(error)) => {
                button_for_result.set_tooltip_text(Some(&error));
                button_for_result.set_sensitive(true);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                button_for_result.set_sensitive(true);
                glib::ControlFlow::Break
            }
        }
    });
}

fn clear_listbox(listbox: &gtk::ListBox) {
    while let Some(child) = listbox.first_child() {
        listbox.remove(&child);
    }
}

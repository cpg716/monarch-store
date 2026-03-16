use crate::context::AppContext;
use crate::ui::auth::{ensure_session_auth, parent_window_for};
use crate::ui::components::operation_dialog::{present_operation_dialog, OperationDialogOptions};
use crate::ui::pages::package_detail::build_package_detail_page;
use adw::prelude::*;
use monarch_core::models::{UpdateSnapshot, UpdateSnapshotItem};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub fn build_updates_page(context: AppContext, navigation: &adw::NavigationView) -> gtk::Widget {
    let packages = Rc::new(RefCell::new(Vec::new()));
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .css_classes(vec!["monarch-page".to_string()])
        .vexpand(true)
        .build();

    let summary_card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .css_classes(vec!["monarch-hero".to_string()])
        .build();
    let summary_title = gtk::Label::builder()
        .label("Stay current without partial-upgrade mistakes")
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["monarch-hero-title".to_string()])
        .build();
    let summary_copy = gtk::Label::builder()
        .label("Updates are discovered from the host system, then executed through the preserved helper path so GUI convenience never bypasses Iron Core safety.")
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["monarch-hero-copy".to_string()])
        .build();
    summary_card.append(&summary_title);
    summary_card.append(&summary_copy);

    let source_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    let update_button = gtk::Button::builder()
        .label("Update System")
        .css_classes(vec!["suggested-action".to_string()])
        .halign(gtk::Align::Start)
        .build();
    let update_status = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    let advisory_banner = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["monarch-chip".to_string()])
        .visible(false)
        .build();

    let listbox = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(vec!["boxed-list".to_string(), "monarch-soft-list".to_string()])
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
    loading.append(&super::discovery::build_skeleton_panel(6));
    let empty = adw::StatusPage::builder()
        .icon_name("software-update-available-symbolic")
        .title("System is up to date")
        .description("No pending repository updates were reported.")
        .build();
    let error = adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title("Update check failed")
        .description("The update snapshot could not be collected.")
        .build();
    let error_detail = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .selectable(true)
        .css_classes(vec!["dim-label".to_string()])
        .build();
    error.set_child(Some(&error_detail));

    let ready = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .vexpand(true)
        .build();
    ready.append(&advisory_banner);
    ready.append(&update_button);
    ready.append(&update_status);
    ready.append(&source_bar);
    ready.append(&scroller);

    let stack = gtk::Stack::new();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&ready, Some("ready"));
    stack.add_named(&empty, Some("empty"));
    stack.add_named(&error, Some("error"));
    stack.set_visible_child_name("loading");

    listbox.connect_row_activated({
        let navigation = navigation.clone();
        let packages = packages.clone();
        let context = context.clone();
        move |_, row| {
            let index = row.index();
            if index >= 0 {
                if let Some(package) = packages.borrow().get(index as usize).cloned() {
                    navigation.push(&build_package_detail_page(context.clone(), &navigation, &package));
                }
            }
        }
    });

    update_button.connect_clicked({
        let context = context.clone();
        let update_button = update_button.clone();
        let update_status = update_status.clone();
        let advisory_banner = advisory_banner.clone();
        let stack_for_result = stack.clone();
        let listbox_for_result = listbox.clone();
        let packages_for_result = packages.clone();
        move |_| {
            if advisory_banner.is_visible() {
                update_status.set_label("Review unread critical advisories before updating.");
                return;
            }
            if let Err(error) =
                ensure_session_auth(&context, parent_window_for(&update_button).as_ref(), false)
            {
                update_status.set_label(&error);
                return;
            }
            update_button.set_sensitive(false);
            update_status.set_label("Running full system update through monarch-helper...");

            let (sender, receiver) = std::sync::mpsc::channel();
            let context_for_dialog = context.clone();
            context.runtime.spawn({
                let catalog = context.catalog.clone();
                async move {
                    let _ = sender.send(catalog.update_system_stream().await);
                }
            });

            let update_button_for_result = update_button.clone();
            let update_status_for_result = update_status.clone();
            let stack_for_timeout = stack_for_result.clone();
            let listbox_for_timeout = listbox_for_result.clone();
            let packages_for_timeout = packages_for_result.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                match receiver.try_recv() {
                    Ok(Ok(stream)) => {
                        let context_for_finish = context_for_dialog.clone();
                        present_operation_dialog(
                            context_for_dialog.clone(),
                            "Updating System",
                            "Running full system update through monarch-helper...",
                            stream,
                            {
                                let update_button_for_result = update_button_for_result.clone();
                                let update_status_for_result = update_status_for_result.clone();
                                let stack_for_result = stack_for_timeout.clone();
                                let listbox_for_result = listbox_for_timeout.clone();
                                let packages_for_result = packages_for_timeout.clone();
                                move |result| {
                                    match result {
                                        Ok(()) => {
                                            packages_for_result.borrow_mut().clear();
                                            while let Some(child) = listbox_for_result.first_child() {
                                                listbox_for_result.remove(&child);
                                            }
                                            stack_for_result.set_visible_child_name("empty");
                                            context_for_finish.mark_catalog_dirty();
                                            update_status_for_result
                                                .set_label("System update completed. Refreshing update snapshot...");
                                        }
                                        Err(error) => {
                                            update_status_for_result.set_label(&error);
                                        }
                                    }
                                    update_button_for_result.set_sensitive(true);
                                }
                            },
                            OperationDialogOptions::default(),
                        );
                        glib::ControlFlow::Break
                    }
                    Ok(Err(error)) => {
                        update_status_for_result.set_label(&error);
                        update_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        update_status_for_result.set_label("Update request was interrupted.");
                        update_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                }
            });
        }
    });

    load_update_snapshot(
        &context,
        &stack,
        &source_bar,
        &listbox,
        &scroller,
        &packages,
        &error_detail,
        true,
    );
    refresh_critical_advisory_banner(&context, &advisory_banner);

    let last_refresh_epoch = Rc::new(Cell::new(context.refresh_epoch()));
    let context_for_refresh = context.clone();
    let stack_for_refresh = stack.clone();
    let source_bar_for_refresh = source_bar.clone();
    let listbox_for_refresh = listbox.clone();
    let packages_for_refresh = packages.clone();
    let error_detail_for_refresh = error_detail.clone();
    let scroller_for_refresh = scroller.clone();
    let last_refresh_epoch_for_timeout = last_refresh_epoch.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(500), move || {
        let current = context_for_refresh.refresh_epoch();
        if current != last_refresh_epoch_for_timeout.get() {
            last_refresh_epoch_for_timeout.set(current);
            load_update_snapshot(
                &context_for_refresh,
                &stack_for_refresh,
                &source_bar_for_refresh,
                &listbox_for_refresh,
                &scroller_for_refresh,
                &packages_for_refresh,
                &error_detail_for_refresh,
                false,
            );
            refresh_critical_advisory_banner(&context_for_refresh, &advisory_banner);
        }
        glib::ControlFlow::Continue
    });

    container.append(&stack);
    container.prepend(&summary_card);
    container.upcast()
}

fn refresh_critical_advisory_banner(context: &AppContext, advisory_banner: &gtk::Label) {
    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let context = context.clone();
        async move {
            let _ = sender.send((context.fetch_news().await, context.settings.load()));
        }
    });
    let advisory_banner = advisory_banner.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || match receiver.try_recv() {
        Ok((Ok(items), Ok(settings))) => {
            let unread_critical = items
                .iter()
                .filter(|item| item.is_critical && !settings.read_news_ids.iter().any(|id| id == &item.id))
                .count();
            if unread_critical > 0 {
                advisory_banner.set_label(&format!(
                    "{unread_critical} unread critical advisories are waiting in News."
                ));
                advisory_banner.set_visible(true);
            } else {
                advisory_banner.set_visible(false);
            }
            glib::ControlFlow::Break
        }
        Ok(_) => {
            advisory_banner.set_visible(false);
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
    });
}

#[allow(clippy::too_many_arguments)] // TODO: group into UpdateLoadRefs struct
fn load_update_snapshot(
    context: &AppContext,
    stack: &gtk::Stack,
    source_bar: &gtk::Box,
    listbox: &gtk::ListBox,
    scroller: &gtk::ScrolledWindow,
    packages: &Rc<RefCell<Vec<monarch_core::models::Package>>>,
    error_detail: &gtk::Label,
    show_loading: bool,
) {
    let adjustment = scroller.vadjustment();
    let scroll_value = adjustment.value();
    if show_loading {
        stack.set_visible_child_name("loading");
    }

    let stack_for_result = stack.clone();
    let source_bar_for_result = source_bar.clone();
    let listbox_for_result = listbox.clone();
    let error_detail_for_result = error_detail.clone();
    let packages_for_result = packages.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let catalog = context.catalog.clone();
        async move {
            let _ = sender.send(catalog.load_updates().await);
        }
    });

    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
        match receiver.try_recv() {
            Ok(Ok(snapshot)) => {
                if snapshot.items.is_empty() {
                    render_source_statuses(&source_bar_for_result, &snapshot);
                    packages_for_result.borrow_mut().clear();
                    while let Some(child) = listbox_for_result.first_child() {
                        listbox_for_result.remove(&child);
                    }
                    stack_for_result.set_visible_child_name("empty");
                } else {
                    render_source_statuses(&source_bar_for_result, &snapshot);
                    render_update_rows(&listbox_for_result, &packages_for_result, &snapshot.items);
                    stack_for_result.set_visible_child_name("ready");
                    restore_scroll_position(&adjustment, scroll_value);
                }
                glib::ControlFlow::Break
            }
            Ok(Err(error)) => {
                error_detail_for_result.set_label(&error);
                stack_for_result.set_visible_child_name("error");
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        }
    });
}

fn restore_scroll_position(adjustment: &gtk::Adjustment, scroll_value: f64) {
    let adjustment = adjustment.clone();
    glib::idle_add_local_once(move || {
        let upper = adjustment.upper();
        let page_size = adjustment.page_size();
        let max_value = (upper - page_size).max(adjustment.lower());
        adjustment.set_value(scroll_value.clamp(adjustment.lower(), max_value));
    });
}

fn render_source_statuses(container: &gtk::Box, snapshot: &UpdateSnapshot) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    for source in &snapshot.sources {
        let label = gtk::Label::builder()
            .label(format!("{}: {}", source.source, source.status))
            .css_classes(vec!["caption".to_string(), "monarch-chip".to_string()])
            .build();
        container.append(&label);
    }
}

fn render_update_rows(
    listbox: &gtk::ListBox,
    packages: &Rc<RefCell<Vec<monarch_core::models::Package>>>,
    items: &[UpdateSnapshotItem],
) {
    let selected_canonical_id = listbox
        .selected_row()
        .and_then(|row| packages.borrow().get(row.index() as usize).cloned())
        .map(|package| package.canonical_id)
        .filter(|id| !id.trim().is_empty());

    listbox.unselect_all();
    while let Some(child) = listbox.first_child() {
        listbox.remove(&child);
    }
    let mut next_packages = Vec::with_capacity(items.len());
    let mut selected_index = None;

    for (index, item) in items.iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(escape_markup(&item.package.effective_title()))
            .subtitle(escape_markup(&format!(
                "{}  ->  {}  •  {}",
                item.current_version, item.new_version, item.package.source.label
            )))
            .activatable(true)
            .build();

        let version = gtk::Label::builder()
            .label(item.new_version.clone())
            .css_classes(vec!["monarch-meta".to_string()])
            .valign(gtk::Align::Center)
            .build();
        row.add_suffix(&version);
        listbox.append(&row);
        if selected_canonical_id
            .as_deref()
            .is_some_and(|id| id == item.package.canonical_id)
        {
            selected_index = Some(index as i32);
        }
        next_packages.push(item.package.clone());
    }

    packages.replace(next_packages);
    if let Some(index) = selected_index {
        let listbox_for_select = listbox.clone();
        glib::idle_add_local_once(move || {
            if let Some(row) = listbox_for_select.row_at_index(index) {
                listbox_for_select.select_row(Some(&row));
            }
        });
    }
}

fn escape_markup(text: &str) -> String {
    glib::markup_escape_text(text).to_string()
}

use crate::context::AppContext;
use adw::prelude::*;
use monarch_core::models::NewsItem;

pub fn build_news_page(context: AppContext) -> gtk::Widget {
    let container = gtk::Box::builder()
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
    let critical_summary = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["monarch-hero-copy".to_string()])
        .build();
    hero.append(
        &gtk::Label::builder()
            .label("System Advisories")
            .xalign(0.0)
            .css_classes(vec!["monarch-hero-title".to_string()])
            .build(),
    );
    hero.append(
        &gtk::Label::builder()
            .label("Review critical distro and package-source announcements before updating or switching sources.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["monarch-hero-copy".to_string()])
            .build(),
    );
    hero.append(&critical_summary);

    let critical_strip = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    let critical_section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    critical_section.append(
        &gtk::Label::builder()
            .label("Critical Now")
            .xalign(0.0)
            .css_classes(vec!["title-4".to_string()])
            .build(),
    );
    let critical_scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .child(&critical_strip)
        .min_content_height(118)
        .build();
    critical_scroller.set_kinetic_scrolling(true);
    critical_section.append(&critical_scroller);

    let advisory_list = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    let advisory_scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&advisory_list)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    advisory_scroller.set_kinetic_scrolling(true);

    let loading = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    loading.append(&super::discovery::build_skeleton_panel(5));

    let stack = gtk::Stack::new();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&advisory_scroller, Some("list"));
    stack.set_visible_child_name("loading");

    container.set_vexpand(false);
    container.append(&hero);
    container.append(&critical_section);
    container.append(&stack);

    let scrolled_page = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&container)
        .build();
    scrolled_page.set_kinetic_scrolling(true);

    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let context = context.clone();
        async move {
            let _ = sender.send((context.fetch_news().await, context.settings.load()));
        }
    });

    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || match receiver.try_recv() {
        Ok((Ok(items), Ok(settings))) => {
            let critical_count = items.iter().filter(|item| item.is_critical).count();
            let summary = if critical_count == 0 {
                "No critical advisories are currently flagged.".to_string()
            } else {
                format!("{critical_count} critical advisories should be reviewed before risky updates.")
            };
            critical_summary.set_label(&summary);
            render_news(&context, &critical_strip, &advisory_list, &items, &settings.read_news_ids);
            stack.set_visible_child_name("list");
            glib::ControlFlow::Break
        }
        Ok((Err(error), _)) => {
            critical_summary.set_label(&error);
            stack.set_visible_child_name("list");
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        _ => glib::ControlFlow::Break,
    });

    scrolled_page.upcast()
}

fn render_news(
    context: &AppContext,
    critical_strip: &gtk::Box,
    advisory_list: &gtk::Box,
    items: &[NewsItem],
    read_news_ids: &[String],
) {
    while let Some(child) = critical_strip.last_child() {
        critical_strip.remove(&child);
    }
    while let Some(child) = advisory_list.first_child() {
        advisory_list.remove(&child);
    }

    let mut has_critical = false;
    for item in items {
        let is_read = read_news_ids.iter().any(|value| value == &item.id);
        if item.is_critical {
            has_critical = true;
            critical_strip.append(&build_critical_card(item, is_read));
        }
        advisory_list.append(&build_news_card(context.clone(), item, is_read));
    }

    if !has_critical {
        critical_strip.append(
            &gtk::Label::builder()
                .label("Nothing urgent is flagged right now.")
                .xalign(0.0)
                .css_classes(vec!["dim-label".to_string()])
                .build(),
        );
    }
}

fn build_critical_card(item: &NewsItem, is_read: bool) -> gtk::Box {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .css_classes(vec!["monarch-security-card".to_string()])
        .width_request(260)
        .build();
    let badges = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    badges.append(&news_badge("Critical"));
    badges.append(&news_badge(&item.source_label));
    if is_read {
        badges.append(&news_badge("Read"));
    }
    card.append(&badges);
    card.append(
        &gtk::Label::builder()
            .label(&item.title)
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["title-5".to_string()])
            .build(),
    );
    card.append(
        &gtk::Label::builder()
            .label(format!("{} • {}", item.source_label, item.pub_date))
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["monarch-meta".to_string()])
            .build(),
    );
    card
}

fn build_news_card(context: AppContext, item: &NewsItem, is_read: bool) -> gtk::Box {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .css_classes(vec!["monarch-panel".to_string()])
        .build();
    let badges = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    badges.append(&news_badge(&item.source_label));
    badges.append(&news_badge(match item.category {
        monarch_core::models::NewsCategory::Critical => "Critical",
        monarch_core::models::NewsCategory::System => "System",
        monarch_core::models::NewsCategory::Discovery => "Discovery",
    }));
    badges.append(&news_badge(if is_read { "Read" } else { "Unread" }));
    card.append(&badges);
    card.append(
        &gtk::Label::builder()
            .label(&item.title)
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["title-4".to_string()])
            .build(),
    );
    card.append(
        &gtk::Label::builder()
            .label(&item.pub_date)
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["monarch-meta".to_string()])
            .build(),
    );
    card.append(
        &gtk::Label::builder()
            .label(news_excerpt(item))
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .build(),
    );
    let actions = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .build();
    let read_button = gtk::Button::builder()
        .label(if is_read { "Read" } else { "Mark Read" })
        .sensitive(!is_read)
        .build();
    let open_button = gtk::Button::builder()
        .label("Open Link")
        .css_classes(vec!["suggested-action".to_string()])
        .build();
    actions.append(&read_button);
    actions.append(&open_button);
    card.append(&actions);

    read_button.connect_clicked({
        let context = context.clone();
        let item_id = item.id.clone();
        let read_button = read_button.clone();
        move |_| {
            let ids = vec![item_id.clone()];
            let (sender, receiver) = std::sync::mpsc::channel();
            context.runtime.spawn({
                let settings = context.settings.clone();
                async move {
                    let _ = sender.send(settings.mark_news_read(&ids));
                }
            });
            let read_button_for_result = read_button.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || match receiver.try_recv() {
                Ok(Ok(_)) => {
                    read_button_for_result.set_sensitive(false);
                    read_button_for_result.set_label("Read");
                    glib::ControlFlow::Break
                }
                Ok(Err(_)) => glib::ControlFlow::Break,
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            });
        }
    });
    open_button.connect_clicked({
        let link = item.link.clone();
        move |_| {
            let _ = gio::AppInfo::launch_default_for_uri(&link, None::<&gio::AppLaunchContext>);
        }
    });

    card
}

fn news_badge(label: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(label)
        .css_classes(vec!["monarch-store-card-badge".to_string()])
        .build()
}

fn news_excerpt(item: &NewsItem) -> String {
    item.content
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&item.link)
        .split_whitespace()
        .take(32)
        .collect::<Vec<_>>()
        .join(" ")
}

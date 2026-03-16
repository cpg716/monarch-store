use crate::context::AppContext;
use crate::ui::components::package_card::{
    bind_compact_package_card_widget, build_compact_package_card_widget,
};
use crate::ui::pages::package_detail::build_package_detail_page;
use adw::prelude::*;
use monarch_core::models::Package;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub fn build_favorites_page(context: AppContext, navigation: &adw::NavigationView) -> gtk::Widget {
    let navigation = navigation.clone();
    let packages = Rc::new(RefCell::new(Vec::<Package>::new()));
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
    hero.append(
        &gtk::Label::builder()
            .label("Favorites")
            .xalign(0.0)
            .css_classes(vec!["monarch-hero-title".to_string()])
            .build(),
    );
    hero.append(
        &gtk::Label::builder()
            .label("Saved apps and tools, ready to revisit quickly.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["monarch-hero-copy".to_string()])
            .build(),
    );

    let flowbox = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .row_spacing(14)
        .column_spacing(14)
        .min_children_per_line(3)
        .max_children_per_line(3)
        .build();

    let icon_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Both);
    let title_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Vertical);
    let desc_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Vertical);
    let card_root_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Both);
    flowbox.set_vexpand(false);
    let scroller = gtk::ScrolledWindow::builder()
        .child(&flowbox)
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
        .icon_name("starred-symbolic")
        .title("No favorites yet")
        .description("Use the favorite action from package details to build a shortlist.")
        .css_classes(vec!["monarch-empty".to_string()])
        .build();
    let stack = gtk::Stack::new();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&empty, Some("empty"));
    stack.add_named(&scroller, Some("list"));
    stack.set_visible_child_name("loading");

    container.append(&hero);
    container.append(&stack);
    load_favorites(
        context.clone(),
        packages.clone(),
        flowbox.clone(),
        scroller.clone(),
        stack.clone(),
        navigation.clone(),
        true,
        icon_group.clone(),
        title_group.clone(),
        desc_group.clone(),
        card_root_group.clone(),
    );

    let last_refresh_epoch = Rc::new(Cell::new(context.refresh_epoch()));
    let last_refresh_epoch_for_timeout = last_refresh_epoch.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(500), move || {
        let current = context.refresh_epoch();
        if current != last_refresh_epoch_for_timeout.get() {
            last_refresh_epoch_for_timeout.set(current);
            load_favorites(
                context.clone(),
                packages.clone(),
                flowbox.clone(),
                scroller.clone(),
                stack.clone(),
                navigation.clone(),
                false,
                icon_group.clone(),
                title_group.clone(),
                desc_group.clone(),
                card_root_group.clone(),
            );
        }
        glib::ControlFlow::Continue
    });

    container.upcast()
}

#[allow(clippy::too_many_arguments)] // TODO: group into FavoritesLoadRefs struct
fn load_favorites(
    context: AppContext,
    packages: Rc<RefCell<Vec<Package>>>,
    flowbox: gtk::FlowBox,
    scroller: gtk::ScrolledWindow,
    stack: gtk::Stack,
    navigation: adw::NavigationView,
    show_loading: bool,
    icon_group: gtk::SizeGroup,
    title_group: gtk::SizeGroup,
    desc_group: gtk::SizeGroup,
    card_root_group: gtk::SizeGroup,
) {
    let adjustment = scroller.vadjustment();
    let scroll_value = adjustment.value();
    if show_loading {
        stack.set_visible_child_name("loading");
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    context.runtime.spawn({
        let catalog = context.catalog.clone();
        let favorites = context.favorites.clone();
        async move {
            let result: Result<Vec<Package>, String> = match favorites.list() {
                Ok(ids) => catalog.load_packages_by_ids(ids).await,
                Err(error) => Err(error),
            };
            let _ = sender.send(result);
        }
    });

    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || match receiver
        .try_recv()
    {
        Ok(Ok(items)) => {
            while let Some(child) = flowbox.first_child() {
                flowbox.remove(&child);
            }
            if items.is_empty() {
                packages.borrow_mut().clear();
                stack.set_visible_child_name("empty");
            } else {
                for item in &items {
                    let card = build_compact_package_card_widget(context.clone());
                    bind_compact_package_card_widget(
                        &card,
                        item,
                        &context,
                        Some(&icon_group),
                        Some(&title_group),
                        Some(&desc_group),
                        Some(&card_root_group),
                    );
                    let gesture = gtk::GestureClick::new();
                    gesture.connect_released({
                        let context = context.clone();
                        let navigation = navigation.clone();
                        let package = item.clone();
                        move |_, _, _, _| {
                            navigation.push(&build_package_detail_page(
                                context.clone(),
                                &navigation,
                                &package,
                            ));
                        }
                    });
                    card.add_controller(gesture);
                    flowbox.insert(&card, -1);
                }
                packages.replace(items);
                stack.set_visible_child_name("list");
                restore_scroll_position(&adjustment, scroll_value);
            }
            glib::ControlFlow::Break
        }
        Ok(Err(_)) => {
            stack.set_visible_child_name("empty");
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
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

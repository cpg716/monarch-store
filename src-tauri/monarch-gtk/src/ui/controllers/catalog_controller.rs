use crate::context::AppContext;
use crate::ui::models::package_row::PackageRowObject;
use gtk::prelude::*;
use monarch_core::models::{Package, SearchOptions, SearchSortMode};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy)]
pub enum CatalogMode {
    Discovery,
}

#[derive(Clone)]
pub struct CatalogController {
    store: gio::ListStore,
    filter: gtk::CustomFilter,
    sorter: gtk::CustomSorter,
    selection: gtk::SingleSelection,
    search_query: Rc<RefCell<String>>,
    source_filter: Rc<RefCell<Option<String>>>,
    category_filter: Rc<RefCell<Option<String>>>,
    installed_only: Rc<RefCell<bool>>,
    context: AppContext,
    mode: CatalogMode,
}

impl CatalogController {
    pub fn new(context: AppContext, mode: CatalogMode) -> Self {
        let store = gio::ListStore::new::<PackageRowObject>();
        let search_query = Rc::new(RefCell::new(String::new()));
        let source_filter = Rc::new(RefCell::new(None::<String>));
        let category_filter = Rc::new(RefCell::new(None::<String>));
        let installed_only = Rc::new(RefCell::new(false));

        let search_query_for_filter = search_query.clone();
        let source_filter_for_filter = source_filter.clone();
        let category_filter_for_filter = category_filter.clone();
        let installed_only_for_filter = installed_only.clone();

        let filter = gtk::CustomFilter::new(move |obj| {
            let Some(row) = obj.downcast_ref::<PackageRowObject>() else {
                return true;
            };

            let package = row.package();
            if *installed_only_for_filter.borrow() && !package.installed {
                return false;
            }

            row.matches_query(&search_query_for_filter.borrow())
                && row.matches_source(source_filter_for_filter.borrow().as_deref())
                && category_filter_for_filter
                    .borrow()
                    .as_deref()
                    .map(|category| {
                        row.package()
                            .categories
                            .as_ref()
                            .map(|categories| {
                                categories
                                    .iter()
                                    .any(|value| value.eq_ignore_ascii_case(category))
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
        });

        let sorter = gtk::CustomSorter::new(move |obj1, obj2| {
            let Some(left) = obj1.downcast_ref::<PackageRowObject>() else {
                return gtk::Ordering::Equal;
            };
            let Some(right) = obj2.downcast_ref::<PackageRowObject>() else {
                return gtk::Ordering::Equal;
            };
            let left_package = left.package();
            let right_package = right.package();

            if left_package.installed != right_package.installed {
                return if left_package.installed {
                    gtk::Ordering::Smaller
                } else {
                    gtk::Ordering::Larger
                };
            }

            let left_title = left_package.effective_title().to_lowercase();
            let right_title = right_package.effective_title().to_lowercase();
            match left_title.cmp(&right_title) {
                std::cmp::Ordering::Less => gtk::Ordering::Smaller,
                std::cmp::Ordering::Greater => gtk::Ordering::Larger,
                std::cmp::Ordering::Equal => gtk::Ordering::Equal,
            }
        });

        let filter_model = gtk::FilterListModel::new(Some(store.clone()), Some(filter.clone()));
        let sort_model = gtk::SortListModel::new(Some(filter_model.clone()), Some(sorter.clone()));
        let selection = gtk::SingleSelection::new(Some(sort_model.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);

        Self {
            store,
            filter,
            sorter,
            selection,
            search_query,
            source_filter,
            category_filter,
            installed_only,
            context,
            mode,
        }
    }

    pub fn context(&self) -> &AppContext {
        &self.context
    }

    pub fn set_search_query(&self, query: &str) {
        self.search_query.replace(query.to_string());
        self.filter.changed(gtk::FilterChange::Different);
    }

    pub fn search_query(&self) -> String {
        self.search_query.borrow().clone()
    }

    pub fn set_source_filter(&self, source: Option<String>) {
        self.source_filter.replace(source);
        self.filter.changed(gtk::FilterChange::Different);
    }

    pub fn source_filter(&self) -> Option<String> {
        self.source_filter.borrow().clone()
    }

    pub fn set_category_filter(&self, category: Option<String>) {
        self.category_filter.replace(category);
        self.filter.changed(gtk::FilterChange::Different);
    }

    pub fn category_filter(&self) -> Option<String> {
        self.category_filter.borrow().clone()
    }

    pub fn set_installed_only(&self, installed_only: bool) {
        self.installed_only.replace(installed_only);
        self.filter.changed(gtk::FilterChange::Different);
    }

    pub fn installed_only(&self) -> bool {
        *self.installed_only.borrow()
    }

    pub fn replace_packages(&self, packages: Vec<Package>) {
        let selected = self.selected_canonical_id();
        self.store.remove_all();
        for package in packages {
            self.store.append(&PackageRowObject::new(package));
        }
        self.sorter.changed(gtk::SorterChange::Different);
        if let Some(selected) = selected {
            self.select_canonical_id(&selected);
        }
    }

    pub fn selected_canonical_id(&self) -> Option<String> {
        let position = self.selection.selected();
        if position == gtk::INVALID_LIST_POSITION {
            return None;
        }

        self.selection
            .selected_item()
            .and_then(|obj| obj.downcast::<PackageRowObject>().ok())
            .map(|row| row.package().canonical_id)
            .filter(|id| !id.trim().is_empty())
    }

    pub fn select_canonical_id(&self, canonical_id: &str) {
        let Some(model) = self.selection.model() else {
            return;
        };
        for position in 0..model.n_items() {
            let Some(obj) = model.item(position) else {
                continue;
            };
            let Ok(row) = obj.downcast::<PackageRowObject>() else {
                continue;
            };
            if row.package().canonical_id == canonical_id {
                self.selection.set_selected(position);
                break;
            }
        }
    }

    pub fn search_async<F>(&self, query: String, on_result: F)
    where
        F: Fn(Result<Vec<Package>, String>) + 'static,
    {
        self.set_search_query(&query);
        self.fetch_async(query, on_result);
    }

    fn fetch_async<F>(&self, query: String, on_result: F)
    where
        F: Fn(Result<Vec<Package>, String>) + 'static,
    {
        let (sender, receiver) = std::sync::mpsc::channel();
        let catalog = self.context.catalog.clone();
        let settings = self.context.settings.clone();
        let mode = self.mode;
        let source_filter = self.source_filter.borrow().clone();
        let category_filter = self.category_filter.borrow().clone();
        let installed_only = *self.installed_only.borrow();

        self.context.runtime.spawn(async move {
            let use_snapshot =
                query.trim().is_empty() && category_filter.is_none() && source_filter.is_none();
            let options = settings
                .load()
                .map(|state| SearchOptions {
                    flatpak_enabled: Some(state.flatpak_enabled),
                    aur_enabled: Some(state.aur_enabled),
                    chaotic_enabled: Some(state.chaotic_enabled),
                    show_system_apps: Some(state.show_system_apps),
                    source_filter,
                    category_filter,
                    installed_only: Some(installed_only),
                    sort_mode: Some(if query.trim().is_empty() {
                        SearchSortMode::Name
                    } else {
                        SearchSortMode::Relevance
                    }),
                    for_installed_lookup: Some(false),
                })
                .unwrap_or_default();
            let _ = mode;
            let result = if use_snapshot {
                catalog.load_discovery_snapshot_with_options(options).await
            } else {
                catalog.search(query, options).await
            };
            let _ = sender.send(result);
        });

        glib::source::timeout_add_local(
            std::time::Duration::from_millis(30),
            move || match receiver.try_recv() {
                Ok(result) => {
                    on_result(result);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            },
        );
    }
}

use glib::subclass::prelude::*;
use glib::ParamSpecBuilderExt;
use glib::{ParamSpec, ParamSpecString, Value};
use once_cell::sync::Lazy;
use std::cell::RefCell;

mod imp {
    use super::*;
    use glib::value::ToValue;

    #[derive(Default)]
    pub struct SourceListItem {
        pub icon_path: RefCell<String>,
        pub title: RefCell<String>,
        pub subtitle: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SourceListItem {
        const NAME: &'static str = "MonarchGtkSourceListItem";
        type Type = super::SourceListItem;
    }

    impl ObjectImpl for SourceListItem {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::builder("icon-path").read_only().build(),
                    ParamSpecString::builder("title").read_only().build(),
                    ParamSpecString::builder("subtitle").read_only().build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "icon-path" => self.icon_path.borrow().to_value(),
                "title" => self.title.borrow().to_value(),
                "subtitle" => self.subtitle.borrow().to_value(),
                name => panic!("Unknown property {name}"),
            }
        }
    }
}

glib::wrapper! {
    pub struct SourceListItem(ObjectSubclass<imp::SourceListItem>);
}

impl SourceListItem {
    pub fn new(icon_path: &str, title: &str, subtitle: &str) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().icon_path.replace(icon_path.to_string());
        obj.imp().title.replace(title.to_string());
        obj.imp().subtitle.replace(subtitle.to_string());
        obj
    }

    pub fn icon_path(&self) -> String {
        self.imp().icon_path.borrow().clone()
    }

    pub fn title(&self) -> String {
        self.imp().title.borrow().clone()
    }

    pub fn subtitle(&self) -> String {
        self.imp().subtitle.borrow().clone()
    }
}

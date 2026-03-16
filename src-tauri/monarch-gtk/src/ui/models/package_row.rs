use glib::subclass::prelude::*;
use glib::value::ToValue;
use glib::{ParamSpec, ParamSpecBoolean, ParamSpecString, Value};
use glib::ParamSpecBuilderExt;
use monarch_core::models::Package;
use once_cell::sync::Lazy;
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PackageRowObject {
        pub package: RefCell<Option<Package>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PackageRowObject {
        const NAME: &'static str = "MonarchGtkPackageRowObject";
        type Type = super::PackageRowObject;
    }

    impl ObjectImpl for PackageRowObject {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::builder("name").read_only().build(),
                    ParamSpecString::builder("display-title").read_only().build(),
                    ParamSpecString::builder("description").read_only().build(),
                    ParamSpecString::builder("version").read_only().build(),
                    ParamSpecString::builder("source-label").read_only().build(),
                    ParamSpecString::builder("icon").read_only().build(),
                    ParamSpecString::builder("canonical-id").read_only().build(),
                    ParamSpecBoolean::builder("installed").read_only().build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            let package = self.package.borrow();
            let package = package
                .as_ref()
                .expect("PackageRowObject package must be initialized");

            match pspec.name() {
                "name" => package.name.to_value(),
                "display-title" => package.effective_title().to_value(),
                "description" => package.description.to_value(),
                "version" => package.version.to_value(),
                "source-label" => package.source.label.to_value(),
                "icon" => package.icon.clone().unwrap_or_default().to_value(),
                "canonical-id" => package.canonical_id.to_value(),
                "installed" => package.installed.to_value(),
                name => panic!("Unknown property {name}"),
            }
        }
    }
}

glib::wrapper! {
    pub struct PackageRowObject(ObjectSubclass<imp::PackageRowObject>);
}

impl PackageRowObject {
    pub fn new(package: Package) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().package.replace(Some(package));
        obj
    }

    pub fn package(&self) -> Package {
        self.imp()
            .package
            .borrow()
            .as_ref()
            .expect("PackageRowObject package must be initialized")
            .clone()
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return true;
        }

        let package = self.package();
        let haystack = format!(
            "{} {} {} {} {} {}",
            package.canonical_id,
            package.name,
            package.effective_title(),
            package.description,
            package.source.id,
            package.source.label
        )
        .to_lowercase();

        haystack.contains(&needle)
    }

    pub fn matches_source(&self, source: Option<&str>) -> bool {
        let Some(source) = source else {
            return true;
        };
        if source == "all" {
            return true;
        }

        let package = self.package();
        package.source.id == source || package.source.source_type == source
    }
}

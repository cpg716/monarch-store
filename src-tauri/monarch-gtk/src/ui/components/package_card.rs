use crate::context::AppContext;
use crate::ui::media::{arch_logo_fallback, set_picture_source};
use adw::prelude::*;
use monarch_core::models::{GtkSettings, Package, PackageSource};
const ARCH_SOURCE_LOGO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/assets/source-logos/arch-official.svg"
);
const FLATPAK_SOURCE_LOGO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/assets/source-logos/flatpak.svg"
);
const AUR_SOURCE_LOGO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/assets/source-logos/aur.svg"
);
const CHAOTIC_SOURCE_LOGO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/assets/source-logos/chaotic-aur.png"
);
const CACHYOS_SOURCE_LOGO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/assets/source-logos/cachyos.svg"
);
const MANJARO_SOURCE_LOGO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/assets/source-logos/manjaro.svg"
);
const GARUDA_SOURCE_LOGO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/assets/source-logos/garuda.svg"
);
const ENDEAVOUROS_SOURCE_LOGO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/assets/source-logos/endeavouros.svg"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PackageCardVariant {
    Compact,
}

#[derive(Clone, Copy)]
struct SourceBrand {
    label: &'static str,
    logo_path: &'static str,
    css_class: &'static str,
}

pub fn build_compact_package_card_widget(context: AppContext) -> gtk::Box {
    build_card_widget(context, PackageCardVariant::Compact)
}

pub fn bind_compact_package_card_widget(
    card: &gtk::Box,
    package: &Package,
    context: &AppContext,
    icon_group: Option<&gtk::SizeGroup>,
    title_group: Option<&gtk::SizeGroup>,
    desc_group: Option<&gtk::SizeGroup>,
    card_root_group: Option<&gtk::SizeGroup>,
) {
    bind_card_widget(
        card,
        package,
        context,
        PackageCardVariant::Compact,
        icon_group,
        title_group,
        desc_group,
        card_root_group,
    );
}

fn build_card_widget(context: AppContext, variant: PackageCardVariant) -> gtk::Box {
    let _ = context;
    match variant {
        PackageCardVariant::Compact => build_compact_widget(),
    }
}

fn build_compact_widget() -> gtk::Box {
    let icon = gtk::Picture::builder()
        .width_request(80)
        .height_request(80)
        .keep_aspect_ratio(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .css_classes(vec!["monarch-card-app-icon".to_string()])
        .build();
    icon.set_paintable(Some(crate::ui::media::placeholder_texture()));

    let title = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .lines(2)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .css_classes(vec!["monarch-store-card-title".to_string()])
        .build();
    let description = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .lines(2)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .css_classes(vec!["monarch-store-card-description".to_string()])
        .build();

    let source_badges = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .css_classes(vec!["monarch-card-badges".to_string()])
        .build();

    let badge_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    badge_spacer.set_vexpand(true);

    let copy = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .hexpand(true)
        .valign(gtk::Align::Start)
        .overflow(gtk::Overflow::Hidden)
        .build();
    copy.append(&title);
    copy.append(&description);
    copy.append(&badge_spacer);
    copy.append(&source_badges);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(16)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .overflow(gtk::Overflow::Hidden)
        .build();
    content.append(&icon);
    content.append(&copy);

    let bin = adw::Bin::builder()
        .child(&content)
        .css_classes(vec!["monarch-store-card".to_string()])
        .overflow(gtk::Overflow::Hidden)
        .build();

    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Fill)
        .valign(gtk::Align::Start)
        .hexpand(false)
        .width_request(308)
        .css_classes(vec!["monarch-store-card-compact".to_string()])
        .build();
    card.append(&bin);
    card
}

#[allow(clippy::too_many_arguments)] // card + package + context + 3 size groups + card_root_group
fn bind_card_widget(
    card: &gtk::Box,
    package: &Package,
    context: &AppContext,
    variant: PackageCardVariant,
    icon_group: Option<&gtk::SizeGroup>,
    title_group: Option<&gtk::SizeGroup>,
    desc_group: Option<&gtk::SizeGroup>,
    card_root_group: Option<&gtk::SizeGroup>,
) {
    match variant {
        PackageCardVariant::Compact => {
            bind_compact_card(card, package, context, icon_group, title_group, desc_group, card_root_group)
        }
    }
}

fn bind_compact_card(
    card: &gtk::Box,
    package: &Package,
    context: &AppContext,
    icon_group: Option<&gtk::SizeGroup>,
    title_group: Option<&gtk::SizeGroup>,
    desc_group: Option<&gtk::SizeGroup>,
    card_root_group: Option<&gtk::SizeGroup>,
) {
    if let Some(group) = card_root_group {
        group.add_widget(card);
    }
    let Some(bin) = card.first_child().and_downcast::<adw::Bin>() else {
        return;
    };
    let Some(content) = bin.child().and_downcast::<gtk::Box>() else {
        return;
    };
    let Some(icon) = content.first_child().and_downcast::<gtk::Picture>() else {
        return;
    };
    let Some(copy) = icon.next_sibling().and_downcast::<gtk::Box>() else {
        return;
    };
    let Some(title) = copy.first_child().and_downcast::<gtk::Label>() else {
        return;
    };
    let Some(description) = title.next_sibling().and_downcast::<gtk::Label>() else {
        return;
    };
    let Some(source_badges) = copy.last_child().and_downcast::<gtk::Box>() else {
        return;
    };

    if let Some(group) = icon_group {
        group.add_widget(&icon);
    }
    if let Some(group) = title_group {
        group.add_widget(&title);
    }
    if let Some(group) = desc_group {
        group.add_widget(&description);
    }

    apply_picture_icon(&icon, package, context, None);
    title.set_label(&package.effective_title());
    description.set_label(&card_summary(package));

    clear_box(&source_badges);
    for badge in source_badges_for(package) {
        source_badges.append(&chip(&badge, context));
    }
}

fn apply_picture_icon(
    icon: &gtk::Picture,
    package: &Package,
    context: &AppContext,
    fallback: Option<String>,
) {
    set_picture_source(
        icon,
        context.runtime.clone(),
        package.icon.clone(),
        fallback.or_else(|| Some(arch_logo_fallback())),
    );
}

fn card_summary(package: &Package) -> String {
    package
        .description
        .split('.')
        .next()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "No description available".to_string())
}

/// Text pill badge: source name in a small colored pill (no logo).
fn chip(source: &PackageSource, _context: &AppContext) -> gtk::Box {
    let brand = source_brand(source);
    let label = gtk::Label::builder()
        .label(brand.label)
        .css_classes(vec!["monarch-source-pill-label".to_string()])
        .build();
    let pill = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(vec![
            "monarch-source-pill".to_string(),
            "monarch-card-source-pill".to_string(),
            brand.css_class.to_string(),
        ])
        .build();
    pill.set_hexpand(false);
    pill.append(&label);
    pill
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn source_badges_for(package: &Package) -> Vec<PackageSource> {
    let mut sources = package
        .available_sources
        .as_ref()
        .filter(|sources| !sources.is_empty())
        .cloned()
        .unwrap_or_else(|| vec![package.source.clone()]);
    sources.sort_by_key(source_priority);
    let mut badges = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for source in sources {
        let key = source_brand(&source).label.to_string();
        if seen.insert(key) {
            badges.push(source);
        }
    }

    badges.truncate(4);
    badges
}

fn source_priority(source: &PackageSource) -> (i32, String) {
    let id = source.id.to_ascii_lowercase();
    let priority = if id.contains("cachyos") {
        0
    } else if id.contains("garuda") || id.contains("endeavour") || id.contains("manjaro") {
        1
    } else if source.source_type == "repo" && !id.contains("chaotic") {
        2
    } else if id.contains("chaotic") {
        3
    } else if source.source_type == "flatpak" {
        4
    } else if source.source_type == "aur" {
        5
    } else {
        6
    };
    (priority, source.label.to_ascii_lowercase())
}

/// Sort available sources by display priority (host distro first, then Arch, Chaotic, Flatpak, AUR).
/// Use before building the source selector so the first item is the recommended default.
pub fn sort_available_sources_by_priority(sources: &mut [PackageSource]) {
    sources.sort_by_key(source_priority);
}

/// Returns (display_label, icon_path) for use in source selector and other UI.
pub fn source_display_label_and_icon_path(source: &PackageSource) -> (String, String) {
    let brand = source_brand(source);
    (brand.label.to_string(), brand.logo_path.to_string())
}

/// Returns (label, logo_path) for each enabled source in settings order, for the home hero.
/// When host_distro_id is a known distro (cachyos, manjaro, garuda, endeavouros), that source
/// is included so the host shows as a source badge when it has its own repo.
pub fn enabled_source_legend(
    settings: &GtkSettings,
    host_distro_id: Option<&str>,
) -> Vec<(String, String)> {
    let mut out = vec![("Arch".to_string(), ARCH_SOURCE_LOGO.to_string())];
    if let Some(id) = host_distro_id {
        let id_lower = id.to_ascii_lowercase();
        if id_lower.contains("cachyos") {
            out.push(("CachyOS".to_string(), CACHYOS_SOURCE_LOGO.to_string()));
        } else if id_lower.contains("manjaro") {
            out.push(("Manjaro".to_string(), MANJARO_SOURCE_LOGO.to_string()));
        } else if id_lower.contains("garuda") {
            out.push(("Garuda".to_string(), GARUDA_SOURCE_LOGO.to_string()));
        } else if id_lower.contains("endeavour") {
            out.push(("EndeavourOS".to_string(), ENDEAVOUROS_SOURCE_LOGO.to_string()));
        }
    }
    if settings.chaotic_enabled {
        out.push(("Chaotic-AUR".to_string(), CHAOTIC_SOURCE_LOGO.to_string()));
    }
    if settings.flatpak_enabled {
        out.push(("Flatpak".to_string(), FLATPAK_SOURCE_LOGO.to_string()));
    }
    if settings.aur_enabled {
        out.push(("AUR".to_string(), AUR_SOURCE_LOGO.to_string()));
    }
    out
}

/// Returns the CSS class for a source pill (e.g. is-arch, is-flatpak, is-aur).
#[allow(dead_code)]
pub fn source_pill_css_class(source: &PackageSource) -> &'static str {
    source_brand(source).css_class
}

/// Maps a source display label (e.g. from dropdown/list) to pill CSS class for styling.
pub fn source_label_to_pill_css_class(label: &str) -> &'static str {
    match label {
        "Arch" => "is-arch",
        "Chaotic" | "Chaotic-AUR" => "is-chaotic",
        "Flatpak" => "is-flatpak",
        "AUR" => "is-aur",
        "CachyOS" => "is-cachyos",
        "Garuda" => "is-garuda",
        "EndeavourOS" => "is-endeavouros",
        "Manjaro" => "is-manjaro",
        _ => "is-arch",
    }
}

fn source_brand(source: &PackageSource) -> SourceBrand {
    let id = source.id.to_ascii_lowercase();
    if id.starts_with("cachyos") {
        SourceBrand {
            label: "CachyOS",
            logo_path: CACHYOS_SOURCE_LOGO,
            css_class: "is-cachyos",
        }
    } else if id.contains("chaotic") {
        SourceBrand {
            label: "Chaotic",
            logo_path: CHAOTIC_SOURCE_LOGO,
            css_class: "is-chaotic",
        }
    } else if source.source_type == "flatpak" || id.contains("flat") {
        SourceBrand {
            label: "Flatpak",
            logo_path: FLATPAK_SOURCE_LOGO,
            css_class: "is-flatpak",
        }
    } else if source.source_type == "aur" || id == "aur" {
        SourceBrand {
            label: "AUR",
            logo_path: AUR_SOURCE_LOGO,
            css_class: "is-aur",
        }
    } else if id.contains("garuda") {
        SourceBrand {
            label: "Garuda",
            logo_path: GARUDA_SOURCE_LOGO,
            css_class: "is-garuda",
        }
    } else if id.contains("endeavour") {
        SourceBrand {
            label: "EndeavourOS",
            logo_path: ENDEAVOUROS_SOURCE_LOGO,
            css_class: "is-endeavouros",
        }
    } else if id.contains("manjaro") {
        SourceBrand {
            label: "Manjaro",
            logo_path: MANJARO_SOURCE_LOGO,
            css_class: "is-manjaro",
        }
    } else {
        SourceBrand {
            label: "Arch",
            logo_path: ARCH_SOURCE_LOGO,
            css_class: "is-arch",
        }
    }
}

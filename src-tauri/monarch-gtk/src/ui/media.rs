use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use gdk_pixbuf::prelude::PixbufLoaderExt;
use gtk::prelude::WidgetExt;
use once_cell::sync::Lazy;
use std::sync::Arc;

/// Path to Arch logo used as package icon fallback (compile-time path).
const ARCH_LOGO_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/assets/arch-logo.png");

/// Returns the best available Arch logo fallback: file:// path if the file exists (e.g. when running from repo), otherwise embedded data URI.
pub fn arch_logo_fallback() -> String {
    if let Ok(canonical) = std::path::Path::new(ARCH_LOGO_PATH).canonicalize() {
        return format!("file://{}", canonical.display());
    }
    arch_logo_fallback_data_uri().to_string()
}

/// Embedded Arch logo (PNG) as data URI when the file is not on disk (e.g. installed app).
pub fn arch_logo_fallback_data_uri() -> &'static str {
    static URI: Lazy<String> = Lazy::new(|| {
        let bytes = include_bytes!("../../../../src/assets/arch-logo.png");
        format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(bytes.as_slice())
        )
    });
    &URI
}

/// Returns a minimal 1x1 transparent texture that lives for the process. Use when a Picture must
/// never have a null paintable (e.g. to avoid gtk_scaler_new assertion) and no fallback is provided.
/// Callers must use the returned reference so the Picture holds a valid paintable.
pub fn placeholder_texture() -> &'static gtk::gdk::Texture {
    static PLACEHOLDER: Lazy<gtk::gdk::Texture> = Lazy::new(|| {
        let bytes = glib::Bytes::from_static(&[0u8; 4]);
        let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(
            &bytes,
            gdk_pixbuf::Colorspace::Rgb,
            true,
            8,
            1,
            1,
            4,
        );
        gtk::gdk::Texture::for_pixbuf(&pixbuf)
    });
    &PLACEHOLDER
}

fn decode_data_url(value: &str) -> Option<Vec<u8>> {
    let (prefix, payload) = value.split_once(',')?;
    if !prefix.contains(";base64") {
        return None;
    }
    BASE64_STANDARD.decode(payload.trim()).ok()
}

fn decode_raw_base64(value: &str) -> Option<Vec<u8>> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if normalized.len() < 48 {
        return None;
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
    {
        return None;
    }
    BASE64_STANDARD.decode(normalized).ok()
}

fn texture_from_bytes(bytes: Vec<u8>, max_size: i32) -> Option<gtk::gdk::Texture> {
    let loader = gdk_pixbuf::PixbufLoader::new();
    loader.write(&bytes).ok()?;
    loader.close().ok()?;
    let pixbuf = loader.pixbuf()?;
    let width = pixbuf.width();
    let height = pixbuf.height();
    if width <= 0 || height <= 0 {
        return None;
    }
    if width > max_size || height > max_size {
        let scale = (max_size as f64) / (std::cmp::max(width, height) as f64);
        let new_width = (width as f64 * scale) as i32;
        let new_height = (height as f64 * scale) as i32;
        if let Some(scaled) =
            pixbuf.scale_simple(new_width, new_height, gdk_pixbuf::InterpType::Bilinear)
        {
            return Some(gtk::gdk::Texture::for_pixbuf(&scaled));
        }
    }
    Some(gtk::gdk::Texture::for_pixbuf(&pixbuf))
}

/// Sets a Picture's paintable from a local file path. Never leaves the picture with a null
/// paintable (avoids gtk_scaler_new assertion). Use for list item icons.
#[allow(dead_code)]
pub fn set_picture_from_file_path(picture: &gtk::Picture, path: &str, max_size: i32) {
    picture.set_paintable(Some(placeholder_texture()));
    if path.is_empty() {
        return;
    }
    if let Some(texture) = texture_from_local_source(path, max_size) {
        picture.set_paintable(Some(&texture));
    }
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let trimmed = bytes.iter().copied().skip_while(|&b| b == b' ' || b == b'\n' || b == b'\r').take(256).collect::<Vec<_>>();
    let s = String::from_utf8_lossy(&trimmed);
    s.starts_with("<?xml") || s.starts_with("<svg") || s.starts_with("<SVG")
}

fn texture_from_svg_bytes(bytes: Vec<u8>, max_size: i32) -> Option<gtk::gdk::Texture> {
    if !looks_like_svg(&bytes) {
        return None;
    }
    let mut path = std::env::temp_dir();
    path.push(format!("monarch-arch-{}.svg", std::process::id()));
    std::fs::write(&path, &bytes).ok()?;
    let result = gdk_pixbuf::Pixbuf::from_file(&path)
        .ok()
        .and_then(|pixbuf| {
            let w = pixbuf.width();
            let h = pixbuf.height();
            let (new_w, new_h) = if w > max_size || h > max_size {
                let scale = (max_size as f64) / (std::cmp::max(w, h) as f64);
                (
                    (w as f64 * scale) as i32,
                    (h as f64 * scale) as i32,
                )
            } else {
                (w, h)
            };
            pixbuf
                .scale_simple(new_w, new_h, gdk_pixbuf::InterpType::Bilinear)
                .map(|p| gtk::gdk::Texture::for_pixbuf(&p))
        });
    let _ = std::fs::remove_file(&path);
    result
}

fn texture_from_local_source(source: &str, max_size: i32) -> Option<gtk::gdk::Texture> {
    let try_bytes = |bytes: Vec<u8>| {
        texture_from_bytes(bytes.clone(), max_size)
            .or_else(|| texture_from_svg_bytes(bytes, max_size))
    };
    if source.starts_with('/') {
        let bytes = std::fs::read(source).ok()?;
        return try_bytes(bytes);
    }
    if std::path::Path::new(source).exists() {
        let bytes = std::fs::read(source).ok()?;
        return try_bytes(bytes);
    }
    if let Some(path) = source.strip_prefix("file://") {
        let bytes = std::fs::read(path).ok()?;
        return try_bytes(bytes);
    }

    if source.starts_with("data:") {
        if let Some(bytes) = decode_data_url(source) {
            return try_bytes(bytes);
        }
    }

    decode_raw_base64(source).and_then(|b| try_bytes(b))
}

fn icon_theme_paintable(picture: &gtk::Picture, source: &str) -> Option<gtk::IconPaintable> {
    let normalized = source.trim();
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("file://")
        || normalized.starts_with("data:")
    {
        return None;
    }

    let display = gtk::gdk::Display::default()?;
    let theme = gtk::IconTheme::for_display(&display);
    let icon_size = picture
        .width_request()
        .max(picture.height_request())
        .max(64);
    let paintable = theme.lookup_icon(
        normalized,
        &[],
        icon_size,
        1,
        gtk::TextDirection::None,
        gtk::IconLookupFlags::empty(),
    );

    Some(paintable)
}

async fn image_bytes_from_source(source: &str) -> Option<Vec<u8>> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let bytes = reqwest::get(source).await.ok()?.bytes().await.ok()?;
        return Some(bytes.to_vec());
    }
    if source.starts_with("data:") {
        return decode_data_url(source);
    }
    if source.starts_with('/') || source.starts_with("file://") {
        let path = source
            .strip_prefix("file://")
            .unwrap_or(source);
        return std::fs::read(path).ok();
    }
    decode_raw_base64(source)
}

pub fn set_picture_source(
    picture: &gtk::Picture,
    runtime: Arc<tokio::runtime::Runtime>,
    source: Option<String>,
    fallback: Option<String>,
) {
    let target_size = picture
        .width_request()
        .max(picture.height_request())
        .max(64);

    // Set fallback or placeholder first so the Picture never has a null paintable (avoids gtk_scaler_new assertion).
    if let Some(paintable) = fallback
        .as_deref()
        .and_then(|value| icon_theme_paintable(picture, value))
    {
        picture.set_paintable(Some(&paintable));
    } else if let Some(fb) = fallback.as_deref() {
        if let Some(texture) = texture_from_local_source(fb, target_size) {
            picture.set_paintable(Some(&texture));
        } else {
            picture.set_paintable(Some(placeholder_texture()));
        }
    } else {
        picture.set_paintable(Some(placeholder_texture()));
    }

    let source = source.filter(|value| !value.trim().is_empty());
    let Some(source) = source else {
        return;
    };

    if let Some(paintable) = icon_theme_paintable(picture, &source) {
        picture.set_paintable(Some(&paintable));
        return;
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    let fallback_for_async = fallback.clone();
    runtime.spawn(async move {
        if let Some(bytes) = image_bytes_from_source(&source).await {
            let _ = sender.send(Ok(bytes));
        } else {
            let _ = sender.send(Err(fallback_for_async));
        }
    });

    let picture = picture.clone();
    let placeholder = placeholder_texture();
    let fallback_for_timeout = fallback.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || match receiver
        .try_recv()
    {
        Ok(Ok(bytes)) => {
            if let Some(texture) = texture_from_bytes(bytes, target_size) {
                picture.set_paintable(Some(&texture));
            } else if let Some(fb) = fallback_for_timeout.as_deref() {
                if let Some(texture) = texture_from_local_source(fb, target_size) {
                    picture.set_paintable(Some(&texture));
                } else {
                    picture.set_paintable(Some(placeholder));
                }
            } else {
                picture.set_paintable(Some(placeholder));
            }
            glib::ControlFlow::Break
        }
        Ok(Err(fb)) => {
            if let Some(texture) = fb
                .as_deref()
                .and_then(|f| texture_from_local_source(f, target_size))
            {
                picture.set_paintable(Some(&texture));
            } else {
                picture.set_paintable(Some(placeholder));
            }
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
    });
}

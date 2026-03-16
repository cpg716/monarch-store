//! Minimal Aptabase-compatible telemetry for the GTK frontend.
//! Only sends events when `telemetry_enabled` is true in settings.
//! Matches the Tauri plugin payload shape so events appear in the same Aptabase project.

use monarch_core::settings::SettingsStore;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const APTABASE_APP_KEY: &str = "A-US-1496058535";
const APTABASE_URL: &str = "https://us.aptabase.com/api/v0/events";

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn session_id() -> String {
    let c = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("gtk-{:x}-{:x}", t, c)
}

fn event_category_and_label(event: &str) -> (&'static str, &'static str) {
    match event {
        "app_started" => ("lifecycle", "App started"),
        "store_installed" => ("lifecycle", "Store installed"),
        "search" | "search_query" => ("search", "Search"),
        "onboarding_completed" => ("engagement", "Onboarding completed"),
        "review_submitted" => ("engagement", "Review submitted"),
        "install_package" => ("install", "Package installed"),
        "uninstall_package" => ("install", "Package uninstalled"),
        "error_reported" => ("error", "Error reported"),
        _ => ("other", "other"),
    }
}

fn system_props() -> Value {
    let (os_name, os_version) = os_info();
    let locale = std::env::var("LANG").unwrap_or_else(|_| std::env::var("LC_ALL").unwrap_or_else(|_| "".into()));
    json!({
        "isDebug": cfg!(debug_assertions),
        "osName": os_name,
        "osVersion": os_version,
        "locale": locale,
        "engineName": "GTK4",
        "engineVersion": env!("CARGO_PKG_VERSION"),
        "appVersion": env!("CARGO_PKG_VERSION"),
        "sdkVersion": "monarch-gtk@".to_string() + env!("CARGO_PKG_VERSION")
    })
}

fn os_info() -> (String, String) {
    let name = std::env::var("ID").or_else(|_| std::env::var("DISTRIB_ID")).unwrap_or_else(|_| {
        #[cfg(target_os = "linux")]
        return "Linux".to_string();
        #[cfg(not(target_os = "linux"))]
        return std::env::consts::OS.to_string();
    });
    let version = std::env::var("VERSION_ID")
        .or_else(|_| std::env::var("DISTRIB_RELEASE"))
        .unwrap_or_else(|_| "".to_string());
    (name, version)
}

/// Sends one event to Aptabase if telemetry is enabled. Call from async context (e.g. context.runtime.spawn).
pub async fn track_event_async(
    settings: &SettingsStore,
    event_name: &str,
    payload: Option<Value>,
) {
    let Ok(gtk_settings) = settings.load() else {
        return;
    };
    if !gtk_settings.telemetry_enabled {
        return;
    }

    let (category, label) = event_category_and_label(event_name);
    let mut props: serde_json::Map<String, Value> = match payload.as_ref() {
        Some(Value::Object(m)) => m.clone(),
        _ => serde_json::Map::new(),
    };
    props.insert("event_category".to_string(), Value::String(category.to_string()));
    props.insert("event_label".to_string(), Value::String(label.to_string()));

    let body = json!([{
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "sessionId": session_id(),
        "eventName": event_name,
        "systemProps": system_props(),
        "props": Value::Object(props)
    }]);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let res = client
        .post(APTABASE_URL)
        .header("App-Key", APTABASE_APP_KEY)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;

    if let Err(e) = res {
        log::debug!("Aptabase track_event failed: {}", e);
    } else if let Ok(r) = res {
        if !r.status().is_success() {
            log::debug!("Aptabase track_event status: {}", r.status());
        }
    }
}

use serde_json::Value;
use tauri::AppHandle;

/// Tracks an event only if telemetry is enabled (consent enforced on backend).
#[tauri::command]
#[specta::specta]
pub async fn track_telemetry_event(
    app: AppHandle,
    event: String,
    payload: Option<Value>,
) -> Result<(), String> {
    crate::utils::track_event_safe(&app, &event, payload).await;
    Ok(())
}

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_aptabase::EventTracker;

#[tauri::command]
#[specta::specta]
pub fn track_telemetry_event(
    app: AppHandle,
    event: String,
    payload: Option<Value>,
) -> Result<(), String> {
    app.track_event(&event, payload).map_err(|e| e.to_string())
}

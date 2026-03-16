use crate::models::GtkSettings;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    cache: Mutex<GtkSettings>,
}

impl SettingsStore {
    pub fn new() -> Result<Self, String> {
        let path = config_dir()?.join("gtk-settings.json");
        let settings = read_json::<GtkSettings>(&path).unwrap_or_default();
        Ok(Self {
            path,
            cache: Mutex::new(settings),
        })
    }

    pub fn load(&self) -> Result<GtkSettings, String> {
        Ok(self.cache.lock().map_err(|e| e.to_string())?.clone())
    }

    pub fn update<F>(&self, mutator: F) -> Result<GtkSettings, String>
    where
        F: FnOnce(&mut GtkSettings),
    {
        let next = {
            let mut guard = self.cache.lock().map_err(|e| e.to_string())?;
            mutator(&mut guard);
            guard.clone()
        };
        write_json(&self.path, &next)?;
        Ok(next)
    }

    pub fn mark_news_read(&self, ids: &[String]) -> Result<GtkSettings, String> {
        self.update(|state| {
            for id in ids {
                let normalized = id.trim().to_string();
                if !normalized.is_empty() && !state.read_news_ids.iter().any(|value| value == &normalized) {
                    state.read_news_ids.push(normalized);
                }
            }
            state.read_news_ids.sort();
            state.read_news_ids.dedup();
        })
    }

    pub fn is_news_read(&self, id: &str) -> Result<bool, String> {
        let normalized = id.trim();
        if normalized.is_empty() {
            return Ok(false);
        }
        Ok(self
            .cache
            .lock()
            .map_err(|e| e.to_string())?
            .read_news_ids
            .iter()
            .any(|value| value == normalized))
    }

    pub fn set_onboarding_completed(&self, completed: bool) -> Result<GtkSettings, String> {
        self.update(|state| state.onboarding_completed = completed)
    }

    pub fn set_sidebar_expanded(&self, expanded: bool) -> Result<GtkSettings, String> {
        self.update(|state| state.sidebar_expanded = expanded)
    }

    pub fn set_active_tab(&self, tab: impl Into<String>) -> Result<GtkSettings, String> {
        let tab = tab.into();
        self.update(|state| {
            state.active_tab = tab.trim().to_string();
        })
    }

    pub fn push_search_history(&self, query: impl Into<String>) -> Result<GtkSettings, String> {
        let query = query.into();
        self.update(|state| {
            let normalized = query.trim().to_string();
            if normalized.is_empty() {
                return;
            }
            state.search_history.retain(|value| value != &normalized);
            state.search_history.insert(0, normalized);
            state.search_history.truncate(12);
        })
    }

    pub fn set_alpha_notice_dismissed(&self, dismissed: bool) -> Result<GtkSettings, String> {
        self.update(|state| state.alpha_notice_dismissed = dismissed)
    }
}

fn config_dir() -> Result<PathBuf, String> {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("monarch-store");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Option<T> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_json<T: serde::Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

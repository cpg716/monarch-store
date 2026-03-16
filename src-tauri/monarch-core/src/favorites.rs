use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug)]
pub struct FavoritesStore {
    path: PathBuf,
    cache: Mutex<Vec<String>>,
}

impl FavoritesStore {
    pub fn new() -> Result<Self, String> {
        let path = config_dir()?.join("favorites.json");
        let favorites = read_json::<Vec<String>>(&path).unwrap_or_default();
        Ok(Self {
            path,
            cache: Mutex::new(normalize_ids(favorites)),
        })
    }

    pub fn list(&self) -> Result<Vec<String>, String> {
        Ok(self.cache.lock().map_err(|e| e.to_string())?.clone())
    }

    pub fn contains(&self, canonical_id: &str) -> Result<bool, String> {
        let normalized = canonical_id.trim().to_lowercase();
        if normalized.is_empty() {
            return Ok(false);
        }

        Ok(self
            .cache
            .lock()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|value| value == &normalized))
    }

    pub fn toggle(&self, canonical_id: &str) -> Result<Vec<String>, String> {
        let normalized = canonical_id.trim().to_lowercase();
        if normalized.is_empty() {
            return self.list();
        }

        let next = {
            let mut guard = self.cache.lock().map_err(|e| e.to_string())?;
            if let Some(index) = guard.iter().position(|value| value == &normalized) {
                guard.remove(index);
            } else {
                guard.push(normalized);
                guard.sort();
                guard.dedup();
            }
            guard.clone()
        };

        write_json(&self.path, &next)?;
        Ok(next)
    }
}

fn normalize_ids(ids: Vec<String>) -> Vec<String> {
    let mut ids = ids
        .into_iter()
        .map(|id| id.trim().to_lowercase())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
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

use monarch_core::catalog::CatalogService;
use monarch_core::favorites::FavoritesStore;
use monarch_core::models::{HomeSnapshot, NewsItem, SettingsView, StartupStatus};
use monarch_core::news;
use monarch_core::registry::RegistryManager;
use monarch_core::settings::SettingsStore;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppContext {
    pub catalog: Arc<CatalogService>,
    pub favorites: Arc<FavoritesStore>,
    pub settings: Arc<SettingsStore>,
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub refresh_epoch: Arc<AtomicU64>,
    toast_tx: Arc<Mutex<Option<mpsc::Sender<String>>>>,
}

impl AppContext {
    pub fn new() -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("monarch-gtk")
            .build()
            .map_err(|e| e.to_string())?;

        let registry = Arc::new(RegistryManager::new()?);
        let favorites = Arc::new(FavoritesStore::new()?);
        let settings = Arc::new(SettingsStore::new()?);
        let catalog = Arc::new(CatalogService::new_with_settings(
            registry,
            settings.clone(),
        ));

        Ok(Self {
            catalog,
            favorites,
            settings,
            runtime: Arc::new(runtime),
            refresh_epoch: Arc::new(AtomicU64::new(0)),
            toast_tx: Arc::new(Mutex::new(None)),
        })
    }

    /// Called from the UI when the toast overlay is ready. Do not call from application code.
    pub fn set_toast_sender(&self, tx: mpsc::Sender<String>) {
        if let Ok(mut guard) = self.toast_tx.lock() {
            *guard = Some(tx);
        }
    }

    /// Show a short-lived toast message (e.g. "Settings saved", "Refresh failed").
    pub fn show_toast(&self, msg: &str) {
        if let Ok(guard) = self.toast_tx.lock() {
            if let Some(ref tx) = *guard {
                let _ = tx.send(msg.to_string());
            }
        }
    }

    pub fn mark_catalog_dirty(&self) {
        self.refresh_epoch.fetch_add(1, Ordering::Relaxed);
    }

    pub fn refresh_epoch(&self) -> u64 {
        self.refresh_epoch.load(Ordering::Relaxed)
    }

    pub async fn fetch_news(&self) -> Result<Vec<NewsItem>, String> {
        news::fetch_news().await
    }

    pub async fn fetch_home_snapshot(&self) -> Result<HomeSnapshot, String> {
        self.catalog.load_home_snapshot().await
    }

    pub async fn fetch_startup_status(&self) -> Result<StartupStatus, String> {
        self.catalog.startup_status().await
    }

    pub async fn fetch_settings_view(&self) -> Result<SettingsView, String> {
        self.catalog.load_settings_view().await
    }
}

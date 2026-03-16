use monarch_core::catalog::CatalogService;
use monarch_core::models::SearchOptions;
use monarch_core::registry::RegistryManager;
use std::sync::Arc;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let registry = Arc::new(RegistryManager::new().unwrap());
    let catalog = CatalogService::new(registry);

    let options = SearchOptions {
        flatpak_enabled: Some(true),
        aur_enabled: Some(true),
        chaotic_enabled: Some(true),
        show_system_apps: Some(true),
        source_filter: None,
        category_filter: None,
        installed_only: Some(false),
        sort_mode: None,
        for_installed_lookup: Some(false),
    };

    let t1 = Instant::now();
    match catalog.search("firefox", options).await {
        Ok(pkgs) => println!(
            "search() total: {:?}, {} packages",
            t1.elapsed(),
            pkgs.len()
        ),
        Err(e) => println!("search error: {} in {:?}", e, t1.elapsed()),
    }
}

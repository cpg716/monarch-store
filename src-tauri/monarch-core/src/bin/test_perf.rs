use std::time::Instant;
fn main() {
    let registry = monarch_core::registry::RegistryManager::new().unwrap();
    let start = Instant::now();
    match registry.search_packages_sql("a", 500) {
        Ok(pkgs) => println!("Found {} packages in {:?}", pkgs.len(), start.elapsed()),
        Err(e) => println!("Error: {}", e),
    }
}

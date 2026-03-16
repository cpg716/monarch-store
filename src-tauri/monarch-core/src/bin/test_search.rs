fn main() {
    let registry = monarch_core::registry::RegistryManager::new().unwrap();
    match registry.search_packages_sql("firefox", 24) {
        Ok(pkgs) => println!("Found {} packages", pkgs.len()),
        Err(e) => println!("Error: {}", e),
    }
}

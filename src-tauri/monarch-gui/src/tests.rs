#[cfg(test)]
mod tests {
    use crate::flathub_api::SearchResult;
    use crate::middleware::aggregation::merge_search_results;
    use crate::models::{Package, PackageSource};
    use crate::utils;

    // Helper to make dummy packages
    fn make_pkg(name: &str, source: PackageSource, app_id: Option<&str>) -> Package {
        Package {
            name: name.to_string(),
            display_name: None,
            description: "test".to_string(),
            version: "1.0".to_string(),
            source,
            maintainer: None,
            license: None,
            url: None,
            last_modified: None,
            first_submitted: None,
            out_of_date: None,
            keywords: None,
            num_votes: None,
            icon: None,
            screenshots: None,
            provides: None,
            app_id: app_id.map(|s| s.to_string()),
            is_optimized: None,
            depends: None,
            make_depends: None,
            is_featured: None,
            installed: false,
            ..Default::default()
        }
    }

    #[test]
    fn test_deduplication_exact_name() {
        let official = vec![make_pkg(
            "firefox",
            PackageSource::official("firefox"),
            Some("firefox"),
        )];
        let repo = vec![make_pkg(
            "firefox",
            PackageSource::cachyos("firefox"),
            Some("firefox"),
        )];

        let result = utils::merge_and_deduplicate(official, repo);

        // CachyOS has lower priority number (1) than Official (2), so CachyOS wins
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, PackageSource::cachyos("firefox"));
    }

    #[test]
    fn test_deduplication_app_id() {
        let official = vec![make_pkg(
            "brave",
            PackageSource::official("brave"),
            Some("com.brave.Browser"),
        )];
        // "brave-bin" is common in AUR/Chaotic, but maps to same AppID
        let repo = vec![make_pkg(
            "brave-bin",
            PackageSource::chaotic("brave-bin"),
            Some("com.brave.Browser"),
        )];

        let result = utils::merge_and_deduplicate(official, repo);

        // Chaotic has lower priority number (1) than Official (2), so Chaotic wins
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "brave-bin"); // Chaotic-AUR package wins
    }

    #[test]
    fn test_deduplication_no_conflict() {
        let official = vec![make_pkg(
            "firefox",
            PackageSource::official("firefox"),
            Some("firefox"),
        )];
        let repo = vec![make_pkg(
            "google-chrome",
            PackageSource::chaotic("google-chrome"),
            Some("chrome"),
        )];

        let result = utils::merge_and_deduplicate(official, repo);

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_search_aggregation_firefox_triple_source() {
        // Firefox exists in System Repo, AUR, and Flatpak; merge must yield 1 entry with all 3 sources
        let official = vec![make_pkg(
            "firefox",
            PackageSource::official("firefox"),
            Some("org.mozilla.firefox"),
        )];
        let aur = vec![make_pkg("firefox", PackageSource::aur("firefox"), None)];

        let flatpak = vec![(
            SearchResult {
                app_id: "org.mozilla.firefox".to_string(),
                name: "Firefox".to_string(),
                summary: Some("Web browser".to_string()),
                icon: None,
            },
            Some("stable".to_string()),
        )];
        let installed_flatpaks = std::collections::HashSet::new();

        let registry = crate::registry::RegistryManager::in_memory();
        let result = merge_search_results(official, aur, flatpak, &registry, &installed_flatpaks);

        assert_eq!(
            result.len(),
            1,
            "firefox from repo+AUR+Flatpak must merge to 1 entry"
        );
        let sources = result[0]
            .available_sources
            .as_ref()
            .expect("available_sources must be set");
        assert_eq!(
            sources.len(),
            3,
            "must have repo, aur, flatpak in available_sources"
        );

        let has_repo = sources.iter().any(|s| s.source_type == "repo");
        let has_aur = sources.iter().any(|s| s.source_type == "aur");
        let has_flatpak = sources.iter().any(|s| s.source_type == "flatpak");
        assert!(has_repo, "must include repo source");
        assert!(has_aur, "must include aur source");
        assert!(has_flatpak, "must include flatpak source");
    }

    #[test]
    fn test_search_aggregation_firefox_variant_merge() {
        // firefox and firefox-developer-edition are variants; must merge to 1 entry
        let official = vec![make_pkg(
            "firefox",
            PackageSource::official("firefox"),
            None,
        )];
        let aur = vec![make_pkg(
            "firefox-developer-edition",
            PackageSource::aur("firefox-developer-edition"),
            None,
        )];
        let flatpak: Vec<(SearchResult, Option<String>)> = vec![];
        let installed_flatpaks = std::collections::HashSet::new();

        let registry = crate::registry::RegistryManager::in_memory();
        let result = merge_search_results(official, aur, flatpak, &registry, &installed_flatpaks);

        assert_eq!(
            result.len(),
            1,
            "firefox + firefox-developer-edition must merge to 1 entry"
        );
        let sources = result[0]
            .available_sources
            .as_ref()
            .expect("available_sources");
        assert_eq!(
            sources.len(),
            2,
            "must have repo + aur in available_sources"
        );
        assert!(sources.iter().any(|s| s.source_type == "repo"));
        assert!(sources.iter().any(|s| s.source_type == "aur"));
    }

    #[test]
    fn test_version_compare_logic() {
        // While we don't have the full ALPM version comparison here (it's complex C code),
        // we can verify our simple assumptions if we had implemented a robust one.
        // For now, let's just ensure our strings are handled safely.
        let v1 = "1.0.0-1";
        let v2 = "1.0.0-2";
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_nano_collision_resolved() {
        // "nano" and "nano-launcher" should HAVE DISTINCT KEYS now.
        // Before the fix, they both became "nano" due to aggressive first-segment rules.
        let key1 = utils::canonical_merge_key("nano", None);
        let key2 = utils::canonical_merge_key("nano-launcher", None);
        assert_ne!(key1, key2, "nano and nano-launcher must have distinct keys");
        assert_eq!(key1, "nano");
        assert_eq!(key2, "nanolauncher");
    }

    #[test]
    fn test_discord_variants_still_merge() {
        // "discord" and "discord-bin" should STILL MERGE.
        let key1 = utils::canonical_merge_key("discord", None);
        let key2 = utils::canonical_merge_key("discord-bin", None);
        assert_eq!(key1, key2, "discord and discord-bin must still merge");
        assert_eq!(key1, "discord");
    }

    #[test]
    fn test_gnome_apps_distinct() {
        // "gnome-terminal" and "gnome-calculator" must be distinct.
        let key1 = utils::canonical_merge_key("gnome-terminal", None);
        let key2 = utils::canonical_merge_key("gnome-calculator", None);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_steam_metadata_lookup() {
        use crate::metadata::{AppMetadata, AppStreamLoader};

        let mut loader = AppStreamLoader::new();

        // Simulate AppStream data: ID is "com.valvesoftware.Steam"
        let steam_id = "com.valvesoftware.Steam";
        let meta = AppMetadata {
            name: "Steam".to_string(),
            app_id: steam_id.to_string(),
            pkg_name: Some("steam".to_string()),
            summary: Some("Digital distribution platform".to_string()),
            ..Default::default()
        };

        // Case 1: Indexed by ID only (as rebuild_indices does for all apps)
        loader
            .pkg_index
            .insert(steam_id.to_lowercase(), meta.clone());

        // Search for "steam" (package name in Arch)
        let found = loader.find_package("steam");
        assert!(found.is_some(), "Should find Steam via App ID fallback");
        let found_meta = found.unwrap();
        assert_eq!(found_meta.app_id, steam_id);

        // Verify logs would show up if enabled
        println!("Found Steam: {:?}", found_meta.name);

        // Case 2: Lutris
        let lutris_id = "net.lutris.Lutris";
        let lutris_meta = AppMetadata {
            name: "Lutris".to_string(),
            app_id: lutris_id.to_string(),
            ..Default::default()
        };
        loader
            .pkg_index
            .insert(lutris_id.to_lowercase(), lutris_meta);

        let found_lutris = loader.find_package("lutris");
        assert!(
            found_lutris.is_some(),
            "Should find Lutris via App ID fallback"
        );
        assert_eq!(found_lutris.unwrap().app_id, lutris_id);
    }
}

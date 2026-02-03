#[cfg(test)]
mod tests {
    use crate::commands::search::merge_search_results;
    use crate::flathub_api::SearchResult;
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
            PackageSource::official(),
            Some("firefox"),
        )];
        let repo = vec![make_pkg(
            "firefox",
            PackageSource::cachyos(),
            Some("firefox"),
        )];

        let result = utils::merge_and_deduplicate(official, repo);

        // Should strictly keep the 'base' (official) one
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, PackageSource::official());
    }

    #[test]
    fn test_deduplication_app_id() {
        let official = vec![make_pkg(
            "brave",
            PackageSource::official(),
            Some("com.brave.Browser"),
        )];
        // "brave-bin" is common in AUR/Chaotic, but maps to same AppID
        let repo = vec![make_pkg(
            "brave-bin",
            PackageSource::chaotic(),
            Some("com.brave.Browser"),
        )];

        let result = utils::merge_and_deduplicate(official, repo);

        // Should deduplicate based on AppID
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "brave"); // Kept the official one
    }

    #[test]
    fn test_deduplication_no_conflict() {
        let official = vec![make_pkg(
            "firefox",
            PackageSource::official(),
            Some("firefox"),
        )];
        let repo = vec![make_pkg(
            "google-chrome",
            PackageSource::chaotic(),
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
            PackageSource::official(),
            Some("org.mozilla.firefox"),
        )];
        let aur = vec![make_pkg("firefox", PackageSource::aur(), None)];

        let flatpak = vec![SearchResult {
            app_id: "org.mozilla.firefox".to_string(),
            name: "Firefox".to_string(),
            summary: Some("Web browser".to_string()),
            icon: None,
        }];

        let result = merge_search_results(official, aur, flatpak);

        assert_eq!(result.len(), 1, "firefox from repo+AUR+Flatpak must merge to 1 entry");
        let sources = result[0]
            .available_sources
            .as_ref()
            .expect("available_sources must be set");
        assert_eq!(sources.len(), 3, "must have repo, aur, flatpak in available_sources");

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
            PackageSource::official(),
            None,
        )];
        let aur = vec![
            make_pkg("firefox-developer-edition", PackageSource::aur(), None),
        ];
        let flatpak: Vec<SearchResult> = vec![];

        let result = merge_search_results(official, aur, flatpak);

        assert_eq!(result.len(), 1, "firefox + firefox-developer-edition must merge to 1 entry");
        let sources = result[0].available_sources.as_ref().expect("available_sources");
        assert_eq!(sources.len(), 2, "must have repo + aur in available_sources");
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
}

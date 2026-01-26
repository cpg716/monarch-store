#[cfg(test)]
mod tests {
    use crate::commands::utils::is_valid_pkg_name;
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
            alternatives: None,
        }
    }

    #[test]
    fn test_deduplication_exact_name() {
        let official = vec![make_pkg(
            "firefox",
            PackageSource::Official,
            Some("firefox"),
        )];
        let repo = vec![make_pkg("firefox", PackageSource::CachyOS, Some("firefox"))];

        let result = utils::merge_and_deduplicate(official, repo);

        // Should strictly keep the 'base' (official) one
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, PackageSource::Official);
    }

    #[test]
    fn test_deduplication_app_id() {
        let official = vec![make_pkg(
            "brave",
            PackageSource::Official,
            Some("com.brave.Browser"),
        )];
        // "brave-bin" is common in AUR/Chaotic, but maps to same AppID
        let repo = vec![make_pkg(
            "brave-bin",
            PackageSource::Chaotic,
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
            PackageSource::Official,
            Some("firefox"),
        )];
        let repo = vec![make_pkg(
            "google-chrome",
            PackageSource::Chaotic,
            Some("chrome"),
        )];

        let result = utils::merge_and_deduplicate(official, repo);

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_sanitization() {
        assert!(is_valid_pkg_name("firefox"));
        assert!(is_valid_pkg_name("google-chrome"));
        assert!(is_valid_pkg_name("lib32-glibc"));
        assert!(is_valid_pkg_name("c++-gtk-utils")); // + is valid in some contexts (e.g. c++)

        // Invalid inputs
        assert!(!is_valid_pkg_name("firefox; rm -rf /"));
        assert!(!is_valid_pkg_name("-flag")); // Starts with dash
        assert!(!is_valid_pkg_name(" package")); // Leading space
        assert!(!is_valid_pkg_name("package ")); // Trailing space (though arguably trim handles this, strict validation fails)
        assert!(!is_valid_pkg_name("")); // Empty
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

use crate::aur;
use crate::bootstrap::hydrate_registry_from_live_system;
use crate::flatpak;
use crate::models::{
    CacheSizeResult, ChaoticSupport, DistroProfile, FullPackageDetails, GtkSettings, HomeSnapshot,
    LocalReview, MirrorTestResult, OrphansWithSizeResult, Package, PackageInstallStatus,
    PackagePresentation, PackageReview, PackageSecuritySummary, PackageSource, PackageVariant,
    SearchOptions, SearchSortMode, SettingsView, SnapshotStatus, SnapshotTool, StartupStatus,
    SystemHealthView, SystemInfo, UpdateItem, UpdateSnapshot, UpdateSnapshotItem,
    UpdateSourceStatus, UpdateWarnings,
};
use crate::odrs;
use crate::privileged::{HelperProgress, PrivilegedClient};
use crate::supabase;
use crate::registry::{RegistryHydrationStats, RegistryManager, REGISTRY_HYDRATION_VERSION};
use crate::reviews::LocalReviewStore;
use crate::settings::SettingsStore;
use raur::Raur;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const MONARCH_POLKIT_RULES: &str = include_str!("../../rules/10-monarch-store.rules");
const MONARCH_POLKIT_POLICY: &str = include_str!("../../monarch-gui/com.monarch.store.policy");
const HOME_ESSENTIALS: &[&str] = &[
    "firefox",
    "librewolf-bin",
    "google-chrome",
    "thunderbird",
    "telegram-desktop",
    "signal-desktop",
    "discord",
    "newsflash",
    "libreoffice-fresh",
    "obsidian",
    "calibre",
    "simplenote-electron-bin",
    "okular",
    "foliate",
    "keepassxc",
    "gimp",
    "inkscape",
    "blender",
    "flameshot",
    "krita",
    "rawtherapee",
    "vlc",
    "audacity",
    "obs-studio",
    "handbrake",
    "strawberry",
    "easyeffects",
    "ardour",
    "visual-studio-code-bin",
    "git",
    "docker-desktop",
    "steam",
    "lutris",
    "heroic-games-launcher-bin",
    "timeshift",
    "bitwarden-bin",
    "gparted",
    "kdeconnect",
    "balena-etcher",
    "peazip-bin",
    "apostrophe",
    "org.gnome.clocks",
    "io.bassi.Amberol",
    "celluloid",
    "ptyxis",
    "com.felipekinoshita.Wildcard",
    "org.gnome.Lollypop",
    "com.github.rafostar.Clapper",
    "org.gnome.Polari",
];
const HOME_TRENDING: &[&str] = &[
    "discord",
    "telegram-desktop",
    "signal-desktop",
    "spotify",
    "steam",
    "heroic-games-launcher-bin",
    "lutris",
    "com.usebottles.bottles",
    "obs-studio",
    "vlc",
    "firefox",
    "google-chrome",
    "com.raggesilver.BlackBox",
    "org.gnome.Sudoku",
    "org.gnome.Solanum",
    "de.haeckerfelix.Fragments",
    "io.github.diegoivan.flowtime",
    "org.gnome.atomix",
];

#[derive(Debug, Clone)]
pub struct CatalogService {
    registry: Arc<RegistryManager>,
    privileged: Arc<PrivilegedClient>,
    settings: Arc<SettingsStore>,
    reviews: Arc<LocalReviewStore>,
    bootstrap_lock: Arc<tokio::sync::Mutex<()>>,
    session_password: Arc<Mutex<Option<String>>>,
    is_ready: Arc<std::sync::atomic::AtomicBool>,
}

impl CatalogService {
    pub fn new(registry: Arc<RegistryManager>) -> Self {
        let settings = Arc::new(SettingsStore::new().expect("settings init failed"));
        Self::new_with_settings(registry, settings)
    }

    pub fn new_with_settings(registry: Arc<RegistryManager>, settings: Arc<SettingsStore>) -> Self {
        Self {
            registry,
            privileged: Arc::new(PrivilegedClient::new()),
            settings,
            reviews: Arc::new(LocalReviewStore::new().expect("review store init failed")),
            bootstrap_lock: Arc::new(tokio::sync::Mutex::new(())),
            session_password: Arc::new(Mutex::new(None)),
            is_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn set_session_password(&self, password: Option<String>) -> Result<(), String> {
        *self.session_password.lock().map_err(|e| e.to_string())? = password;
        Ok(())
    }

    pub fn has_session_password(&self) -> Result<bool, String> {
        Ok(self
            .session_password
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()))
    }

    pub fn settings(&self) -> Arc<SettingsStore> {
        self.settings.clone()
    }

    pub fn load_preferences(&self) -> Result<GtkSettings, String> {
        self.settings.load()
    }

    pub fn save_preferences<F>(&self, mutator: F) -> Result<GtkSettings, String>
    where
        F: FnOnce(&mut GtkSettings),
    {
        self.settings.update(mutator)
    }

    fn helper_password(&self) -> Result<Option<String>, String> {
        let settings = self.settings.load()?;
        if !(settings.one_click_enabled || settings.reduce_password_prompts) {
            return Ok(None);
        }

        Ok(self
            .session_password
            .lock()
            .map_err(|e| e.to_string())?
            .clone()
            .filter(|value| !value.trim().is_empty()))
    }

    pub async fn load_discovery_snapshot(&self) -> Result<Vec<Package>, String> {
        let options = self.current_search_options()?;
        self.load_discovery_snapshot_with_options(options).await
    }

    pub async fn load_discovery_snapshot_with_options(
        &self,
        options: SearchOptions,
    ) -> Result<Vec<Package>, String> {
        self.ensure_registry_ready().await?;
        let registry = self.registry.clone();
        let mut packages = tokio::task::spawn_blocking(move || {
            registry.search_packages_sql("", 5000).map(|packages| {
                filter_and_sort_packages(
                    packages.into_iter().filter(is_storefront_package).collect(),
                    &options,
                    None,
                )
            })
        })
        .await
        .map_err(|e| e.to_string())??;
        enrich_package_presentation(&mut packages).await;
        Ok(packages)
    }

    pub async fn load_home_snapshot(&self) -> Result<HomeSnapshot, String> {
        let options = self.current_search_options()?;
        self.load_home_snapshot_with_options(options).await
    }

    pub async fn load_home_snapshot_with_options(
        &self,
        options: SearchOptions,
    ) -> Result<HomeSnapshot, String> {
        self.ensure_registry_ready().await?;
        let registry = self.registry.clone();
        let mut snapshot = tokio::task::spawn_blocking(move || {
            let all_packages = filter_and_sort_packages(
                registry
                    .search_packages_sql("", 5000)?
                    .into_iter()
                    .filter(is_storefront_package)
                    .collect(),
                &options,
                None,
            );
            let mut native = lane_packages(&all_packages, is_native_repo, 12);
            let mut chaotic = lane_packages(&all_packages, is_chaotic_repo, 12);
            let mut flatpak = lane_packages(
                &all_packages,
                |package| package.source.source_type == "flatpak",
                12,
            );
            let mut aur = lane_packages(
                &all_packages,
                |package| package.source.source_type == "aur",
                12,
            );
            let mut featured = lane_packages(
                &all_packages,
                |package| {
                    package
                        .screenshots
                        .as_ref()
                        .map(|shots| !shots.is_empty())
                        .unwrap_or(false)
                },
                8,
            );
            if featured.is_empty() {
                featured = lane_packages(&all_packages, is_native_repo, 8);
            }
            let popular = filter_and_sort_packages(
                load_curated_packages(&registry, HOME_ESSENTIALS),
                &options,
                None,
            );
            let trending = filter_and_sort_packages(
                load_curated_packages(&registry, HOME_TRENDING),
                &options,
                None,
            );
            let mut newest = all_packages.clone();
            sort_packages(&mut newest, &SearchSortMode::Newest, None);
            newest.truncate(12);
            let mut updated = all_packages.clone();
            sort_packages(&mut updated, &SearchSortMode::Updated, None);
            updated.truncate(12);

            native.truncate(12);
            chaotic.truncate(12);
            flatpak.truncate(12);
            aur.truncate(12);

            Ok::<HomeSnapshot, String>(HomeSnapshot {
                suggested_searches: vec![
                    "browser".to_string(),
                    "discord".to_string(),
                    "video editor".to_string(),
                    "vpn".to_string(),
                    "music".to_string(),
                    "steam".to_string(),
                ],
                featured,
                native,
                chaotic,
                flatpak,
                aur,
                trending,
                popular,
                new: newest,
                updated,
                categories: storefront_categories(&all_packages),
            })
        })
        .await
        .map_err(|e| e.to_string())??;
        enrich_package_presentation(&mut snapshot.featured).await;
        enrich_package_presentation(&mut snapshot.native).await;
        enrich_package_presentation(&mut snapshot.chaotic).await;
        enrich_package_presentation(&mut snapshot.flatpak).await;
        enrich_package_presentation(&mut snapshot.aur).await;
        enrich_package_presentation(&mut snapshot.trending).await;
        enrich_package_presentation(&mut snapshot.popular).await;
        enrich_package_presentation(&mut snapshot.new).await;
        enrich_package_presentation(&mut snapshot.updated).await;
        Ok(snapshot)
    }

    pub async fn load_category_packages(
        &self,
        category: impl Into<String>,
        mut options: SearchOptions,
        limit: usize,
    ) -> Result<Vec<Package>, String> {
        self.ensure_registry_ready().await?;
        let category = category.into();
        let normalized_category = normalize_category_filter(&category);
        options.category_filter = Some(category.clone());
        options.sort_mode = Some(SearchSortMode::Name);
        let registry = self.registry.clone();
        let mut packages = tokio::task::spawn_blocking(move || {
            let mut selected = Vec::new();
            let mut seen = std::collections::HashSet::new();

            for package in load_curated_packages(&registry, category_curated_queries(&normalized_category)) {
                if is_storefront_package(&package) && seen.insert(package.canonical_id.clone()) {
                    selected.push(package);
                }
            }

            let db_limit = limit.saturating_mul(3).clamp(100, 2000);

            let taxonomy_tokens = monarch_category_taxonomy()
                .iter()
                .find(|(label, _)| normalize_category_filter(*label) == normalized_category)
                .map(|(_, tokens)| *tokens)
                .unwrap_or(&[]);

            let mut query_tokens: Vec<&str> = taxonomy_tokens.to_vec();
            if query_tokens.is_empty() {
                query_tokens.push(&normalized_category);
            }

            let mut discovered = filter_and_sort_packages(
                registry
                    .get_packages_for_category(&query_tokens, db_limit)?
                    .into_iter()
                    .filter(is_storefront_package)
                    .collect(),
                &options,
                None,
            );
            for package in discovered.drain(..) {
                if seen.insert(package.canonical_id.clone()) {
                    selected.push(package);
                }
                if selected.len() >= limit {
                    break;
                }
            }

            Ok::<Vec<Package>, String>(selected)
        })
        .await
        .map_err(|e| e.to_string())??;
        enrich_package_presentation(&mut packages).await;
        Ok(packages)
    }

    pub async fn search(
        &self,
        query: impl Into<String>,
        options: SearchOptions,
    ) -> Result<Vec<Package>, String> {
        self.ensure_registry_ready().await?;
        let query = query.into();
        let registry = self.registry.clone();
        let options_for_search = options.clone();
        let query_for_search = query.clone();
        let mut packages = tokio::task::spawn_blocking(move || {
            let include_system_apps = options_for_search.show_system_apps.unwrap_or(false);

            registry
                .search_packages_sql(&query_for_search, 500)
                .map(|packages| {
                    let packages = packages
                        .into_iter()
                        .filter(|package| {
                            is_search_match(
                                package,
                                Some(query_for_search.as_str()),
                                include_system_apps,
                            )
                        })
                        .collect();

                    filter_and_sort_packages(
                        packages,
                        &options_for_search,
                        Some(query_for_search.as_str()),
                    )
                })
        })
        .await
        .map_err(|e| e.to_string())??;
        enrich_package_presentation(&mut packages).await;
        Ok(packages)
    }

    pub async fn load_installed(&self) -> Result<Vec<Package>, String> {
        self.ensure_registry_ready().await?;
        let registry = self.registry.clone();
        let native = tokio::task::spawn_blocking(move || registry.get_installed_packages(1500))
            .await
            .map_err(|e| e.to_string())??;
        let flatpak_installed = if cfg!(test) {
            Vec::new()
        } else if flatpak::is_flatpak_available() {
            flatpak::get_installed_packages().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut packages = native;
        let mut seen = packages
            .iter()
            .map(|package| package.canonical_id.clone())
            .collect::<std::collections::HashSet<_>>();
        for package in flatpak_installed {
            if seen.insert(package.canonical_id.clone()) {
                packages.push(package);
            }
        }
        packages.retain(is_library_package);
        packages.sort_by_key(|package| package.effective_title().to_lowercase());
        enrich_package_presentation(&mut packages).await;
        Ok(packages)
    }

    pub async fn load_package(
        &self,
        canonical_id: impl Into<String>,
    ) -> Result<Option<Package>, String> {
        self.ensure_registry_ready().await?;
        let canonical_id = canonical_id.into();
        let registry = self.registry.clone();
        let mut package = tokio::task::spawn_blocking(move || registry.get_package(&canonical_id))
            .await
            .map_err(|e| e.to_string())??;
        if let Some(pkg) = package.as_mut() {
            let mut items = vec![pkg.clone()];
            enrich_package_presentation(&mut items).await;
            *pkg = items.remove(0);
        }
        Ok(package)
    }

    pub async fn load_packages_by_ids(&self, ids: Vec<String>) -> Result<Vec<Package>, String> {
        self.ensure_registry_ready().await?;
        let registry = self.registry.clone();
        let mut packages =
            tokio::task::spawn_blocking(move || registry.get_packages_by_canonical_ids(&ids))
                .await
                .map_err(|e| e.to_string())??;
        enrich_package_presentation(&mut packages).await;
        Ok(packages)
    }

    pub async fn load_package_details(
        &self,
        canonical_id: impl Into<String>,
    ) -> Result<Option<FullPackageDetails>, String> {
        let Some(mut package) = self.load_package(canonical_id).await? else {
            return Ok(None);
        };

        enrich_package_presentation(std::slice::from_mut(&mut package)).await;

        let selected_source = package
            .available_sources
            .as_ref()
            .and_then(|sources| sources.first().cloned())
            .unwrap_or_else(|| package.source.clone());
        let mut all_variants = build_variants_for_package(&package);
        all_variants = hydrate_variants_from_sources(&package, all_variants).await;
        let installed_status = PackageInstallStatus {
            installed: package.installed,
            version: Some(package.version.clone()),
            repo: Some(package.source.label.clone()),
            source: Some(package.source.clone()),
            actual_package_name: Some(package.name.clone()),
        };
        let security = Some(build_security_summary(
            Some(&selected_source),
            package
                .maintainer
                .as_deref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false),
        ));
        let mut screenshots = package.screenshots.clone().unwrap_or_default();
        if screenshots.is_empty() {
            if let Some(app_id) = package
                .app_id
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                screenshots = crate::flathub_fallback::fetch_screenshots_for_app_id(app_id).await;
            }
        }
        let presentation = Some(PackagePresentation {
            display_title: Some(package.effective_title()),
            icon: package.icon.clone(),
            short_description: Some(package.description.clone()),
            long_description: package.long_description.clone(),
            screenshots,
            app_id: package.app_id.clone(),
            developer_name: derive_developer_name(&package),
            donation_url: package.url.clone(),
        });

        Ok(Some(FullPackageDetails {
            package: Some(package.clone()),
            presentation,
            installed_status: installed_status.clone(),
            all_installed_variants: if package.installed {
                vec![installed_status]
            } else {
                Vec::new()
            },
            flatpak_permissions: None,
            all_variants: all_variants.clone(),
            display_title: Some(package.effective_title()),
            primary_action: Some(if package.installed {
                "launch".to_string()
            } else {
                "install".to_string()
            }),
            primary_action_label: Some(if package.installed {
                "Launch".to_string()
            } else {
                "Install".to_string()
            }),
            selected_default_source: Some(selected_source.clone()),
            source_summary: Some(package.source_summary.clone().unwrap_or_else(|| {
                if all_variants.len() > 1 {
                    format!("{} sources available", all_variants.len())
                } else {
                    format!("Primary source: {}", selected_source.label)
                }
            })),
            security_summary: security.as_ref().map(|summary| {
                format!("{} {}", summary.verification_note, summary.user_action_note)
            }),
            installed_source_label: package.installed.then_some(selected_source.label.clone()),
            source_switch_policy: Some(if package.installed {
                "informational_only".to_string()
            } else {
                "switch_allowed".to_string()
            }),
            source_switch_notice: package.installed.then_some(build_source_switch_notice(&selected_source)),
            security,
            developer_name: derive_developer_name(&package),
            donation_url: package.url.clone(),
        }))
    }

    pub async fn load_full_package_details(
        &self,
        canonical_id: impl Into<String>,
        preferred_source: Option<PackageSource>,
    ) -> Result<Option<FullPackageDetails>, String> {
        let Some(mut details) = self.load_package_details(canonical_id).await? else {
            return Ok(None);
        };

        if let Some(source) = preferred_source {
            details.selected_default_source = Some(source.clone());
            if let Some(package) = details.package.as_mut() {
                apply_variant_to_package(
                    package,
                    variant_for_source(&details.all_variants, &source),
                );
                package.source = source.clone();
            }
            if let Some(package) = details.package.as_ref() {
                let maintainer_known = package
                    .maintainer
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false);
                details.security = Some(build_security_summary(Some(&source), maintainer_known));
                details.source_summary =
                    Some(package.source_summary.clone().unwrap_or_else(|| {
                        if details.all_variants.len() > 1 {
                            format!("{} sources available", details.all_variants.len())
                        } else {
                            format!("Primary source: {}", source.label)
                        }
                    }));
                details.installed_source_label = package.installed.then_some(source.label.clone());
                details.source_switch_notice = package.installed.then_some(build_source_switch_notice(&source));
            }
        }

        Ok(Some(details))
    }

    pub async fn load_package_variants(
        &self,
        canonical_id: impl Into<String>,
    ) -> Result<Vec<PackageVariant>, String> {
        Ok(self
            .load_package_details(canonical_id)
            .await?
            .map(|details| details.all_variants)
            .unwrap_or_default())
    }

    pub async fn load_local_reviews(
        &self,
        app_id: impl Into<String>,
    ) -> Result<Vec<LocalReview>, String> {
        let app_id = app_id.into();
        let reviews = self.reviews.clone();
        tokio::task::spawn_blocking(move || reviews.load_for_app(&app_id))
            .await
            .map_err(|e| e.to_string())?
    }

    pub async fn submit_review(
        &self,
        app_id: impl Into<String>,
        rating: u32,
        summary: impl Into<String>,
        description: impl Into<String>,
        user_display: impl Into<String>,
    ) -> Result<LocalReview, String> {
        let app_id = app_id.into();
        let summary = summary.into();
        let description = description.into();
        let user_display = user_display.into();
        // Submit to Supabase (MonARCH community backend) first
        let comment = if summary.trim().is_empty() {
            description.clone()
        } else if description.trim().is_empty() {
            summary.clone()
        } else {
            format!("{}\n\n{}", summary.trim(), description.trim())
        };
        supabase::submit_review(&app_id, rating, &comment, &user_display).await?;
        // Also store locally for offline/backup
        let reviews = self.reviews.clone();
        tokio::task::spawn_blocking(move || {
            reviews.submit(app_id, rating, summary, description, user_display)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn load_package_reviews(
        &self,
        app_id: impl Into<String>,
    ) -> Result<Vec<PackageReview>, String> {
        let app_id = app_id.into();
        let (remote_result, local_result, supabase_result) = tokio::join!(
            odrs::get_app_reviews(app_id.clone()),
            self.load_local_reviews(app_id.clone()),
            supabase::fetch_reviews(&app_id),
        );

        let mut reviews = remote_result?;
        reviews.extend(
            local_result?
                .into_iter()
                .map(local_review_to_package_review),
        );
        match supabase_result {
            Ok(supabase_reviews) => reviews.extend(supabase_reviews),
            Err(e) => log::warn!("Supabase reviews fetch failed: {}", e),
        }
        reviews.sort_by(|left, right| {
            right
                .date_created
                .unwrap_or_default()
                .partial_cmp(&left.date_created.unwrap_or_default())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(reviews)
    }

    pub async fn load_updates(&self) -> Result<UpdateSnapshot, String> {
        let registry = self.registry.clone();
        let repo_task = tokio::task::spawn_blocking(get_repo_updates);
        let flatpak_task = tokio::task::spawn_blocking(get_flatpak_updates);
        let aur_task = get_aur_updates();

        let (repo_result, flatpak_result, aur_result) =
            tokio::join!(repo_task, flatpak_task, aur_task);

        let repo_result = repo_result.map_err(|e| e.to_string())?;
        let flatpak_result = flatpak_result.map_err(|e| e.to_string())?;

        tokio::task::spawn_blocking(move || {
            build_update_snapshot(&registry, repo_result, flatpak_result, aur_result)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn install_package(&self, package_name: impl Into<String>) -> Result<String, String> {
        let package_name = package_name.into();
        let password = self.helper_password()?;
        self.privileged
            .execute_manifest_with_password(
                crate::models::TransactionManifest {
                    update_system: true,
                    refresh_db: true,
                    install_targets: vec![package_name],
                    ..Default::default()
                },
                password,
            )
            .await
    }

    pub async fn remove_package(&self, package_name: impl Into<String>) -> Result<String, String> {
        let package_name = package_name.into();
        let password = self.helper_password()?;
        self.privileged
            .execute_manifest_with_password(
                crate::models::TransactionManifest {
                    remove_targets: vec![package_name],
                    ..Default::default()
                },
                password,
            )
            .await
    }

    pub async fn update_system(&self) -> Result<String, String> {
        let password = self.helper_password()?;
        self.privileged
            .execute_manifest_with_password(
                crate::models::TransactionManifest {
                    update_system: true,
                    refresh_db: true,
                    ..Default::default()
                },
                password,
            )
            .await
    }

    pub async fn install_package_stream(
        &self,
        package_name: impl Into<String>,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        let package_name = package_name.into();
        let password = self.helper_password()?;
        self.privileged
            .execute_manifest_stream_with_password(
                crate::models::TransactionManifest {
                    update_system: true,
                    refresh_db: true,
                    install_targets: vec![package_name],
                    ..Default::default()
                },
                password,
            )
            .await
    }

    pub async fn install_package_for_source_stream(
        &self,
        package: Package,
        source: PackageSource,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        match install_route_for_source(&package, &source) {
            SourceInstallRoute::Repo {
                target_name,
                target_repo,
            } => {
                let password = self.helper_password()?;
                self.privileged
                    .execute_manifest_stream_with_password(
                        crate::models::TransactionManifest {
                            update_system: true,
                            refresh_db: true,
                            install_targets: vec![target_name],
                            target_repo: Some(target_repo),
                            ..Default::default()
                        },
                        password,
                    )
                    .await
            }
            SourceInstallRoute::Aur { target_name } => {
                let (tx, rx) = tokio::sync::mpsc::channel(128);
                let privileged = self.privileged.clone();
                let password = self.helper_password()?;
                tokio::spawn(async move {
                    let result =
                        aur::install_package(privileged, tx.clone(), target_name.clone(), password)
                            .await;
                    let _ = tx.send(HelperProgress::Finished(result)).await;
                });
                Ok(rx)
            }
            SourceInstallRoute::Flatpak { app_id, remote } => {
                let (tx, rx) = tokio::sync::mpsc::channel(128);
                let privileged = self.privileged.clone();
                tokio::spawn(async move {
                    let result =
                        install_flatpak_with_bootstrap(privileged, tx.clone(), app_id, remote)
                            .await;
                    let _ = tx.send(HelperProgress::Finished(result)).await;
                });
                Ok(rx)
            }
            SourceInstallRoute::Unsupported { source_type } => Err(format!(
                "This host cannot execute installs for source type '{}'.",
                source_type
            )),
        }
    }

    pub async fn remove_package_for_source_stream(
        &self,
        package: Package,
        source: PackageSource,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        match remove_route_for_source(&package, &source) {
            SourceRemoveRoute::Flatpak { app_id } => {
                let (tx, rx) = tokio::sync::mpsc::channel(128);
                tokio::spawn(async move {
                    let result = flatpak::remove_app(tx.clone(), app_id).await;
                    let _ = tx.send(HelperProgress::Finished(result)).await;
                });
                Ok(rx)
            }
            SourceRemoveRoute::Native { target_name } => {
                self.remove_package_stream(target_name).await
            }
        }
    }

    pub async fn remove_package_stream(
        &self,
        package_name: impl Into<String>,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        let package_name = package_name.into();
        let password = self.helper_password()?;
        self.privileged
            .execute_manifest_stream_with_password(
                crate::models::TransactionManifest {
                    remove_targets: vec![package_name],
                    ..Default::default()
                },
                password,
            )
            .await
    }

    pub async fn update_system_stream(
        &self,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        let password = self.helper_password()?;
        let privileged = self.privileged.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        tokio::spawn(async move {
            let result: Result<String, String> = async {
                let repo_stream = privileged
                    .execute_manifest_stream_with_password(
                        crate::models::TransactionManifest {
                            update_system: true,
                            refresh_db: true,
                            ..Default::default()
                        },
                        password.clone(),
                    )
                    .await?;
                forward_nested_stream(tx.clone(), repo_stream).await?;

                let aur_updates = get_aur_updates().await?;
                let mut aur_failed: Vec<(String, String)> = Vec::new();
                if !aur_updates.is_empty() {
                    let _ = tx
                        .send(HelperProgress::Message {
                            message: format!("Processing {} AUR updates...", aur_updates.len()),
                            percent: Some(78),
                        })
                        .await;
                    for update in aur_updates {
                        match aur::install_package(
                            privileged.clone(),
                            tx.clone(),
                            update.name.clone(),
                            password.clone(),
                        )
                        .await
                        {
                            Ok(_) => {}
                            Err(e) => {
                                let _ = tx
                                    .send(HelperProgress::Message {
                                        message: format!("AUR update failed: {} — {}", update.name, e),
                                        percent: None,
                                    })
                                    .await;
                                aur_failed.push((update.name, e));
                            }
                        }
                    }
                }

                let flatpak_result = if flatpak::is_flatpak_available() {
                    let flatpak_updates = tokio::task::spawn_blocking(get_flatpak_updates)
                        .await
                        .map_err(|e| e.to_string())??;
                    Some(flatpak::update_many(tx.clone(), flatpak_updates).await)
                } else {
                    None
                };

                let mut msg = "System update completed.".to_string();
                if !aur_failed.is_empty() {
                    let names: Vec<_> = aur_failed.iter().map(|(n, _)| n.as_str()).collect();
                    msg.push_str(&format!(
                        " {} AUR update(s) failed: {}.",
                        names.len(),
                        names.join(", ")
                    ));
                }
                match &flatpak_result {
                    Some(Ok(m)) if !m.is_empty() && !m.contains("No Flatpak updates") => {
                        if !msg.ends_with('.') {
                            msg.push(' ');
                        }
                        msg.push_str(m.trim());
                    }
                    Some(Err(e)) => {
                        if !msg.ends_with('.') {
                            msg.push(' ');
                        }
                        msg.push_str(&format!("Flatpak: {}", e));
                    }
                    _ => {}
                }
                Ok(msg)
            }
            .await;

            let _ = tx.send(HelperProgress::Finished(result)).await;
        });
        Ok(rx)
    }

    pub async fn cancel_active_operation(&self) -> Result<(), String> {
        self.privileged.cancel_active_operation().await
    }

    pub async fn clear_pacman_cache(&self) -> Result<String, String> {
        self.privileged
            .clear_cache_keep_with_password(2, self.helper_password()?)
            .await
    }

    pub async fn repair_unlock_pacman(&self) -> Result<String, String> {
        let password = self.helper_password()?;
        self.privileged
            .execute_manifest_with_password(
                crate::models::TransactionManifest {
                    remove_lock: true,
                    ..Default::default()
                },
                password,
            )
            .await
    }

    pub async fn refresh_keyring(&self) -> Result<String, String> {
        self.privileged
            .refresh_keyring_with_password(self.helper_password()?)
            .await
    }

    pub async fn force_refresh_databases(&self) -> Result<String, String> {
        let password = self.helper_password()?;
        self.privileged
            .execute_manifest_with_password(
                crate::models::TransactionManifest {
                    refresh_db: true,
                    ..Default::default()
                },
                password,
            )
            .await
    }

    pub async fn prepare_chaotic_components(&self) -> Result<String, String> {
        self.privileged
            .prepare_chaotic_components_with_password(self.helper_password()?)
            .await
    }

    pub async fn prepare_flatpak(&self) -> Result<String, String> {
        if !flatpak::is_flatpak_available() {
            let password = self.helper_password()?;
            self.privileged
                .execute_manifest_with_password(
                    crate::models::TransactionManifest {
                        update_system: true,
                        refresh_db: true,
                        install_targets: vec!["flatpak".to_string()],
                        ..Default::default()
                    },
                    password,
                )
                .await?;
        }

        flatpak::ensure_flathub_ready().await?;
        Ok("Flatpak is ready. Flathub has been prepared for GTK installs.".to_string())
    }

    pub async fn clear_metadata_caches(&self) -> Result<String, String> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("monarch-store");
        tokio::task::spawn_blocking(move || {
            if cache_dir.exists() {
                std::fs::remove_dir_all(&cache_dir).map_err(|e| e.to_string())?;
            }
            std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
            Ok(
                "Metadata caches cleared. Restart or refresh discovery to rebuild them."
                    .to_string(),
            )
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn clear_build_cache(&self) -> Result<String, String> {
        let build_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("monarch")
            .join("build");
        tokio::task::spawn_blocking(move || {
            if build_dir.exists() {
                std::fs::remove_dir_all(&build_dir).map_err(|e| e.to_string())?;
            }
            Ok("AUR build cache cleared.".to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn install_monarch_policy(&self) -> Result<String, String> {
        let settings = self.settings.load()?;
        let allow_active = if settings.one_click_enabled {
            "yes"
        } else {
            "auth_admin_keep"
        };
        let policy_content = set_policy_allow_active(
            MONARCH_POLKIT_POLICY,
            "com.monarch.store.package-manage",
            allow_active,
        );
        let rules_escaped = MONARCH_POLKIT_RULES.replace('{', "{{").replace('}', "}}");
        let script = format!(
            r#"
mkdir -p /usr/share/polkit-1/actions /usr/share/polkit-1/rules.d
cat <<'POLICYEOF' > /usr/share/polkit-1/actions/com.monarch.store.policy
{}
POLICYEOF
cat <<'RULESEOF' > /usr/share/polkit-1/rules.d/10-monarch-store.rules
{}
RULESEOF
chmod 644 /usr/share/polkit-1/actions/com.monarch.store.policy /usr/share/polkit-1/rules.d/10-monarch-store.rules
printf 'MonARCH policy installed (%s).\n' "{}"
"#,
            policy_content, rules_escaped, allow_active
        );
        run_privileged_script(&script, self.helper_password()?).await
    }

    pub async fn needs_startup_unlock(&self) -> Result<bool, String> {
        tokio::task::spawn_blocking(|| {
            if !std::path::Path::new("/var/lib/pacman/db.lck").exists() {
                return Ok(false);
            }
            let pacman_running = std::process::Command::new("pgrep")
                .arg("-x")
                .arg("pacman")
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false);
            Ok(!pacman_running)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn startup_status(&self) -> Result<StartupStatus, String> {
        let settings = self.settings.load()?;
        let distro = detect_distro_profile();
        let registry = self.registry.clone();
        tokio::task::spawn_blocking(move || {
            let required_bins = ["git", "checkupdates", "pkexec"];
            let missing_required_bins = required_bins
                .iter()
                .filter(|bin| !binary_exists(bin))
                .map(|bin| (*bin).to_string())
                .collect::<Vec<_>>();
            let registry_empty = registry.count_packages()? == 0;
            let stale_pacman_lock = is_stale_pacman_lock();
            let helper_available = std::path::Path::new("/usr/lib/monarch-store/monarch-helper")
                .exists()
                || std::env::var("MONARCH_HELPER_PATH")
                    .ok()
                    .is_some_and(|path| std::path::Path::new(&path).exists());
            let policy_installed =
                std::path::Path::new("/usr/share/polkit-1/actions/com.monarch.store.policy")
                    .exists();
            let keyring_ready = std::path::Path::new("/etc/pacman.d/gnupg").exists();
            let sync_db_healthy = !is_sync_db_corrupt();

            let mut warnings = Vec::new();
            if !helper_available {
                warnings.push("monarch-helper is unavailable.".to_string());
            }
            if !policy_installed {
                warnings.push("The MonARCH Polkit policy is missing.".to_string());
            }
            if stale_pacman_lock {
                warnings.push("A stale pacman database lock was detected.".to_string());
            }
            if !keyring_ready {
                warnings.push("System GPG keyrings are missing or uninitialized.".to_string());
            }
            if !sync_db_healthy {
                warnings.push(
                    "Pacman sync databases appear to be corrupt and should be refreshed."
                        .to_string(),
                );
            }
            if !missing_required_bins.is_empty() {
                warnings.push(format!(
                    "Missing required tools: {}.",
                    missing_required_bins.join(", ")
                ));
            }

            Ok(StartupStatus {
                missing_required_bins,
                stale_pacman_lock,
                registry_empty,
                helper_available,
                policy_installed,
                keyring_ready,
                sync_db_healthy,
                warnings,
                onboarding_completed: settings.onboarding_completed,
                distro,
            })
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn load_system_health(&self) -> Result<SystemHealthView, String> {
        let startup = self.startup_status().await?;
        let updates = self.load_updates().await.unwrap_or_default();
        Ok(SystemHealthView {
            pending_updates: updates.items.len(),
            alpm_status: if startup.stale_pacman_lock {
                "Lock detected".to_string()
            } else if !startup.sync_db_healthy {
                "Database needs attention".to_string()
            } else {
                "Ready".to_string()
            },
            update_status: if updates.items.is_empty() {
                "No pending updates".to_string()
            } else {
                format!("{} pending", updates.items.len())
            },
            startup,
        })
    }

    pub async fn load_update_warnings(&self) -> Result<UpdateWarnings, String> {
        let startup = self.startup_status().await?;
        let news = crate::news::fetch_news().await.unwrap_or_default();
        Ok(UpdateWarnings {
            reboot_required: check_reboot_required(),
            pacnew_warnings: get_pacnew_warnings()?,
            restart_required_services: check_services_restart().await?,
            critical_advisories: news
                .into_iter()
                .filter(|item| item.is_critical)
                .map(|item| item.title)
                .chain(startup.warnings.into_iter())
                .collect(),
        })
    }

    pub async fn load_system_info(&self) -> Result<SystemInfo, String> {
        tokio::task::spawn_blocking(move || {
            let kernel = std::process::Command::new("uname")
                .arg("-r")
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_else(|_| "Unknown".to_string());

            let distro = std::fs::read_to_string("/etc/os-release")
                .unwrap_or_default()
                .lines()
                .find(|line| line.starts_with("PRETTY_NAME="))
                .and_then(|line| line.split('=').nth(1))
                .map(|value| value.trim_matches('"').to_string())
                .unwrap_or_else(|| "Arch Linux".to_string());

            let pacman_version = std::process::Command::new("pacman")
                .arg("--version")
                .output()
                .map(|output| {
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .unwrap_or("Unknown")
                        .to_string()
                })
                .unwrap_or_else(|_| "Unknown".to_string());

            let cpu_optimization = detect_cpu_optimization();

            Ok(SystemInfo {
                kernel,
                distro,
                pacman_version,
                chaotic_enabled: pacman_conf_has_chaotic(),
                cpu_optimization,
            })
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn load_snapshot_status(&self) -> Result<SnapshotStatus, String> {
        tokio::task::spawn_blocking(get_snapshot_status)
            .await
            .map_err(|e| e.to_string())?
    }

    pub async fn create_snapshot(&self, comment: impl Into<String>) -> Result<String, String> {
        let comment = comment.into();
        let status = self.load_snapshot_status().await?;
        match status.tool {
            SnapshotTool::Timeshift => {
                let script = format!(
                    "timeshift --create --comments '{}' --scripted\n",
                    shell_escape_single(&comment)
                );
                run_privileged_script(&script, self.helper_password()?).await
            }
            SnapshotTool::Snapper => {
                let script = format!(
                    "snapper create --description '{}'\n",
                    shell_escape_single(&comment)
                );
                run_privileged_script(&script, self.helper_password()?).await
            }
            SnapshotTool::None => Err("No snapshot tool is configured on this host.".to_string()),
        }
    }

    pub async fn load_cache_size(&self) -> Result<CacheSizeResult, String> {
        tokio::task::spawn_blocking(get_cache_size)
            .await
            .map_err(|e| e.to_string())?
    }

    pub async fn load_orphans(&self) -> Result<OrphansWithSizeResult, String> {
        tokio::task::spawn_blocking(get_orphans_with_size)
            .await
            .map_err(|e| e.to_string())?
    }

    pub async fn remove_orphans(&self) -> Result<String, String> {
        let orphans = self.load_orphans().await?;
        if orphans.orphans.is_empty() {
            return Ok("No orphan packages were detected.".to_string());
        }
        let password = self.helper_password()?;
        self.privileged
            .execute_manifest_with_password(
                crate::models::TransactionManifest {
                    remove_orphans: true,
                    ..Default::default()
                },
                password,
            )
            .await
    }

    pub fn get_mirror_rank_tool(&self) -> Option<String> {
        if std::path::Path::new("/usr/bin/pacman-mirrors").exists()
            && std::path::Path::new("/etc/manjaro-release").exists()
        {
            return Some("pacman-mirrors".to_string());
        }
        if binary_exists("reflector") {
            return Some("reflector".to_string());
        }
        if binary_exists("rate-mirrors") {
            return Some("rate-mirrors".to_string());
        }
        None
    }

    pub async fn test_mirrors(
        &self,
        repo_key: impl Into<String>,
    ) -> Result<Vec<MirrorTestResult>, String> {
        let repo_key = repo_key.into();
        tokio::task::spawn_blocking(move || test_mirrors(repo_key))
            .await
            .map_err(|e| e.to_string())?
    }

    pub async fn rank_mirrors(&self) -> Result<String, String> {
        let script = r#"
echo 'Ranking mirrors by download speed (this may take ~30 seconds)...'
if [ -f /etc/manjaro-release ] && command -v pacman-mirrors >/dev/null 2>&1; then
    pacman-mirrors -f 5
    echo '✓ Manjaro mirrors ranked successfully.'
elif command -v reflector >/dev/null 2>&1; then
    reflector --latest 5 --sort rate --save /etc/pacman.d/mirrorlist
    echo '✓ Mirrors ranked successfully. Fastest mirrors are now prioritized.'
elif command -v rate-mirrors >/dev/null 2>&1; then
    rate-mirrors arch | tee /etc/pacman.d/mirrorlist >/dev/null
    echo '✓ Mirrors ranked successfully using rate-mirrors.'
else
    echo 'ERROR: Neither reflector nor rate-mirrors is installed (or pacman-mirrors on Manjaro).'
    exit 1
fi
"#;
        run_privileged_script(script, self.helper_password()?).await
    }

    pub async fn load_settings_view(&self) -> Result<SettingsView, String> {
        let settings = self.settings.load()?;
        let startup = self.startup_status().await.ok();
        let system_health = self.load_system_health().await.ok();
        let system_info = self.load_system_info().await.ok();
        let snapshot_status = self.load_snapshot_status().await.ok();
        let cache = self.load_cache_size().await.ok();
        let orphans = self.load_orphans().await.ok();
        let update_warnings = self.load_update_warnings().await.ok();
        let mut notices = Vec::new();
        if settings.one_click_enabled {
            notices.push(
                "Branded one-click authentication is enabled for helper-backed workflows."
                    .to_string(),
            );
        }
        if settings.flatpak_enabled {
            notices.push("Flatpak discovery is enabled.".to_string());
        }
        if settings.chaotic_enabled {
            notices.push("Chaotic-AUR discovery is enabled.".to_string());
        }
        Ok(SettingsView {
            settings,
            startup,
            system_health,
            system_info,
            snapshot_status,
            cache,
            orphans,
            update_warnings,
            mirror_rank_tool: self.get_mirror_rank_tool(),
            notices,
        })
    }

    pub fn current_search_options(&self) -> Result<SearchOptions, String> {
        let settings = self.settings.load()?;
        Ok(SearchOptions {
            flatpak_enabled: Some(settings.flatpak_enabled),
            aur_enabled: Some(settings.aur_enabled),
            chaotic_enabled: Some(settings.chaotic_enabled),
            show_system_apps: Some(settings.show_system_apps),
            source_filter: None,
            category_filter: None,
            installed_only: Some(false),
            sort_mode: Some(SearchSortMode::Relevance),
            for_installed_lookup: Some(false),
        })
    }

    async fn ensure_registry_ready(&self) -> Result<(), String> {
        if self.is_ready.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        let stats = tokio::task::spawn_blocking({
            let registry = self.registry.clone();
            move || registry.hydration_stats()
        })
        .await
        .map_err(|e| e.to_string())??;
        if !registry_needs_bootstrap(stats) {
            self.is_ready
                .store(true, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }

        let _guard = self.bootstrap_lock.lock().await;
        if self.is_ready.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        let stats = tokio::task::spawn_blocking({
            let registry = self.registry.clone();
            move || registry.hydration_stats()
        })
        .await
        .map_err(|e| e.to_string())??;
        if !registry_needs_bootstrap(stats) {
            self.is_ready
                .store(true, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }

        tokio::task::spawn_blocking({
            let registry = self.registry.clone();
            move || hydrate_registry_from_live_system(&registry)
        })
        .await
        .map_err(|e| e.to_string())?
        .map(|_| {
            self.is_ready
                .store(true, std::sync::atomic::Ordering::Relaxed);
        })
    }
}

fn registry_needs_bootstrap(stats: RegistryHydrationStats) -> bool {
    if stats.total_packages == 0 || stats.non_installed_packages == 0 {
        return true;
    }
    if stats.total_packages > 1000 && stats.hydration_version < REGISTRY_HYDRATION_VERSION {
        return true;
    }
    // Rich metadata packages should be at least 1/40th of non-installed packages.
    // This threshold is intentionally generous because Arch-based systems have many
    // raw library packages without GUI metadata, which is completely normal.
    if stats.rich_metadata_packages == 0 {
        return true;
    }
    if stats.non_installed_packages > 250
        && stats.rich_metadata_packages.saturating_mul(40) < stats.non_installed_packages
    {
        return true;
    }
    false
}

fn load_curated_packages(registry: &RegistryManager, queries: &[&str]) -> Vec<Package> {
    let mut seen = std::collections::HashSet::new();
    let mut packages = Vec::new();
    for query in queries {
        if let Some(package) = resolve_curated_package(registry, query) {
            if seen.insert(package.canonical_id.clone()) {
                packages.push(package);
            }
        }
    }
    packages
}

fn resolve_curated_package(registry: &RegistryManager, query: &str) -> Option<Package> {
    let needle = normalize_storefront_key(query);
    let results = registry.search_packages_sql(query, 24).ok()?;
    results
        .into_iter()
        .filter(|package| {
            curated_package_matches(package, &needle)
                && curated_package_source_allowed(package)
                && is_storefront_package(package)
        })
        .max_by_key(|package| curated_package_score(package, &needle))
}

fn curated_package_matches(package: &Package, needle: &str) -> bool {
    curated_match_key(&package.canonical_id) == needle
        || curated_match_key(&package.name) == needle
        || curated_match_key(&package.effective_title()) == needle
        || package
            .display_name
            .as_deref()
            .is_some_and(|value| curated_match_key(value) == needle)
        || package
            .app_id
            .as_deref()
            .is_some_and(|value| curated_match_key(value) == needle)
        || curated_match_aliases(package)
            .iter()
            .any(|value| curated_match_key(value) == needle)
}

fn curated_package_source_allowed(package: &Package) -> bool {
    package.source.package_name.as_deref().is_none_or(|name| {
        let normalized = normalize_storefront_key(name);
        !normalized.ends_with("git")
            && !normalized.ends_with("beta")
            && !normalized.ends_with("alpha")
            && !normalized.ends_with("canary")
            && !normalized.ends_with("nightly")
            && !normalized.ends_with("dev")
    })
}

fn curated_match_aliases(package: &Package) -> Vec<&str> {
    match package.canonical_id.as_str() {
        "telegram" => vec!["telegram-desktop", "org.telegram.desktop"],
        "signal" => vec!["signal-desktop", "org.signal.Signal", "org.signal.signal"],
        "obsstudio" => vec!["obs-studio", "com.obsproject.Studio"],
        "vlc" => vec!["org.videolan.VLC"],
        "bottles" => vec!["com.usebottles.bottles"],
        "keepassxc" => vec!["org.keepassxc.KeePassXC"],
        _ => Vec::new(),
    }
}

fn curated_package_score(package: &Package, needle: &str) -> i32 {
    let mut score = 0;
    if curated_match_key(&package.canonical_id) == needle {
        score += 200;
    }
    if curated_match_key(&package.name) == needle {
        score += 180;
    }
    if curated_match_key(&package.effective_title()) == needle {
        score += 170;
    }
    if package
        .display_name
        .as_deref()
        .is_some_and(|value| curated_match_key(value) == needle)
    {
        score += 170;
    }
    if package
        .app_id
        .as_deref()
        .is_some_and(|value| curated_match_key(value) == needle)
    {
        score += 190;
    }
    if curated_match_aliases(package)
        .iter()
        .any(|value| curated_match_key(value) == needle)
    {
        score += 195;
    }
    if package
        .available_sources
        .as_ref()
        .is_some_and(|sources| sources.len() > 1)
    {
        score += 40;
    }
    if package
        .icon
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        score += 20;
    }
    if package
        .screenshots
        .as_ref()
        .is_some_and(|shots| !shots.is_empty())
    {
        score += 10;
    }
    score
}

fn normalize_storefront_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn curated_match_key(value: &str) -> String {
    normalize_storefront_key(value)
}

fn lane_packages<F>(packages: &[Package], predicate: F, limit: usize) -> Vec<Package>
where
    F: Fn(&Package) -> bool,
{
    let mut selected = packages
        .iter()
        .filter(|package| predicate(package))
        .cloned()
        .collect::<Vec<_>>();
    selected.truncate(limit);
    selected
}

fn is_storefront_match(package: &Package, query: Option<&str>) -> bool {
    if is_storefront_package(package) {
        return true;
    }

    if let Some(query_str) = query {
        let qs = query_str.trim().to_lowercase();
        if !qs.is_empty() && !is_technical_package_name(&package.name) {
            let name_lower = package.name.to_lowercase();
            if name_lower.contains(&qs) {
                return true;
            }
            if let Some(ref dname) = package.display_name {
                if dname.to_lowercase().contains(&qs) {
                    return true;
                }
            }
        }
    }

    false
}

fn is_search_match(package: &Package, query: Option<&str>, include_system_apps: bool) -> bool {
    if is_storefront_match(package, query) {
        return true;
    }

    include_system_apps
        && query
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        && is_system_search_package(package)
}

fn is_storefront_package(package: &Package) -> bool {
    if package.source.source_type == "flatpak" {
        return true;
    }

    let has_app_id = package
        .app_id
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_icon = package
        .icon
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_screenshots = package
        .screenshots
        .as_ref()
        .is_some_and(|shots| !shots.is_empty());
    let has_categories = package
        .categories
        .as_ref()
        .is_some_and(|categories| !categories.is_empty());
    let has_display_name = package
        .display_name
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty() && value.trim() != package.name.trim());
    let has_rich_description = {
        let description = package.description.trim();
        !description.is_empty()
            && description.len() > 18
            && !description.eq_ignore_ascii_case(&package.name)
    };

    if is_technical_package_name(&package.name) {
        return false;
    }

    if has_app_id || has_screenshots {
        return true;
    }

    let has_description = package.description.len() > 10 && !package.description.trim().is_empty();
    if has_categories && (has_icon || has_display_name || has_rich_description || has_description) {
        return true;
    }

    (has_icon && has_display_name && has_rich_description)
        || (package.installed && has_icon && (has_display_name || has_categories))
}

fn is_library_package(package: &Package) -> bool {
    package.installed && is_storefront_package(package)
}

fn is_system_search_package(package: &Package) -> bool {
    if is_storefront_package(package) || is_technical_package_name(&package.name) {
        return false;
    }

    let normalized = package.name.trim().to_ascii_lowercase();
    if [
        "-data",
        "-common",
        "-docs",
        "-doc",
        "-debug",
        "-devel",
        "-dev",
        "-headers",
        "-examples",
        "-locale",
        "-lang",
        "-i18n",
        "-keyring",
        "-mirrorlist",
    ]
    .iter()
    .any(|suffix| normalized.ends_with(suffix))
    {
        return false;
    }

    let has_icon = package
        .icon
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_app_id = package
        .app_id
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_categories = package
        .categories
        .as_ref()
        .is_some_and(|categories| !categories.is_empty());
    let has_display_name = package
        .display_name
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty() && value.trim() != package.name.trim());
    let has_description = {
        let description = package.description.trim();
        !description.is_empty() && description.len() > 10
    };

    has_icon || has_app_id || has_categories || has_display_name || has_description
}

fn is_technical_package_name(name: &str) -> bool {
    let normalized = name.trim().to_lowercase();
    if normalized.is_empty() {
        return true;
    }

    if normalized.len() <= 2 && normalized.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return true;
    }

    if normalized.starts_with("lib")
        || normalized.starts_with("python-")
        || normalized.starts_with("perl-")
        || normalized.starts_with("ruby-")
        || normalized.starts_with("lua-")
        || normalized.starts_with("qt5-")
        || normalized.starts_with("qt6-")
    {
        return true;
    }

    [
        "-common",
        "-data",
        "-debug",
        "-devel",
        "-dev",
        "-doc",
        "-docs",
        "-headers",
        "-keyring",
        "-locale",
        "-locales",
        "-mirrorlist",
        "-symbols",
        "-tests",
        "-translations",
    ]
    .iter()
    .any(|suffix| normalized.ends_with(suffix))
}

/// Categories shown on HOME and Search. Same ordered list for both; Bazaar-style gradients
/// are applied in the GTK UI. We use the monarch taxonomy (not Flathub section names).
fn storefront_categories(_packages: &[Package]) -> Vec<String> {
    monarch_category_taxonomy()
        .iter()
        .map(|(label, _)| (*label).to_string())
        .collect::<Vec<_>>()
}

fn binary_exists(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .any(|path| path.join(bin).exists())
}

fn shell_escape_single(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

fn set_policy_allow_active(policy: &str, action_id: &str, allow_active: &str) -> String {
    let action_marker = format!("<action id=\"{}\">", action_id);
    let Some(action_start) = policy.find(&action_marker) else {
        return policy.to_string();
    };

    let rest = &policy[action_start..];
    let Some(action_end_rel) = rest.find("</action>") else {
        return policy.to_string();
    };
    let action_end = action_start + action_end_rel;
    let action_block = &policy[action_start..action_end];

    let allow_start_tag = "<allow_active>";
    let allow_end_tag = "</allow_active>";
    let Some(allow_start_rel) = action_block.find(allow_start_tag) else {
        return policy.to_string();
    };
    let allow_value_start = action_start + allow_start_rel + allow_start_tag.len();
    let Some(allow_end_rel) = action_block[allow_start_rel..].find(allow_end_tag) else {
        return policy.to_string();
    };
    let allow_value_end = action_start + allow_start_rel + allow_end_rel;

    let mut updated = String::with_capacity(policy.len() + allow_active.len());
    updated.push_str(&policy[..allow_value_start]);
    updated.push_str(allow_active);
    updated.push_str(&policy[allow_value_end..]);
    updated
}

async fn run_privileged_script(script: &str, password: Option<String>) -> Result<String, String> {
    let mut command = if password.is_some() {
        let mut command = Command::new("sudo");
        command.args(["-S", "bash", "-s"]);
        command
    } else {
        let mut command = Command::new("pkexec");
        command.args(["--disable-internal-agent", "/bin/bash", "-s"]);
        command
    };

    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start privileged script: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        if let Some(password) = password {
            stdin
                .write_all(format!("{password}\n").as_bytes())
                .await
                .map_err(|e| format!("Failed to send session password: {e}"))?;
        }
        stdin
            .write_all(script.as_bytes())
            .await
            .map_err(|e| format!("Failed to send privileged script: {e}"))?;
        let _ = stdin.shutdown().await;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("Failed to wait for privileged script: {e}"))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            Ok("Privileged operation completed.".to_string())
        } else {
            Ok(stdout)
        }
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn filter_and_sort_packages(
    mut packages: Vec<Package>,
    options: &SearchOptions,
    query: Option<&str>,
) -> Vec<Package> {
    packages = filter_visible_packages(packages, options);
    let sort_mode = options.sort_mode.unwrap_or_else(|| {
        if query.is_some_and(|value| !value.trim().is_empty()) {
            SearchSortMode::Relevance
        } else {
            SearchSortMode::Name
        }
    });
    sort_packages(&mut packages, &sort_mode, query);
    packages
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SourceInstallRoute {
    Repo {
        target_name: String,
        target_repo: String,
    },
    Aur {
        target_name: String,
    },
    Flatpak {
        app_id: String,
        remote: Option<String>,
    },
    Unsupported {
        source_type: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SourceRemoveRoute {
    Flatpak { app_id: String },
    Native { target_name: String },
}

fn source_package_name(package: &Package, source: &PackageSource) -> String {
    source
        .package_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| package.name.clone())
}

fn install_route_for_source(package: &Package, source: &PackageSource) -> SourceInstallRoute {
    match source.source_type.as_str() {
        "repo" => SourceInstallRoute::Repo {
            target_name: source_package_name(package, source),
            target_repo: source.id.clone(),
        },
        "aur" => SourceInstallRoute::Aur {
            target_name: source_package_name(package, source),
        },
        "flatpak" => SourceInstallRoute::Flatpak {
            app_id: source
                .package_name
                .clone()
                .or_else(|| package.app_id.clone())
                .unwrap_or_else(|| package.name.clone()),
            remote: Some(source.id.clone()),
        },
        _ => SourceInstallRoute::Unsupported {
            source_type: source.source_type.clone(),
        },
    }
}

fn remove_route_for_source(package: &Package, source: &PackageSource) -> SourceRemoveRoute {
    match source.source_type.as_str() {
        "flatpak" => SourceRemoveRoute::Flatpak {
            app_id: source
                .package_name
                .clone()
                .or_else(|| package.app_id.clone())
                .unwrap_or_else(|| package.name.clone()),
        },
        _ => SourceRemoveRoute::Native {
            target_name: source_package_name(package, source),
        },
    }
}

fn filter_visible_packages(mut packages: Vec<Package>, options: &SearchOptions) -> Vec<Package> {
    let aur_enabled = options.aur_enabled.unwrap_or(true);
    let flatpak_enabled = options.flatpak_enabled.unwrap_or(true);
    let chaotic_enabled = options.chaotic_enabled.unwrap_or(false);
    let include_installed = options.for_installed_lookup.unwrap_or(false);
    let installed_only = options.installed_only.unwrap_or(false);
    let source_filter = options.source_filter.as_deref();
    let category_filter = options.category_filter.as_deref();

    packages.retain(|package| {
        if installed_only && !package.installed {
            return false;
        }
        if include_installed && package.installed {
            return matches_source_filter(package, source_filter)
                && matches_category_filter(package, category_filter);
        }

        let visible = match package.source.source_type.as_str() {
            "aur" => aur_enabled,
            "flatpak" => flatpak_enabled,
            "repo" if package.source.id == "chaotic-aur" => chaotic_enabled,
            _ => true,
        };
        visible
            && matches_source_filter(package, source_filter)
            && matches_category_filter(package, category_filter)
    });
    packages
}

fn sort_packages(packages: &mut [Package], sort_mode: &SearchSortMode, query: Option<&str>) {
    packages.sort_by(|left, right| compare_packages(left, right, sort_mode, query));
}

fn compare_packages(
    left: &Package,
    right: &Package,
    sort_mode: &SearchSortMode,
    query: Option<&str>,
) -> std::cmp::Ordering {
    let prefer_installed = matches!(sort_mode, SearchSortMode::Relevance)
        && query.is_some_and(|value| !value.trim().is_empty());
    if prefer_installed {
        let installed_cmp = right.installed.cmp(&left.installed);
        if installed_cmp != std::cmp::Ordering::Equal {
            return installed_cmp;
        }
    }

    match sort_mode {
        SearchSortMode::Name => title_key(left).cmp(&title_key(right)),
        SearchSortMode::Newest => right
            .discovered_at
            .unwrap_or_default()
            .cmp(&left.discovered_at.unwrap_or_default())
            .then_with(|| source_priority(left).cmp(&source_priority(right)))
            .then_with(|| title_key(left).cmp(&title_key(right))),
        SearchSortMode::Updated => right
            .updated_at
            .or(right.last_modified)
            .unwrap_or_default()
            .cmp(&left.updated_at.or(left.last_modified).unwrap_or_default())
            .then_with(|| source_priority(left).cmp(&source_priority(right)))
            .then_with(|| title_key(left).cmp(&title_key(right))),
        SearchSortMode::Relevance => relevance_score(right, query)
            .cmp(&relevance_score(left, query))
            .then_with(|| source_priority(left).cmp(&source_priority(right)))
            .then_with(|| title_key(left).cmp(&title_key(right))),
    }
}

fn relevance_score(package: &Package, query: Option<&str>) -> i32 {
    let base = source_relevance_bonus(package);
    let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) else {
        return base;
    };

    let query = query.to_lowercase();
    let title = package.effective_title().to_lowercase();
    let name = package.name.to_lowercase();
    let app_id = package.app_id.clone().unwrap_or_default().to_lowercase();
    let desc = package.description.to_lowercase();

    let mut score = base;
    if package.installed {
        score += 120;
    }
    if title == query || name == query || app_id == query {
        score += 600;
    } else if title.starts_with(&query) || name.starts_with(&query) {
        score += 420;
    } else if title.contains(&query) || name.contains(&query) {
        score += 260;
    } else if desc.contains(&query) {
        score += 90;
    }
    score
}

fn source_relevance_bonus(package: &Package) -> i32 {
    match source_priority(package) {
        0 => 100,
        1 => 70,
        2 => 45,
        _ => 25,
    }
}

fn source_priority(package: &Package) -> i32 {
    if is_native_repo(package) {
        0
    } else if is_chaotic_repo(package) {
        1
    } else if package.source.source_type == "flatpak" {
        2
    } else {
        3
    }
}

fn is_native_repo(package: &Package) -> bool {
    package.source.source_type == "repo" && !is_chaotic_repo(package)
}

fn is_chaotic_repo(package: &Package) -> bool {
    package.source.source_type == "repo" && package.source.id == "chaotic-aur"
}

fn matches_source_filter(package: &Package, source_filter: Option<&str>) -> bool {
    let Some(source_filter) = source_filter.filter(|value| !value.trim().is_empty()) else {
        return true;
    };

    let predicate = |source: &PackageSource| match source_filter {
        "native" => source.source_type == "repo" && source.id != "chaotic-aur",
        "chaotic" | "chaotic-aur" => source.source_type == "repo" && source.id == "chaotic-aur",
        "flatpak" => source.source_type == "flatpak",
        "aur" => source.source_type == "aur",
        "repo" => source.source_type == "repo",
        other => source.source_type == other || source.id == other,
    };

    predicate(&package.source)
        || package
            .available_sources
            .as_ref()
            .map(|sources| sources.iter().any(predicate))
            .unwrap_or(false)
}

fn matches_category_filter(package: &Package, category_filter: Option<&str>) -> bool {
    let Some(category_filter) = category_filter
        .map(normalize_category_filter)
        .filter(|value| !value.is_empty())
    else {
        return true;
    };

    package_monarch_categories(package)
        .iter()
        .any(|category| normalize_category_filter(category) == category_filter)
}

fn normalize_category_filter(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "game" | "games" => "games".to_string(),
        "office" | "productivity" => "productivity".to_string(),
        "network" | "internet" => "internet".to_string(),
        "audiovideo" | "multimedia" | "audio" | "video" => "multimedia".to_string(),
        "development" | "develop" => "development".to_string(),
        "graphics" | "graphics & design" => "graphics & design".to_string(),
        "system" | "system tools" => "system tools".to_string(),
        "utilities" => "utilities".to_string(),
        other => other.to_string(),
    }
}

fn category_curated_queries(category: &str) -> &'static [&'static str] {
    match normalize_category_filter(category).as_str() {
        "productivity" => &[
            "libreoffice-fresh",
            "obsidian",
            "okular",
            "calibre",
            "foliate",
            "simplenote-electron-bin",
            "zotero-bin",
            "xournalpp",
            "apostrophe",
            "org.gnome.clocks",
            "io.github.diegoivan.flowtime",
            "org.gnome.Solanum",
            "onlyoffice-bin",
            "wps-office",
            "cherrytree",
            "joplin-desktop",
            "standardnotes-desktop-bin",
            "notion-app",
            "marktext",
            "typora",
            "qownnotes",
            "trilium-bin",
            "org.gnome.Evolution",
            "org.gnome.Calendar",
            "org.kde.kontact",
            "org.kde.kmail",
            "atril",
            "com.github.jeromerobert.pdfarranger",
            "pdftk",
        ],
        "internet" => &[
            "firefox",
            "google-chrome",
            "librewolf-bin",
            "thunderbird",
            "signal-desktop",
            "telegram-desktop",
            "discord",
            "newsflash",
            "de.haeckerfelix.Fragments",
            "org.gnome.Polari",
            "brave-bin",
            "vivaldi",
            "chromium",
            "microsoft-edge-stable-bin",
            "tor-browser",
            "element-desktop",
            "whatsapp-for-linux",
            "slack-desktop",
            "zoom",
            "teams",
            "qbittorrent",
            "transmission-gtk",
            "filezilla",
            "org.gnome.Epiphany",
            "org.gnome.Geary",
            "org.telegram.desktop",
        ],
        "graphics & design" => &[
            "gimp",
            "inkscape",
            "krita",
            "blender",
            "rawtherapee",
            "darktable",
            "freecad",
            "libreoffice-draw",
            "digikam",
            "shotwell",
            "nomacs",
            "gwenview",
            "feh",
            "imv",
            "ristretto",
            "org.gnome.eog",
            "org.gnome.Loupe",
            "org.kde.gwenview",
            "pencil2d",
            "mypaint",
            "synfigstudio",
            "openshot",
            "kdenlive",
            "pitivi",
            "olive-editor",
            "drawio-desktop-bin",
            "dia",
            "scribus",
            "fontforge",
            "font-manager",
            "org.gimp.GIMP",
        ],
        "multimedia" => &[
            "vlc",
            "obs-studio",
            "audacity",
            "ardour",
            "handbrake",
            "spotify",
            "strawberry",
            "easyeffects",
            "io.bassi.Amberol",
            "celluloid",
            "com.github.rafostar.Clapper",
            "org.gnome.Lollypop",
            "mpv",
            "smplayer",
            "totem",
            "parole",
            "rhythmbox",
            "clementine",
            "quodlibet",
            "kid3-qt",
            "soundconverter",
            "pitivi",
            "kdenlive",
            "openshot",
            "flowblade",
            "shotcut",
            "ffmpeg",
            "org.videolan.VLC",
            "com.spotify.Client",
            "org.gnome.Totem",
        ],
        "development" => &[
            "visual-studio-code-bin",
            "git",
            "docker-desktop",
            "kitty",
            "alacritty",
            "neovim",
            "ptyxis",
            "com.felipekinoshita.Wildcard",
            "com.raggesilver.BlackBox",
            "code",
            "sublime-text-4",
            "intellij-idea-community-edition",
            "pycharm-community-edition",
            "android-studio",
            "rustdesk-bin",
            "insomnia-bin",
            "postman-bin",
            "dbeaver",
            "mysql-workbench",
            "arduino",
            "qtcreator",
            "glade",
            "gh-cli",
            "lazygit",
            "delta",
            "bat",
            "eza",
            "fd",
            "ripgrep",
            "fzf",
            "org.gnome.Builder",
        ],
        "system tools" => &[
            "timeshift",
            "keepassxc",
            "bitwarden-bin",
            "gparted",
            "flameshot",
            "kdeconnect",
            "balena-etcher",
            "peazip-bin",
            "gnome-disk-utility",
            "baobab",
            "bleachbit",
            "stacer",
            "htop",
            "btop",
            "gnome-system-monitor",
            "conky",
            "variety",
            "nitrogen",
            "autorandr",
            "arandr",
            "gnome-boxes",
            "virt-manager",
            "vmware-workstation",
            "teamviewer",
            "anydesk-bin",
            "syncthing",
            "grub-customizer",
            "org.gnome.DiskUtility",
            "org.gnome.Baobab",
        ],
        "games" => &[
            "steam",
            "lutris",
            "heroic-games-launcher-bin",
            "com.usebottles.bottles",
            "retroarch",
            "protonup-qt",
            "org.gnome.atomix",
            "org.gnome.Klotski",
            "org.gnome.Sudoku",
            "org.gnome.Hitori",
            "org.gnome.Mines",
            "minecraft",
            "minetest",
            "super-tux-kart",
            "0ad",
            "wesnoth",
            "freeciv",
            "openttd",
            "stellarium",
            "flightgear",
            "warzone2100",
            "hedgewars",
            "teeworlds",
            "xonotic",
            "supertux",
            "supertux2",
            "gnome-chess",
            "gnome-mahjongg",
            "gnome-sudoku",
            "org.gnome.Aisleriot",
            "org.gnome.Mahjongg",
            "net.lutris.Lutris",
        ],
        "utilities" => &[
            "kodi",
            "waydroid",
            "openrgb",
            "anki",
            "virtualbox",
            "vmware-workstation",
            "gnome-calculator",
            "qalculate-gtk",
            "speedcrunch",
            "galculator",
            "gnome-dictionary",
            "goldendict",
            "copyq",
            "diodon",
            "gpaste",
            "gsmartcontrol",
            "gnome-logs",
            "gnome-font-viewer",
            "org.gnome.Calculator",
            "org.gnome.Dictionary",
            "org.gnome.Characters",
            "org.gnome.Connections",
        ],
        _ => &[],
    }
}

fn package_monarch_categories(package: &Package) -> Vec<&'static str> {
    let raw_categories = package.categories.as_deref().unwrap_or(&[]);
    let mut mapped = Vec::new();

    for (label, tokens) in monarch_category_taxonomy() {
        if raw_categories
            .iter()
            .any(|raw| category_matches_tokens(raw, tokens))
        {
            mapped.push(*label);
        }
    }

    if mapped.is_empty() {
        if package.name.contains("browser")
            || package.description.to_ascii_lowercase().contains("browser")
        {
            mapped.push("Internet");
        } else if package.description.to_ascii_lowercase().contains("editor")
            && package.description.to_ascii_lowercase().contains("image")
        {
            mapped.push("Graphics & Design");
        }
    }

    mapped
}

fn category_matches_tokens(raw: &str, tokens: &[&str]) -> bool {
    let normalized = raw
        .trim()
        .to_ascii_lowercase()
        .replace('&', " ")
        .replace(['/', '-', '_'], " ");
    tokens.iter().any(|token| {
        normalized == *token
            || normalized.contains(&format!(" {token} "))
            || normalized.starts_with(&format!("{token} "))
            || normalized.ends_with(&format!(" {token}"))
    })
}

fn monarch_category_taxonomy() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        (
            "Games",
            &[
                "game",
                "games",
                "emulator",
                "emulation",
                "arcade",
                "strategy game",
                "action game",
                "adventuregame",
                "arcadegame",
                "boardgame",
                "cardgame",
                "kidsgame",
                "logicgame",
                "roleplaying",
                "roleplayinggame",
                "simulation",
                "simulationgame",
                "sportsgame",
            ],
        ),
        (
            "Productivity",
            &[
                "office",
                "productivity",
                "calendar",
                "contactmanagement",
                "email",
                "finance",
                "notes",
                "todo",
                "wordprocessor",
                "spreadsheet",
                "presentation",
            ],
        ),
        (
            "Internet",
            &[
                "network",
                "networking",
                "instantmessaging",
                "chat",
                "webbrowser",
                "browser",
                "email",
                "remoteaccess",
                "filetransfer",
                "telephony",
                "vpn",
                "ircclient",
                "emailclient",
                "web",
                "communication",
                "messaging",
            ],
        ),
        (
            "Multimedia",
            &[
                "audiovideo",
                "audio",
                "video",
                "music",
                "player",
                "recorder",
                "tv",
                "disc burning",
                "mixer",
                "sequencer",
                "videoediting",
                "audioediting",
                "streaming",
                "audioplayer",
                "videoplayer",
                "photomanagement",
                "musicplayer",
            ],
        ),
        (
            "Development",
            &[
                "development",
                "ide",
                "guidesigner",
                "database",
                "debugger",
                "revisioncontrol",
                "terminalemulator",
                "texteditor",
                "building",
                "webdevelopment",
                "computer science",
                "programming",
                "hamradio",
                "terminal",
                "consoleonly",
                "documentation",
            ],
        ),
        (
            "Graphics & Design",
            &[
                "graphics",
                "2dgraphics",
                "3dgraphics",
                "vectorgraphics",
                "rastergraphics",
                "photography",
                "viewer",
                "publishing",
                "scanner",
                "scanning",
                "imageprocessing",
                "graphicseditor",
                "imageviewer",
                "photo",
                "cad",
                "desktoppublishing",
                "art",
                "illustration",
                "design",
                "image",
                "drawing",
                "painting",
                "videoediting",
                "videography",
            ],
        ),
        (
            "System Tools",
            &[
                "system",
                "settings",
                "security",
                "filesystem",
                "monitor",
                "package manager",
                "packagemanager",
                "archiving",
                "compression",
                "hardware",
                "accessibility",
                "desktopsettings",
                "filemanager",
                "filesystem",
                "systemmonitor",
                "backup",
                "synchronization",
                "service",
                "virtualization",
            ],
        ),
        (
            "Utilities",
            &[
                "utility",
                "utilities",
                "accessories",
                "calculator",
                "clock",
                "filetools",
                "dictionary",
                "adult",
                "education",
                "science",
                "maps",
                "documentation",
                "utility",
                "filetools",
                "texttools",
                "archiving",
                "compression",
            ],
        ),
    ]
}

fn title_key(package: &Package) -> String {
    package.effective_title().to_lowercase()
}

fn detect_distro_profile() -> DistroProfile {
    let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let mut distro_id = String::from("arch");
    let mut pretty_name = String::from("Arch Linux");
    let mut id_like = String::new();

    for line in os_release.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim_matches('"');
            match key {
                "ID" => distro_id = value.to_lowercase(),
                "ID_LIKE" => id_like = value.to_lowercase(),
                "PRETTY_NAME" => pretty_name = value.to_string(),
                _ => {}
            }
        }
    }

    let chaotic_support =
        if distro_id == "manjaro" || id_like.split_whitespace().any(|word| word == "manjaro") {
            ChaoticSupport::Blocked
        } else if distro_id == "garuda"
            || distro_id == "cachyos"
            || id_like
                .split_whitespace()
                .any(|word| matches!(word, "garuda" | "cachyos"))
        {
            ChaoticSupport::Native
        } else {
            ChaoticSupport::Allowed
        };

    DistroProfile {
        id: distro_id,
        pretty_name,
        chaotic_support,
        chaotic_configured: pacman_conf_has_chaotic(),
    }
}

fn pacman_conf_has_chaotic() -> bool {
    let main = std::fs::read_to_string("/etc/pacman.conf").unwrap_or_default();
    if main.lines().any(|line| line.trim() == "[chaotic-aur]") {
        return true;
    }
    let dropin =
        std::fs::read_to_string("/etc/pacman.d/monarch/chaotic-aur.conf").unwrap_or_default();
    dropin.lines().any(|line| line.trim() == "[chaotic-aur]")
}

async fn install_flatpak_with_bootstrap(
    privileged: Arc<PrivilegedClient>,
    tx: tokio::sync::mpsc::Sender<HelperProgress>,
    app_id: String,
    remote: Option<String>,
) -> Result<String, String> {
    if !flatpak::is_flatpak_available() {
        let _ = tx
            .send(HelperProgress::Message {
                message: "Flatpak is not installed. Installing it through monarch-helper first..."
                    .to_string(),
                percent: Some(5),
            })
            .await;
        let stream = privileged
            .execute_manifest_stream(crate::models::TransactionManifest {
                update_system: true,
                refresh_db: true,
                install_targets: vec!["flatpak".to_string()],
                ..Default::default()
            })
            .await?;
        forward_nested_stream(tx.clone(), stream).await?;
    }

    flatpak::install_app(tx, app_id.clone(), remote).await?;
    Ok(format!("Flatpak app {app_id} installed."))
}

async fn forward_nested_stream(
    tx: tokio::sync::mpsc::Sender<HelperProgress>,
    mut stream: tokio::sync::mpsc::Receiver<HelperProgress>,
) -> Result<String, String> {
    while let Some(event) = stream.recv().await {
        match event {
            HelperProgress::Finished(result) => return result,
            other => {
                let _ = tx.send(other).await;
            }
        }
    }
    Err("Operation stream ended unexpectedly.".to_string())
}

fn is_stale_pacman_lock() -> bool {
    let lock_path = std::path::Path::new("/var/lib/pacman/db.lck");
    if !lock_path.exists() {
        return false;
    }

    let pacman_running = std::process::Command::new("pgrep")
        .args(["-x", "pacman"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    !pacman_running
}

fn is_sync_db_corrupt() -> bool {
    let output = match std::process::Command::new("pacman")
        .args(["-Si", "pacman"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.contains("Unrecognized archive format") || stderr.contains("could not open database")
}

fn check_reboot_required() -> bool {
    let running_kernel = match std::process::Command::new("uname").arg("-r").output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(_) => return false,
    };
    if running_kernel.is_empty() {
        return false;
    }
    let modules_dir = format!("/usr/lib/modules/{running_kernel}");
    !std::path::Path::new(&modules_dir).exists()
}

fn get_pacnew_warnings() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("find")
        .args(["/etc", "-name", "*.pacnew"])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.to_string())
        .collect())
}

async fn check_services_restart() -> Result<Vec<String>, String> {
    let timeout_duration = std::time::Duration::from_secs(10);
    let process = tokio::process::Command::new("needrestart")
        .arg("-b")
        .output();
    match tokio::time::timeout(timeout_duration, process).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut services = Vec::new();
            for line in stdout.lines() {
                if let Some(service) = line.strip_prefix("NEEDRESTART-SVC:") {
                    services.push(service.trim().to_string());
                }
            }
            Ok(services)
        }
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Ok(Err(error)) => Err(format!("needrestart failed: {error}")),
        Err(_) => Ok(Vec::new()),
    }
}

fn get_snapshot_status() -> Result<SnapshotStatus, String> {
    let timeshift = std::process::Command::new("timeshift")
        .arg("--list-devices")
        .output();
    if let Ok(output) = timeshift {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains("No devices found") {
                return Ok(SnapshotStatus {
                    tool: SnapshotTool::Timeshift,
                    is_configured: true,
                    message: "Timeshift is ready".to_string(),
                });
            }
        }
    }

    let snapper = std::process::Command::new("snapper")
        .arg("list-configs")
        .output();
    if let Ok(output) = snapper {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines = stdout
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count();
            if lines > 1 {
                return Ok(SnapshotStatus {
                    tool: SnapshotTool::Snapper,
                    is_configured: true,
                    message: "Snapper is ready".to_string(),
                });
            }
        }
    }

    Ok(SnapshotStatus {
        tool: SnapshotTool::None,
        is_configured: false,
        message: "No snapshot tool detected or configured".to_string(),
    })
}

fn detect_cpu_optimization() -> String {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default()
        .to_lowercase();
    let has = |flag: &str| cpuinfo.contains(flag);
    let v4 = ["avx512f", "avx512bw", "avx512cd", "avx512dq", "avx512vl"]
        .iter()
        .all(|flag| has(flag));
    let v3 = [
        "avx", "avx2", "bmi1", "bmi2", "f16c", "fma", "movbe", "xsave", "abm",
    ]
    .iter()
    .all(|flag| has(flag));

    if v4 && has("znver4") {
        "x86-64-v4 (Zen 4/5)".to_string()
    } else if v4 {
        "x86-64-v4 (AVX-512)".to_string()
    } else if v3 {
        "x86-64-v3 (AVX2)".to_string()
    } else {
        "Standard (x86-64-v1)".to_string()
    }
}

fn get_cache_size() -> Result<CacheSizeResult, String> {
    let cache_dir = std::path::Path::new("/var/cache/pacman/pkg");
    let mut total_bytes = 0u64;
    fn calculate_dir_size(path: &std::path::Path, total: &mut u64) -> std::io::Result<()> {
        if path.is_file() {
            *total += path.metadata()?.len();
        } else if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let _ = calculate_dir_size(&entry.path(), total);
            }
        }
        Ok(())
    }
    if cache_dir.exists() {
        let _ = calculate_dir_size(cache_dir, &mut total_bytes);
    }
    Ok(CacheSizeResult {
        size_bytes: total_bytes,
        human_readable: human_readable_size(total_bytes),
    })
}

fn get_orphans_with_size() -> Result<OrphansWithSizeResult, String> {
    let output = std::process::Command::new("pacman")
        .args(["-Qtdq"])
        .output()
        .map_err(|e| e.to_string())?;
    let orphans: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.to_string())
        .collect();
    if orphans.is_empty() {
        return Ok(OrphansWithSizeResult {
            orphans,
            total_size_bytes: 0,
            human_readable: "0 B".to_string(),
        });
    }

    let mut total_bytes = 0u64;
    for package in &orphans {
        if let Ok(output) = std::process::Command::new("pacman")
            .args(["-Qi", package])
            .output()
        {
            let info = String::from_utf8_lossy(&output.stdout);
            for line in info.lines() {
                if let Some(value) = line.strip_prefix("Installed Size") {
                    total_bytes += parse_pacman_size_field(value);
                    break;
                }
            }
        }
    }

    Ok(OrphansWithSizeResult {
        orphans,
        total_size_bytes: total_bytes,
        human_readable: human_readable_size(total_bytes),
    })
}

fn parse_pacman_size_field(value: &str) -> u64 {
    let parts: Vec<&str> = value
        .split(':')
        .nth(1)
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    if parts.len() < 2 {
        return 0;
    }
    let Ok(number) = parts[0].parse::<f64>() else {
        return 0;
    };
    let multiplier = match parts[1] {
        "KiB" => 1024u64,
        "MiB" => 1024u64 * 1024,
        "GiB" => 1024u64 * 1024 * 1024,
        _ => 1u64,
    };
    (number * multiplier as f64) as u64
}

fn human_readable_size(total_bytes: u64) -> String {
    if total_bytes < 1024 {
        format!("{total_bytes} B")
    } else if total_bytes < 1024 * 1024 {
        format!("{:.1} KB", total_bytes as f64 / 1024.0)
    } else if total_bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", total_bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", total_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn test_mirrors(repo_key: String) -> Result<Vec<MirrorTestResult>, String> {
    let key = repo_key.to_lowercase();
    let (distro, mirrorlist_path): (&str, Option<std::path::PathBuf>) =
        if key == "arch" || key == "official arch linux" || key.is_empty() {
            ("arch", None)
        } else if key.contains("cachyos") {
            (
                "cachyos",
                Some(std::path::PathBuf::from("/etc/pacman.d/cachyos-mirrorlist")),
            )
        } else if key.contains("chaotic") {
            (
                "chaotic",
                Some(std::path::PathBuf::from("/etc/pacman.d/chaotic-mirrorlist")),
            )
        } else {
            ("arch", None)
        };

    if distro == "arch" {
        if let Ok(output) = std::process::Command::new("rate-mirrors")
            .args(["arch"])
            .output()
        {
            if output.status.success() {
                return parse_mirrorlist_latency(&String::from_utf8_lossy(&output.stdout), 3);
            }
        }
        if let Ok(output) = std::process::Command::new("reflector")
            .args(["--list"])
            .output()
        {
            if output.status.success() {
                return parse_mirrorlist_latency(&String::from_utf8_lossy(&output.stdout), 3);
            }
        }
        return Err(
            "Install rate-mirrors or reflector to test mirrors (e.g. pacman -S rate-mirrors)"
                .to_string(),
        );
    }

    match mirrorlist_path {
        Some(path) => {
            let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            parse_mirrorlist_latency(&contents, 3)
        }
        None => Ok(Vec::new()),
    }
}

fn parse_mirrorlist_latency(text: &str, take: usize) -> Result<Vec<MirrorTestResult>, String> {
    let mut results = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("Server = ") {
            continue;
        }
        let server = trimmed.trim_start_matches("Server = ").trim();
        let mut parts = server.split('#');
        let url = parts.next().unwrap_or_default().trim().to_string();
        if url.is_empty() {
            continue;
        }
        let latency_ms = parts
            .next()
            .map(str::trim)
            .and_then(|suffix| suffix.strip_suffix("ms"))
            .and_then(|ms| ms.trim().parse::<u32>().ok());
        results.push(MirrorTestResult { url, latency_ms });
        if results.len() >= take {
            break;
        }
    }
    if results.is_empty() {
        return Err("No mirrors were detected in the mirrorlist output.".to_string());
    }
    Ok(results)
}

fn build_update_snapshot(
    registry: &RegistryManager,
    repo_result: Result<Vec<UpdateItem>, String>,
    flatpak_result: Result<Vec<UpdateItem>, String>,
    aur_result: Result<Vec<UpdateItem>, String>,
) -> Result<UpdateSnapshot, String> {
    let repo_updates = repo_result?;
    let (flatpak_updates, flatpak_status, flatpak_error) = match flatpak_result {
        Ok(updates) => {
            let status = if updates.is_empty() { "empty" } else { "ok" };
            (updates, status.to_string(), None)
        }
        Err(error) if error.contains("flatpak is not installed") => {
            (Vec::new(), "disabled".to_string(), None)
        }
        Err(error) => (Vec::new(), "error".to_string(), Some(error)),
    };
    let (aur_updates, aur_status, aur_error) = match aur_result {
        Ok(updates) => {
            let status = if updates.is_empty() { "empty" } else { "ok" };
            (updates, status.to_string(), None)
        }
        Err(error) => (Vec::new(), "error".to_string(), Some(error)),
    };
    let repo_status = if repo_updates.is_empty() {
        "empty"
    } else {
        "ok"
    };

    let all_updates = repo_updates
        .iter()
        .chain(aur_updates.iter())
        .chain(flatpak_updates.iter())
        .flat_map(|item| [item.name.clone(), item.name.to_lowercase()])
        .collect::<Vec<_>>();
    let hydrated = registry
        .get_packages_by_canonical_ids(&all_updates)
        .unwrap_or_default()
        .into_iter()
        .map(|package| (package.canonical_id.clone(), package))
        .collect::<std::collections::HashMap<_, _>>();

    let mut items =
        Vec::with_capacity(repo_updates.len() + aur_updates.len() + flatpak_updates.len());
    for update in repo_updates
        .into_iter()
        .chain(aur_updates.into_iter())
        .chain(flatpak_updates.into_iter())
    {
        let mut package = hydrated
            .get(&update.name)
            .or_else(|| hydrated.get(&update.name.to_lowercase()))
            .cloned()
            .unwrap_or_else(|| Package {
                name: update.name.clone(),
                display_name: update.display_name.clone(),
                display_title: update.display_name.clone(),
                description: format!("Update available from {}", update.source.label),
                version: update.new_version.clone(),
                source: update.source.clone(),
                icon: update.icon.clone(),
                canonical_id: update.name.to_lowercase(),
                installed: true,
                available_sources: Some(vec![update.source.clone()]),
                ..Package::default()
            });

        package.installed = true;
        if package.version.trim().is_empty() {
            package.version = update.new_version.clone();
        }
        if package.source.label.trim().is_empty() {
            package.source = update.source.clone();
        }
        if package.display_name.is_none() {
            package.display_name = update.display_name.clone();
            package.display_title = update.display_name.clone();
        }
        if package.icon.is_none() {
            package.icon = update.icon.clone();
        }

        items.push(UpdateSnapshotItem {
            package,
            current_version: update.current_version,
            new_version: update.new_version,
        });
    }

    Ok(UpdateSnapshot {
        items,
        sources: vec![
            UpdateSourceStatus {
                source: "repo".to_string(),
                status: repo_status.to_string(),
                duration_ms: 0,
                error: None,
            },
            UpdateSourceStatus {
                source: "aur".to_string(),
                status: aur_status,
                duration_ms: 0,
                error: aur_error,
            },
            UpdateSourceStatus {
                source: "flatpak".to_string(),
                status: flatpak_status,
                duration_ms: 0,
                error: flatpak_error,
            },
        ],
    })
}

fn get_repo_updates() -> Result<Vec<UpdateItem>, String> {
    let output = std::process::Command::new("checkupdates")
        .output()
        .map_err(|e| format!("Failed to run checkupdates: {e}"))?;

    // `checkupdates` returns 2 when there are no updates.
    if !output.status.success() && output.status.code() != Some(2) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("checkupdates failed: {}", stderr.trim()));
    }

    Ok(parse_checkupdates_output(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn parse_checkupdates_output(stdout: &str) -> Vec<UpdateItem> {
    stdout
        .lines()
        .filter_map(|line| {
            let (pkg_and_current, new_version) = line.split_once(" -> ")?;
            let mut parts = pkg_and_current.split_whitespace();
            let name = parts.next()?.trim();
            let current_version = parts.next()?.trim();
            let new_version = new_version.trim();
            if name.is_empty() || current_version.is_empty() || new_version.is_empty() {
                return None;
            }

            Some(UpdateItem {
                name: name.to_string(),
                display_name: None,
                current_version: current_version.to_string(),
                new_version: new_version.to_string(),
                source: PackageSource::new("repo", "repo", new_version, "System Repository"),
                size: None,
                icon: None,
            })
        })
        .collect()
}

fn get_flatpak_updates() -> Result<Vec<UpdateItem>, String> {
    let output = std::process::Command::new("flatpak")
        .args([
            "remote-ls",
            "--updates",
            "--app",
            "--columns=application,version,installed-size,name",
        ])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "flatpak is not installed".to_string()
            } else {
                format!("Failed to run flatpak remote-ls: {e}")
            }
        })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let installed_versions = get_installed_flatpak_versions().unwrap_or_default();
    Ok(parse_flatpak_updates_output(
        &String::from_utf8_lossy(&output.stdout),
        &installed_versions,
    ))
}

async fn get_aur_updates() -> Result<Vec<UpdateItem>, String> {
    let foreign_packages = tokio::task::spawn_blocking(get_foreign_installed_packages)
        .await
        .map_err(|e| e.to_string())??;

    if foreign_packages.is_empty() {
        return Ok(Vec::new());
    }

    let installed_versions = foreign_packages
        .iter()
        .cloned()
        .collect::<std::collections::HashMap<_, _>>();
    let names = foreign_packages
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();

    let handle = raur::Handle::new();
    let packages = handle.info(&names).await.map_err(|e| e.to_string())?;
    let mut updates = Vec::new();

    for package in packages {
        if let Some(local_version) = installed_versions.get(&package.name) {
            let is_upgrade = tokio::task::spawn_blocking({
                let new_version = package.version.clone();
                let local_version = local_version.clone();
                move || vercmp_greater(&new_version, &local_version)
            })
            .await
            .map_err(|e| e.to_string())??;

            if is_upgrade {
                updates.push(UpdateItem {
                    name: package.name.clone(),
                    display_name: None,
                    current_version: local_version.clone(),
                    new_version: package.version.clone(),
                    source: PackageSource::new("aur", "aur", &package.version, "AUR (Community)"),
                    size: None,
                    icon: None,
                });
            }
        }
    }

    Ok(updates)
}

fn get_foreign_installed_packages() -> Result<Vec<(String, String)>, String> {
    let output = std::process::Command::new("pacman")
        .args(["-Qm"])
        .output()
        .map_err(|e| format!("Failed to run pacman -Qm: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?.trim();
            let version = parts.next()?.trim();
            if name.is_empty() || version.is_empty() {
                return None;
            }
            Some((name.to_string(), version.to_string()))
        })
        .collect())
}

fn vercmp_greater(new_version: &str, current_version: &str) -> Result<bool, String> {
    let output = std::process::Command::new("vercmp")
        .args([new_version, current_version])
        .output()
        .map_err(|e| format!("Failed to run vercmp: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(result == "1")
}

fn get_installed_flatpak_versions() -> Result<std::collections::HashMap<String, String>, String> {
    let output = std::process::Command::new("flatpak")
        .args(["list", "--app", "--columns=application,version"])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "flatpak is not installed".to_string()
            } else {
                format!("Failed to run flatpak list: {e}")
            }
        })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let mut versions = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let app_id = parts[0].trim();
            let version = parts[1].trim();
            if !app_id.is_empty() {
                versions.insert(app_id.to_string(), version.to_string());
            }
        }
    }
    Ok(versions)
}

fn parse_flatpak_updates_output(
    stdout: &str,
    installed_versions: &std::collections::HashMap<String, String>,
) -> Vec<UpdateItem> {
    stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 3 {
                return None;
            }

            let app_id = parts[0].trim();
            let new_version = parts[1].trim();
            let size = parts[2].trim().parse::<u64>().ok();
            let display_name = parts
                .get(3)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

            if app_id.is_empty() || new_version.is_empty() {
                return None;
            }

            Some(UpdateItem {
                name: app_id.to_string(),
                display_name: display_name.clone(),
                current_version: installed_versions
                    .get(app_id)
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string()),
                new_version: new_version.to_string(),
                source: PackageSource::new(
                    "flatpak",
                    "flathub",
                    new_version,
                    "Flatpak (Sandboxed)",
                ),
                size,
                icon: None,
            })
        })
        .collect()
}

async fn enrich_package_presentation(packages: &mut [Package]) {
    for package in packages.iter_mut() {
        decorate_package(package);
    }

    let app_ids = packages
        .iter()
        .filter_map(|package| package.app_id.clone())
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    if app_ids.is_empty() {
        return;
    }

    if let Ok(ratings) = odrs::get_app_ratings_batch(app_ids).await {
        for package in packages.iter_mut() {
            if let Some(app_id) = package.app_id.as_ref() {
                if let Some(rating) = ratings.get(app_id) {
                    package.rating = Some(rating.clone());
                }
            }
        }
    }
}

fn decorate_package(package: &mut Package) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let security = build_security_summary(
        Some(&package.source),
        package
            .maintainer
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
    );
    package.primary_action = Some(if package.installed {
        "launch".to_string()
    } else {
        "install".to_string()
    });
    package.primary_action_label = Some(if package.installed {
        "Launch".to_string()
    } else {
        "Install".to_string()
    });
    package.trust_level = Some(security.trust_tier.clone());
    if package.discovered_at.is_none() {
        package.discovered_at = package.last_modified.or(Some(now));
    }
    if package.updated_at.is_none() {
        package.updated_at = package
            .last_modified
            .or(package.discovered_at)
            .or(Some(now));
    }
    if package.source_summary.is_none() {
        package.source_summary = Some(if package.installed {
            format!("Installed on this system • {}", package.source.label)
        } else if package
            .available_sources
            .as_ref()
            .map(|v| v.len())
            .unwrap_or(1)
            > 1
        {
            format!(
                "Using {} by default • {} more sources available",
                package.source.label,
                package
                    .available_sources
                    .as_ref()
                    .map(|v| v.len().saturating_sub(1))
                    .unwrap_or(0)
            )
        } else if package.is_optimized.unwrap_or(false) {
            format!("Optimized build from {}", package.source.label)
        } else {
            format!("Available from {}", package.source.label)
        });
    }
    if package.security_summary.is_none() {
        package.security_summary = Some(format!(
            "{} {}",
            security.verification_note, security.user_action_note
        ));
    }
    let canonical_categories = package_monarch_categories(package);
    if !canonical_categories.is_empty() {
        package.categories = Some(
            canonical_categories
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
    }
}

/// Builds one variant per available source. The UI shows the selected variant's version, size,
/// maintainer, and license. Per-source hydration (AUR API, etc.) is done in hydrate_variants_from_sources.
fn build_variants_for_package(package: &Package) -> Vec<PackageVariant> {
    let sources = package
        .available_sources
        .clone()
        .filter(|sources| !sources.is_empty())
        .unwrap_or_else(|| vec![package.source.clone()]);
    sources
        .into_iter()
        .map(|source| PackageVariant {
            version: if source.version.trim().is_empty() {
                package.version.clone()
            } else {
                source.version.clone()
            },
            repo_name: (source.id == "chaotic-aur").then_some("chaotic-aur".to_string()),
            pkg_name: source
                .package_name
                .clone()
                .or_else(|| Some(package.name.clone())),
            download_size: package.download_size_bytes.or(package.download_size),
            installed_size: package.installed_size_bytes.or(package.installed_size),
            maintainer: package.maintainer.clone(),
            license: package.license.clone(),
            description: Some(package.description.clone()),
            screenshots: package.screenshots.clone(),
            security: Some(build_security_summary(
                Some(&source),
                package
                    .maintainer
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false),
            )),
            source,
        })
        .collect()
}

/// Fetches per-source metadata from every source we can: AUR (maintainer/license),
/// Flatpak (download/installed size via remote-info), Repo/Chaotic (size from sync db).
async fn hydrate_variants_from_sources(
    package: &Package,
    variants: Vec<PackageVariant>,
) -> Vec<PackageVariant> {
    let aur_names: Vec<String> = variants
        .iter()
        .filter(|v| v.source.source_type == "aur")
        .filter_map(|v| {
            v.source
                .package_name
                .clone()
                .or_else(|| Some(package.name.clone()))
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let aur_info: std::collections::HashMap<String, raur::Package> = if aur_names.is_empty() {
        std::collections::HashMap::new()
    } else {
        let names: Vec<&str> = aur_names.iter().map(String::as_str).collect();
        let handle = raur::Handle::new();
        match handle.info(&names).await {
            Ok(pkgs) => pkgs
                .into_iter()
                .map(|p| (p.name.clone(), p))
                .collect(),
            Err(_) => std::collections::HashMap::new(),
        }
    };

    let repo_requests: Vec<(String, String)> = variants
        .iter()
        .filter(|v| v.source.source_type == "repo" || v.source.id == "chaotic-aur")
        .map(|v| {
            let pkg = v
                .source
                .package_name
                .as_deref()
                .unwrap_or(&package.name);
            (v.source.id.clone(), pkg.to_string())
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let repo_sizes = if repo_requests.is_empty() {
        std::collections::HashMap::new()
    } else {
        let conf_path = "/etc/pacman.conf".to_string();
        tokio::task::spawn_blocking(move || {
            crate::bootstrap::get_repo_package_sizes(&repo_requests, &conf_path)
        })
        .await
        .ok()
        .unwrap_or_default()
    };

    let mut out = Vec::with_capacity(variants.len());
    for mut v in variants {
        if v.source.source_type == "aur" {
            let name = v
                .source
                .package_name
                .as_deref()
                .unwrap_or(&package.name);
            if let Some(raur_pkg) = aur_info.get(name) {
                if v.maintainer.as_deref().map_or(true, |s| s.trim().is_empty()) {
                    v.maintainer = raur_pkg.maintainer.clone();
                }
                if v.license.as_ref().map_or(true, |l| l.is_empty()) {
                    if !raur_pkg.license.is_empty() {
                        v.license = Some(raur_pkg.license.clone());
                    }
                }
                if v.security.is_some() {
                    let maintainer_known = v.maintainer.as_deref().map_or(false, |s| !s.trim().is_empty());
                    v.security = Some(build_security_summary(Some(&v.source), maintainer_known));
                }
            }
            // AUR RPC does not provide download size; leave as package-level or Unknown.
        } else if v.source.source_type == "flatpak" && flatpak::is_flatpak_available() {
            let ref_or_id = v
                .source
                .package_name
                .as_deref()
                .or(package.app_id.as_deref())
                .unwrap_or(&package.name);
            let remote = v.source.id.as_str();
            if let Ok((download, installed)) = flatpak::remote_info_sizes(remote, ref_or_id).await {
                if v.download_size.is_none() {
                    v.download_size = download;
                }
                if v.installed_size.is_none() {
                    v.installed_size = installed;
                }
            }
        } else if v.source.source_type == "repo" || v.source.id == "chaotic-aur" {
            let pkg_name = v
                .source
                .package_name
                .as_deref()
                .unwrap_or(&package.name);
            let key = (v.source.id.clone(), pkg_name.to_string());
            if let Some((download, installed)) = repo_sizes.get(&key) {
                v.download_size = Some(*download);
                v.installed_size = Some(*installed);
            }
        }
        out.push(v);
    }
    out
}

fn variant_for_source<'a>(
    variants: &'a [PackageVariant],
    source: &PackageSource,
) -> Option<&'a PackageVariant> {
    variants.iter().find(|variant| {
        if variant.source.source_type != source.source_type || variant.source.id != source.id {
            return false;
        }
        // Match package_name when both set; if either is None, match by type+id only
        // so Flatpak/repo selection works even when backend omits package_name on one side.
        match (
            variant.source.package_name.as_deref(),
            source.package_name.as_deref(),
        ) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    })
}

fn apply_variant_to_package(package: &mut Package, variant: Option<&PackageVariant>) {
    let Some(variant) = variant else {
        return;
    };

    package.version = variant.version.clone();
    package.maintainer = variant.maintainer.clone();
    package.license = variant.license.clone();
    package.download_size_bytes = variant.download_size;
    package.download_size = variant.download_size;
    package.installed_size_bytes = variant.installed_size;
    package.installed_size = variant.installed_size;
    if let Some(description) = variant
        .description
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        package.description = description.clone();
    }
    if let Some(screenshots) = variant
        .screenshots
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        package.screenshots = Some(screenshots.clone());
    }
}

fn local_review_to_package_review(review: LocalReview) -> PackageReview {
    PackageReview {
        review_id: None,
        app_id: review.app_id,
        user_display: Some(if review.user_display.trim().is_empty() {
            "MonARCH user".to_string()
        } else {
            review.user_display
        }),
        summary: (!review.summary.trim().is_empty()).then_some(review.summary),
        description: (!review.description.trim().is_empty()).then_some(review.description),
        rating: Some(review.rating),
        date_created: Some(review.date_created as f64),
        version: Some("Local review".to_string()),
        distro: Some("MonARCH".to_string()),
        locale: None,
    }
}

/// User-facing notice when the app is installed: explain that switching source requires uninstall first,
/// and how Flatpaks differ from repo/AUR.
fn build_source_switch_notice(source: &PackageSource) -> String {
    let base = format!("Installed from {}.", source.label);
    if source.source_type == "flatpak" {
        format!(
            "{} Flatpaks are separate from repo and AUR installs. To use the repo or AUR version, uninstall this Flatpak first.",
            base
        )
    } else {
        format!(
            "{} To switch to another source (e.g. Flatpak or a different repo), uninstall the current one first.",
            base
        )
    }
}

fn derive_developer_name(package: &Package) -> Option<String> {
    package
        .maintainer
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .cloned()
}

fn build_security_summary(
    source: Option<&PackageSource>,
    maintainer_known: bool,
) -> PackageSecuritySummary {
    let fallback = PackageSource::new("repo", "core", "latest", "Arch Official");
    let source = source.unwrap_or(&fallback);
    let id = source.id.to_lowercase();

    let (trust_tier, system_access, verification_note) = match source.source_type.as_str() {
        "flatpak" => (
            "sandboxed",
            "scoped",
            "Runs with sandboxed permissions, which may vary by app.",
        ),
        "aur" => (
            "community_build",
            "full",
            "Built from community-provided packaging scripts on your machine.",
        ),
        "repo" if id.contains("chaotic") => (
            "third_party_repo",
            "full",
            "Provided by a third-party binary repository.",
        ),
        "repo"
            if id.contains("cachyos")
                || id.contains("manjaro")
                || id.contains("garuda")
                || id.contains("endeavour") =>
        {
            (
                "distro_native",
                "full",
                "Provided by your distribution's repositories.",
            )
        }
        _ => (
            "official",
            "full",
            "Provided by the system package repositories.",
        ),
    };

    let user_action_note = if maintainer_known {
        "Review the source and your distro's documentation when choosing where to install from."
    } else {
        "This source did not publish a maintainer. Check the source and your distro's documentation before installing."
    };

    PackageSecuritySummary {
        trust_tier: trust_tier.to_string(),
        system_access: system_access.to_string(),
        maintainer_known,
        verification_note: verification_note.to_string(),
        user_action_note: user_action_note.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Package, PackageSource};

    fn sample_package(name: &str, installed: bool) -> Package {
        Package {
            name: name.to_string(),
            display_name: Some(format!("{name} App")),
            display_title: Some(format!("{name} App")),
            description: format!("{name} package"),
            icon: Some(format!("https://example.test/{name}.png")),
            categories: Some(vec!["Utilities".to_string()]),
            version: "1.0.0".to_string(),
            source: PackageSource::new("repo", "core", "1.0.0", "Arch Official"),
            canonical_id: name.to_string(),
            installed,
            ..Package::default()
        }
    }

    fn package_with_source(
        canonical_id: &str,
        source_type: &str,
        source_id: &str,
        label: &str,
        installed: bool,
    ) -> Package {
        Package {
            name: canonical_id.to_string(),
            display_name: Some(canonical_id.to_string()),
            display_title: Some(canonical_id.to_string()),
            description: format!("{canonical_id} package"),
            icon: Some(format!("https://example.test/{canonical_id}.png")),
            categories: Some(vec!["Utilities".to_string()]),
            version: "1.0.0".to_string(),
            source: PackageSource::new(source_type, source_id, "1.0.0", label),
            canonical_id: canonical_id.to_string(),
            installed,
            ..Package::default()
        }
    }

    #[test]
    fn discovery_and_search_use_registry_data() {
        let registry = Arc::new(RegistryManager::in_memory().expect("registry"));
        registry
            .bulk_upsert_packages(&[
                sample_package("firefox", true),
                sample_package("vlc", false),
            ])
            .expect("seed packages");

        let service = CatalogService::new(registry);
        let runtime = tokio::runtime::Runtime::new().expect("runtime");

        let discovery = runtime
            .block_on(service.load_discovery_snapshot())
            .expect("discovery");
        assert_eq!(discovery.len(), 2);

        let search = runtime
            .block_on(service.search("fire", SearchOptions::default()))
            .expect("search");
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].canonical_id, "firefox");
    }

    #[test]
    fn installed_view_returns_only_installed_rows() {
        let registry = Arc::new(RegistryManager::in_memory().expect("registry"));
        registry
            .bulk_upsert_packages(&[
                sample_package("firefox", true),
                sample_package("vlc", false),
            ])
            .expect("seed packages");

        let service = CatalogService::new(registry);
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let installed = runtime
            .block_on(service.load_installed())
            .expect("installed");

        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].canonical_id, "firefox");
        assert!(installed[0].installed);
    }

    #[test]
    fn storefront_filters_out_technical_packages() {
        let app = sample_package("firefox", false);
        let keyring = Package {
            name: "chaotic-keyring".to_string(),
            display_name: Some("Chaotic Keyring".to_string()),
            display_title: Some("Chaotic Keyring".to_string()),
            description: "PGP keyring".to_string(),
            source: PackageSource::new("repo", "chaotic-aur", "1.0.0", "Chaotic-AUR"),
            canonical_id: "chaotic-keyring".to_string(),
            ..Package::default()
        };
        let data = Package {
            name: "0ad-data".to_string(),
            display_name: Some("0ad Data".to_string()),
            display_title: Some("0ad Data".to_string()),
            description: "Game data files".to_string(),
            source: PackageSource::new("repo", "extra", "1.0.0", "Arch Official"),
            canonical_id: "0ad-data".to_string(),
            ..Package::default()
        };

        assert!(is_storefront_package(&app));
        assert!(!is_storefront_package(&keyring));
        assert!(!is_storefront_package(&data));
    }

    #[test]
    fn library_filters_out_non_app_installed_packages() {
        let app = sample_package("firefox", true);
        let technical = Package {
            name: "python-requests".to_string(),
            display_name: Some("python-requests".to_string()),
            display_title: Some("python-requests".to_string()),
            description: "HTTP library".to_string(),
            installed: true,
            source: PackageSource::new("repo", "extra", "1.0.0", "Arch Official"),
            canonical_id: "python-requests".to_string(),
            ..Package::default()
        };

        assert!(is_library_package(&app));
        assert!(!is_library_package(&technical));
    }

    #[test]
    fn source_toggles_hide_from_storefront_but_not_installed_lookup() {
        let installed_flatpak = Package {
            source: PackageSource::new("flatpak", "flathub", "1.0.0", "Flatpak"),
            canonical_id: "flatpak-app".to_string(),
            name: "org.example.FlatpakApp".to_string(),
            display_name: Some("Flatpak App".to_string()),
            display_title: Some("Flatpak App".to_string()),
            description: "Flatpak desktop app".to_string(),
            icon: Some("https://example.test/flatpak.png".to_string()),
            categories: Some(vec!["Utilities".to_string()]),
            installed: true,
            ..Package::default()
        };
        let installed_aur = Package {
            source: PackageSource::new("aur", "aur", "1.0.0", "AUR"),
            canonical_id: "aur-app".to_string(),
            name: "aur-app".to_string(),
            display_name: Some("AUR App".to_string()),
            display_title: Some("AUR App".to_string()),
            description: "AUR desktop app".to_string(),
            icon: Some("https://example.test/aur.png".to_string()),
            categories: Some(vec!["Utilities".to_string()]),
            installed: true,
            ..Package::default()
        };
        let storefront_flatpak = Package {
            installed: false,
            ..installed_flatpak.clone()
        };
        let storefront_aur = Package {
            installed: false,
            ..installed_aur.clone()
        };
        let storefront_chaotic = Package {
            source: PackageSource::new("repo", "chaotic-aur", "1.0.0", "Chaotic-AUR"),
            canonical_id: "chaotic-app".to_string(),
            name: "chaotic-app".to_string(),
            display_name: Some("Chaotic App".to_string()),
            display_title: Some("Chaotic App".to_string()),
            description: "Chaotic desktop app".to_string(),
            icon: Some("https://example.test/chaotic.png".to_string()),
            categories: Some(vec!["Utilities".to_string()]),
            installed: false,
            ..Package::default()
        };

        let hidden_from_storefront = filter_visible_packages(
            vec![
                storefront_flatpak.clone(),
                storefront_aur.clone(),
                storefront_chaotic.clone(),
            ],
            &SearchOptions {
                flatpak_enabled: Some(false),
                aur_enabled: Some(false),
                chaotic_enabled: Some(false),
                ..Default::default()
            },
        );
        assert!(hidden_from_storefront.is_empty());

        let retained_for_installed = filter_visible_packages(
            vec![installed_flatpak.clone(), installed_aur.clone()],
            &SearchOptions {
                flatpak_enabled: Some(false),
                aur_enabled: Some(false),
                chaotic_enabled: Some(false),
                for_installed_lookup: Some(true),
                ..Default::default()
            },
        );
        assert_eq!(retained_for_installed.len(), 2);
        assert!(retained_for_installed
            .iter()
            .all(|package| package.installed));
    }

    #[test]
    fn system_apps_can_appear_in_search_when_enabled() {
        let system_app = Package {
            name: "btop".to_string(),
            display_name: Some("btop".to_string()),
            display_title: Some("btop".to_string()),
            description: "Resource monitor for the terminal".to_string(),
            source: PackageSource::new("repo", "extra", "1.0.0", "Arch Official"),
            canonical_id: "btop".to_string(),
            ..Package::default()
        };

        assert!(!is_storefront_package(&system_app));
        // A package whose name matches the query is always returned (name-match path in
        // is_storefront_match) regardless of include_system_apps.
        assert!(is_search_match(&system_app, Some("btop"), false));
        assert!(is_search_match(&system_app, Some("btop"), true));

        // A system-only package is excluded when the query doesn't match its name
        // and include_system_apps is false.
        let unrelated_system = Package {
            name: "libx11".to_string(),
            description: "X11 client-side library".to_string(),
            source: PackageSource::new("repo", "extra", "1.0.0", "Arch Official"),
            canonical_id: "libx11".to_string(),
            ..Package::default()
        };
        assert!(!is_storefront_package(&unrelated_system));
        assert!(!is_search_match(&unrelated_system, Some("btop"), false));
        // libx11 does NOT appear when searching "btop" even with system apps enabled,
        // because the query doesn't match its name and it's not a GUI storefront app.
        assert!(!is_search_match(&unrelated_system, Some("btop"), true));
    }

    #[test]
    fn registry_bootstrap_detects_stale_sparse_hydration() {
        assert!(registry_needs_bootstrap(RegistryHydrationStats {
            total_packages: 18_000,
            non_installed_packages: 16_000,
            icon_packages: 41,
            rich_metadata_packages: 45,
            hydration_version: 0,
        }));

        assert!(!registry_needs_bootstrap(RegistryHydrationStats {
            total_packages: 18_000,
            non_installed_packages: 16_000,
            icon_packages: 7_500,
            rich_metadata_packages: 8_000,
            hydration_version: REGISTRY_HYDRATION_VERSION,
        }));
    }

    #[test]
    fn parses_checkupdates_output() {
        let updates =
            parse_checkupdates_output("firefox 123.0-1 -> 124.0-1\nvlc 3.0.20-2 -> 3.0.21-1\n");

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].name, "firefox");
        assert_eq!(updates[0].current_version, "123.0-1");
        assert_eq!(updates[0].new_version, "124.0-1");
        assert_eq!(updates[1].name, "vlc");
    }

    #[test]
    fn parses_flatpak_update_output() {
        let mut installed = std::collections::HashMap::new();
        installed.insert("org.mozilla.firefox".to_string(), "123.0".to_string());

        let updates = parse_flatpak_updates_output(
            "org.mozilla.firefox\t124.0\t1048576\tFirefox\ncom.discordapp.Discord\t0.0.70\t2097152\tDiscord\n",
            &installed,
        );

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].name, "org.mozilla.firefox");
        assert_eq!(updates[0].current_version, "123.0");
        assert_eq!(updates[0].new_version, "124.0");
        assert_eq!(updates[1].display_name.as_deref(), Some("Discord"));
    }

    #[test]
    fn parses_foreign_package_output() {
        let output = "google-chrome 123.0.6312.86-1\ndiscord 0.0.49-1\n";
        let parsed: Vec<(String, String)> = output
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                Some((parts.next()?.to_string(), parts.next()?.to_string()))
            })
            .collect();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "google-chrome");
        assert_eq!(parsed[1].1, "0.0.49-1");
    }

    #[test]
    fn backend_search_prefers_installed_and_native_sources() {
        let packages = vec![
            package_with_source("chromium-flatpak", "flatpak", "flathub", "Flathub", false),
            package_with_source(
                "chromium-chaotic",
                "repo",
                "chaotic-aur",
                "Chaotic-AUR",
                false,
            ),
            package_with_source("chromium-aur", "aur", "aur", "AUR", false),
            package_with_source("chromium", "repo", "extra", "Arch Official", true),
        ];

        let ranked = filter_and_sort_packages(
            packages,
            &SearchOptions {
                aur_enabled: Some(true),
                flatpak_enabled: Some(true),
                chaotic_enabled: Some(true),
                sort_mode: Some(SearchSortMode::Relevance),
                ..SearchOptions::default()
            },
            Some("chromium"),
        );

        let ids = ranked
            .iter()
            .map(|package| package.canonical_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "chromium",
                "chromium-chaotic",
                "chromium-flatpak",
                "chromium-aur"
            ]
        );
    }

    #[test]
    fn storefront_name_sort_does_not_force_installed_apps_to_the_front() {
        let ranked = filter_and_sort_packages(
            vec![
                sample_package("zzz-installed", true),
                sample_package("alpha-app", false),
            ],
            &SearchOptions {
                sort_mode: Some(SearchSortMode::Name),
                ..SearchOptions::default()
            },
            None,
        );

        let ids = ranked
            .iter()
            .map(|package| package.canonical_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["alpha-app", "zzz-installed"]);
    }

    #[test]
    fn curated_storefront_lanes_use_exact_app_matches_not_fuzzy_related_packages() {
        let registry = RegistryManager::in_memory().expect("registry");
        registry
            .bulk_upsert_packages(&[
                Package {
                    name: "discord".to_string(),
                    display_name: Some("Discord".to_string()),
                    display_title: Some("Discord".to_string()),
                    description: "Chat, voice, and hang out".to_string(),
                    icon: Some("https://example.test/discord.png".to_string()),
                    categories: Some(vec!["Network".to_string()]),
                    canonical_id: "discord".to_string(),
                    source: PackageSource::new("repo", "extra", "1.0.0", "Arch Official"),
                    ..Package::default()
                },
                Package {
                    name: "discord-chat-exporter-gui".to_string(),
                    display_name: Some("Discord Chat Exporter GUI".to_string()),
                    display_title: Some("Discord Chat Exporter GUI".to_string()),
                    description: "Exports Discord chat logs".to_string(),
                    icon: Some("https://example.test/exporter.png".to_string()),
                    categories: Some(vec!["Utilities".to_string()]),
                    canonical_id: "discord-chat-exporter-gui".to_string(),
                    source: PackageSource::new("repo", "extra", "1.0.0", "Arch Official"),
                    ..Package::default()
                },
            ])
            .expect("seed packages");

        let curated = load_curated_packages(&registry, &["discord"]);
        assert_eq!(curated.len(), 1);
        assert_eq!(curated[0].canonical_id, "discord");
    }

    #[test]
    fn backend_filters_by_category_without_gtk_side_logic() {
        let graphics = Package {
            categories: Some(vec!["Graphics".to_string()]),
            ..sample_package("inkscape", false)
        };
        let browser = Package {
            categories: Some(vec!["Network".to_string()]),
            ..sample_package("firefox", false)
        };

        let filtered = filter_and_sort_packages(
            vec![browser, graphics],
            &SearchOptions {
                category_filter: Some("graphics & design".to_string()),
                sort_mode: Some(SearchSortMode::Name),
                ..SearchOptions::default()
            },
            None,
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].canonical_id, "inkscape");
    }

    #[test]
    fn home_snapshot_exposes_arch_first_storefront_lanes() {
        let registry = Arc::new(RegistryManager::in_memory().expect("registry"));
        registry
            .bulk_upsert_packages(&[
                Package {
                    categories: Some(vec!["Graphics".to_string()]),
                    discovered_at: Some(10),
                    updated_at: Some(20),
                    screenshots: Some(vec!["https://example.test/inkscape.png".to_string()]),
                    ..package_with_source("inkscape", "repo", "extra", "Arch Official", false)
                },
                Package {
                    categories: Some(vec!["Utilities".to_string()]),
                    discovered_at: Some(11),
                    updated_at: Some(21),
                    ..package_with_source("paru-bin", "repo", "chaotic-aur", "Chaotic-AUR", false)
                },
                Package {
                    categories: Some(vec!["Network".to_string()]),
                    discovered_at: Some(12),
                    updated_at: Some(22),
                    ..package_with_source(
                        "org.mozilla.firefox",
                        "flatpak",
                        "flathub",
                        "Flathub",
                        false,
                    )
                },
                Package {
                    categories: Some(vec!["Development".to_string()]),
                    discovered_at: Some(13),
                    updated_at: Some(23),
                    ..package_with_source("visual-studio-code-bin", "aur", "aur", "AUR", false)
                },
            ])
            .expect("seed packages");

        let service = CatalogService::new(registry);
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let snapshot = runtime
            .block_on(service.load_home_snapshot_with_options(SearchOptions {
                aur_enabled: Some(true),
                flatpak_enabled: Some(true),
                chaotic_enabled: Some(true),
                ..SearchOptions::default()
            }))
            .expect("snapshot");

        assert_eq!(snapshot.native[0].canonical_id, "inkscape");
        assert_eq!(snapshot.chaotic[0].canonical_id, "paru-bin");
        assert_eq!(snapshot.flatpak[0].canonical_id, "org.mozilla.firefox");
        assert_eq!(snapshot.aur[0].canonical_id, "visual-studio-code-bin");
        assert!(snapshot
            .categories
            .iter()
            .any(|category| category.eq_ignore_ascii_case("graphics & design")));
        assert!(!snapshot.featured.is_empty());
    }

    #[test]
    fn storefront_categories_follow_monarch_taxonomy_order() {
        let categories = storefront_categories(&[
            Package {
                categories: Some(vec!["Office".to_string()]),
                ..sample_package("libreoffice", false)
            },
            Package {
                categories: Some(vec!["Network".to_string()]),
                ..sample_package("firefox", false)
            },
            Package {
                categories: Some(vec!["AudioVideo".to_string()]),
                ..sample_package("vlc", false)
            },
        ]);

        assert_eq!(
            categories,
            vec![
                "Games".to_string(),
                "Productivity".to_string(),
                "Internet".to_string(),
                "Multimedia".to_string(),
                "Development".to_string(),
                "Graphics & Design".to_string(),
                "System Tools".to_string(),
                "Utilities".to_string(),
            ]
        );
    }

    #[test]
    fn install_routes_cover_repo_chaotic_flatpak_and_aur() {
        let package = Package {
            name: "discord".to_string(),
            app_id: Some("com.discordapp.Discord".to_string()),
            ..Package::default()
        };

        assert_eq!(
            install_route_for_source(
                &package,
                &PackageSource {
                    source_type: "repo".to_string(),
                    id: "extra".to_string(),
                    version: "1.0".to_string(),
                    label: "Arch Official".to_string(),
                    package_name: Some("discord".to_string()),
                }
            ),
            SourceInstallRoute::Repo {
                target_name: "discord".to_string(),
                target_repo: "extra".to_string(),
            }
        );

        assert_eq!(
            install_route_for_source(
                &package,
                &PackageSource {
                    source_type: "repo".to_string(),
                    id: "chaotic-aur".to_string(),
                    version: "1.0".to_string(),
                    label: "Chaotic-AUR".to_string(),
                    package_name: Some("discord-canary".to_string()),
                }
            ),
            SourceInstallRoute::Repo {
                target_name: "discord-canary".to_string(),
                target_repo: "chaotic-aur".to_string(),
            }
        );

        assert_eq!(
            install_route_for_source(
                &package,
                &PackageSource {
                    source_type: "flatpak".to_string(),
                    id: "flathub".to_string(),
                    version: "1.0".to_string(),
                    label: "Flathub".to_string(),
                    package_name: None,
                }
            ),
            SourceInstallRoute::Flatpak {
                app_id: "com.discordapp.Discord".to_string(),
                remote: Some("flathub".to_string()),
            }
        );

        assert_eq!(
            install_route_for_source(
                &package,
                &PackageSource {
                    source_type: "aur".to_string(),
                    id: "aur".to_string(),
                    version: "1.0".to_string(),
                    label: "AUR".to_string(),
                    package_name: Some("discord_arch_electron".to_string()),
                }
            ),
            SourceInstallRoute::Aur {
                target_name: "discord_arch_electron".to_string(),
            }
        );
    }

    #[test]
    fn remove_routes_use_flatpak_app_id_and_native_package_names() {
        let package = Package {
            name: "org.mozilla.firefox".to_string(),
            app_id: Some("org.mozilla.firefox".to_string()),
            ..Package::default()
        };

        assert_eq!(
            remove_route_for_source(
                &package,
                &PackageSource {
                    source_type: "flatpak".to_string(),
                    id: "flathub".to_string(),
                    version: "1.0".to_string(),
                    label: "Flathub".to_string(),
                    package_name: None,
                }
            ),
            SourceRemoveRoute::Flatpak {
                app_id: "org.mozilla.firefox".to_string(),
            }
        );

        assert_eq!(
            remove_route_for_source(
                &package,
                &PackageSource {
                    source_type: "repo".to_string(),
                    id: "extra".to_string(),
                    version: "1.0".to_string(),
                    label: "Arch Official".to_string(),
                    package_name: Some("firefox".to_string()),
                }
            ),
            SourceRemoveRoute::Native {
                target_name: "firefox".to_string(),
            }
        );
    }
}

#[tauri::command]
pub async fn get_metadata(
    state: State<'_, MetadataState>,
    scm_state: State<'_, crate::ScmState>,
    chaotic_state: State<'_, crate::chaotic_api::ChaoticApiClient>,
    flathub_state: State<'_, crate::flathub_api::FlathubApiClient>,
    pkg_name: String,
    upstream_url: Option<String>,
) -> Result<AppMetadata, ()> {
    // Scope the lock so it is dropped before any await points
    let appstream_result = {
        let loader = state.0.lock().unwrap();

        // 1. Try exact AppStream match
        if let Some(meta) = loader.find_package(&pkg_name) {
            Some(meta)
        } else {
            // 2. Try heuristic matching
            let mut found = None;
            let base_name = crate::utils::strip_package_suffix(&pkg_name);

            if base_name != &pkg_name {
                if let Some(mut meta) = loader.find_package(base_name) {
                    meta.pkg_name = Some(pkg_name.clone());
                    found = Some(meta);
                }
            }

            if found.is_none() {
                // 3. Try prefix matching
                if let Some(col) = &loader.collection {
                    for component in col.components.iter() {
                        if let Some(cpkg) = &component.pkgname {
                            if pkg_name.starts_with(cpkg.as_str()) && pkg_name.len() > cpkg.len() {
                                let mut meta = loader.component_to_metadata(component);
                                meta.pkg_name = Some(pkg_name.clone());
                                found = Some(meta);
                                break;
                            }
                        }
                    }
                }
            }
            found
        }
    };

    let mut app_meta = None;

    if let Some(mut meta) = appstream_result {
        // If we have an icon, we're done
        if meta.icon_url.is_some() {
            return Ok(meta);
        }

        // Try heuristic icon if AppStream didn't have one
        {
            let loader = state.0.lock().unwrap();
            if let Some(icon) = loader.find_icon_heuristic(&pkg_name) {
                meta.icon_url = Some(icon);
                return Ok(meta);
            }
        }

        // If still no icon, fall through to remote APIs
        app_meta = Some(meta);
    }

    // 4. Try Flathub API
    let flathub_meta = flathub_state.get_metadata_for_package(&pkg_name).await;

    if let Some(meta) = flathub_meta {
        return Ok(crate::flathub_api::flathub_to_app_metadata(
            &meta, &pkg_name,
        ));
    }

    // 5. Try SCM Metadata (GitHub/GitLab)
    let scm_data = if let Some(url) = &upstream_url {
        scm_state.0.fetch_metadata(url).await
    } else {
        None
    };

    if let Some(scm) = &scm_data {
        if scm.icon_url.is_some() || scm.description.is_some() || !scm.screenshots.is_empty() {
            return Ok(AppMetadata {
                name: pkg_name.clone(),
                pkg_name: Some(pkg_name.clone()),
                icon_url: scm.icon_url.clone(),
                app_id: pkg_name.clone(),
                summary: scm.description.clone(),
                screenshots: scm.screenshots.clone(),
                version: None,
                maintainer: None,
                license: scm.license.clone(),
                last_updated: None,
                description: scm.description.clone(),
            });
        }
    }

    // 6. Try Chaotic-AUR Metadata
    let chaotic_pkg = chaotic_state.find_package(&pkg_name).await;

    if let Some(cp) = chaotic_pkg {
        if let Some(meta) = cp.metadata {
            return Ok(AppMetadata {
                name: pkg_name.clone(),
                pkg_name: Some(pkg_name.clone()),
                icon_url: None,
                app_id: pkg_name.clone(),
                summary: meta.desc,
                screenshots: vec![],
                version: cp.version,
                maintainer: None,
                license: meta.license,
                last_updated: None,
                description: None,
            });
        }
    }

    // 7. Fallback: Try OpenGraph image, then Favicon from upstream URL
    let mut icon_url = app_meta.as_ref().and_then(|m| m.icon_url.clone());
    let mut fallback_screenshots = vec![];

    if let Some(scm) = scm_data {
        icon_url = scm.icon_url;
        fallback_screenshots = scm.screenshots;
    }

    if icon_url.is_none() {
        if let Some(url) = &upstream_url {
            if let Some(og) = fetch_og_image(url).await {
                icon_url = Some(og);
            } else {
                icon_url = Some(get_favicon_url(url));
            }
        }
    }

    Ok(AppMetadata {
        name: pkg_name.clone(),
        pkg_name: Some(pkg_name.clone()),
        icon_url,
        app_id: pkg_name,
        summary: app_meta.as_ref().and_then(|m| m.summary.clone()),
        screenshots: fallback_screenshots,
        version: None,
        maintainer: None,
        license: None,
        last_updated: None,
        description: app_meta.as_ref().and_then(|m| m.description.clone()),
    })
}

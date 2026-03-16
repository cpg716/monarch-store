use crate::models::{Package, PackageSource};
use rusqlite::{params, params_from_iter, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

pub const REGISTRY_HYDRATION_VERSION: i64 = 6;

#[derive(Debug)]
pub struct RegistryManager {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RegistryHydrationStats {
    pub total_packages: usize,
    pub non_installed_packages: usize,
    pub icon_packages: usize,
    pub rich_metadata_packages: usize,
    pub hydration_version: i64,
}

impl RegistryManager {
    pub fn new() -> Result<Self, String> {
        let path = Self::get_db_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        let _ = conn.execute("PRAGMA journal_mode=WAL", []);
        let _ = conn.execute("PRAGMA synchronous=NORMAL", []);

        let registry = Self {
            conn: Mutex::new(conn),
        };
        registry.ensure_schema()?;
        Ok(registry)
    }

    pub fn in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        let registry = Self {
            conn: Mutex::new(conn),
        };
        registry.ensure_schema()?;
        Ok(registry)
    }

    fn get_db_path() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("monarch-store")
            .join("registry.db")
    }

    pub fn search_packages_sql(&self, query: &str, limit: usize) -> Result<Vec<Package>, String> {
        self.ensure_schema()?;
        let query = query.trim().to_lowercase();
        let sql = if query.is_empty() {
            "SELECT canonical_id FROM packages
             ORDER BY COALESCE(display_name, name) COLLATE NOCASE ASC
             LIMIT ?1"
                .to_string()
        } else {
            "SELECT canonical_id FROM packages
             WHERE lower(name) LIKE ?1
                OR lower(COALESCE(display_name, '')) LIKE ?1
                OR lower(COALESCE(description, '')) LIKE ?1
                OR lower(COALESCE(app_id, '')) LIKE ?1
             ORDER BY 
                CASE 
                    WHEN lower(name) = ?2 THEN 0
                    WHEN lower(COALESCE(display_name, '')) = ?2 THEN 1
                    WHEN lower(name) LIKE ?3 THEN 2
                    WHEN lower(COALESCE(display_name, '')) LIKE ?3 THEN 3
                    WHEN lower(name) LIKE ?1 THEN 4
                    WHEN lower(COALESCE(display_name, '')) LIKE ?1 THEN 5
                    ELSE 6 
                END ASC,
                LENGTH(name) ASC,
                COALESCE(display_name, name) COLLATE NOCASE ASC
             LIMIT ?4"
                .to_string()
        };

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let ids = if query.is_empty() {
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![limit as i64], |row| row.get::<_, String>(0))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            rows
        } else {
            let contains = format!("%{}%", query);
            let exact = query.clone();
            let starts_with = format!("{}%", query);
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![contains, exact, starts_with, limit as i64], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            rows
        };
        drop(conn);

        self.get_packages_by_canonical_ids(&ids)
    }

    pub fn get_packages_for_category(
        &self,
        category_tokens: &[&str],
        limit: usize,
    ) -> Result<Vec<Package>, String> {
        if category_tokens.is_empty() {
            return Ok(Vec::new());
        }
        self.ensure_schema()?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Build a dynamic IN() clause for all token aliases.
        // e.g. Graphics & Design → Graphics, 2DGraphics, Photography, ...
        let placeholders = category_tokens
            .iter()
            .enumerate()
            .map(|(i, _)| format!("lower(?{})", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT p.canonical_id
             FROM packages p
             INNER JOIN package_categories pc ON pc.canonical_id = p.canonical_id
             WHERE lower(pc.category) IN ({placeholders})
             ORDER BY (p.app_id IS NOT NULL AND p.app_id != '') DESC, (p.icon IS NOT NULL AND p.icon != '') DESC, p.installed DESC
             LIMIT ?{}",
            category_tokens.len() + 1
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

        let ids = stmt
            .query_map(
                rusqlite::params_from_iter(
                    category_tokens
                        .iter()
                        .map(|s| s.to_lowercase())
                        .chain(std::iter::once((limit as i64).to_string()))
                        .collect::<Vec<_>>(),
                ),
                |row| row.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        drop(conn);

        self.get_packages_by_canonical_ids(&ids)
    }

    pub fn count_packages(&self) -> Result<usize, String> {
        self.ensure_schema()?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let count = conn
            .query_row("SELECT COUNT(*) FROM packages", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|e| e.to_string())?;
        Ok(count.max(0) as usize)
    }

    pub fn hydration_stats(&self) -> Result<RegistryHydrationStats, String> {
        self.ensure_schema()?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let version = Self::load_hydration_version(&conn).unwrap_or_default();

        // Try a fast path: read pre-computed counts from registry_meta.
        let fast_total: Option<i64> = conn
            .query_row(
                "SELECT value FROM registry_meta WHERE key = 'cached_total_packages'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok());

        if let Some(total) = fast_total {
            let non_installed: i64 = conn
                .query_row(
                    "SELECT value FROM registry_meta WHERE key = 'cached_non_installed_packages'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let icon_packages: i64 = conn
                .query_row(
                    "SELECT value FROM registry_meta WHERE key = 'cached_icon_packages'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let rich_packages: i64 = conn
                .query_row(
                    "SELECT value FROM registry_meta WHERE key = 'cached_rich_metadata_packages'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);

            return Ok(RegistryHydrationStats {
                total_packages: total.max(0) as usize,
                non_installed_packages: non_installed.max(0) as usize,
                icon_packages: icon_packages.max(0) as usize,
                rich_metadata_packages: rich_packages.max(0) as usize,
                hydration_version: version,
            });
        }

        // Fallback: compute from the database (slow path, only reached for new/empty DBs
        // or small in-memory test registries where the fast-path cache is not populated).
        let stats = conn
            .query_row(
                "SELECT
                    COUNT(*) AS total_packages,
                    COALESCE(SUM(CASE WHEN installed = 0 THEN 1 ELSE 0 END), 0) AS non_installed_packages,
                    COALESCE(SUM(CASE WHEN icon IS NOT NULL AND trim(icon) != '' THEN 1 ELSE 0 END), 0) AS icon_packages,
                    COALESCE(SUM(CASE
                        WHEN (icon IS NOT NULL AND trim(icon) != '')
                          OR (app_id IS NOT NULL AND trim(app_id) != '')
                          OR (long_description IS NOT NULL AND trim(long_description) != '')
                          OR (screenshots IS NOT NULL AND trim(screenshots) != '' AND trim(screenshots) != '[]')
                        THEN 1 ELSE 0 END), 0) AS rich_metadata_packages
                 FROM packages",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .unwrap_or((0, 0, 0, 0));

        Ok(RegistryHydrationStats {
            total_packages: stats.0.max(0) as usize,
            non_installed_packages: stats.1.max(0) as usize,
            icon_packages: stats.2.max(0) as usize,
            rich_metadata_packages: stats.3.max(0) as usize,
            hydration_version: version,
        })
    }

    pub fn get_installed_packages(&self, limit: usize) -> Result<Vec<Package>, String> {
        self.ensure_schema()?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT canonical_id FROM packages
                 WHERE installed = 1
                 ORDER BY COALESCE(display_name, name) COLLATE NOCASE ASC
                 LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let ids = stmt
            .query_map(params![limit as i64], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        drop(conn);

        self.get_packages_by_canonical_ids(&ids)
    }

    pub fn get_packages_by_canonical_ids(&self, ids: &[String]) -> Result<Vec<Package>, String> {
        self.ensure_schema()?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT canonical_id, name, display_name, description, version, app_id, icon, installed,
                    last_modified, maintainer, license, url, long_description, screenshots,
                    discovered_at, updated_at
             FROM packages
             WHERE canonical_id IN ({})
             ORDER BY COALESCE(display_name, name) COLLATE NOCASE ASC",
            placeholders
        );

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mut packages = stmt
            .query_map(params_from_iter(ids.iter()), |row| {
                let canonical_id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let display_name: Option<String> = row.get(2)?;
                let description: Option<String> = row.get(3)?;
                let version: Option<String> = row.get(4)?;
                let app_id: Option<String> = row.get(5)?;
                let icon: Option<String> = row.get(6)?;
                let installed: bool = row.get(7)?;
                let last_modified: Option<i64> = row.get(8)?;
                let maintainer: Option<String> = row.get(9)?;
                let license_csv: Option<String> = row.get(10)?;
                let url: Option<String> = row.get(11)?;
                let long_description: Option<String> = row.get(12)?;
                let screenshots_json: Option<String> = row.get(13)?;
                let discovered_at: Option<i64> = row.get(14)?;
                let updated_at: Option<i64> = row.get(15)?;

                let sources = Self::load_sources(&conn, &canonical_id)?;
                let categories = Self::load_categories(&conn, &canonical_id)?;
                let source = sources
                    .first()
                    .cloned()
                    .unwrap_or_else(|| PackageSource::new("repo", "core", "", "Arch Official"));

                Ok(Package {
                    name: name.clone(),
                    display_name,
                    display_title: None,
                    description: description.unwrap_or_default(),
                    version: version.unwrap_or_default(),
                    source,
                    maintainer,
                    license: license_csv.map(|value| {
                        value
                            .split(',')
                            .map(|item| item.trim().to_string())
                            .filter(|item| !item.is_empty())
                            .collect::<Vec<_>>()
                    }),
                    url,
                    last_modified,
                    last_modified_unix: last_modified,
                    icon,
                    screenshots: screenshots_json
                        .as_deref()
                        .and_then(|value| serde_json::from_str(value).ok()),
                    app_id,
                    canonical_id,
                    installed,
                    categories: Some(categories),
                    available_sources: Some(sources),
                    long_description,
                    discovered_at,
                    updated_at,
                    ..Package::default()
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, rusqlite::Error>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        drop(conn);

        let order = ids
            .iter()
            .enumerate()
            .map(|(index, id)| (id.clone(), index))
            .collect::<std::collections::HashMap<_, _>>();
        packages.sort_by_key(|pkg| order.get(&pkg.canonical_id).copied().unwrap_or(usize::MAX));

        Ok(packages)
    }

    pub fn get_package(&self, canonical_id: &str) -> Result<Option<Package>, String> {
        self.get_packages_by_canonical_ids(&[canonical_id.to_string()])
            .map(|mut packages| packages.drain(..).next())
    }

    pub fn bulk_upsert_packages(&self, packages: &[Package]) -> Result<(), String> {
        self.ensure_schema()?;
        if packages.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        {
            let mut upsert_pkg = tx
                .prepare_cached(
                    "INSERT INTO packages (
                        canonical_id, name, display_name, description, version, app_id, icon,
                        installed, last_modified, maintainer, license, url, long_description, screenshots,
                        discovered_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                    ON CONFLICT(canonical_id) DO UPDATE SET
                        name = excluded.name,
                        display_name = excluded.display_name,
                        description = excluded.description,
                        version = excluded.version,
                        app_id = excluded.app_id,
                        icon = excluded.icon,
                        installed = excluded.installed,
                        last_modified = excluded.last_modified,
                        maintainer = excluded.maintainer,
                        license = excluded.license,
                        url = excluded.url,
                        long_description = excluded.long_description,
                        screenshots = excluded.screenshots,
                        discovered_at = COALESCE(packages.discovered_at, excluded.discovered_at),
                        updated_at = excluded.updated_at",
                )
                .map_err(|e| e.to_string())?;
            let mut delete_sources = tx
                .prepare_cached("DELETE FROM package_sources WHERE canonical_id = ?1")
                .map_err(|e| e.to_string())?;
            let mut insert_source = tx
                .prepare_cached(
                    "INSERT INTO package_sources (canonical_id, source_type, source_id, version, label, package_name)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(|e| e.to_string())?;
            let mut delete_categories = tx
                .prepare_cached("DELETE FROM package_categories WHERE canonical_id = ?1")
                .map_err(|e| e.to_string())?;
            let mut insert_category = tx
                .prepare_cached(
                    "INSERT INTO package_categories (canonical_id, category) VALUES (?1, ?2)",
                )
                .map_err(|e| e.to_string())?;

            for package in packages {
                let canonical_id = if package.canonical_id.is_empty() {
                    package.name.clone()
                } else {
                    package.canonical_id.clone()
                };
                let discovered_at = package.discovered_at.unwrap_or(now);
                let updated_at = package
                    .updated_at
                    .or(package.last_modified)
                    .unwrap_or(discovered_at);

                upsert_pkg
                    .execute(params![
                        canonical_id,
                        package.name,
                        package.display_name,
                        package.description,
                        package.version,
                        package.app_id,
                        package.icon,
                        package.installed,
                        package.last_modified,
                        package.maintainer,
                        package.license.as_ref().map(|value| value.join(",")),
                        package.url,
                        package.long_description,
                        package
                            .screenshots
                            .as_ref()
                            .and_then(|value| serde_json::to_string(value).ok()),
                        discovered_at,
                        updated_at
                    ])
                    .map_err(|e| e.to_string())?;

                delete_sources
                    .execute(params![canonical_id])
                    .map_err(|e| e.to_string())?;
                delete_categories
                    .execute(params![canonical_id])
                    .map_err(|e| e.to_string())?;

                let sources = package
                    .available_sources
                    .clone()
                    .unwrap_or_else(|| vec![package.source.clone()]);
                for source in sources {
                    insert_source
                        .execute(params![
                            canonical_id,
                            source.source_type,
                            source.id,
                            source.version,
                            source.label,
                            source.package_name
                        ])
                        .map_err(|e| e.to_string())?;
                }

                for category in package.categories.clone().unwrap_or_default() {
                    if category.trim().is_empty() {
                        continue;
                    }
                    insert_category
                        .execute(params![canonical_id, category])
                        .map_err(|e| e.to_string())?;
                }
            }
        }

        tx.commit().map_err(|e| e.to_string())
    }

    pub fn replace_all_packages(&self, packages: &[Package]) -> Result<(), String> {
        self.ensure_schema()?;
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM package_sources", [])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM package_categories", [])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM packages", [])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        drop(conn);
        self.bulk_upsert_packages(packages)?;

        // Compute hydration stats once and cache in registry_meta for fast subsequent reads.
        let total = packages.len() as i64;
        let non_installed = packages.iter().filter(|p| !p.installed).count() as i64;
        let icon_packages = packages
            .iter()
            .filter(|p| p.icon.as_deref().is_some_and(|v| !v.trim().is_empty()))
            .count() as i64;
        let rich_packages = packages
            .iter()
            .filter(|p| {
                p.icon.as_deref().is_some_and(|v| !v.trim().is_empty())
                    || p.screenshots
                        .as_ref()
                        .is_some_and(|shots| !shots.is_empty())
                    || p.app_id.as_deref().is_some_and(|v| !v.trim().is_empty())
                    || p.long_description
                        .as_deref()
                        .is_some_and(|v| !v.trim().is_empty())
            })
            .count() as i64;

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        for (key, value) in [
            ("hydration_version", REGISTRY_HYDRATION_VERSION.to_string()),
            ("cached_total_packages", total.to_string()),
            ("cached_non_installed_packages", non_installed.to_string()),
            ("cached_icon_packages", icon_packages.to_string()),
            ("cached_rich_metadata_packages", rich_packages.to_string()),
        ] {
            conn.execute(
                "INSERT INTO registry_meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn ensure_schema(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS packages (
                canonical_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                display_name TEXT,
                description TEXT,
                version TEXT,
                app_id TEXT,
                icon TEXT,
                installed BOOLEAN,
                last_modified INTEGER,
                maintainer TEXT,
                license TEXT,
                url TEXT,
                long_description TEXT,
                screenshots TEXT,
                discovered_at INTEGER,
                updated_at INTEGER
            );
            CREATE TABLE IF NOT EXISTS package_sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                canonical_id TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                version TEXT,
                label TEXT,
                package_name TEXT
            );
            CREATE TABLE IF NOT EXISTS package_categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                canonical_id TEXT NOT NULL,
                category TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS registry_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_packages_installed ON packages(installed);
            CREATE INDEX IF NOT EXISTS idx_packages_display_name ON packages(display_name);
            CREATE INDEX IF NOT EXISTS idx_sources_canonical ON package_sources(canonical_id);
            CREATE INDEX IF NOT EXISTS idx_categories_canonical ON package_categories(canonical_id);
            CREATE INDEX IF NOT EXISTS idx_categories_name ON package_categories(category);",
        )
        .map_err(|e| e.to_string())?;
        let _ = conn.execute("ALTER TABLE packages ADD COLUMN discovered_at INTEGER", []);
        let _ = conn.execute("ALTER TABLE packages ADD COLUMN updated_at INTEGER", []);
        Ok(())
    }

    fn load_hydration_version(conn: &Connection) -> Result<i64, rusqlite::Error> {
        conn.query_row(
            "SELECT value FROM registry_meta WHERE key = 'hydration_version'",
            [],
            |row| {
                let value: String = row.get(0)?;
                Ok(value.parse::<i64>().unwrap_or_default())
            },
        )
    }

    fn load_sources(
        conn: &Connection,
        canonical_id: &str,
    ) -> Result<Vec<PackageSource>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT source_type, source_id, version, label, package_name
             FROM package_sources
             WHERE canonical_id = ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([canonical_id], |row| {
            Ok(PackageSource {
                source_type: row.get(0)?,
                id: row.get(1)?,
                version: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                label: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                package_name: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    fn load_categories(
        conn: &Connection,
        canonical_id: &str,
    ) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT category
             FROM package_categories
             WHERE canonical_id = ?1
             ORDER BY category COLLATE NOCASE ASC",
        )?;
        let rows = stmt.query_map([canonical_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_package() -> Package {
        Package {
            name: "inkscape".to_string(),
            display_name: Some("Inkscape".to_string()),
            description: "Vector graphics editor".to_string(),
            version: "1.4.0-1".to_string(),
            source: PackageSource::new("repo", "extra", "1.4.0-1", "Arch Official"),
            canonical_id: "inkscape".to_string(),
            categories: Some(vec!["Graphics".to_string(), "Photography".to_string()]),
            discovered_at: Some(1_700_000_000),
            updated_at: Some(1_700_000_123),
            ..Package::default()
        }
    }

    #[test]
    fn round_trips_categories_and_freshness_fields() {
        let registry = RegistryManager::in_memory().expect("registry");
        registry
            .bulk_upsert_packages(&[sample_package()])
            .expect("seed package");

        let package = registry
            .get_package("inkscape")
            .expect("load package")
            .expect("package exists");

        assert_eq!(
            package.categories.expect("categories"),
            vec!["Graphics".to_string(), "Photography".to_string()]
        );
        assert_eq!(package.discovered_at, Some(1_700_000_000));
        assert_eq!(package.updated_at, Some(1_700_000_123));
    }

    #[test]
    fn category_lookup_uses_persisted_appstream_categories() {
        let registry = RegistryManager::in_memory().expect("registry");
        registry
            .bulk_upsert_packages(&[
                sample_package(),
                Package {
                    name: "kitty".to_string(),
                    display_name: Some("kitty".to_string()),
                    description: "Terminal emulator".to_string(),
                    version: "0.35.0-1".to_string(),
                    source: PackageSource::new("repo", "extra", "0.35.0-1", "Arch Official"),
                    canonical_id: "kitty".to_string(),
                    categories: Some(vec!["System".to_string()]),
                    ..Package::default()
                },
            ])
            .expect("seed packages");

        let graphics = registry
            .get_packages_for_category(&["graphics"], 10)
            .expect("graphics query");

        assert_eq!(graphics.len(), 1);
        assert_eq!(graphics[0].canonical_id, "inkscape");
    }

    #[test]
    fn hydration_stats_track_non_installed_and_rich_metadata() {
        let registry = RegistryManager::in_memory().expect("registry");
        let mut installed = sample_package();
        installed.installed = true;
        registry
            .bulk_upsert_packages(&[
                installed,
                Package {
                    name: "discord".to_string(),
                    display_name: Some("Discord".to_string()),
                    description: "Voice and text chat".to_string(),
                    version: "0.0.90-1".to_string(),
                    source: PackageSource::new("repo", "extra", "0.0.90-1", "Arch Official"),
                    canonical_id: "discord".to_string(),
                    installed: false,
                    icon: Some("data:image/png;base64,abc".to_string()),
                    screenshots: Some(vec!["https://cdn.example/discord.png".to_string()]),
                    app_id: Some("com.discordapp.Discord".to_string()),
                    long_description: Some("Rich metadata".to_string()),
                    ..Package::default()
                },
            ])
            .expect("seed packages");

        let stats = registry.hydration_stats().expect("stats");
        assert_eq!(stats.total_packages, 2);
        assert_eq!(stats.non_installed_packages, 1);
        assert_eq!(stats.icon_packages, 1);
        assert!(stats.rich_metadata_packages >= 1);
    }
}

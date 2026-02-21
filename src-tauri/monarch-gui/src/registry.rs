use crate::models::{Package, PackageSource};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

/// RegistryState wraps the RegistryManager for Tauri state management.
pub struct RegistryState {
    pub manager: RegistryManager,
    update_rx: Arc<Mutex<Option<mpsc::Receiver<RegistryUpdate>>>>,
}

impl RegistryState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            manager: RegistryManager::new(tx),
            update_rx: Arc::new(Mutex::new(Some(rx))),
        }
    }

    pub fn get_repo_names_for_canonical_ids(
        &self,
        ids: &[String],
    ) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
        self.manager.get_repo_names_for_canonical_ids(ids)
    }

    pub fn get_packages_by_canonical_ids(
        &self,
        ids: &[String],
    ) -> Result<Vec<crate::models::Package>, String> {
        self.manager.get_packages_by_canonical_ids(ids)
    }

    pub fn spawn_actor<R: tauri::Runtime>(&self, app: AppHandle<R>) {
        let mut rx_guard = self.update_rx.lock().unwrap();
        if let Some(mut rx) = rx_guard.take() {
            tokio::spawn(async move {
                let mut batch = std::collections::HashSet::new();
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));

                loop {
                    tokio::select! {
                        Some(update) = rx.recv() => {
                            match update {
                                RegistryUpdate::Package(id) => {
                                    batch.insert(id);
                                }
                                RegistryUpdate::Bulk => {
                                    let _ = Emitter::emit(&app, "registry-sync-bulk", ());
                                }
                            }
                        }
                        _ = interval.tick() => {
                            if !batch.is_empty() {
                                let ids: Vec<String> = batch.drain().collect();
                                let _ = Emitter::emit(&app, "registry-sync", ids);
                            }
                        }
                    }
                }
            });
        }
    }
}

pub enum RegistryUpdate {
    Package(String),
    Bulk,
}

pub struct RegistryManager {
    conn: Mutex<Connection>,
    update_tx: mpsc::Sender<RegistryUpdate>,
}

impl RegistryManager {
    pub fn new(update_tx: mpsc::Sender<RegistryUpdate>) -> Self {
        let path = Self::get_db_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(path).expect("Failed to open registry.db");
        // Enable WAL mode for better concurrency (readers don't block writers)
        let _ = conn.execute("PRAGMA journal_mode=WAL", []);
        // Optimization: Rely on OS file cache for speed while keeping crash safety (application vs system crash)
        let _ = conn.execute("PRAGMA synchronous=NORMAL", []);
        Self::init_with_conn(conn, update_tx)
    }

    /// Internal helper to initialize schema on any connection.
    fn init_with_conn(conn: Connection, update_tx: mpsc::Sender<RegistryUpdate>) -> Self {
        // Initialize schema
        // - canonical_id: the stable key (e.g. "discord", "com.discordapp.discord")
        // - metadata: joined rich data from AppStream/Flathub
        conn.execute(
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
                is_featured BOOLEAN,
                last_updated INTEGER,
                long_description TEXT,
                screenshots TEXT
            )",
            [],
        )
        .expect("Failed to create packages table");

        // Migrations: Add new columns if they don't exist
        let _ = conn.execute("ALTER TABLE packages ADD COLUMN long_description TEXT", []);
        let _ = conn.execute("ALTER TABLE packages ADD COLUMN screenshots TEXT", []);

        conn.execute(
            "CREATE TABLE IF NOT EXISTS package_sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                canonical_id TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                version TEXT,
                label TEXT,
                package_name TEXT,
                FOREIGN KEY(canonical_id) REFERENCES packages(canonical_id)
            )",
            [],
        )
        .expect("Failed to create package_sources table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS package_categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                canonical_id TEXT NOT NULL,
                category TEXT NOT NULL,
                FOREIGN KEY(canonical_id) REFERENCES packages(canonical_id)
            )",
            [],
        )
        .expect("Failed to create package_categories table");

        // Index for performance
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sources_canonical ON package_sources(canonical_id)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_packages_app_id ON packages(app_id)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_categories_name ON package_categories(category)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_categories_canonical ON package_categories(canonical_id)",
            [],
        );

        Self {
            conn: Mutex::new(conn),
            update_tx,
        }
    }

    pub fn trigger_bulk_sync(&self) {
        let _ = self.update_tx.try_send(RegistryUpdate::Bulk);
    }

    #[allow(dead_code)]
    pub fn in_memory() -> Self {
        let (tx, _rx) = mpsc::channel(100);
        let conn = Connection::open_in_memory().expect("Failed to open in-memory DB");
        Self::init_with_conn(conn, tx)
    }

    fn get_db_path() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("monarch-store")
            .join("registry.db")
    }

    /// Add or update multiple packages in chunked transactions.
    /// Uses prepared statements and Immediate transactions for maximum performance.
    pub fn bulk_upsert_packages(&self, packages: &[Package]) -> Result<(), String> {
        if packages.is_empty() {
            return Ok(());
        }

        let mut conn_guard = self.conn.lock().map_err(|e| e.to_string())?;
        let chunks = packages.chunks(500);

        for chunk in chunks {
            // Use Immediate behavior to prevent mid-transaction deadlocks
            let tx = conn_guard
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;

            {
                // Optimization: Prepare statements once per chunk
                let mut stmt_pkg = tx.prepare_cached(
                    "INSERT INTO packages (
                        canonical_id, name, display_name, description, version, 
                        app_id, icon, installed, last_modified, maintainer, 
                        license, url, is_featured, last_updated, long_description,
                        screenshots
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                    ON CONFLICT(canonical_id) DO UPDATE SET
                        display_name = CASE WHEN excluded.display_name IS NOT NULL AND excluded.display_name != '' THEN excluded.display_name ELSE display_name END,
                        description = CASE WHEN excluded.description != '' THEN excluded.description ELSE description END,
                        icon = CASE WHEN excluded.icon IS NOT NULL AND excluded.icon != '' THEN excluded.icon ELSE icon END,
                        long_description = CASE WHEN excluded.long_description IS NOT NULL AND excluded.long_description != '' THEN excluded.long_description ELSE long_description END,
                        screenshots = CASE WHEN excluded.screenshots IS NOT NULL AND excluded.screenshots != '' THEN excluded.screenshots ELSE screenshots END,
                        last_updated = excluded.last_updated",
                ).map_err(|e| e.to_string())?;

                let mut stmt_del_src = tx
                    .prepare_cached("DELETE FROM package_sources WHERE canonical_id = ?1")
                    .map_err(|e| e.to_string())?;

                let mut stmt_ins_src = tx.prepare_cached(
                    "INSERT INTO package_sources (canonical_id, source_type, source_id, version, label, package_name)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                ).map_err(|e| e.to_string())?;

                let now = chrono::Utc::now().timestamp();

                for pkg in chunk {
                    let canon_id = if pkg.canonical_id.is_empty() {
                        crate::utils::canonical_merge_key(&pkg.name, pkg.app_id.as_deref())
                    } else {
                        pkg.canonical_id.clone()
                    };

                    if canon_id.is_empty() {
                        continue;
                    }

                    stmt_pkg
                        .execute(params![
                            canon_id,
                            pkg.name,
                            pkg.display_name,
                            pkg.description,
                            pkg.version,
                            pkg.app_id,
                            pkg.icon,
                            pkg.installed,
                            pkg.last_modified,
                            pkg.maintainer,
                            pkg.license.as_ref().map(|l| l.join(",")),
                            pkg.url,
                            pkg.is_featured,
                            now,
                            pkg.long_description,
                            pkg.screenshots
                                .as_ref()
                                .and_then(|s| serde_json::to_string(s).ok())
                        ])
                        .map_err(|e| e.to_string())?;

                    if let Some(sources) = &pkg.available_sources {
                        stmt_del_src
                            .execute(params![canon_id])
                            .map_err(|e| e.to_string())?;
                        for src in sources {
                            stmt_ins_src
                                .execute(params![
                                    canon_id,
                                    src.source_type,
                                    src.id,
                                    src.version,
                                    src.label,
                                    src.package_name
                                ])
                                .map_err(|e| e.to_string())?;
                        }
                    }

                    // Notify actor of changed package - Only for small total batches to avoid bridge flood
                    if packages.len() <= 20 {
                        let _ = self.update_tx.try_send(RegistryUpdate::Package(canon_id));
                    }
                }
            }

            tx.commit().map_err(|e| e.to_string())?;
        }

        // If we did a large batch without individual notifications, trigger a bulk sync
        if packages.len() > 20 {
            let _ = self.update_tx.try_send(RegistryUpdate::Bulk);
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn upsert_package(&self, pkg: &Package) -> Result<(), String> {
        self.bulk_upsert_packages(&[pkg.clone()])
    }

    /// Specialized bulk sync for AppStream data which includes categories.
    /// Uses chunked transactions (Immediate) and prepared statements for speed.
    pub fn sync_appstream_entries(
        &self,
        entries: Vec<(Package, Vec<String>)>,
    ) -> Result<(), String> {
        let mut conn_guard = self.conn.lock().map_err(|e| e.to_string())?;
        let chunks = entries.chunks(500);

        for chunk in chunks {
            let tx = conn_guard
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;

            {
                let mut stmt_pkg = tx.prepare_cached(
                    "INSERT INTO packages (
                        canonical_id, name, display_name, description, version, 
                        app_id, icon, installed, last_modified, maintainer, 
                        license, url, is_featured, last_updated, long_description,
                        screenshots
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                    ON CONFLICT(canonical_id) DO UPDATE SET
                        display_name = CASE WHEN excluded.display_name IS NOT NULL AND excluded.display_name != '' THEN excluded.display_name ELSE display_name END,
                        description = CASE WHEN excluded.description != '' THEN excluded.description ELSE description END,
                        icon = CASE WHEN excluded.icon IS NOT NULL AND excluded.icon != '' THEN excluded.icon ELSE icon END,
                        long_description = CASE WHEN excluded.long_description IS NOT NULL AND excluded.long_description != '' THEN excluded.long_description ELSE long_description END,
                        screenshots = CASE WHEN excluded.screenshots IS NOT NULL AND excluded.screenshots != '' THEN excluded.screenshots ELSE screenshots END,
                        last_updated = excluded.last_updated",
                ).map_err(|e| e.to_string())?;

                let mut stmt_del_cat = tx
                    .prepare_cached("DELETE FROM package_categories WHERE canonical_id = ?1")
                    .map_err(|e| e.to_string())?;

                let mut stmt_ins_cat = tx
                    .prepare_cached(
                        "INSERT INTO package_categories (canonical_id, category) VALUES (?1, ?2)",
                    )
                    .map_err(|e| e.to_string())?;

                let now = chrono::Utc::now().timestamp();

                for (pkg, categories) in chunk {
                    let canon_id = if pkg.canonical_id.is_empty() {
                        crate::utils::canonical_merge_key(&pkg.name, pkg.app_id.as_deref())
                    } else {
                        pkg.canonical_id.clone()
                    };

                    if canon_id.is_empty() {
                        continue;
                    }

                    stmt_pkg
                        .execute(params![
                            canon_id,
                            pkg.name,
                            pkg.display_name,
                            pkg.description,
                            pkg.version,
                            pkg.app_id,
                            pkg.icon,
                            pkg.installed,
                            pkg.last_modified,
                            pkg.maintainer,
                            pkg.license.as_ref().map(|l| l.join(",")),
                            pkg.url,
                            pkg.is_featured,
                            now,
                            pkg.long_description,
                            pkg.screenshots
                                .as_ref()
                                .and_then(|s| serde_json::to_string(s).ok())
                        ])
                        .map_err(|e| e.to_string())?;

                    stmt_del_cat
                        .execute(params![canon_id])
                        .map_err(|e| e.to_string())?;
                    for cat in categories {
                        stmt_ins_cat
                            .execute(params![canon_id, cat.to_lowercase()])
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            tx.commit().map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Search for packages by category.
    pub fn get_packages_by_category(
        &self,
        category: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Package>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let cat_lower = category.to_lowercase();
        let query_cats = match cat_lower.as_str() {
            "game" | "games" => vec![
                "game",
                "games",
                "kidsgame",
                "logicgame",
                "strategygame",
                "actiongame",
                "adventuregame",
                "arcadegame",
                "boardgame",
                "cardgame",
                "emulation",
                "roleplaying",
                "simulation",
                "sportsgame",
            ],
            "office" | "productivity" => vec![
                "office",
                "productivity",
                "wordprocessor",
                "spreadsheet",
                "presentation",
                "projectmanagement",
            ],
            "audiovideo" | "multimedia" | "audio" | "video" => vec![
                "audiovideo",
                "audiovideofinished",
                "multimedia",
                "player",
                "recorder",
                "sequencer",
                "mixer",
                "midi",
                "video",
                "audio",
            ],
            "graphics" => vec![
                "graphics",
                "rastergraphics",
                "vectorgraphics",
                "photography",
                "scanning",
                "viewer",
            ],
            "network" | "internet" => vec![
                "network",
                "internet",
                "webbrowser",
                "email",
                "chat",
                "instantmessaging",
                "telephony",
                "ircclient",
                "news",
                "p2p",
            ],
            "development" | "develop" => vec![
                "development",
                "develop",
                "ide",
                "debugger",
                "revisioncontrol",
                "webdevelopment",
                "guidesigner",
            ],
            "system" => vec![
                "system",
                "terminalemulator",
                "filemanager",
                "monitor",
                "security",
                "settings",
                "packagemanager",
            ],
            "utility" | "utilities" => vec![
                "utility",
                "utilities",
                "texteditor",
                "texttools",
                "viewer",
                "handwriting",
                "maps",
            ],
            _ => vec![cat_lower.as_str()],
        };

        // Create placeholders for IN clause
        let placeholders = query_cats.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT p.canonical_id, p.name, p.display_name, p.description, p.version, 
                    p.app_id, p.icon, p.installed, p.last_modified, p.maintainer, 
                    p.license, p.url, p.is_featured, p.long_description, p.screenshots
             FROM packages p
             JOIN package_categories pc ON p.canonical_id = pc.canonical_id
             WHERE pc.category IN ({})
             GROUP BY p.canonical_id
             LIMIT ?{} OFFSET ?{}",
            placeholders,
            query_cats.len() + 1,
            query_cats.len() + 2
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

        // Construct parameters
        let mut params_vec: Vec<rusqlite::types::Value> = query_cats
            .iter()
            .map(|s| rusqlite::types::Value::Text(s.to_string()))
            .collect();
        params_vec.push(rusqlite::types::Value::Integer(limit as i64));
        params_vec.push(rusqlite::types::Value::Integer(offset as i64));

        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_vec), |row| {
                let license_str: Option<String> = row.get(10)?;
                let license = license_str.map(|s| s.split(',').map(|s| s.to_string()).collect());

                Ok(Package {
                    canonical_id: row.get(0)?,
                    name: row.get(1)?,
                    display_name: row.get(2)?,
                    description: row.get(3)?,
                    version: row.get(4)?,
                    app_id: row.get(5)?,
                    icon: row.get(6)?,
                    installed: row.get(7)?,
                    last_modified: row.get(8)?,
                    maintainer: row.get(9)?,
                    license,
                    url: row.get(11)?,
                    is_featured: row.get(12)?,
                    long_description: row.get(13)?,
                    screenshots: row
                        .get::<_, Option<String>>(14)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    ..Default::default()
                })
            })
            .map_err(|e| e.to_string())?;

        let mut pkgs = Vec::new();
        for pkg_res in rows {
            pkgs.push(pkg_res.map_err(|e| e.to_string())?);
        }
        Ok(pkgs)
    }

    /// Retrieve a finalized package by its canonical ID.
    /// Returns a map of Canonical ID -> [Package Names] for ALPM lookup.
    /// E.g. "telegram" -> ["telegram-desktop"]
    pub fn get_repo_names_for_canonical_ids(
        &self,
        canonical_ids: &[String],
    ) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT canonical_id, name FROM packages WHERE canonical_id = ?1")
            .map_err(|e| e.to_string())?;

        let mut map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        // Optimized: We could use "WHERE canonical_id IN (...)" but for simplicity/safety with params
        // we'll loop prepared statement. Since < 100 items usually, it's fast.
        for id in canonical_ids {
            let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
            while let Ok(Some(row)) = rows.next() {
                let c_id: String = row.get(0).unwrap_or_default();
                let name: String = row.get(1).unwrap_or_default();
                if !name.is_empty() {
                    map.entry(c_id).or_default().push(name);
                }
            }
        }
        Ok(map)
    }

    pub fn get_package(&self, canonical_id: &str) -> Result<Option<Package>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT canonical_id, name, display_name, description, version, 
                    app_id, icon, installed, last_modified, maintainer, 
                    license, url, is_featured, long_description, screenshots
             FROM packages WHERE canonical_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let pkg_res = stmt
            .query_row(params![canonical_id], |row| {
                let license_str: Option<String> = row.get(10)?;
                let license = license_str.map(|s| s.split(',').map(|s| s.to_string()).collect());

                Ok(Package {
                    canonical_id: row.get(0)?,
                    name: row.get(1)?,
                    display_name: row.get(2)?,
                    description: row.get(3)?,
                    version: row.get(4)?,
                    app_id: row.get(5)?,
                    icon: row.get(6)?,
                    installed: row.get(7)?,
                    last_modified: row.get(8)?,
                    maintainer: row.get(9)?,
                    license,
                    url: row.get(11)?,
                    is_featured: row.get(12)?,
                    long_description: row.get(13)?,
                    screenshots: row
                        .get::<_, Option<String>>(14)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    ..Default::default()
                })
            })
            .optional()
            .map_err(|e| e.to_string())?;

        if let Some(mut pkg) = pkg_res {
            // Load sources
            let mut stmt_src = conn
                .prepare(
                    "SELECT source_type, source_id, version, label, package_name 
                 FROM package_sources WHERE canonical_id = ?1",
                )
                .map_err(|e| e.to_string())?;

            let sources = stmt_src
                .query_map(params![canonical_id], |row| {
                    Ok(PackageSource {
                        source_type: row.get(0)?,
                        id: row.get(1)?,
                        version: row.get(2)?,
                        label: row.get(3)?,
                        package_name: row.get(4)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            pkg.available_sources = Some(sources);
            Ok(Some(pkg))
        } else {
            Ok(None)
        }
    }

    /// Bulk retrieve packages by canonical ID (Iron Core SSOT).
    pub fn get_packages_by_canonical_ids(
        &self,
        canonical_ids: &[String],
    ) -> Result<Vec<Package>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT canonical_id, name, display_name, description, version, 
                    app_id, icon, installed, last_modified, maintainer, 
                    license, url, is_featured, long_description, screenshots
             FROM packages WHERE canonical_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let mut stmt_src = conn
            .prepare(
                "SELECT source_type, source_id, version, label, package_name 
                 FROM package_sources WHERE canonical_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let mut packages = Vec::new();

        for id in canonical_ids {
            let pkg_res = stmt
                .query_row(params![id], |row| {
                    let license_str: Option<String> = row.get(10)?;
                    let license =
                        license_str.map(|s| s.split(',').map(|s| s.to_string()).collect());

                    Ok(Package {
                        canonical_id: row.get(0)?,
                        name: row.get(1)?,
                        display_name: row.get(2)?,
                        description: row.get(3)?,
                        version: row.get(4)?,
                        app_id: row.get(5)?,
                        icon: row.get(6)?,
                        installed: row.get(7)?,
                        last_modified: row.get(8)?,
                        maintainer: row.get(9)?,
                        license,
                        url: row.get(11)?,
                        is_featured: row.get(12)?,
                        long_description: row.get(13)?,
                        screenshots: row
                            .get::<_, Option<String>>(14)?
                            .and_then(|s| serde_json::from_str(&s).ok()),
                        ..Default::default()
                    })
                })
                .optional()
                .map_err(|e| e.to_string())?;

            if let Some(mut pkg) = pkg_res {
                let sources = stmt_src
                    .query_map(params![id], |row| {
                        Ok(PackageSource {
                            source_type: row.get(0)?,
                            id: row.get(1)?,
                            version: row.get(2)?,
                            label: row.get(3)?,
                            package_name: row.get(4)?,
                        })
                    })
                    .map_err(|e| e.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;

                pkg.available_sources = Some(sources);
                packages.push(pkg);
            }
        }

        Ok(packages)
    }

    /// Optimized SQL Search for "Iron Core" speed.
    /// Performs LIKE query on name/description/app_id inside SQLite.
    pub fn search_packages_sql(&self, query: &str, limit: usize) -> Result<Vec<Package>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let pattern = format!("%{}%", query);

        let mut stmt = conn
            .prepare(
                "SELECT canonical_id, name, display_name, description, version, 
                    app_id, icon, installed, last_modified, maintainer, 
                    license, url, is_featured, long_description, screenshots
             FROM packages 
             WHERE name LIKE ?1 OR display_name LIKE ?1 OR description LIKE ?1 OR app_id LIKE ?1
             ORDER BY 
                CASE WHEN name LIKE ?2 THEN 0 ELSE 1 END, 
                CASE WHEN is_featured = 1 THEN 0 ELSE 1 END,
                name ASC
             LIMIT ?3",
            )
            .map_err(|e| e.to_string())?;

        // Params: 1=pattern, 2=exact_match_check (no wildcards), 3=limit
        // Actually for exact match check in ORDER BY we need exact string.
        let rows = stmt
            .query_map(params![pattern, query, limit], |row| {
                let license_str: Option<String> = row.get(10)?;
                let license = license_str.map(|s| s.split(',').map(|s| s.to_string()).collect());

                Ok(Package {
                    canonical_id: row.get(0)?,
                    name: row.get(1)?,
                    display_name: row.get(2)?,
                    description: row.get(3)?,
                    version: row.get(4)?,
                    app_id: row.get(5)?,
                    icon: row.get(6)?,
                    installed: row.get(7)?,
                    last_modified: row.get(8)?,
                    maintainer: row.get(9)?,
                    license,
                    url: row.get(11)?,
                    is_featured: row.get(12)?,
                    long_description: row.get(13)?,
                    screenshots: row
                        .get::<_, Option<String>>(14)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    ..Default::default()
                })
            })
            .map_err(|e| e.to_string())?;

        let mut pkgs = Vec::new();
        for pkg_res in rows {
            let mut pkg = pkg_res.map_err(|e| e.to_string())?;

            // N+1 query for sources (acceptable for limit=50)
            let mut stmt_src = conn
                .prepare(
                    "SELECT source_type, source_id, version, label, package_name 
                 FROM package_sources WHERE canonical_id = ?1",
                )
                .map_err(|e| e.to_string())?;

            let sources = stmt_src
                .query_map(params![pkg.canonical_id], |row| {
                    Ok(PackageSource {
                        source_type: row.get(0)?,
                        id: row.get(1)?,
                        version: row.get(2)?,
                        label: row.get(3)?,
                        package_name: row.get(4)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            pkg.available_sources = Some(sources);
            pkgs.push(pkg);
        }
        Ok(pkgs)
    }
}

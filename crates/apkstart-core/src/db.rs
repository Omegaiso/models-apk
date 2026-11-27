use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::model::{App, WindowState};
use crate::{Error, Result};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS apps (
    package           TEXT PRIMARY KEY,
    label             TEXT NOT NULL,
    version_name      TEXT,
    version_code      INTEGER NOT NULL DEFAULT 0,
    activity          TEXT,
    icon_path         TEXT,
    apk_path          TEXT,
    min_sdk           INTEGER,
    target_sdk        INTEGER,
    abis              TEXT NOT NULL DEFAULT '[]',
    categories        TEXT NOT NULL DEFAULT '[]',
    orientation       TEXT,
    installed_at      INTEGER NOT NULL,
    last_launched_at  INTEGER,
    enabled           INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS window_state (
    package           TEXT PRIMARY KEY,
    width             INTEGER NOT NULL,
    height            INTEGER NOT NULL,
    maximized         INTEGER NOT NULL DEFAULT 0,
    fullscreen        INTEGER NOT NULL DEFAULT 0,
    output            TEXT,
    scale             REAL NOT NULL DEFAULT 1.0,
    updated_at        INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key               TEXT PRIMARY KEY,
    value             TEXT NOT NULL
);

PRAGMA user_version = 2;
"#;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::from_connection(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::from_connection(conn)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;",
        )?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => {
                self.conn.execute_batch(SCHEMA)?;
                Ok(())
            }
            // v1 -> v2: categorías freedesktop por app. `ALTER TABLE` añade la
            // columna con valor por defecto y no toca las filas existentes.
            1 => {
                self.conn.execute_batch(
                    "ALTER TABLE apps ADD COLUMN categories TEXT NOT NULL DEFAULT '[]';
                     PRAGMA user_version = 2;",
                )?;
                Ok(())
            }
            2 => Ok(()),
            other => Err(Error::InvalidState(format!(
                "versión de base de datos desconocida: {other}"
            ))),
        }
    }

    pub fn upsert_app(&self, app: &App) -> Result<()> {
        let abis = serde_json::to_string(&app.abis).unwrap_or_else(|_| "[]".to_string());
        let categories =
            serde_json::to_string(&app.categories).unwrap_or_else(|_| "[]".to_string());
        self.conn.execute(
            "INSERT INTO apps (
                package, label, version_name, version_code, activity, icon_path, apk_path,
                min_sdk, target_sdk, abis, categories, orientation, installed_at, last_launched_at, enabled
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(package) DO UPDATE SET
                label = excluded.label,
                version_name = excluded.version_name,
                version_code = excluded.version_code,
                activity = excluded.activity,
                icon_path = excluded.icon_path,
                apk_path = excluded.apk_path,
                min_sdk = excluded.min_sdk,
                target_sdk = excluded.target_sdk,
                abis = excluded.abis,
                categories = excluded.categories,
                orientation = excluded.orientation,
                installed_at = excluded.installed_at,
                last_launched_at = excluded.last_launched_at,
                enabled = excluded.enabled",
            params![
                app.package,
                app.label,
                app.version_name,
                app.version_code,
                app.activity,
                path_to_string(app.icon_path.as_deref()),
                path_to_string(app.apk_path.as_deref()),
                app.min_sdk,
                app.target_sdk,
                abis,
                categories,
                app.orientation,
                app.installed_at,
                app.last_launched_at,
                if app.enabled { 1 } else { 0 },
            ],
        )?;
        Ok(())
    }

    pub fn get_app(&self, package: &str) -> Result<Option<App>> {
        let mut statement = self.conn.prepare(
            "SELECT package, label, version_name, version_code, activity, icon_path, apk_path,
                    min_sdk, target_sdk, abis, categories, orientation, installed_at, last_launched_at, enabled
             FROM apps WHERE package = ?1",
        )?;
        let app = statement.query_row([package], row_to_app).optional()?;
        Ok(app)
    }

    pub fn list_apps(&self) -> Result<Vec<App>> {
        let mut statement = self.conn.prepare(
            "SELECT package, label, version_name, version_code, activity, icon_path, apk_path,
                    min_sdk, target_sdk, abis, categories, orientation, installed_at, last_launched_at, enabled
             FROM apps ORDER BY label COLLATE NOCASE ASC",
        )?;
        let apps = statement
            .query_map([], row_to_app)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(apps)
    }

    pub fn delete_app(&self, package: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM apps WHERE package = ?1", [package])?;
        self.conn
            .execute("DELETE FROM window_state WHERE package = ?1", [package])?;
        Ok(())
    }

    pub fn touch_launched(&self, package: &str, timestamp: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE apps SET last_launched_at = ?2 WHERE package = ?1",
            params![package, timestamp],
        )?;
        Ok(())
    }

    pub fn set_window_state(&self, state: &WindowState) -> Result<()> {
        self.conn.execute(
            "INSERT INTO window_state
                (package, width, height, maximized, fullscreen, output, scale, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(package) DO UPDATE SET
                width = excluded.width,
                height = excluded.height,
                maximized = excluded.maximized,
                fullscreen = excluded.fullscreen,
                output = excluded.output,
                scale = excluded.scale,
                updated_at = excluded.updated_at",
            params![
                state.package,
                state.width,
                state.height,
                if state.maximized { 1 } else { 0 },
                if state.fullscreen { 1 } else { 0 },
                state.output,
                state.scale,
                state.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_window_state(&self, package: &str) -> Result<Option<WindowState>> {
        let mut statement = self.conn.prepare(
            "SELECT package, width, height, maximized, fullscreen, output, scale, updated_at
             FROM window_state WHERE package = ?1",
        )?;
        let state = statement
            .query_row([package], |row| {
                Ok(WindowState {
                    package: row.get(0)?,
                    width: row.get(1)?,
                    height: row.get(2)?,
                    maximized: row.get::<_, i64>(3)? != 0,
                    fullscreen: row.get::<_, i64>(4)? != 0,
                    output: row.get(5)?,
                    scale: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .optional()?;
        Ok(state)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let value = self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(value)
    }
}

fn path_to_string(path: Option<&Path>) -> Option<String> {
    path.map(|path| path.to_string_lossy().into_owned())
}

fn row_to_app(row: &rusqlite::Row<'_>) -> rusqlite::Result<App> {
    let abis: String = row.get(9)?;
    let categories: String = row.get(10)?;
    Ok(App {
        package: row.get(0)?,
        label: row.get(1)?,
        version_name: row.get(2)?,
        version_code: row.get(3)?,
        activity: row.get(4)?,
        icon_path: row.get::<_, Option<String>>(5)?.map(PathBuf::from),
        apk_path: row.get::<_, Option<String>>(6)?.map(PathBuf::from),
        min_sdk: row.get(7)?,
        target_sdk: row.get(8)?,
        abis: serde_json::from_str(&abis).unwrap_or_default(),
        categories: serde_json::from_str(&categories).unwrap_or_default(),
        orientation: row.get(11)?,
        installed_at: row.get(12)?,
        last_launched_at: row.get(13)?,
        enabled: row.get::<_, i64>(14)? != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{WindowState, now_unix};

    fn sample_app() -> App {
        App {
            package: "org.example.app".to_string(),
            label: "Example".to_string(),
            version_name: Some("1.2.3".to_string()),
            version_code: 42,
            activity: Some("org.example.app.MainActivity".to_string()),
            icon_path: None,
            apk_path: Some(PathBuf::from("/tmp/example.apk")),
            min_sdk: Some(34),
            target_sdk: Some(35),
            abis: vec!["x86_64".to_string(), "arm64-v8a".to_string()],
            categories: vec!["Network".to_string(), "WebBrowser".to_string()],
            orientation: None,
            installed_at: now_unix(),
            last_launched_at: None,
            enabled: true,
        }
    }

    #[test]
    fn upsert_and_read_app() {
        let db = Db::open_in_memory().unwrap();
        let app = sample_app();
        db.upsert_app(&app).unwrap();
        let stored = db.get_app(&app.package).unwrap().unwrap();
        assert_eq!(stored, app);
        assert_eq!(db.list_apps().unwrap(), vec![app]);
    }

    #[test]
    fn upsert_updates_existing_app() {
        let db = Db::open_in_memory().unwrap();
        let mut app = sample_app();
        db.upsert_app(&app).unwrap();
        app.label = "Example actualizada".to_string();
        app.version_code = 43;
        db.upsert_app(&app).unwrap();
        let stored = db.get_app(&app.package).unwrap().unwrap();
        assert_eq!(stored.label, "Example actualizada");
        assert_eq!(stored.version_code, 43);
        assert_eq!(db.list_apps().unwrap().len(), 1);
    }

    #[test]
    fn migrates_v1_database_to_v2_without_losing_data() {
        // Esquema v1 (sin `categories`) con una fila ya registrada.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE apps (
                package           TEXT PRIMARY KEY,
                label             TEXT NOT NULL,
                version_name      TEXT,
                version_code      INTEGER NOT NULL DEFAULT 0,
                activity          TEXT,
                icon_path         TEXT,
                apk_path          TEXT,
                min_sdk           INTEGER,
                target_sdk        INTEGER,
                abis              TEXT NOT NULL DEFAULT '[]',
                orientation       TEXT,
                installed_at      INTEGER NOT NULL,
                last_launched_at  INTEGER,
                enabled           INTEGER NOT NULL DEFAULT 1
            );
            INSERT INTO apps (package, label, installed_at)
            VALUES ('org.example.app', 'Example', 12345);
            PRAGMA user_version = 1;",
        )
        .unwrap();

        let db = Db::from_connection(conn).unwrap();
        let app = db.get_app("org.example.app").unwrap().unwrap();
        assert_eq!(app.label, "Example");
        assert!(app.categories.is_empty());

        let version: i64 = db
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 2);

        // La columna migrada acepta escrituras y las conserva.
        let mut updated = app;
        updated.categories = vec!["Game".to_string()];
        db.upsert_app(&updated).unwrap();
        assert_eq!(
            db.get_app("org.example.app").unwrap().unwrap().categories,
            vec!["Game".to_string()]
        );
    }

    #[test]
    fn window_state_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_app(&sample_app()).unwrap();
        let mut state = WindowState::new("org.example.app", 900, 1600);
        state.maximized = true;
        state.scale = 1.5;
        db.set_window_state(&state).unwrap();
        let stored = db.get_window_state("org.example.app").unwrap().unwrap();
        assert_eq!(stored.width, 900);
        assert_eq!(stored.height, 1600);
        assert!(stored.maximized);
        assert_eq!(stored.scale, 1.5);
    }

    #[test]
    fn settings_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.get_setting("proxy").unwrap(), None);
        db.set_setting("proxy", "http://127.0.0.1:8080").unwrap();
        assert_eq!(
            db.get_setting("proxy").unwrap().as_deref(),
            Some("http://127.0.0.1:8080")
        );
    }

    #[test]
    fn delete_app_removes_window_state() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_app(&sample_app()).unwrap();
        db.set_window_state(&WindowState::new("org.example.app", 800, 600))
            .unwrap();
        db.delete_app("org.example.app").unwrap();
        assert!(db.get_app("org.example.app").unwrap().is_none());
        assert!(db.get_window_state("org.example.app").unwrap().is_none());
    }
}

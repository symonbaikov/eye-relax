use std::path::Path;

use rusqlite::{params, Connection};

use crate::config::AppConfig;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BreakRecord {
    pub id: String,
    pub break_type: String, // "short" | "long"
    pub status: String,     // "completed" | "skipped" | "snoozed"
    pub started_at: String, // ISO 8601
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub date: String, // YYYY-MM-DD
    pub work_seconds: u64,
    pub break_count: u32,
    pub skip_count: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DayStat {
    pub date: String,
    pub work_seconds: u64,
    pub break_count: u32,
    pub skip_count: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DateRange {
    pub start: String, // YYYY-MM-DD
    pub end: String,   // YYYY-MM-DD
}

// ---------------------------------------------------------------------------
// Port (trait)
// ---------------------------------------------------------------------------

/// Persistent storage abstraction. All implementations must be `Send + Sync`.
pub trait StoragePort: Send + Sync {
    fn load_config(&self) -> Result<AppConfig>;
    fn save_config(&self, config: &AppConfig) -> Result<()>;

    fn record_break(&self, record: &BreakRecord) -> Result<()>;
    fn upsert_session(&self, session: &Session) -> Result<()>;
    fn get_today_session(&self, date: &str) -> Result<Option<Session>>;
    fn get_stats(&self, range: &DateRange) -> Result<Vec<DayStat>>;
}

// ---------------------------------------------------------------------------
// SQLite implementation
// ---------------------------------------------------------------------------

pub struct SqliteStorage {
    conn: std::sync::Mutex<Connection>,
}

impl SqliteStorage {
    /// Open (or create) the database at `path` and run pending migrations.
    pub fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| StorageError::Io(e.to_string()))?;
        }

        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let storage = SqliteStorage {
            conn: std::sync::Mutex::new(conn),
        };
        storage.migrate()?;
        Ok(storage)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let version: i32 =
            conn.pragma_query_value(None, "user_version", |r| r.get(0))?;

        if version < 1 {
            migrate_v0_to_v1(&conn)?;
        }
        if version < 2 {
            migrate_v1_to_v2(&conn)?;
        }

        Ok(())
    }
}

fn migrate_v0_to_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "BEGIN;
         CREATE TABLE IF NOT EXISTS config (
             key   TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );
         PRAGMA user_version = 1;
         COMMIT;",
    )?;
    tracing::info!("Storage migrated to v1");
    Ok(())
}

fn migrate_v1_to_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "BEGIN;
         CREATE TABLE IF NOT EXISTS breaks (
             id         TEXT PRIMARY KEY,
             type       TEXT NOT NULL,
             status     TEXT NOT NULL,
             started_at TEXT NOT NULL,
             ended_at   TEXT
         );
         CREATE TABLE IF NOT EXISTS sessions (
             id           TEXT PRIMARY KEY,
             date         TEXT NOT NULL UNIQUE,
             work_seconds INTEGER NOT NULL DEFAULT 0,
             break_count  INTEGER NOT NULL DEFAULT 0,
             skip_count   INTEGER NOT NULL DEFAULT 0
         );
         PRAGMA user_version = 2;
         COMMIT;",
    )?;
    tracing::info!("Storage migrated to v2");
    Ok(())
}

impl StoragePort for SqliteStorage {
    fn load_config(&self) -> Result<AppConfig> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT value FROM config WHERE key = 'app_config'",
            [],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(json) => Ok(serde_json::from_str(&json)?),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                tracing::info!("No config found in DB, using defaults");
                Ok(AppConfig::default())
            }
            Err(e) => Err(e.into()),
        }
    }

    fn save_config(&self, config: &AppConfig) -> Result<()> {
        let json = serde_json::to_string(config)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES ('app_config', ?1)",
            params![json],
        )?;
        Ok(())
    }

    fn record_break(&self, record: &BreakRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO breaks (id, type, status, started_at, ended_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                record.id,
                record.break_type,
                record.status,
                record.started_at,
                record.ended_at,
            ],
        )?;
        Ok(())
    }

    fn upsert_session(&self, session: &Session) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO sessions (id, date, work_seconds, break_count, skip_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session.id,
                session.date,
                session.work_seconds,
                session.break_count,
                session.skip_count,
            ],
        )?;
        Ok(())
    }

    fn get_today_session(&self, date: &str) -> Result<Option<Session>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, date, work_seconds, break_count, skip_count
             FROM sessions WHERE date = ?1",
            params![date],
            |row| {
                Ok(Session {
                    id: row.get(0)?,
                    date: row.get(1)?,
                    work_seconds: row.get::<_, i64>(2)? as u64,
                    break_count: row.get::<_, i64>(3)? as u32,
                    skip_count: row.get::<_, i64>(4)? as u32,
                })
            },
        );
        match result {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_stats(&self, range: &DateRange) -> Result<Vec<DayStat>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT date, work_seconds, break_count, skip_count
             FROM sessions WHERE date >= ?1 AND date <= ?2
             ORDER BY date",
        )?;
        let rows = stmt.query_map(params![range.start, range.end], |row| {
            Ok(DayStat {
                date: row.get(0)?,
                work_seconds: row.get::<_, i64>(1)? as u64,
                break_count: row.get::<_, i64>(2)? as u32,
                skip_count: row.get::<_, i64>(3)? as u32,
            })
        })?;
        rows.map(|r| r.map_err(StorageError::from)).collect()
    }
}

// ---------------------------------------------------------------------------
// Mock (for tests)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub mod mock {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::{BreakRecord, DateRange, DayStat, Result, Session, StoragePort};
    use crate::config::AppConfig;

    pub struct MockStorage {
        pub config: Mutex<AppConfig>,
        pub save_count: Mutex<u32>,
        pub sessions: Mutex<HashMap<String, Session>>,    // id → Session
        pub breaks: Mutex<HashMap<String, BreakRecord>>,   // id → BreakRecord
    }

    impl MockStorage {
        pub fn new() -> Self {
            MockStorage {
                config: Mutex::new(AppConfig::default()),
                save_count: Mutex::new(0),
                sessions: Mutex::new(HashMap::new()),
                breaks: Mutex::new(HashMap::new()),
            }
        }
    }

    impl StoragePort for MockStorage {
        fn load_config(&self) -> Result<AppConfig> {
            Ok(self.config.lock().unwrap().clone())
        }

        fn save_config(&self, config: &AppConfig) -> Result<()> {
            *self.config.lock().unwrap() = config.clone();
            *self.save_count.lock().unwrap() += 1;
            Ok(())
        }

        fn record_break(&self, record: &BreakRecord) -> Result<()> {
            self.breaks
                .lock()
                .unwrap()
                .insert(record.id.clone(), record.clone());
            Ok(())
        }

        fn upsert_session(&self, session: &Session) -> Result<()> {
            self.sessions
                .lock()
                .unwrap()
                .insert(session.id.clone(), session.clone());
            Ok(())
        }

        fn get_today_session(&self, date: &str) -> Result<Option<Session>> {
            let s = self
                .sessions
                .lock()
                .unwrap()
                .values()
                .find(|s| s.date == date)
                .cloned();
            Ok(s)
        }

        fn get_stats(&self, range: &DateRange) -> Result<Vec<DayStat>> {
            let mut stats: Vec<DayStat> = self
                .sessions
                .lock()
                .unwrap()
                .values()
                .filter(|s| s.date >= range.start && s.date <= range.end)
                .map(|s| DayStat {
                    date: s.date.clone(),
                    work_seconds: s.work_seconds,
                    break_count: s.break_count,
                    skip_count: s.skip_count,
                })
                .collect();
            stats.sort_by(|a, b| a.date.cmp(&b.date));
            Ok(stats)
        }
    }
}

// ---------------------------------------------------------------------------
// Integration tests (SQLite round-trip)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn storage_config_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");

        let storage = SqliteStorage::new(&db).unwrap();

        let mut cfg = AppConfig::default();
        cfg.work_interval_secs = 600;
        storage.save_config(&cfg).unwrap();

        // Re-open the same file to simulate restart
        drop(storage);
        let storage2 = SqliteStorage::new(&db).unwrap();
        let loaded = storage2.load_config().unwrap();

        assert_eq!(loaded.work_interval_secs, 600);
    }

    #[test]
    fn storage_save_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let storage = SqliteStorage::new(&db).unwrap();

        let cfg = AppConfig::default();
        storage.save_config(&cfg).unwrap();
        storage.save_config(&cfg).unwrap(); // second call must not fail or duplicate

        let loaded = storage.load_config().unwrap();
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn storage_missing_config_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("fresh.db");
        let storage = SqliteStorage::new(&db).unwrap();

        let loaded = storage.load_config().unwrap();
        assert_eq!(loaded, AppConfig::default());
    }

    #[test]
    fn storage_upsert_session_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let storage = SqliteStorage::new(&db).unwrap();

        let session = Session {
            id: "test-uuid-1".to_string(),
            date: "2025-01-01".to_string(),
            work_seconds: 600,
            break_count: 3,
            skip_count: 1,
        };

        storage.upsert_session(&session).unwrap();
        storage.upsert_session(&session).unwrap(); // second call must not duplicate

        let stats = storage
            .get_stats(&DateRange {
                start: "2025-01-01".to_string(),
                end: "2025-01-01".to_string(),
            })
            .unwrap();

        assert_eq!(stats.len(), 1, "two upserts with same ID should yield 1 row");
        assert_eq!(stats[0].break_count, 3);
    }

    #[test]
    fn storage_get_stats_empty_range_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let storage = SqliteStorage::new(&db).unwrap();

        let stats = storage
            .get_stats(&DateRange {
                start: "2020-01-01".to_string(),
                end: "2020-01-07".to_string(),
            })
            .unwrap();

        assert!(stats.is_empty(), "empty range should return empty vec, not error");
    }
}

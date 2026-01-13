//! SQLite database manager
//!
//! Thread-safe SQLite connection wrapper with initialization and utility methods.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

use super::models::DatabaseStats;

/// Thread-safe SQLite database connection wrapper
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open or create a database at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create database directory")?;
        }

        let conn = Connection::open(path).context("Failed to open SQLite database")?;

        // Configure connection for performance
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 5000;
            PRAGMA cache_size = -64000;
            PRAGMA temp_store = MEMORY;
            "#,
        )
        .context("Failed to configure database pragmas")?;

        info!("Opened database at {:?}", path);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Initialize the database schema
    pub fn initialize(&self) -> Result<()> {
        let conn = self.conn.lock();

        // Include schema from file at compile time
        const SCHEMA: &str = include_str!("schema.sql");

        conn.execute_batch(SCHEMA)
            .context("Failed to initialize database schema")?;

        info!("Database schema initialized");
        Ok(())
    }

    /// Execute a read operation with the connection
    pub fn with_conn<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock();
        f(&conn)
    }

    /// Execute a write operation with the connection
    pub fn with_conn_mut<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> Result<T>,
    {
        let mut conn = self.conn.lock();
        f(&mut conn)
    }

    /// Get database statistics
    pub fn stats(&self) -> Result<DatabaseStats> {
        self.with_conn(|conn| {
            let meeting_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM meetings", [], |row| row.get(0))?;

            let segment_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM transcript_segments",
                [],
                |row| row.get(0),
            )?;

            let total_duration_ms: i64 = conn.query_row(
                "SELECT COALESCE(SUM(duration_ms), 0) FROM meetings WHERE duration_ms IS NOT NULL",
                [],
                |row| row.get(0),
            )?;

            let note_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;

            debug!(
                "Database stats: {} meetings, {} segments, {} notes",
                meeting_count, segment_count, note_count
            );

            Ok(DatabaseStats {
                meeting_count: meeting_count as u64,
                segment_count: segment_count as u64,
                total_duration_ms: total_duration_ms as u64,
                note_count: note_count as u64,
            })
        })
    }

    /// Vacuum the database to reclaim space
    pub fn vacuum(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch("VACUUM")?;
            info!("Database vacuumed");
            Ok(())
        })
    }

    /// Get the database file size in bytes
    pub fn file_size(&self) -> Result<u64> {
        self.with_conn(|conn| {
            let page_count: i64 =
                conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
            let page_size: i64 =
                conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;

            Ok((page_count * page_size) as u64)
        })
    }

    /// Check database integrity
    pub fn check_integrity(&self) -> Result<bool> {
        self.with_conn(|conn| {
            let result: String =
                conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;

            let ok = result == "ok";
            if !ok {
                tracing::warn!("Database integrity check failed: {}", result);
            }

            Ok(ok)
        })
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_open_and_initialize() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");

        let db = Database::open(&db_path).unwrap();
        db.initialize().unwrap();

        // Verify tables exist
        db.with_conn(|conn| {
            let tables: Vec<String> = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table'")?
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;

            assert!(tables.contains(&"meetings".to_string()));
            assert!(tables.contains(&"transcript_segments".to_string()));
            assert!(tables.contains(&"notes".to_string()));
            assert!(tables.contains(&"summaries".to_string()));
            assert!(tables.contains(&"settings".to_string()));

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_database_stats() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");

        let db = Database::open(&db_path).unwrap();
        db.initialize().unwrap();

        let stats = db.stats().unwrap();

        assert_eq!(stats.meeting_count, 0);
        assert_eq!(stats.segment_count, 0);
        assert_eq!(stats.note_count, 0);
    }

    #[test]
    fn test_database_clone() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");

        let db1 = Database::open(&db_path).unwrap();
        db1.initialize().unwrap();

        let db2 = db1.clone();

        // Both should work
        let stats1 = db1.stats().unwrap();
        let stats2 = db2.stats().unwrap();

        assert_eq!(stats1.meeting_count, stats2.meeting_count);
    }

    #[test]
    fn test_database_integrity() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");

        let db = Database::open(&db_path).unwrap();
        db.initialize().unwrap();

        assert!(db.check_integrity().unwrap());
    }

    #[test]
    fn test_fts_table_created() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");

        let db = Database::open(&db_path).unwrap();
        db.initialize().unwrap();

        db.with_conn(|conn| {
            // FTS5 virtual tables appear in sqlite_master
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='transcript_fts'",
                [],
                |row| row.get(0),
            )?;

            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
    }
}

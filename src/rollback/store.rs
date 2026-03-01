//! SQLite-based snapshot metadata storage.
//!
//! Provides persistent storage for snapshot metadata using SQLite.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, params};

use super::snapshot::{ConfigSnapshot, SnapshotFilter};

pub struct SnapshotStore {
    conn: Mutex<Connection>,
}

impl SnapshotStore {
    pub fn new(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open snapshot database: {:?}", path))?;

        let store = Self { conn: Mutex::new(conn) };
        store.initialize()?;
        Ok(store)
    }

    fn initialize(&self) -> Result<()> {
        self.conn.lock().execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS snapshots (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                label TEXT,
                description TEXT,
                config_files TEXT NOT NULL,
                checksum TEXT NOT NULL,
                size_bytes INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_snapshots_timestamp ON snapshots(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_snapshots_label ON snapshots(label);
            "#,
        ).context("Failed to initialize snapshot database schema")?;

        Ok(())
    }

    pub fn insert_snapshot(&self, snapshot: &ConfigSnapshot) -> Result<()> {
        let config_files_json = serde_json::to_string(&snapshot.config_files)
            .context("Failed to serialize config files")?;

        self.conn.lock().execute(
            r#"
            INSERT INTO snapshots (id, timestamp, label, description, config_files, checksum, size_bytes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                snapshot.id,
                snapshot.timestamp,
                snapshot.label,
                snapshot.description,
                config_files_json,
                snapshot.checksum,
                snapshot.size_bytes as i64,
            ],
        ).context("Failed to insert snapshot")?;

        Ok(())
    }

    pub fn get_snapshot(&self, id: &str) -> Result<Option<ConfigSnapshot>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, timestamp, label, description, config_files, checksum, size_bytes
            FROM snapshots
            WHERE id = ?1
            "#,
        ).context("Failed to prepare get snapshot statement")?;

        let result = stmt.query_row(params![id], |row| {
            let config_files_json: String = row.get(4)?;
            let config_files: Vec<String> = serde_json::from_str(&config_files_json)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

            Ok(ConfigSnapshot {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                label: row.get(2)?,
                description: row.get(3)?,
                config_files,
                checksum: row.get(5)?,
                size_bytes: row.get::<_, i64>(6)? as u64,
            })
        });

        match result {
            Ok(snapshot) => Ok(Some(snapshot)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e).context("Failed to query snapshot"),
        }
    }

    pub fn list_snapshots(&self, filter: SnapshotFilter) -> Result<Vec<ConfigSnapshot>> {
        let conn = self.conn.lock();
        let mut sql = String::from(
            "SELECT id, timestamp, label, description, config_files, checksum, size_bytes FROM snapshots WHERE 1=1"
        );
        let mut bind_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(label) = &filter.label {
            sql.push_str(" AND label = ?");
            bind_params.push(Box::new(label.clone()));
        }

        if let Some(since) = filter.since {
            sql.push_str(" AND timestamp >= ?");
            bind_params.push(Box::new(since));
        }

        if let Some(until) = filter.until {
            sql.push_str(" AND timestamp <= ?");
            bind_params.push(Box::new(until));
        }

        sql.push_str(" ORDER BY timestamp DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = bind_params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare_cached(&sql)
            .context("Failed to prepare list snapshots statement")?;

        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            let config_files_json: String = row.get(4)?;
            let config_files: Vec<String> = serde_json::from_str(&config_files_json)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

            Ok(ConfigSnapshot {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                label: row.get(2)?,
                description: row.get(3)?,
                config_files,
                checksum: row.get(5)?,
                size_bytes: row.get::<_, i64>(6)? as u64,
            })
        }).context("Failed to query snapshots")?;

        let mut snapshots = Vec::new();
        for row in rows {
            snapshots.push(row?);
        }

        Ok(snapshots)
    }

    pub fn delete_snapshot(&self, id: &str) -> Result<()> {
        self.conn.lock().execute(
            "DELETE FROM snapshots WHERE id = ?1",
            params![id],
        ).context("Failed to delete snapshot")?;

        Ok(())
    }

    pub fn count_snapshots(&self) -> Result<usize> {
        let count: i64 = self.conn.lock().query_row(
            "SELECT COUNT(*) FROM snapshots",
            [],
            |row| row.get(0),
        ).context("Failed to count snapshots")?;

        Ok(count as usize)
    }

    pub fn total_size_bytes(&self) -> Result<u64> {
        let total: i64 = self.conn.lock().query_row(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM snapshots",
            [],
            |row| row.get(0),
        ).context("Failed to calculate total size")?;

        Ok(total as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_store() -> (TempDir, SnapshotStore) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SnapshotStore::new(&db_path).unwrap();
        (dir, store)
    }

    fn create_test_snapshot(label: Option<&str>) -> ConfigSnapshot {
        ConfigSnapshot::new(
            label.map(str::to_string),
            Some("Test description".to_string()),
            vec!["network".to_string(), "firewall".to_string()],
            "abc123def456".to_string(),
            1024,
        )
    }

    #[test]
    fn inserts_and_retrieves_snapshot() {
        let (_dir, store) = setup_store();
        let snapshot = create_test_snapshot(Some("test-label"));

        store.insert_snapshot(&snapshot).unwrap();

        let retrieved = store.get_snapshot(&snapshot.id).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, snapshot.id);
        assert_eq!(retrieved.label, snapshot.label);
        assert_eq!(retrieved.config_files, snapshot.config_files);
    }

    #[test]
    fn returns_none_for_nonexistent_snapshot() {
        let (_dir, store) = setup_store();
        let result = store.get_snapshot("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn lists_snapshots_with_filter() {
        let (_dir, store) = setup_store();

        let s1 = create_test_snapshot(Some("label-a"));
        let s2 = create_test_snapshot(Some("label-b"));
        let s3 = create_test_snapshot(Some("label-a"));

        store.insert_snapshot(&s1).unwrap();
        store.insert_snapshot(&s2).unwrap();
        store.insert_snapshot(&s3).unwrap();

        let all = store.list_snapshots(SnapshotFilter::default()).unwrap();
        assert_eq!(all.len(), 3);

        let filtered = store.list_snapshots(SnapshotFilter {
            label: Some("label-a".to_string()),
            ..Default::default()
        }).unwrap();
        assert_eq!(filtered.len(), 2);

        let limited = store.list_snapshots(SnapshotFilter {
            limit: Some(1),
            ..Default::default()
        }).unwrap();
        assert_eq!(limited.len(), 1);
    }

    #[test]
    fn deletes_snapshot() {
        let (_dir, store) = setup_store();
        let snapshot = create_test_snapshot(None);

        store.insert_snapshot(&snapshot).unwrap();
        assert!(store.get_snapshot(&snapshot.id).unwrap().is_some());

        store.delete_snapshot(&snapshot.id).unwrap();
        assert!(store.get_snapshot(&snapshot.id).unwrap().is_none());
    }

    #[test]
    fn counts_snapshots() {
        let (_dir, store) = setup_store();

        assert_eq!(store.count_snapshots().unwrap(), 0);

        store.insert_snapshot(&create_test_snapshot(None)).unwrap();
        assert_eq!(store.count_snapshots().unwrap(), 1);

        store.insert_snapshot(&create_test_snapshot(None)).unwrap();
        assert_eq!(store.count_snapshots().unwrap(), 2);
    }

    #[test]
    fn calculates_total_size() {
        let (_dir, store) = setup_store();

        let mut s1 = create_test_snapshot(None);
        s1.size_bytes = 1000;
        let mut s2 = create_test_snapshot(None);
        s2.size_bytes = 2000;

        store.insert_snapshot(&s1).unwrap();
        store.insert_snapshot(&s2).unwrap();

        let total = store.total_size_bytes().unwrap();
        assert_eq!(total, 3000);
    }

    #[test]
    fn filters_by_time_range() {
        let (_dir, store) = setup_store();

        let mut s1 = create_test_snapshot(None);
        s1.timestamp = 1000;
        let mut s2 = create_test_snapshot(None);
        s2.timestamp = 2000;
        let mut s3 = create_test_snapshot(None);
        s3.timestamp = 3000;

        store.insert_snapshot(&s1).unwrap();
        store.insert_snapshot(&s2).unwrap();
        store.insert_snapshot(&s3).unwrap();

        let filtered = store.list_snapshots(SnapshotFilter {
            since: Some(1500),
            until: Some(2500),
            ..Default::default()
        }).unwrap();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].timestamp, 2000);
    }
}

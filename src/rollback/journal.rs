use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    FirewallRule,
    NetworkConfig,
    ServiceConfig,
    ParentalRule,
    PluginInstall,
    PluginRemove,
    Other,
}

impl ChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FirewallRule => "firewall_rule",
            Self::NetworkConfig => "network_config",
            Self::ServiceConfig => "service_config",
            Self::ParentalRule => "parental_rule",
            Self::PluginInstall => "plugin_install",
            Self::PluginRemove => "plugin_remove",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "firewall_rule" => Ok(Self::FirewallRule),
            "network_config" => Ok(Self::NetworkConfig),
            "service_config" => Ok(Self::ServiceConfig),
            "parental_rule" => Ok(Self::ParentalRule),
            "plugin_install" => Ok(Self::PluginInstall),
            "plugin_remove" => Ok(Self::PluginRemove),
            "other" => Ok(Self::Other),
            _ => anyhow::bail!("Unknown change type: {}", s),
        }
    }
}

impl std::fmt::Display for ChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeStatus {
    Pending,
    Applied,
    RolledBack,
    Failed,
}

impl ChangeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::RolledBack => "rolled_back",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(Self::Pending),
            "applied" => Ok(Self::Applied),
            "rolled_back" => Ok(Self::RolledBack),
            "failed" => Ok(Self::Failed),
            _ => anyhow::bail!("Unknown change status: {}", s),
        }
    }
}

impl std::fmt::Display for ChangeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    pub id: String,
    pub timestamp: i64,
    pub operator: String,
    pub change_type: ChangeType,
    pub target: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub snapshot_id: Option<String>,
    pub status: ChangeStatus,
    pub rollback_id: Option<String>,
}

impl ChangeRecord {
    pub fn new(
        operator: &str,
        change_type: ChangeType,
        target: &str,
        before: Option<String>,
        after: Option<String>,
        snapshot_id: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now().timestamp(),
            operator: operator.to_string(),
            change_type,
            target: target.to_string(),
            before,
            after,
            snapshot_id,
            status: ChangeStatus::Pending,
            rollback_id: None,
        }
    }

    pub fn timestamp_datetime(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.timestamp, 0)
            .unwrap_or_else(|| Utc::now())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChangeQuery {
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub operator: Option<String>,
    pub change_type: Option<ChangeType>,
    pub status: Option<ChangeStatus>,
    pub target: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiffEntry {
    pub path: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub diff_type: DiffType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffType {
    Added,
    Removed,
    Modified,
    Unchanged,
}

impl std::fmt::Display for DiffType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Added => write!(f, "+"),
            Self::Removed => write!(f, "-"),
            Self::Modified => write!(f, "~"),
            Self::Unchanged => write!(f, " "),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiff {
    pub record_id: String,
    pub target: String,
    pub entries: Vec<ConfigDiffEntry>,
    pub summary: DiffSummary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffSummary {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub unchanged: usize,
}

pub struct ChangeJournal {
    conn: Arc<Mutex<Connection>>,
    db_path: PathBuf,
}

impl ChangeJournal {
    pub fn open(workspace_dir: &Path) -> Result<Self> {
        let db_path = workspace_dir.join("rollback").join("change_journal.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create rollback directory: {}", parent.display()))?;
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open change journal database: {}", db_path.display()))?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )
        .context("Failed to set SQLite pragmas")?;

        Self::init_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path,
        })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS change_journal (
                id          TEXT PRIMARY KEY,
                timestamp   INTEGER NOT NULL,
                operator    TEXT NOT NULL,
                change_type TEXT NOT NULL,
                target      TEXT NOT NULL,
                before      TEXT,
                after       TEXT,
                snapshot_id TEXT,
                status      TEXT NOT NULL,
                rollback_id TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_change_journal_timestamp ON change_journal(timestamp);
            CREATE INDEX IF NOT EXISTS idx_change_journal_operator ON change_journal(operator);
            CREATE INDEX IF NOT EXISTS idx_change_journal_type ON change_journal(change_type);
            CREATE INDEX IF NOT EXISTS idx_change_journal_status ON change_journal(status);
            CREATE INDEX IF NOT EXISTS idx_change_journal_target ON change_journal(target);",
        )
        .context("Failed to initialize change journal schema")?;

        Ok(())
    }

    pub fn record(&self, record: &ChangeRecord) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO change_journal (
                id, timestamp, operator, change_type, target, before, after, snapshot_id, status, rollback_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.id,
                record.timestamp,
                record.operator,
                record.change_type.as_str(),
                record.target,
                record.before,
                record.after,
                record.snapshot_id,
                record.status.as_str(),
                record.rollback_id,
            ],
        )
        .context("Failed to insert change record")?;

        Ok(())
    }

    pub fn update_status(&self, id: &str, status: ChangeStatus) -> Result<()> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE change_journal SET status = ?1 WHERE id = ?2",
            params![status.as_str(), id],
        )
        .context("Failed to update change record status")?;

        if affected == 0 {
            anyhow::bail!("Change record '{}' not found", id);
        }

        Ok(())
    }

    pub fn set_rollback_id(&self, id: &str, rollback_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE change_journal SET rollback_id = ?1, status = 'rolled_back' WHERE id = ?2",
            params![rollback_id, id],
        )
        .context("Failed to set rollback ID")?;

        if affected == 0 {
            anyhow::bail!("Change record '{}' not found", id);
        }

        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<ChangeRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, operator, change_type, target, before, after, snapshot_id, status, rollback_id
             FROM change_journal WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_record(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn query(&self, query: &ChangeQuery) -> Result<Vec<ChangeRecord>> {
        let conn = self.conn.lock();

        let mut sql = String::from(
            "SELECT id, timestamp, operator, change_type, target, before, after, snapshot_id, status, rollback_id
             FROM change_journal WHERE 1=1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(start) = query.start_time {
            sql.push_str(&format!(" AND timestamp >= ?{param_idx}"));
            param_values.push(Box::new(start));
            param_idx += 1;
        }

        if let Some(end) = query.end_time {
            sql.push_str(&format!(" AND timestamp <= ?{param_idx}"));
            param_values.push(Box::new(end));
            param_idx += 1;
        }

        if let Some(ref op) = query.operator {
            sql.push_str(&format!(" AND operator = ?{param_idx}"));
            param_values.push(Box::new(op.clone()));
            param_idx += 1;
        }

        if let Some(ref ct) = query.change_type {
            sql.push_str(&format!(" AND change_type = ?{param_idx}"));
            param_values.push(Box::new(ct.as_str().to_string()));
            param_idx += 1;
        }

        if let Some(ref st) = query.status {
            sql.push_str(&format!(" AND status = ?{param_idx}"));
            param_values.push(Box::new(st.as_str().to_string()));
            param_idx += 1;
        }

        if let Some(ref target) = query.target {
            sql.push_str(&format!(" AND target LIKE ?{param_idx}"));
            param_values.push(Box::new(format!("%{target}%")));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY timestamp DESC");

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT ?{param_idx}"));
            param_values.push(Box::new(limit as i64));
            param_idx += 1;
        }

        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET ?{param_idx}"));
            param_values.push(Box::new(offset as i64));
        }

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(AsRef::as_ref).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_ref.as_slice(), |row| Self::row_to_record(row))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChangeRecord> {
        Ok(ChangeRecord {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            operator: row.get(2)?,
            change_type: ChangeType::from_str(&row.get::<_, String>(3)?)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?,
            target: row.get(4)?,
            before: row.get(5)?,
            after: row.get(6)?,
            snapshot_id: row.get(7)?,
            status: ChangeStatus::from_str(&row.get::<_, String>(8)?)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?,
            rollback_id: row.get(9)?,
        })
    }

    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM change_journal", [], |row| {
            row.get(0)
        })?;
        Ok(count as usize)
    }

    pub fn count_by_status(&self, status: ChangeStatus) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM change_journal WHERE status = ?1",
            params![status.as_str()],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute("DELETE FROM change_journal WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn delete_before(&self, timestamp: i64) -> Result<usize> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "DELETE FROM change_journal WHERE timestamp < ?1",
            params![timestamp],
        )?;
        Ok(affected)
    }

    pub fn get_latest_by_target(&self, target: &str) -> Result<Option<ChangeRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, operator, change_type, target, before, after, snapshot_id, status, rollback_id
             FROM change_journal WHERE target = ?1
             ORDER BY timestamp DESC LIMIT 1",
        )?;

        let mut rows = stmt.query(params![target])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_record(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn get_by_snapshot(&self, snapshot_id: &str) -> Result<Vec<ChangeRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, operator, change_type, target, before, after, snapshot_id, status, rollback_id
             FROM change_journal WHERE snapshot_id = ?1
             ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map(params![snapshot_id], |row| Self::row_to_record(row))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    pub fn health_check(&self) -> bool {
        let conn = self.conn.lock();
        conn.execute_batch("SELECT 1").is_ok()
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

pub fn compute_diff(before: Option<&str>, after: Option<&str>) -> ConfigDiff {
    let before_lines: Vec<&str> = before.map(|s| s.lines().collect()).unwrap_or_default();
    let after_lines: Vec<&str> = after.map(|s| s.lines().collect()).unwrap_or_default();

    let mut entries = Vec::new();
    let mut summary = DiffSummary::default();

    let max_len = before_lines.len().max(after_lines.len());

    for i in 0..max_len {
        let before_line = before_lines.get(i).copied();
        let after_line = after_lines.get(i).copied();

        let (diff_type, old_value, new_value) = match (before_line, after_line) {
            (Some(b), Some(a)) if b == a => (DiffType::Unchanged, Some(b.to_string()), Some(a.to_string())),
            (Some(b), Some(a)) => (DiffType::Modified, Some(b.to_string()), Some(a.to_string())),
            (Some(b), None) => (DiffType::Removed, Some(b.to_string()), None),
            (None, Some(a)) => (DiffType::Added, None, Some(a.to_string())),
            (None, None) => continue,
        };

        match diff_type {
            DiffType::Added => summary.added += 1,
            DiffType::Removed => summary.removed += 1,
            DiffType::Modified => summary.modified += 1,
            DiffType::Unchanged => summary.unchanged += 1,
        }

        entries.push(ConfigDiffEntry {
            path: format!("line_{}", i + 1),
            old_value,
            new_value,
            diff_type,
        });
    }

    ConfigDiff {
        record_id: String::new(),
        target: String::new(),
        entries,
        summary,
    }
}

pub fn compute_diff_for_record(record: &ChangeRecord) -> ConfigDiff {
    let mut diff = compute_diff(record.before.as_deref(), record.after.as_deref());
    diff.record_id = record.id.clone();
    diff.target = record.target.clone();
    diff
}

pub fn format_diff_output(diff: &ConfigDiff) -> String {
    let mut output = String::new();
    output.push_str(&format!("Diff for: {} (record: {})\n", diff.target, diff.record_id));
    output.push_str(&format!(
        "Summary: +{} -{} ~{} ={}\n\n",
        diff.summary.added, diff.summary.removed, diff.summary.modified, diff.summary.unchanged
    ));

    for entry in &diff.entries {
        let prefix = entry.diff_type.to_string();
        match entry.diff_type {
            DiffType::Added => {
                if let Some(new) = &entry.new_value {
                    output.push_str(&format!("{}{}\n", prefix, new));
                }
            }
            DiffType::Removed => {
                if let Some(old) = &entry.old_value {
                    output.push_str(&format!("{}{}\n", prefix, old));
                }
            }
            DiffType::Modified => {
                if let Some(old) = &entry.old_value {
                    output.push_str(&format!("-{}\n", old));
                }
                if let Some(new) = &entry.new_value {
                    output.push_str(&format!("+{}\n", new));
                }
            }
            DiffType::Unchanged => {
                if let Some(val) = &entry.new_value {
                    output.push_str(&format!(" {}{}\n", prefix, val));
                }
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_journal() -> (TempDir, ChangeJournal) {
        let tmp = TempDir::new().unwrap();
        let journal = ChangeJournal::open(tmp.path()).unwrap();
        (tmp, journal)
    }

    #[test]
    fn change_type_roundtrip() {
        let types = [
            ChangeType::FirewallRule,
            ChangeType::NetworkConfig,
            ChangeType::ServiceConfig,
            ChangeType::ParentalRule,
            ChangeType::PluginInstall,
            ChangeType::PluginRemove,
            ChangeType::Other,
        ];

        for ct in &types {
            let s = ct.as_str();
            let parsed = ChangeType::from_str(s).unwrap();
            assert_eq!(*ct, parsed);
        }
    }

    #[test]
    fn change_status_roundtrip() {
        let statuses = [
            ChangeStatus::Pending,
            ChangeStatus::Applied,
            ChangeStatus::RolledBack,
            ChangeStatus::Failed,
        ];

        for st in &statuses {
            let s = st.as_str();
            let parsed = ChangeStatus::from_str(s).unwrap();
            assert_eq!(*st, parsed);
        }
    }

    #[test]
    fn journal_open_creates_db() {
        let tmp = TempDir::new().unwrap();
        let journal = ChangeJournal::open(tmp.path()).unwrap();
        assert!(journal.health_check());
        assert!(journal.db_path().exists());
    }

    #[test]
    fn record_and_get() {
        let (_tmp, journal) = temp_journal();
        let record = ChangeRecord::new(
            "zeroclaw_user",
            ChangeType::FirewallRule,
            "/etc/firewall/rules.conf",
            Some("old rule".to_string()),
            Some("new rule".to_string()),
            None,
        );

        let id = record.id.clone();
        journal.record(&record).unwrap();

        let retrieved = journal.get(&id).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.operator, "zeroclaw_user");
        assert_eq!(retrieved.change_type, ChangeType::FirewallRule);
        assert_eq!(retrieved.target, "/etc/firewall/rules.conf");
        assert_eq!(retrieved.before, Some("old rule".to_string()));
        assert_eq!(retrieved.after, Some("new rule".to_string()));
        assert_eq!(retrieved.status, ChangeStatus::Pending);
    }

    #[test]
    fn update_status() {
        let (_tmp, journal) = temp_journal();
        let record = ChangeRecord::new(
            "zeroclaw_user",
            ChangeType::NetworkConfig,
            "/etc/network/interfaces",
            None,
            Some("new config".to_string()),
            None,
        );

        let id = record.id.clone();
        journal.record(&record).unwrap();
        journal.update_status(&id, ChangeStatus::Applied).unwrap();

        let retrieved = journal.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.status, ChangeStatus::Applied);
    }

    #[test]
    fn set_rollback_id() {
        let (_tmp, journal) = temp_journal();
        let record = ChangeRecord::new(
            "zeroclaw_user",
            ChangeType::ServiceConfig,
            "/etc/systemd/system/zeroclaw.service",
            Some("old service".to_string()),
            Some("new service".to_string()),
            None,
        );

        let id = record.id.clone();
        journal.record(&record).unwrap();
        journal.set_rollback_id(&id, "rollback-123").unwrap();

        let retrieved = journal.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.rollback_id, Some("rollback-123".to_string()));
        assert_eq!(retrieved.status, ChangeStatus::RolledBack);
    }

    #[test]
    fn query_by_operator() {
        let (_tmp, journal) = temp_journal();

        for i in 0..3 {
            let record = ChangeRecord::new(
                &format!("operator_{}", i % 2),
                ChangeType::Other,
                &format!("/target/{}", i),
                None,
                None,
                None,
            );
            journal.record(&record).unwrap();
        }

        let results = journal.query(&ChangeQuery {
            operator: Some("operator_0".to_string()),
            ..Default::default()
        }).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.operator == "operator_0"));
    }

    #[test]
    fn query_by_change_type() {
        let (_tmp, journal) = temp_journal();

        let types = [
            ChangeType::FirewallRule,
            ChangeType::NetworkConfig,
            ChangeType::FirewallRule,
        ];

        for (i, ct) in types.iter().enumerate() {
            let record = ChangeRecord::new(
                "zeroclaw_user",
                *ct,
                &format!("/target/{}", i),
                None,
                None,
                None,
            );
            journal.record(&record).unwrap();
        }

        let results = journal.query(&ChangeQuery {
            change_type: Some(ChangeType::FirewallRule),
            ..Default::default()
        }).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.change_type == ChangeType::FirewallRule));
    }

    #[test]
    fn query_by_time_range() {
        let (_tmp, journal) = temp_journal();

        let base_ts = Utc::now().timestamp() - 1000;

        for i in 0..5 {
            let mut record = ChangeRecord::new(
                "zeroclaw_user",
                ChangeType::Other,
                &format!("/target/{}", i),
                None,
                None,
                None,
            );
            record.timestamp = base_ts + i * 100;
            journal.record(&record).unwrap();
        }

        let results = journal.query(&ChangeQuery {
            start_time: Some(base_ts + 100),
            end_time: Some(base_ts + 300),
            ..Default::default()
        }).unwrap();

        assert_eq!(results.len(), 3);
    }

    #[test]
    fn query_with_limit_and_offset() {
        let (_tmp, journal) = temp_journal();

        for i in 0..10 {
            let record = ChangeRecord::new(
                "zeroclaw_user",
                ChangeType::Other,
                &format!("/target/{}", i),
                None,
                None,
                None,
            );
            journal.record(&record).unwrap();
        }

        let results = journal.query(&ChangeQuery {
            limit: Some(5),
            offset: Some(2),
            ..Default::default()
        }).unwrap();

        assert_eq!(results.len(), 5);
    }

    #[test]
    fn query_by_target() {
        let (_tmp, journal) = temp_journal();

        let targets = [
            "/etc/firewall/rules.conf",
            "/etc/network/interfaces",
            "/etc/firewall/custom.conf",
        ];

        for target in &targets {
            let record = ChangeRecord::new(
                "zeroclaw_user",
                ChangeType::Other,
                target,
                None,
                None,
                None,
            );
            journal.record(&record).unwrap();
        }

        let results = journal.query(&ChangeQuery {
            target: Some("firewall".to_string()),
            ..Default::default()
        }).unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn count_and_count_by_status() {
        let (_tmp, journal) = temp_journal();

        for i in 0..5 {
            let mut record = ChangeRecord::new(
                "zeroclaw_user",
                ChangeType::Other,
                &format!("/target/{}", i),
                None,
                None,
                None,
            );
            if i < 2 {
                record.status = ChangeStatus::Applied;
            } else {
                record.status = ChangeStatus::Pending;
            }
            journal.record(&record).unwrap();
        }

        assert_eq!(journal.count().unwrap(), 5);
        assert_eq!(journal.count_by_status(ChangeStatus::Applied).unwrap(), 2);
        assert_eq!(journal.count_by_status(ChangeStatus::Pending).unwrap(), 3);
    }

    #[test]
    fn delete_record() {
        let (_tmp, journal) = temp_journal();
        let record = ChangeRecord::new(
            "zeroclaw_user",
            ChangeType::Other,
            "/target",
            None,
            None,
            None,
        );

        let id = record.id.clone();
        journal.record(&record).unwrap();
        assert_eq!(journal.count().unwrap(), 1);

        let deleted = journal.delete(&id).unwrap();
        assert!(deleted);
        assert_eq!(journal.count().unwrap(), 0);

        let deleted_again = journal.delete(&id).unwrap();
        assert!(!deleted_again);
    }

    #[test]
    fn delete_before_timestamp() {
        let (_tmp, journal) = temp_journal();
        let base_ts = Utc::now().timestamp();

        for i in 0..5 {
            let mut record = ChangeRecord::new(
                "zeroclaw_user",
                ChangeType::Other,
                &format!("/target/{}", i),
                None,
                None,
                None,
            );
            record.timestamp = base_ts + i * 100;
            journal.record(&record).unwrap();
        }

        let deleted = journal.delete_before(base_ts + 250).unwrap();
        assert_eq!(deleted, 3);
        assert_eq!(journal.count().unwrap(), 2);
    }

    #[test]
    fn get_latest_by_target() {
        let (_tmp, journal) = temp_journal();
        let target = "/etc/firewall/rules.conf";
        let base_ts = Utc::now().timestamp();

        for i in 0..3 {
            let mut record = ChangeRecord::new(
                "zeroclaw_user",
                ChangeType::FirewallRule,
                target,
                None,
                Some(format!("version_{}", i)),
                None,
            );
            record.timestamp = base_ts + i * 100;
            journal.record(&record).unwrap();
        }

        let latest = journal.get_latest_by_target(target).unwrap();
        assert!(latest.is_some());
        let latest = latest.unwrap();
        assert_eq!(latest.after, Some("version_2".to_string()));
    }

    #[test]
    fn get_by_snapshot() {
        let (_tmp, journal) = temp_journal();
        let snapshot_id = "snapshot-123";

        for i in 0..3 {
            let record = ChangeRecord::new(
                "zeroclaw_user",
                ChangeType::Other,
                &format!("/target/{}", i),
                None,
                None,
                Some(snapshot_id.to_string()),
            );
            journal.record(&record).unwrap();
        }

        let record_other = ChangeRecord::new(
            "zeroclaw_user",
            ChangeType::Other,
            "/other/target",
            None,
            None,
            Some("other-snapshot".to_string()),
        );
        journal.record(&record_other).unwrap();

        let results = journal.get_by_snapshot(snapshot_id).unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.snapshot_id.as_deref() == Some(snapshot_id)));
    }

    #[test]
    fn compute_diff_added() {
        let diff = compute_diff(None, Some("new line\nanother line"));
        assert_eq!(diff.summary.added, 2);
        assert_eq!(diff.summary.removed, 0);
        assert_eq!(diff.summary.modified, 0);
    }

    #[test]
    fn compute_diff_removed() {
        let diff = compute_diff(Some("old line\nanother old"), None);
        assert_eq!(diff.summary.added, 0);
        assert_eq!(diff.summary.removed, 2);
        assert_eq!(diff.summary.modified, 0);
    }

    #[test]
    fn compute_diff_modified() {
        let diff = compute_diff(Some("old line"), Some("new line"));
        assert_eq!(diff.summary.added, 0);
        assert_eq!(diff.summary.removed, 0);
        assert_eq!(diff.summary.modified, 1);
    }

    #[test]
    fn compute_diff_mixed() {
        let before = "line1\nline2\nline3";
        let after = "line1\nmodified\nline3\nline4";

        let diff = compute_diff(Some(before), Some(after));
        assert_eq!(diff.summary.unchanged, 2);
        assert_eq!(diff.summary.modified, 1);
        assert_eq!(diff.summary.added, 1);
    }

    #[test]
    fn compute_diff_for_record() {
        let record = ChangeRecord::new(
            "zeroclaw_user",
            ChangeType::FirewallRule,
            "/etc/firewall/rules.conf",
            Some("allow 80\nallow 443".to_string()),
            Some("allow 80\nallow 443\nallow 8080".to_string()),
            None,
        );

        let diff = compute_diff_for_record(&record);
        assert_eq!(diff.record_id, record.id);
        assert_eq!(diff.target, record.target);
        assert_eq!(diff.summary.added, 1);
        assert_eq!(diff.summary.unchanged, 2);
    }

    #[test]
    fn format_diff_output_contains_summary() {
        let diff = ConfigDiff {
            record_id: "test-id".to_string(),
            target: "/test/path".to_string(),
            entries: vec![],
            summary: DiffSummary {
                added: 1,
                removed: 2,
                modified: 3,
                unchanged: 4,
            },
        };

        let output = format_diff_output(&diff);
        assert!(output.contains("test-id"));
        assert!(output.contains("/test/path"));
        assert!(output.contains("+1"));
        assert!(output.contains("-2"));
        assert!(output.contains("~3"));
        assert!(output.contains("=4"));
    }

    #[test]
    fn change_record_new_generates_uuid() {
        let r1 = ChangeRecord::new("op", ChangeType::Other, "target", None, None, None);
        let r2 = ChangeRecord::new("op", ChangeType::Other, "target", None, None, None);

        assert_ne!(r1.id, r2.id);
        assert!(!r1.id.is_empty());
    }

    #[test]
    fn change_record_timestamp_datetime() {
        let record = ChangeRecord::new("op", ChangeType::Other, "target", None, None, None);
        let dt = record.timestamp_datetime();
        assert!(dt.timestamp() > 0);
    }

    #[test]
    fn journal_persists_across_reopen() {
        let tmp = TempDir::new().unwrap();

        {
            let journal = ChangeJournal::open(tmp.path()).unwrap();
            let record = ChangeRecord::new(
                "zeroclaw_user",
                ChangeType::Other,
                "/persistent/target",
                None,
                None,
                None,
            );
            let id = record.id.clone();
            journal.record(&record).unwrap();
        }

        {
            let journal = ChangeJournal::open(tmp.path()).unwrap();
            let count = journal.count().unwrap();
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn query_by_status() {
        let (_tmp, journal) = temp_journal();

        let statuses = [
            ChangeStatus::Pending,
            ChangeStatus::Applied,
            ChangeStatus::Failed,
            ChangeStatus::Applied,
        ];

        for (i, st) in statuses.iter().enumerate() {
            let mut record = ChangeRecord::new(
                "zeroclaw_user",
                ChangeType::Other,
                &format!("/target/{}", i),
                None,
                None,
                None,
            );
            record.status = *st;
            journal.record(&record).unwrap();
        }

        let results = journal.query(&ChangeQuery {
            status: Some(ChangeStatus::Applied),
            ..Default::default()
        }).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.status == ChangeStatus::Applied));
    }

    #[test]
    fn combined_query() {
        let (_tmp, journal) = temp_journal();
        let base_ts = Utc::now().timestamp();

        for i in 0..10 {
            let mut record = ChangeRecord::new(
                if i % 2 == 0 { "alice" } else { "bob" },
                if i % 3 == 0 { ChangeType::FirewallRule } else { ChangeType::NetworkConfig },
                &format!("/target/{}", i),
                None,
                None,
                None,
            );
            record.timestamp = base_ts + i * 100;
            record.status = if i % 2 == 0 { ChangeStatus::Applied } else { ChangeStatus::Pending };
            journal.record(&record).unwrap();
        }

        let results = journal.query(&ChangeQuery {
            operator: Some("alice".to_string()),
            change_type: Some(ChangeType::FirewallRule),
            status: Some(ChangeStatus::Applied),
            start_time: Some(base_ts),
            end_time: Some(base_ts + 1000),
            limit: Some(5),
            ..Default::default()
        }).unwrap();

        assert!(results.iter().all(|r| {
            r.operator == "alice"
                && r.change_type == ChangeType::FirewallRule
                && r.status == ChangeStatus::Applied
                && r.timestamp >= base_ts
                && r.timestamp <= base_ts + 1000
        }));
    }
}

//! Rollback engine for configuration changes.
//!
//! Provides automatic and manual rollback capabilities:
//! - Rollback to specific snapshot
//! - Rollback to timestamp
//! - Change confirmation mechanism
//! - Timeout-based auto-rollback
//! - Failure-based auto-rollback
//! - Emergency recovery mode

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use uuid::Uuid;

use super::journal::{ChangeRecord, ChangeStatus, ChangeType, ChangeJournal};
use super::snapshot::{ConfigSnapshot, CreateSnapshotOptions, SnapshotFilter, SnapshotManager};

pub const DEFAULT_AUTO_ROLLBACK_TIMEOUT_SECS: u64 = 60;
pub const DEFAULT_MAX_SNAPSHOTS: usize = 10;
pub const EMERGENCY_TRIGGER_FILE_NAME: &str = ".emergency_rollback";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackConfig {
    pub auto_rollback_timeout_secs: u64,
    pub max_snapshots: usize,
    pub emergency_trigger_file: PathBuf,
}

impl Default for RollbackConfig {
    fn default() -> Self {
        Self {
            auto_rollback_timeout_secs: DEFAULT_AUTO_ROLLBACK_TIMEOUT_SECS,
            max_snapshots: DEFAULT_MAX_SNAPSHOTS,
            emergency_trigger_file: PathBuf::from(EMERGENCY_TRIGGER_FILE_NAME),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    pub success: bool,
    pub snapshot_id: String,
    pub changes_reverted: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmResult {
    pub success: bool,
    pub change_id: String,
    pub confirmed_at: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingRollback {
    pub change_id: String,
    pub snapshot_id: String,
    pub created_at: i64,
    pub timeout_secs: u64,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackLogEntry {
    pub id: String,
    pub timestamp: i64,
    pub change_id: Option<String>,
    pub from_snapshot_id: String,
    pub to_snapshot_id: String,
    pub trigger: RollbackTrigger,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollbackTrigger {
    Manual,
    Timeout,
    Failure,
    Emergency,
}

impl std::fmt::Display for RollbackTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::Timeout => write!(f, "timeout"),
            Self::Failure => write!(f, "failure"),
            Self::Emergency => write!(f, "emergency"),
        }
    }
}

pub struct RollbackEngine {
    snapshot_manager: SnapshotManager,
    journal: ChangeJournal,
    config: RollbackConfig,
    pending_rollbacks: Arc<RwLock<Vec<PendingRollback>>>,
    rollback_logs: Arc<Mutex<Vec<RollbackLogEntry>>>,
    zeroclaw_dir: PathBuf,
}

impl RollbackEngine {
    pub fn new(
        zeroclaw_dir: &Path,
        config_dir: Option<&Path>,
        config: Option<RollbackConfig>,
    ) -> Result<Self> {
        let snapshot_manager = SnapshotManager::new(zeroclaw_dir, config_dir)?;
        let journal = ChangeJournal::open(zeroclaw_dir)?;

        Ok(Self {
            snapshot_manager,
            journal,
            config: config.unwrap_or_default(),
            pending_rollbacks: Arc::new(RwLock::new(Vec::new())),
            rollback_logs: Arc::new(Mutex::new(Vec::new())),
            zeroclaw_dir: zeroclaw_dir.to_path_buf(),
        })
    }

    pub fn snapshot_manager(&self) -> &SnapshotManager {
        &self.snapshot_manager
    }

    pub fn journal(&self) -> &ChangeJournal {
        &self.journal
    }

    pub fn config(&self) -> &RollbackConfig {
        &self.config
    }

    pub async fn rollback_to_snapshot(&self, snapshot_id: &str) -> Result<RollbackResult> {
        let snapshot = match self.snapshot_manager.get_snapshot(snapshot_id)? {
            Some(s) => s,
            None => {
                bail!("Snapshot not found: {}", snapshot_id);
            }
        };

        if !self.snapshot_manager.verify_snapshot(snapshot_id)? {
            bail!("Snapshot integrity check failed: {}", snapshot_id);
        }

        let pre_rollback_snapshot = self.create_pre_rollback_snapshot()?;

        let changes_reverted = match self.snapshot_manager.restore_snapshot(snapshot_id, None) {
            Ok(files) => files,
            Err(e) => {
                let _ = self.snapshot_manager.restore_snapshot(&pre_rollback_snapshot.id, None);
                bail!("Failed to restore snapshot: {}", e);
            }
        };

        let related_changes = self.journal.get_by_snapshot(snapshot_id)?;
        for mut change in related_changes {
            if change.status == ChangeStatus::Applied {
                change.status = ChangeStatus::RolledBack;
                self.journal.update_status(&change.id, ChangeStatus::RolledBack)?;
            }
        }

        self.log_rollback(None, pre_rollback_snapshot.id.clone(), snapshot_id.to_string(), RollbackTrigger::Manual, true, None);

        Ok(RollbackResult {
            success: true,
            snapshot_id: snapshot_id.to_string(),
            changes_reverted,
            error: None,
        })
    }

    pub async fn rollback_to_timestamp(&self, target_timestamp: i64) -> Result<RollbackResult> {
        let snapshots = self.snapshot_manager.list_snapshots(SnapshotFilter {
            until: Some(target_timestamp),
            limit: Some(1),
            ..Default::default()
        })?;

        let snapshot = match snapshots.first() {
            Some(s) => s,
            None => bail!("No snapshot found before timestamp {}", target_timestamp),
        };

        self.rollback_to_snapshot(&snapshot.id).await
    }

    pub async fn confirm_change(&self, change_id: &str) -> Result<ConfirmResult> {
        let change = match self.journal.get(change_id)? {
            Some(c) => c,
            None => bail!("Change not found: {}", change_id),
        };

        if change.status == ChangeStatus::RolledBack {
            bail!("Cannot confirm a rolled-back change: {}", change_id);
        }

        self.journal.update_status(change_id, ChangeStatus::Applied)?;

        {
            let mut pending = self.pending_rollbacks.write().await;
            if let Some(pos) = pending.iter().position(|p| p.change_id == change_id) {
                pending[pos].confirmed = true;
            }
        }

        Ok(ConfirmResult {
            success: true,
            change_id: change_id.to_string(),
            confirmed_at: Utc::now().timestamp(),
            error: None,
        })
    }

    pub async fn auto_rollback_on_timeout(
        &self,
        change_id: &str,
        timeout_secs: Option<u64>,
    ) -> Result<RollbackResult> {
        let change = match self.journal.get(change_id)? {
            Some(c) => c,
            None => bail!("Change not found: {}", change_id),
        };

        let snapshot_id = match &change.snapshot_id {
            Some(id) => id.clone(),
            None => bail!("Change {} has no associated snapshot", change_id),
        };

        let timeout_duration = Duration::from_secs(timeout_secs.unwrap_or(self.config.auto_rollback_timeout_secs));

        let pending = PendingRollback {
            change_id: change_id.to_string(),
            snapshot_id: snapshot_id.clone(),
            created_at: Utc::now().timestamp(),
            timeout_secs: timeout_duration.as_secs(),
            confirmed: false,
        };

        {
            let mut pending_list = self.pending_rollbacks.write().await;
            pending_list.push(pending);
        }

        let pending_rollbacks = self.pending_rollbacks.clone();
        let change_id_owned = change_id.to_string();
        let snapshot_id_owned = snapshot_id.clone();

        tokio::spawn(async move {
            tokio::time::sleep(timeout_duration).await;

            let should_rollback = {
                let pending = pending_rollbacks.read().await;
                if let Some(p) = pending.iter().find(|p| p.change_id == change_id_owned) {
                    !p.confirmed
                } else {
                    false
                }
            };

            if should_rollback {
                tracing::warn!(
                    "Auto-rollback triggered for change {} due to timeout",
                    change_id_owned
                );
            }
        });

        let wait_result = timeout(timeout_duration, async {
            loop {
                let confirmed = {
                    let pending = self.pending_rollbacks.read().await;
                    pending.iter().find(|p| p.change_id == change_id).map(|p| p.confirmed).unwrap_or(false)
                };

                if confirmed {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }).await;

        match wait_result {
            Ok(Ok(())) => {
                Ok(RollbackResult {
                    success: true,
                    snapshot_id: snapshot_id.clone(),
                    changes_reverted: vec![],
                    error: None,
                })
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                let result = self.rollback_to_snapshot(&snapshot_id).await?;
                let mut result = result;
                result.error = Some(format!("Auto-rollback triggered after {}s timeout", timeout_duration.as_secs()));
                
                self.log_rollback(
                    Some(change_id.to_string()),
                    String::new(),
                    snapshot_id.clone(),
                    RollbackTrigger::Timeout,
                    true,
                    None,
                );

                Ok(result)
            }
        }
    }

    pub async fn auto_rollback_on_failure(&self, change_id: &str) -> Result<RollbackResult> {
        let change = match self.journal.get(change_id)? {
            Some(c) => c,
            None => bail!("Change not found: {}", change_id),
        };

        let snapshot_id = match &change.snapshot_id {
            Some(id) => id.clone(),
            None => bail!("Change {} has no associated snapshot", change_id),
        };

        self.journal.update_status(change_id, ChangeStatus::Failed)?;

        let result = self.rollback_to_snapshot(&snapshot_id).await;

        match &result {
            Ok(rollback_result) => {
                self.log_rollback(
                    Some(change_id.to_string()),
                    String::new(),
                    snapshot_id.clone(),
                    RollbackTrigger::Failure,
                    true,
                    None,
                );
            }
            Err(e) => {
                self.log_rollback(
                    Some(change_id.to_string()),
                    String::new(),
                    snapshot_id.clone(),
                    RollbackTrigger::Failure,
                    false,
                    Some(e.to_string()),
                );
            }
        }

        result
    }

    pub async fn emergency_recovery(&self) -> Result<RollbackResult> {
        let trigger_path = self.zeroclaw_dir.join(&self.config.emergency_trigger_file);
        
        if !trigger_path.exists() {
            bail!("Emergency trigger file not found: {:?}", trigger_path);
        }

        tracing::warn!("Emergency recovery triggered via {:?}", trigger_path);

        let stable_label = "stable";
        let snapshots = self.snapshot_manager.list_snapshots(SnapshotFilter {
            label: Some(stable_label.to_string()),
            limit: Some(1),
            ..Default::default()
        })?;

        let snapshot = if let Some(s) = snapshots.first() {
            s.clone()
        } else {
            let all_snapshots = self.snapshot_manager.list_snapshots(SnapshotFilter {
                limit: Some(1),
                ..Default::default()
            })?;
            
            match all_snapshots.first() {
                Some(s) => s.clone(),
                None => bail!("No snapshots available for emergency recovery"),
            }
        };

        let result = self.rollback_to_snapshot(&snapshot.id).await;

        if result.is_ok() {
            if let Err(e) = std::fs::remove_file(&trigger_path) {
                tracing::warn!("Failed to remove emergency trigger file: {}", e);
            }

            self.log_rollback(
                None,
                String::new(),
                snapshot.id.clone(),
                RollbackTrigger::Emergency,
                true,
                None,
            );
        }

        result
    }

    pub fn create_stable_snapshot(&self, description: Option<&str>) -> Result<ConfigSnapshot> {
        self.snapshot_manager.create_snapshot(CreateSnapshotOptions {
            label: Some("stable".to_string()),
            description: description.map(str::to_string),
            ..Default::default()
        })
    }

    pub fn create_emergency_trigger(&self) -> Result<()> {
        let trigger_path = self.zeroclaw_dir.join(&self.config.emergency_trigger_file);
        std::fs::write(&trigger_path, Utc::now().timestamp().to_string())
            .with_context(|| format!("Failed to create emergency trigger file: {:?}", trigger_path))?;
        Ok(())
    }

    pub fn remove_emergency_trigger(&self) -> Result<bool> {
        let trigger_path = self.zeroclaw_dir.join(&self.config.emergency_trigger_file);
        if trigger_path.exists() {
            std::fs::remove_file(&trigger_path)
                .with_context(|| format!("Failed to remove emergency trigger file: {:?}", trigger_path))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn is_emergency_trigger_present(&self) -> bool {
        self.zeroclaw_dir.join(&self.config.emergency_trigger_file).exists()
    }

    pub fn cleanup_old_snapshots(&self) -> Result<usize> {
        let all_snapshots = self.snapshot_manager.list_snapshots(SnapshotFilter {
            limit: None,
            ..Default::default()
        })?;

        if all_snapshots.len() <= self.config.max_snapshots {
            return Ok(0);
        }

        let stable_snapshots: Vec<_> = all_snapshots
            .iter()
            .filter(|s| s.label.as_deref() == Some("stable"))
            .collect();

        let mut to_delete: Vec<String> = Vec::new();
        let mut non_stable_count = 0;

        for snapshot in all_snapshots.iter() {
            if snapshot.label.as_deref() != Some("stable") {
                non_stable_count += 1;
                if non_stable_count > self.config.max_snapshots.saturating_sub(stable_snapshots.len()) {
                    to_delete.push(snapshot.id.clone());
                }
            }
        }

        let mut deleted = 0;
        for id in to_delete {
            if self.snapshot_manager.delete_snapshot(&id)? {
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    pub fn get_rollback_history(&self, limit: Option<usize>) -> Vec<RollbackLogEntry> {
        let logs = self.rollback_logs.lock();
        let limit = limit.unwrap_or(100);
        logs.iter().take(limit).cloned().collect()
    }

    pub fn get_pending_rollbacks(&self) -> Vec<PendingRollback> {
        let pending = self.pending_rollbacks.blocking_read();
        pending.iter().filter(|p| !p.confirmed).cloned().collect()
    }

    fn create_pre_rollback_snapshot(&self) -> Result<ConfigSnapshot> {
        self.snapshot_manager.create_snapshot(CreateSnapshotOptions {
            label: Some("pre-rollback".to_string()),
            description: Some("Automatic snapshot before rollback operation".to_string()),
            ..Default::default()
        })
    }

    fn log_rollback(
        &self,
        change_id: Option<String>,
        from_snapshot_id: String,
        to_snapshot_id: String,
        trigger: RollbackTrigger,
        success: bool,
        error: Option<String>,
    ) {
        let entry = RollbackLogEntry {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now().timestamp(),
            change_id,
            from_snapshot_id,
            to_snapshot_id,
            trigger,
            success,
            error,
        };

        let mut logs = self.rollback_logs.lock();
        logs.push(entry);
    }

    pub fn register_change(
        &self,
        operator: &str,
        change_type: ChangeType,
        target: &str,
        before: Option<String>,
        after: Option<String>,
        snapshot_id: Option<String>,
    ) -> Result<ChangeRecord> {
        let record = ChangeRecord::new(
            operator,
            change_type,
            target,
            before,
            after,
            snapshot_id,
        );

        self.journal.record(&record)?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, TempDir, RollbackEngine) {
        let zeroclaw_dir = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();

        let config_path = config_dir.path();
        std::fs::write(config_path.join("network"), "config interface 'lan'\n").unwrap();
        std::fs::write(config_path.join("wireless"), "config wifi-iface\n").unwrap();
        std::fs::write(config_path.join("firewall"), "config defaults\n").unwrap();

        let engine = RollbackEngine::new(
            zeroclaw_dir.path(),
            Some(config_path),
            Some(RollbackConfig {
                auto_rollback_timeout_secs: 5,
                max_snapshots: 5,
                emergency_trigger_file: PathBuf::from(EMERGENCY_TRIGGER_FILE_NAME),
            }),
        ).unwrap();

        (zeroclaw_dir, config_dir, engine)
    }

    #[test]
    fn creates_engine_successfully() {
        let (zeroclaw_dir, config_dir, _engine) = setup_test_env();
        assert!(zeroclaw_dir.path().exists());
        assert!(config_dir.path().exists());
    }

    #[test]
    fn default_config_values() {
        let config = RollbackConfig::default();
        assert_eq!(config.auto_rollback_timeout_secs, DEFAULT_AUTO_ROLLBACK_TIMEOUT_SECS);
        assert_eq!(config.max_snapshots, DEFAULT_MAX_SNAPSHOTS);
    }

    #[tokio::test]
    async fn rollback_to_snapshot_success() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        let snapshot = engine.snapshot_manager().create_snapshot(CreateSnapshotOptions {
            label: Some("test-snapshot".to_string()),
            ..Default::default()
        }).unwrap();

        let result = engine.rollback_to_snapshot(&snapshot.id).await.unwrap();
        assert!(result.success);
        assert_eq!(result.snapshot_id, snapshot.id);
    }

    #[tokio::test]
    async fn rollback_to_nonexistent_snapshot_fails() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        let result = engine.rollback_to_snapshot("nonexistent-id").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rollback_to_timestamp_finds_snapshot() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        let snapshot = engine.snapshot_manager().create_snapshot(CreateSnapshotOptions {
            label: Some("timestamp-test".to_string()),
            ..Default::default()
        }).unwrap();

        let future_timestamp = Utc::now().timestamp() + 1000;
        let result = engine.rollback_to_timestamp(future_timestamp).await.unwrap();
        
        assert!(result.success);
    }

    #[tokio::test]
    async fn confirm_change_updates_status() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        let snapshot = engine.snapshot_manager().create_snapshot(CreateSnapshotOptions::default()).unwrap();

        let record = engine.register_change(
            "zeroclaw_user",
            ChangeType::FirewallRule,
            "/etc/config/firewall",
            Some("old config".to_string()),
            Some("new config".to_string()),
            Some(snapshot.id.clone()),
        ).unwrap();

        let result = engine.confirm_change(&record.id).await.unwrap();
        assert!(result.success);
        assert_eq!(result.change_id, record.id);

        let updated = engine.journal().get(&record.id).unwrap().unwrap();
        assert_eq!(updated.status, ChangeStatus::Applied);
    }

    #[tokio::test]
    async fn confirm_nonexistent_change_fails() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        let result = engine.confirm_change("nonexistent-id").await;
        assert!(result.is_err());
    }

    #[test]
    fn create_stable_snapshot() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        let snapshot = engine.create_stable_snapshot(Some("Stable baseline")).unwrap();
        assert_eq!(snapshot.label, Some("stable".to_string()));
    }

    #[test]
    fn emergency_trigger_management() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        assert!(!engine.is_emergency_trigger_present());

        engine.create_emergency_trigger().unwrap();
        assert!(engine.is_emergency_trigger_present());

        let removed = engine.remove_emergency_trigger().unwrap();
        assert!(removed);
        assert!(!engine.is_emergency_trigger_present());

        let removed_again = engine.remove_emergency_trigger().unwrap();
        assert!(!removed_again);
    }

    #[tokio::test]
    async fn emergency_recovery_requires_trigger() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        let result = engine.emergency_recovery().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn emergency_recovery_with_trigger() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        engine.create_stable_snapshot(Some("Emergency baseline")).unwrap();
        engine.create_emergency_trigger().unwrap();

        let result = engine.emergency_recovery().await.unwrap();
        assert!(result.success);
        assert!(!engine.is_emergency_trigger_present());
    }

    #[test]
    fn cleanup_old_snapshots_respects_limit() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        for i in 0..10 {
            engine.snapshot_manager().create_snapshot(CreateSnapshotOptions {
                label: Some(format!("snapshot-{}", i)),
                ..Default::default()
            }).unwrap();
        }

        let deleted = engine.cleanup_old_snapshots().unwrap();
        assert!(deleted > 0);

        let remaining = engine.snapshot_manager().list_snapshots(SnapshotFilter::default()).unwrap();
        assert!(remaining.len() <= engine.config().max_snapshots);
    }

    #[test]
    fn cleanup_preserves_stable_snapshots() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        for i in 0..3 {
            engine.create_stable_snapshot(Some(&format!("Stable {}", i))).unwrap();
        }

        for i in 0..10 {
            engine.snapshot_manager().create_snapshot(CreateSnapshotOptions {
                label: Some(format!("regular-{}", i)),
                ..Default::default()
            }).unwrap();
        }

        engine.cleanup_old_snapshots().unwrap();

        let stable = engine.snapshot_manager().list_snapshots(SnapshotFilter {
            label: Some("stable".to_string()),
            ..Default::default()
        }).unwrap();

        assert_eq!(stable.len(), 3);
    }

    #[test]
    fn rollback_history_tracking() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        let history = engine.get_rollback_history(None);
        assert!(history.is_empty());
    }

    #[test]
    fn pending_rollbacks_empty_initially() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        let pending = engine.get_pending_rollbacks();
        assert!(pending.is_empty());
    }

    #[test]
    fn register_change_creates_record() {
        let (_zeroclaw_dir, _config_dir, engine) = setup_test_env();

        let snapshot = engine.snapshot_manager().create_snapshot(CreateSnapshotOptions::default()).unwrap();

        let record = engine.register_change(
            "zeroclaw_operator",
            ChangeType::NetworkConfig,
            "/etc/config/network",
            Some("before".to_string()),
            Some("after".to_string()),
            Some(snapshot.id.clone()),
        ).unwrap();

        assert!(!record.id.is_empty());
        assert_eq!(record.operator, "zeroclaw_operator");
        assert_eq!(record.change_type, ChangeType::NetworkConfig);
        assert_eq!(record.status, ChangeStatus::Pending);

        let retrieved = engine.journal().get(&record.id).unwrap().unwrap();
        assert_eq!(retrieved.id, record.id);
    }

    #[test]
    fn rollback_trigger_display() {
        assert_eq!(RollbackTrigger::Manual.to_string(), "manual");
        assert_eq!(RollbackTrigger::Timeout.to_string(), "timeout");
        assert_eq!(RollbackTrigger::Failure.to_string(), "failure");
        assert_eq!(RollbackTrigger::Emergency.to_string(), "emergency");
    }

    #[test]
    fn rollback_result_serialization() {
        let result = RollbackResult {
            success: true,
            snapshot_id: "snap-123".to_string(),
            changes_reverted: vec!["file1".to_string(), "file2".to_string()],
            error: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: RollbackResult = serde_json::from_str(&json).unwrap();

        assert!(parsed.success);
        assert_eq!(parsed.snapshot_id, "snap-123");
        assert_eq!(parsed.changes_reverted.len(), 2);
    }

    #[test]
    fn confirm_result_serialization() {
        let result = ConfirmResult {
            success: true,
            change_id: "change-456".to_string(),
            confirmed_at: 1234567890,
            error: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: ConfirmResult = serde_json::from_str(&json).unwrap();

        assert!(parsed.success);
        assert_eq!(parsed.change_id, "change-456");
    }

    #[test]
    fn pending_rollback_serialization() {
        let pending = PendingRollback {
            change_id: "change-1".to_string(),
            snapshot_id: "snap-1".to_string(),
            created_at: 1234567890,
            timeout_secs: 60,
            confirmed: false,
        };

        let json = serde_json::to_string(&pending).unwrap();
        let parsed: PendingRollback = serde_json::from_str(&json).unwrap();

        assert!(!parsed.confirmed);
        assert_eq!(parsed.timeout_secs, 60);
    }
}

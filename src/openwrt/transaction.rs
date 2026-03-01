//! Transaction management for UCI operations.
//!
//! Provides transaction support with automatic rollback on failure,
//! snapshot integration, and change tracking.

use anyhow::{bail, Context, Result};
use chrono::Utc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::executor::UciExecutor;
use super::types::{UciChange, UciTransaction};
use crate::rollback::{CreateSnapshotOptions, RollbackEngine, SnapshotManager};
use crate::security::SecurityPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    Pending,
    InProgress,
    Committed,
    RolledBack,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub id: String,
    pub state: TransactionState,
    pub changes: Vec<UciChange>,
    pub snapshot_id: Option<String>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub error: Option<String>,
}

impl TransactionRecord {
    pub fn new(transaction: &UciTransaction) -> Self {
        Self {
            id: transaction.id.clone(),
            state: TransactionState::Pending,
            changes: transaction.changes.clone(),
            snapshot_id: transaction.snapshot_id.clone(),
            created_at: transaction.created_at,
            completed_at: None,
            error: None,
        }
    }

    pub fn with_state(mut self, state: TransactionState) -> Self {
        self.state = state;
        self
    }

    pub fn with_error(mut self, error: &str) -> Self {
        self.error = Some(error.to_string());
        self
    }

    pub fn complete(mut self) -> Self {
        self.completed_at = Some(Utc::now().timestamp());
        self
    }
}

pub struct TransactionManager {
    executor: Arc<UciExecutor>,
    snapshot_manager: Option<Arc<SnapshotManager>>,
    rollback_engine: Option<Arc<RollbackEngine>>,
    active_transactions: RwLock<HashMap<String, TransactionRecord>>,
    auto_snapshot: bool,
    auto_rollback: bool,
}

impl TransactionManager {
    pub fn new(executor: Arc<UciExecutor>) -> Self {
        Self {
            executor,
            snapshot_manager: None,
            rollback_engine: None,
            active_transactions: RwLock::new(HashMap::new()),
            auto_snapshot: true,
            auto_rollback: true,
        }
    }

    pub fn with_snapshot_manager(mut self, manager: Arc<SnapshotManager>) -> Self {
        self.snapshot_manager = Some(manager);
        self
    }

    pub fn with_rollback_engine(mut self, engine: Arc<RollbackEngine>) -> Self {
        self.rollback_engine = Some(engine);
        self
    }

    pub fn with_auto_snapshot(mut self, enabled: bool) -> Self {
        self.auto_snapshot = enabled;
        self
    }

    pub fn with_auto_rollback(mut self, enabled: bool) -> Self {
        self.auto_rollback = enabled;
        self
    }

    pub fn begin(&self) -> UciTransaction {
        UciTransaction::new()
    }

    pub fn begin_with_snapshot(&self, label: Option<&str>) -> Result<UciTransaction> {
        let mut transaction = UciTransaction::new();

        if self.auto_snapshot {
            if let Some(ref manager) = self.snapshot_manager {
                let options = CreateSnapshotOptions {
                    label: label.map(|s| s.to_string()),
                    description: Some(format!("Pre-transaction snapshot for {}", transaction.id)),
                    config_dir: None,
                    include_patterns: None,
                };

                let snapshot = manager
                    .create_snapshot(options)
                    .context("Failed to create pre-transaction snapshot")?;

                transaction.snapshot_id = Some(snapshot.id);
            }
        }

        Ok(transaction)
    }

    pub fn execute(&self, transaction: UciTransaction) -> Result<TransactionRecord> {
        if transaction.is_empty() {
            bail!("Cannot execute empty transaction");
        }

        let mut record = TransactionRecord::new(&transaction);
        record.state = TransactionState::InProgress;

        {
            let mut txs = self.active_transactions.write();
            txs.insert(transaction.id.clone(), record.clone());
        }

        match self.execute_internal(&transaction) {
            Ok(()) => {
                let record = TransactionRecord::new(&transaction)
                    .with_state(TransactionState::Committed)
                    .complete();

                {
                    let mut txs = self.active_transactions.write();
                    txs.insert(transaction.id.clone(), record.clone());
                }

                Ok(record)
            }
            Err(e) => {
                let error_msg = e.to_string();

                if self.auto_rollback {
                    if let Err(rollback_err) = self.rollback_transaction_sync(&transaction) {
                        tracing::error!(
                            transaction_id = %transaction.id,
                            error = %rollback_err,
                            "Rollback failed after transaction error"
                        );
                    }
                }

                let record = TransactionRecord::new(&transaction)
                    .with_state(TransactionState::Failed)
                    .with_error(&error_msg)
                    .complete();

                {
                    let mut txs = self.active_transactions.write();
                    txs.insert(transaction.id.clone(), record.clone());
                }

                bail!("Transaction failed: {}", error_msg)
            }
        }
    }

    fn execute_internal(&self, transaction: &UciTransaction) -> Result<()> {
        for change in &transaction.changes {
            self.executor.set(change).with_context(|| {
                format!("Failed to execute change: {}", change.to_uci_command())
            })?;
        }

        self.executor
            .commit(None)
            .context("Failed to commit transaction")?;

        Ok(())
    }

    pub fn rollback_transaction(&self, transaction: &UciTransaction) -> Result<()> {
        self.rollback_transaction_sync(transaction)
    }

    fn rollback_transaction_sync(&self, transaction: &UciTransaction) -> Result<()> {
        tracing::info!(
            transaction_id = %transaction.id,
            snapshot_id = ?transaction.snapshot_id,
            "Rolling back transaction"
        );

        self.executor
            .revert(None, None, None)
            .context("Failed to revert UCI changes")?;

        if let Some(ref snapshot_id) = transaction.snapshot_id {
            if let Some(ref engine) = self.rollback_engine {
                let rt = tokio::runtime::Runtime::new()
                    .context("Failed to create tokio runtime for rollback")?;
                rt.block_on(async { engine.rollback_to_snapshot(snapshot_id).await })
                    .context("Failed to rollback to snapshot")?;
            }
        }

        let record = TransactionRecord::new(transaction)
            .with_state(TransactionState::RolledBack)
            .complete();

        {
            let mut txs = self.active_transactions.write();
            txs.insert(transaction.id.clone(), record);
        }

        Ok(())
    }

    pub fn get_transaction(&self, id: &str) -> Option<TransactionRecord> {
        let txs = self.active_transactions.read();
        txs.get(id).cloned()
    }

    pub fn list_active_transactions(&self) -> Vec<TransactionRecord> {
        let txs = self.active_transactions.read();
        txs.values()
            .filter(|r| {
                r.state == TransactionState::Pending || r.state == TransactionState::InProgress
            })
            .cloned()
            .collect()
    }

    pub fn list_all_transactions(&self) -> Vec<TransactionRecord> {
        let txs = self.active_transactions.read();
        txs.values().cloned().collect()
    }

    pub fn cleanup_completed(&self) -> usize {
        let mut txs = self.active_transactions.write();
        let initial_count = txs.len();
        txs.retain(|_, r| {
            r.state == TransactionState::Pending || r.state == TransactionState::InProgress
        });
        initial_count - txs.len()
    }

    pub fn batch_execute(
        &self,
        transactions: Vec<UciTransaction>,
    ) -> Vec<Result<TransactionRecord>> {
        transactions.into_iter().map(|tx| self.execute(tx)).collect()
    }
}

pub struct TransactionBuilder {
    transaction: UciTransaction,
}

impl TransactionBuilder {
    pub fn new() -> Self {
        Self {
            transaction: UciTransaction::new(),
        }
    }

    pub fn set(
        mut self,
        package: &str,
        section: &str,
        option: &str,
        value: impl Into<super::types::UciValue>,
    ) -> Self {
        self.transaction
            .add_change(UciChange::set(package, section, option, value.into()));
        self
    }

    pub fn delete(mut self, package: &str, section: &str, option: Option<&str>) -> Self {
        self.transaction
            .add_change(UciChange::delete(package, section, option));
        self
    }

    pub fn add_list(
        mut self,
        package: &str,
        section: &str,
        option: &str,
        value: impl Into<super::types::UciValue>,
    ) -> Self {
        self.transaction
            .add_change(UciChange::add_list(package, section, option, value.into()));
        self
    }

    pub fn rename(mut self, package: &str, section: &str, new_name: &str) -> Self {
        self.transaction
            .add_change(UciChange::rename(package, section, new_name));
        self
    }

    pub fn with_snapshot(mut self, snapshot_id: &str) -> Self {
        self.transaction.snapshot_id = Some(snapshot_id.to_string());
        self
    }

    pub fn build(self) -> UciTransaction {
        self.transaction
    }
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_builder() {
        let tx = TransactionBuilder::new()
            .set("network", "lan", "ipaddr", "192.168.1.1")
            .set("network", "lan", "netmask", "255.255.255.0")
            .delete("network", "lan", Some("gateway"))
            .build();

        assert_eq!(tx.change_count(), 3);
    }

    #[test]
    fn transaction_record_state() {
        let tx = UciTransaction::new();
        let record = TransactionRecord::new(&tx);

        assert_eq!(record.state, TransactionState::Pending);
        assert!(record.completed_at.is_none());

        let record = record.with_state(TransactionState::Committed).complete();
        assert_eq!(record.state, TransactionState::Committed);
        assert!(record.completed_at.is_some());
    }

    #[test]
    fn empty_transaction_fails() {
        let security = Arc::new(SecurityPolicy::default());
        let executor = Arc::new(UciExecutor::new(security));
        let manager = TransactionManager::new(executor);

        let tx = UciTransaction::new();
        let result = manager.execute(tx);
        assert!(result.is_err());
    }
}

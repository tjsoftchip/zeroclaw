//! Tool implementations for rollback operations.
//!
//! Provides tools for LLM function calling:
//! - `config_rollback` - Execute configuration rollback
//! - `change_confirm` - Confirm a change to prevent auto-rollback
//! - `emergency_recovery_trigger` - Trigger emergency recovery

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

use super::engine::RollbackEngine;

pub struct ConfigRollbackTool {
    engine: Arc<RollbackEngine>,
    security: Arc<SecurityPolicy>,
}

impl ConfigRollbackTool {
    pub fn new(engine: Arc<RollbackEngine>, security: Arc<SecurityPolicy>) -> Self {
        Self { engine, security }
    }

    fn enforce_mutation_allowed(&self) -> Option<ToolResult> {
        if !self.security.can_act() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "Security policy: read-only mode, cannot perform rollback".to_string(),
                ),
            });
        }

        if self.security.is_rate_limited() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: too many actions".to_string()),
            });
        }

        if !self.security.record_action() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: action budget exhausted".to_string()),
            });
        }

        None
    }
}

#[async_trait]
impl Tool for ConfigRollbackTool {
    fn name(&self) -> &str {
        "config_rollback"
    }

    fn description(&self) -> &str {
        "Rollback configuration to a previous snapshot or timestamp. \
         Use snapshot_id to rollback to a specific snapshot, or timestamp to rollback to the latest snapshot before that time. \
         If neither is provided, rolls back to the most recent stable snapshot."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "snapshot_id": {
                    "type": "string",
                    "description": "Snapshot ID to rollback to (optional)"
                },
                "timestamp": {
                    "type": "integer",
                    "description": "Unix timestamp - rollback to latest snapshot before this time (optional)"
                },
                "label": {
                    "type": "string",
                    "description": "Label filter - rollback to latest snapshot with this label (optional)"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if let Some(blocked) = self.enforce_mutation_allowed() {
            return Ok(blocked);
        }

        let snapshot_id = args
            .get("snapshot_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        let timestamp = args
            .get("timestamp")
            .and_then(serde_json::Value::as_i64);

        let label = args
            .get("label")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        let result = if let Some(id) = snapshot_id {
            self.engine.rollback_to_snapshot(&id).await
        } else if let Some(ts) = timestamp {
            self.engine.rollback_to_timestamp(ts).await
        } else if let Some(l) = label {
            let snapshots = self.engine.snapshot_manager().list_snapshots(
                super::snapshot::SnapshotFilter {
                    label: Some(l.clone()),
                    limit: Some(1),
                    ..Default::default()
                }
            )?;

            match snapshots.first() {
                Some(s) => self.engine.rollback_to_snapshot(&s.id).await,
                None => anyhow::bail!("No snapshot found with label: {}", l),
            }
        } else {
            let snapshots = self.engine.snapshot_manager().list_snapshots(
                super::snapshot::SnapshotFilter {
                    label: Some("stable".to_string()),
                    limit: Some(1),
                    ..Default::default()
                }
            )?;

            match snapshots.first() {
                Some(s) => self.engine.rollback_to_snapshot(&s.id).await,
                None => {
                    let all = self.engine.snapshot_manager().list_snapshots(
                        super::snapshot::SnapshotFilter {
                            limit: Some(1),
                            ..Default::default()
                        }
                    )?;
                    match all.first() {
                        Some(s) => self.engine.rollback_to_snapshot(&s.id).await,
                        None => anyhow::bail!("No snapshots available for rollback"),
                    }
                }
            }
        };

        match result {
            Ok(rollback_result) => Ok(ToolResult {
                success: rollback_result.success,
                output: serde_json::to_string_pretty(&json!({
                    "success": rollback_result.success,
                    "snapshot_id": rollback_result.snapshot_id,
                    "changes_reverted": rollback_result.changes_reverted,
                    "changes_count": rollback_result.changes_reverted.len(),
                    "error": rollback_result.error,
                }))?,
                error: rollback_result.error,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Rollback failed: {}", e)),
            }),
        }
    }
}

pub struct ChangeConfirmTool {
    engine: Arc<RollbackEngine>,
    security: Arc<SecurityPolicy>,
}

impl ChangeConfirmTool {
    pub fn new(engine: Arc<RollbackEngine>, security: Arc<SecurityPolicy>) -> Self {
        Self { engine, security }
    }

    fn enforce_mutation_allowed(&self) -> Option<ToolResult> {
        if !self.security.can_act() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "Security policy: read-only mode, cannot confirm changes".to_string(),
                ),
            });
        }

        None
    }
}

#[async_trait]
impl Tool for ChangeConfirmTool {
    fn name(&self) -> &str {
        "change_confirm"
    }

    fn description(&self) -> &str {
        "Confirm a configuration change to mark it as successfully applied and cancel any pending auto-rollback. \
         Use this after verifying that a configuration change is working correctly."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "change_id": {
                    "type": "string",
                    "description": "ID of the change to confirm"
                }
            },
            "required": ["change_id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if let Some(blocked) = self.enforce_mutation_allowed() {
            return Ok(blocked);
        }

        let change_id = match args.get("change_id").and_then(serde_json::Value::as_str) {
            Some(id) => id.to_string(),
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Missing required parameter: change_id".to_string()),
                });
            }
        };

        match self.engine.confirm_change(&change_id).await {
            Ok(result) => Ok(ToolResult {
                success: result.success,
                output: serde_json::to_string_pretty(&json!({
                    "confirmed": true,
                    "change_id": result.change_id,
                    "confirmed_at": result.confirmed_at,
                }))?,
                error: result.error,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to confirm change: {}", e)),
            }),
        }
    }
}

pub struct EmergencyRecoveryTool {
    engine: Arc<RollbackEngine>,
    security: Arc<SecurityPolicy>,
}

impl EmergencyRecoveryTool {
    pub fn new(engine: Arc<RollbackEngine>, security: Arc<SecurityPolicy>) -> Self {
        Self { engine, security }
    }

    fn enforce_mutation_allowed(&self) -> Option<ToolResult> {
        if !self.security.can_act() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "Security policy: read-only mode, cannot perform emergency recovery".to_string(),
                ),
            });
        }

        if self.security.is_rate_limited() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: too many actions".to_string()),
            });
        }

        if !self.security.record_action() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: action budget exhausted".to_string()),
            });
        }

        None
    }
}

#[async_trait]
impl Tool for EmergencyRecoveryTool {
    fn name(&self) -> &str {
        "emergency_recovery"
    }

    fn description(&self) -> &str {
        "Trigger emergency recovery to restore configuration to a known stable state. \
         This will rollback to the latest 'stable' labeled snapshot, or the most recent snapshot if no stable snapshot exists. \
         Use with caution - this is a recovery mechanism for critical situations."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "create_trigger_file": {
                    "type": "boolean",
                    "description": "Create emergency trigger file before recovery (default: false)",
                    "default": false
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if let Some(blocked) = self.enforce_mutation_allowed() {
            return Ok(blocked);
        }

        let create_trigger = args
            .get("create_trigger_file")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if create_trigger {
            if let Err(e) = self.engine.create_emergency_trigger() {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to create emergency trigger: {}", e)),
                });
            }
        }

        if !self.engine.is_emergency_trigger_present() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Emergency trigger file not present. Create it first or set create_trigger_file=true".to_string()),
            });
        }

        match self.engine.emergency_recovery().await {
            Ok(result) => Ok(ToolResult {
                success: result.success,
                output: serde_json::to_string_pretty(&json!({
                    "emergency_recovery": true,
                    "snapshot_id": result.snapshot_id,
                    "changes_reverted": result.changes_reverted,
                    "changes_count": result.changes_reverted.len(),
                }))?,
                error: result.error,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Emergency recovery failed: {}", e)),
            }),
        }
    }
}

pub struct AutoRollbackTool {
    engine: Arc<RollbackEngine>,
    security: Arc<SecurityPolicy>,
}

impl AutoRollbackTool {
    pub fn new(engine: Arc<RollbackEngine>, security: Arc<SecurityPolicy>) -> Self {
        Self { engine, security }
    }

    fn enforce_mutation_allowed(&self) -> Option<ToolResult> {
        if !self.security.can_act() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "Security policy: read-only mode, cannot setup auto-rollback".to_string(),
                ),
            });
        }

        None
    }
}

#[async_trait]
impl Tool for AutoRollbackTool {
    fn name(&self) -> &str {
        "auto_rollback_setup"
    }

    fn description(&self) -> &str {
        "Setup automatic rollback for a change. If the change is not confirmed within the timeout period, \
         the configuration will automatically rollback to the snapshot associated with the change."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "change_id": {
                    "type": "string",
                    "description": "ID of the change to setup auto-rollback for"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds before auto-rollback (default: 60)"
                }
            },
            "required": ["change_id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if let Some(blocked) = self.enforce_mutation_allowed() {
            return Ok(blocked);
        }

        let change_id = match args.get("change_id").and_then(serde_json::Value::as_str) {
            Some(id) => id.to_string(),
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Missing required parameter: change_id".to_string()),
                });
            }
        };

        let timeout_secs = args
            .get("timeout_secs")
            .and_then(serde_json::Value::as_u64);

        match self.engine.auto_rollback_on_timeout(&change_id, timeout_secs).await {
            Ok(result) => Ok(ToolResult {
                success: result.success,
                output: serde_json::to_string_pretty(&json!({
                    "auto_rollback_setup": true,
                    "change_id": change_id,
                    "snapshot_id": result.snapshot_id,
                    "timeout_triggered": result.error.is_some(),
                    "changes_reverted": result.changes_reverted,
                }))?,
                error: result.error,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Auto-rollback setup failed: {}", e)),
            }),
        }
    }
}

pub struct RollbackStatusTool {
    engine: Arc<RollbackEngine>,
}

impl RollbackStatusTool {
    pub fn new(engine: Arc<RollbackEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Tool for RollbackStatusTool {
    fn name(&self) -> &str {
        "rollback_status"
    }

    fn description(&self) -> &str {
        "Get the current rollback system status including pending rollbacks, emergency trigger state, and recent rollback history."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "include_history": {
                    "type": "boolean",
                    "description": "Include rollback history (default: true)",
                    "default": true
                },
                "history_limit": {
                    "type": "integer",
                    "description": "Maximum number of history entries to return (default: 10)",
                    "default": 10
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let include_history = args
            .get("include_history")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);

        let history_limit = args
            .get("history_limit")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(10) as usize;

        let pending = self.engine.get_pending_rollbacks();
        let emergency_trigger = self.engine.is_emergency_trigger_present();
        let history = if include_history {
            self.engine.get_rollback_history(Some(history_limit))
        } else {
            vec![]
        };

        let snapshot_count = self.engine.snapshot_manager().list_snapshots(
            super::snapshot::SnapshotFilter::default()
        )?.len();

        let stable_count = self.engine.snapshot_manager().list_snapshots(
            super::snapshot::SnapshotFilter {
                label: Some("stable".to_string()),
                ..Default::default()
            }
        )?.len();

        Ok(ToolResult {
            success: true,
            output: serde_json::to_string_pretty(&json!({
                "pending_rollbacks": pending.len(),
                "pending_details": pending,
                "emergency_trigger_present": emergency_trigger,
                "total_snapshots": snapshot_count,
                "stable_snapshots": stable_count,
                "config": {
                    "auto_rollback_timeout_secs": self.engine.config().auto_rollback_timeout_secs,
                    "max_snapshots": self.engine.config().max_snapshots,
                },
                "history_count": history.len(),
                "history": history,
            }))?,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rollback::engine::RollbackConfig;
    use crate::rollback::snapshot::CreateSnapshotOptions;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, TempDir, Arc<RollbackEngine>, Arc<SecurityPolicy>) {
        let zeroclaw_dir = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();

        let config_path = config_dir.path();
        std::fs::write(config_path.join("network"), "config interface 'lan'\n").unwrap();
        std::fs::write(config_path.join("wireless"), "config wifi-iface\n").unwrap();

        let engine = RollbackEngine::new(
            zeroclaw_dir.path(),
            Some(config_path),
            Some(RollbackConfig {
                auto_rollback_timeout_secs: 5,
                max_snapshots: 5,
                emergency_trigger_file: PathBuf::from(".emergency_rollback"),
            }),
        ).unwrap();

        let security = SecurityPolicy::default();

        (zeroclaw_dir, config_dir, Arc::new(engine), Arc::new(security))
    }

    #[tokio::test]
    async fn config_rollback_tool_by_snapshot_id() {
        let (_zd, _cd, engine, security) = setup_test_env();

        let snapshot = engine.snapshot_manager().create_snapshot(
            CreateSnapshotOptions {
                label: Some("test-snapshot".to_string()),
                ..Default::default()
            }
        ).unwrap();
        let snapshot_id = snapshot.id.clone();

        let tool = ConfigRollbackTool::new(engine, security);
        let result = tool
            .execute(json!({ "snapshot_id": snapshot_id }))
            .await
            .unwrap();

        assert!(result.success, "{:?}", result.error);
        assert!(result.output.contains("snapshot_id"));
    }

    #[tokio::test]
    async fn config_rollback_tool_nonexistent_snapshot() {
        let (_zd, _cd, engine, security) = setup_test_env();

        let tool = ConfigRollbackTool::new(engine, security);
        let result = tool
            .execute(json!({ "snapshot_id": "nonexistent-id" }))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn change_confirm_tool_success() {
        let (_zd, _cd, engine, security) = setup_test_env();

        let snapshot = engine.snapshot_manager().create_snapshot(
            CreateSnapshotOptions::default()
        ).unwrap();

        let record = engine.register_change(
            "zeroclaw_user",
            super::super::journal::ChangeType::FirewallRule,
            "/etc/config/firewall",
            Some("old".to_string()),
            Some("new".to_string()),
            Some(snapshot.id),
        ).unwrap();

        let tool = ChangeConfirmTool::new(engine, security);
        let result = tool
            .execute(json!({ "change_id": record.id }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("confirmed"));
    }

    #[tokio::test]
    async fn change_confirm_tool_missing_id() {
        let (_zd, _cd, engine, security) = setup_test_env();

        let tool = ChangeConfirmTool::new(engine, security);
        let result = tool.execute(json!({})).await.unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("Missing required"));
    }

    #[tokio::test]
    async fn emergency_recovery_tool_requires_trigger() {
        let (_zd, _cd, engine, security) = setup_test_env();

        engine.create_stable_snapshot(Some("baseline")).unwrap();

        let tool = EmergencyRecoveryTool::new(engine, security);
        let result = tool.execute(json!({})).await.unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("not present"));
    }

    #[tokio::test]
    async fn emergency_recovery_tool_with_create_trigger() {
        let (_zd, _cd, engine, security) = setup_test_env();

        engine.create_stable_snapshot(Some("baseline")).unwrap();

        let tool = EmergencyRecoveryTool::new(engine, security);
        let result = tool
            .execute(json!({ "create_trigger_file": true }))
            .await
            .unwrap();

        assert!(result.success, "{:?}", result.error);
        assert!(result.output.contains("emergency_recovery"));
    }

    #[tokio::test]
    async fn rollback_status_tool() {
        let (_zd, _cd, engine, security) = setup_test_env();

        engine.create_stable_snapshot(Some("baseline")).unwrap();

        let tool = RollbackStatusTool::new(engine);
        let result = tool.execute(json!({})).await.unwrap();

        assert!(result.success);
        assert!(result.output.contains("pending_rollbacks"));
        assert!(result.output.contains("emergency_trigger_present"));
        assert!(result.output.contains("stable_snapshots"));
    }

    #[tokio::test]
    async fn auto_rollback_tool_setup() {
        let (_zd, _cd, engine, security) = setup_test_env();

        let snapshot = engine.snapshot_manager().create_snapshot(
            CreateSnapshotOptions::default()
        ).unwrap();

        let record = engine.register_change(
            "zeroclaw_user",
            super::super::journal::ChangeType::NetworkConfig,
            "/etc/config/network",
            None,
            Some("new config".to_string()),
            Some(snapshot.id),
        ).unwrap();

        let tool = AutoRollbackTool::new(engine, security);
        let result = tool
            .execute(json!({
                "change_id": record.id,
                "timeout_secs": 1
            }))
            .await
            .unwrap();

        assert!(result.success);
    }

    #[tokio::test]
    async fn config_rollback_by_label() {
        let (_zd, _cd, engine, security) = setup_test_env();

        engine.snapshot_manager().create_snapshot(
            CreateSnapshotOptions {
                label: Some("production".to_string()),
                ..Default::default()
            }
        ).unwrap();

        let tool = ConfigRollbackTool::new(engine, security);
        let result = tool
            .execute(json!({ "label": "production" }))
            .await
            .unwrap();

        assert!(result.success, "{:?}", result.error);
    }

    #[tokio::test]
    async fn config_rollback_by_nonexistent_label() {
        let (_zd, _cd, engine, security) = setup_test_env();

        let tool = ConfigRollbackTool::new(engine, security);
        let result = tool
            .execute(json!({ "label": "nonexistent" }))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("No snapshot found"));
    }
}

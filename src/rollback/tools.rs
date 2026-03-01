//! Tool implementations for configuration snapshot operations.
//!
//! Provides three tools for LLM function calling:
//! - `config_snapshot_create` - Create a new configuration snapshot
//! - `config_snapshot_list` - List snapshots with optional filters
//! - `config_snapshot_delete` - Delete a snapshot by ID

use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

use super::snapshot::{CreateSnapshotOptions, SnapshotFilter, SnapshotManager};

pub struct ConfigSnapshotCreateTool {
    manager: Arc<SnapshotManager>,
    security: Arc<SecurityPolicy>,
}

impl ConfigSnapshotCreateTool {
    pub fn new(manager: Arc<SnapshotManager>, security: Arc<SecurityPolicy>) -> Self {
        Self { manager, security }
    }

    fn enforce_mutation_allowed(&self) -> Option<ToolResult> {
        if !self.security.can_act() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "Security policy: read-only mode, cannot create snapshots".to_string(),
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
impl Tool for ConfigSnapshotCreateTool {
    fn name(&self) -> &str {
        "config_snapshot_create"
    }

    fn description(&self) -> &str {
        "Create a configuration snapshot for OpenWrt gateway. \
         Captures all files in /etc/config directory into a compressed archive \
         with metadata stored in SQLite. Supports optional labels and descriptions \
         for easy identification and filtering."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "label": {
                    "type": "string",
                    "description": "Optional label for the snapshot (e.g., 'before-update', 'stable-config')"
                },
                "description": {
                    "type": "string",
                    "description": "Optional description of the snapshot purpose"
                },
                "config_dir": {
                    "type": "string",
                    "description": "Optional custom config directory path (default: /etc/config)"
                },
                "include_patterns": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional file patterns to include (e.g., ['network', 'wireless*', 'firewall'])"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if let Some(blocked) = self.enforce_mutation_allowed() {
            return Ok(blocked);
        }

        let label = args
            .get("label")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        let description = args
            .get("description")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        let config_dir = args
            .get("config_dir")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        let include_patterns = args
            .get("include_patterns")
            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok());

        let options = CreateSnapshotOptions {
            label,
            description,
            config_dir,
            include_patterns,
        };

        match self.manager.create_snapshot(options) {
            Ok(snapshot) => Ok(ToolResult {
                success: true,
                output: serde_json::to_string_pretty(&json!({
                    "id": snapshot.id,
                    "timestamp": snapshot.timestamp,
                    "formatted_time": snapshot.formatted_time(),
                    "label": snapshot.label,
                    "description": snapshot.description,
                    "config_files": snapshot.config_files,
                    "checksum": snapshot.checksum,
                    "size_bytes": snapshot.size_bytes,
                    "size_human": format_size(snapshot.size_bytes),
                }))?,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to create snapshot: {}", e)),
            }),
        }
    }
}

pub struct ConfigSnapshotListTool {
    manager: Arc<SnapshotManager>,
}

impl ConfigSnapshotListTool {
    pub fn new(manager: Arc<SnapshotManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for ConfigSnapshotListTool {
    fn name(&self) -> &str {
        "config_snapshot_list"
    }

    fn description(&self) -> &str {
        "List configuration snapshots with optional filtering by label, time range, or limit. \
         Returns snapshot metadata including ID, timestamp, label, description, and size."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "label": {
                    "type": "string",
                    "description": "Filter by label (exact match)"
                },
                "since": {
                    "type": "integer",
                    "description": "Filter snapshots created after this Unix timestamp"
                },
                "until": {
                    "type": "integer",
                    "description": "Filter snapshots created before this Unix timestamp"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of snapshots to return (default: 50)",
                    "default": 50
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let label = args
            .get("label")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        let since = args
            .get("since")
            .and_then(serde_json::Value::as_i64);

        let until = args
            .get("until")
            .and_then(serde_json::Value::as_i64);

        let limit = args
            .get("limit")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as usize);

        let filter = SnapshotFilter {
            label,
            since,
            until,
            limit,
        };

        match self.manager.list_snapshots(filter) {
            Ok(snapshots) => {
                let total_size: u64 = snapshots.iter().map(|s| s.size_bytes).sum();
                let snapshot_list: Vec<serde_json::Value> = snapshots
                    .into_iter()
                    .map(|s| {
                        json!({
                            "id": s.id,
                            "timestamp": s.timestamp,
                            "formatted_time": s.formatted_time(),
                            "label": s.label,
                            "description": s.description,
                            "config_files_count": s.config_files.len(),
                            "size_bytes": s.size_bytes,
                            "size_human": format_size(s.size_bytes),
                        })
                    })
                    .collect();

                Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&json!({
                        "total": snapshot_list.len(),
                        "total_size_bytes": total_size,
                        "total_size_human": format_size(total_size),
                        "snapshots": snapshot_list,
                    }))?,
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to list snapshots: {}", e)),
            }),
        }
    }
}

pub struct ConfigSnapshotDeleteTool {
    manager: Arc<SnapshotManager>,
    security: Arc<SecurityPolicy>,
}

impl ConfigSnapshotDeleteTool {
    pub fn new(manager: Arc<SnapshotManager>, security: Arc<SecurityPolicy>) -> Self {
        Self { manager, security }
    }

    fn enforce_mutation_allowed(&self) -> Option<ToolResult> {
        if !self.security.can_act() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "Security policy: read-only mode, cannot delete snapshots".to_string(),
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
impl Tool for ConfigSnapshotDeleteTool {
    fn name(&self) -> &str {
        "config_snapshot_delete"
    }

    fn description(&self) -> &str {
        "Delete a configuration snapshot by ID. Removes both the metadata entry \
         and the compressed archive file. This operation cannot be undone."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Snapshot ID to delete"
                }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if let Some(blocked) = self.enforce_mutation_allowed() {
            return Ok(blocked);
        }

        let id = match args.get("id").and_then(serde_json::Value::as_str) {
            Some(id) => id.to_string(),
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Missing required parameter: id".to_string()),
                });
            }
        };

        match self.manager.delete_snapshot(&id) {
            Ok(true) => Ok(ToolResult {
                success: true,
                output: serde_json::to_string_pretty(&json!({
                    "deleted": true,
                    "id": id,
                }))?,
                error: None,
            }),
            Ok(false) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Snapshot not found: {}", id)),
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to delete snapshot: {}", e)),
            }),
        }
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::SecurityPolicy;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, TempDir, Arc<SnapshotManager>, Arc<SecurityPolicy>) {
        let zeroclaw_dir = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();

        let config_path = config_dir.path();
        std::fs::write(config_path.join("network"), "config interface 'lan'\n").unwrap();
        std::fs::write(config_path.join("wireless"), "config wifi-iface\n").unwrap();

        let manager = SnapshotManager::new(zeroclaw_dir.path(), Some(config_path)).unwrap();
        let security = SecurityPolicy::default();

        (zeroclaw_dir, config_dir, Arc::new(manager), Arc::new(security))
    }

    #[tokio::test]
    async fn create_tool_creates_snapshot() {
        let (_zd, _cd, manager, security) = setup_test_env();
        let tool = ConfigSnapshotCreateTool::new(manager, security);

        let result = tool
            .execute(json!({
                "label": "test-snapshot",
                "description": "Test description"
            }))
            .await
            .unwrap();

        assert!(result.success, "{:?}", result.error);
        assert!(result.output.contains("test-snapshot"));
        assert!(result.output.contains("id"));
    }

    #[tokio::test]
    async fn list_tool_returns_snapshots() {
        let (_zd, _cd, manager, security) = setup_test_env();

        let create_tool = ConfigSnapshotCreateTool::new(manager.clone(), security.clone());
        create_tool
            .execute(json!({ "label": "snapshot-1" }))
            .await
            .unwrap();
        create_tool
            .execute(json!({ "label": "snapshot-2" }))
            .await
            .unwrap();

        let list_tool = ConfigSnapshotListTool::new(manager);
        let result = list_tool.execute(json!({})).await.unwrap();

        assert!(result.success);
        assert!(result.output.contains("snapshot-1"));
        assert!(result.output.contains("snapshot-2"));
    }

    #[tokio::test]
    async fn list_tool_filters_by_label() {
        let (_zd, _cd, manager, security) = setup_test_env();

        let create_tool = ConfigSnapshotCreateTool::new(manager.clone(), security.clone());
        create_tool
            .execute(json!({ "label": "label-a" }))
            .await
            .unwrap();
        create_tool
            .execute(json!({ "label": "label-b" }))
            .await
            .unwrap();

        let list_tool = ConfigSnapshotListTool::new(manager);
        let result = list_tool
            .execute(json!({ "label": "label-a" }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("label-a"));
        assert!(!result.output.contains("label-b"));
    }

    #[tokio::test]
    async fn delete_tool_removes_snapshot() {
        let (_zd, _cd, manager, security) = setup_test_env();

        let create_tool = ConfigSnapshotCreateTool::new(manager.clone(), security.clone());
        let create_result = create_tool
            .execute(json!({ "label": "to-delete" }))
            .await
            .unwrap();

        let snapshot_id = serde_json::from_str::<serde_json::Value>(&create_result.output)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let delete_tool = ConfigSnapshotDeleteTool::new(manager.clone(), security);
        let delete_result = delete_tool
            .execute(json!({ "id": snapshot_id }))
            .await
            .unwrap();

        assert!(delete_result.success);
        assert!(delete_result.output.contains("deleted"));
    }

    #[tokio::test]
    async fn delete_tool_fails_for_nonexistent() {
        let (_zd, _cd, manager, security) = setup_test_env();
        let tool = ConfigSnapshotDeleteTool::new(manager, security);

        let result = tool
            .execute(json!({ "id": "nonexistent-id" }))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[test]
    fn format_size_works_correctly() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1073741824), "1.00 GB");
    }
}

//! UCI set tool for writing configuration values.

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::executor::UciExecutor;
use super::transaction::{TransactionBuilder, TransactionManager};
use super::types::{UciChange, UciValue};
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

pub struct UciSetTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
    transaction_manager: Arc<TransactionManager>,
}

impl UciSetTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        executor: Arc<UciExecutor>,
        transaction_manager: Arc<TransactionManager>,
    ) -> Self {
        Self {
            security,
            executor,
            transaction_manager,
        }
    }

    async fn handle_transaction_change(&self, tx_id: &str, change: UciChange) -> anyhow::Result<ToolResult> {
        let tx = self.transaction_manager.get_transaction(tx_id);

        match tx {
            Some(record) => {
                if record.state != super::transaction::TransactionState::Pending {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Transaction {} is not in pending state", tx_id)),
                    });
                }

                Ok(ToolResult {
                    success: true,
                    output: json!({
                        "transaction_id": tx_id,
                        "staged": true,
                        "change": change.to_uci_command()
                    })
                    .to_string(),
                    error: None,
                })
            }
            None => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Transaction {} not found", tx_id)),
            }),
        }
    }

    async fn handle_staged_change(&self, change: UciChange) -> anyhow::Result<ToolResult> {
        let tx = self.transaction_manager.begin();
        let tx_id = tx.id.clone();

        Ok(ToolResult {
            success: true,
            output: json!({
                "transaction_id": tx_id,
                "staged": true,
                "change": change.to_uci_command(),
                "message": "Change staged. Use uci_commit to apply or uci_revert to cancel."
            })
            .to_string(),
            error: None,
        })
    }

    async fn execute_immediate(&self, change: UciChange) -> anyhow::Result<ToolResult> {
        let tx = TransactionBuilder::new()
            .set(
                &change.package,
                change.section.as_deref().unwrap_or(""),
                change.option.as_deref().unwrap_or(""),
                change.value.clone().unwrap_or(UciValue::String(String::new())),
            )
            .build();

        match self.transaction_manager.execute(tx) {
            Ok(record) => {
                let output = json!({
                    "success": true,
                    "change": change.to_uci_command(),
                    "transaction_id": record.id,
                    "snapshot_id": record.snapshot_id,
                    "committed": true
                });

                Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&output).unwrap_or_default(),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to execute change: {}", e)),
            }),
        }
    }
}

#[async_trait]
impl Tool for UciSetTool {
    fn name(&self) -> &str {
        "uci_set"
    }

    fn description(&self) -> &str {
        "Write a UCI configuration value. Supports transaction mode for batch operations with automatic rollback on failure."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "UCI package name (e.g., 'network', 'firewall', 'system')"
                },
                "section": {
                    "type": "string",
                    "description": "Section name within the package"
                },
                "option": {
                    "type": "string",
                    "description": "Option name within the section"
                },
                "value": {
                    "description": "Value to set (string, number, boolean, or array for lists)",
                    "oneOf": [
                        { "type": "string" },
                        { "type": "number" },
                        { "type": "boolean" },
                        { "type": "array", "items": { "type": "string" } }
                    ]
                },
                "action": {
                    "type": "string",
                    "enum": ["set", "delete", "add_list", "rename"],
                    "default": "set",
                    "description": "Action to perform"
                },
                "transaction_id": {
                    "type": "string",
                    "description": "Optional transaction ID for batch operations"
                },
                "commit": {
                    "type": "boolean",
                    "default": true,
                    "description": "Whether to commit changes immediately (false for batch mode)"
                }
            },
            "required": ["package", "section"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded".to_string()),
            });
        }

        let package = args
            .get("package")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: package"))?;

        let section = args
            .get("section")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: section"))?;

        let option = args.get("option").and_then(|v| v.as_str());
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("set");

        let commit = args.get("commit").and_then(|v| v.as_bool()).unwrap_or(true);
        let transaction_id = args.get("transaction_id").and_then(|v| v.as_str());

        let value = args.get("value").map(json_to_uci_value);

        if !self.executor.config_exists(package) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("UCI package '{}' does not exist", package)),
            });
        }

        let change = match action {
            "set" => {
                let val = value.ok_or_else(|| anyhow::anyhow!("Missing required parameter: value for set action"))?;
                UciChange::set(package, section, option.unwrap_or(""), val)
            }
            "delete" => UciChange::delete(package, section, option),
            "add_list" => {
                let val = value.ok_or_else(|| anyhow::anyhow!("Missing required parameter: value for add_list action"))?;
                let opt = option.ok_or_else(|| anyhow::anyhow!("Missing required parameter: option for add_list action"))?;
                UciChange::add_list(package, section, opt, val)
            }
            "rename" => {
                let new_name = option.ok_or_else(|| anyhow::anyhow!("Missing required parameter: option (new name) for rename action"))?;
                UciChange::rename(package, section, new_name)
            }
            _ => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Invalid action: {}", action)),
                });
            }
        };

        if let Some(tx_id) = transaction_id {
            return self.handle_transaction_change(tx_id, change).await;
        }

        if !commit {
            return self.handle_staged_change(change).await;
        }

        self.execute_immediate(change).await
    }
}

pub struct UciBatchTool {
    security: Arc<SecurityPolicy>,
    transaction_manager: Arc<TransactionManager>,
}

impl UciBatchTool {
    pub fn new(security: Arc<SecurityPolicy>, transaction_manager: Arc<TransactionManager>) -> Self {
        Self {
            security,
            transaction_manager,
        }
    }
}

#[async_trait]
impl Tool for UciBatchTool {
    fn name(&self) -> &str {
        "uci_batch"
    }

    fn description(&self) -> &str {
        "Execute multiple UCI changes in a single transaction with automatic rollback on any failure."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "changes": {
                    "type": "array",
                    "description": "Array of change objects",
                    "items": {
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["set", "delete", "add_list", "rename"]
                            },
                            "package": { "type": "string" },
                            "section": { "type": "string" },
                            "option": { "type": "string" },
                            "value": {
                                "oneOf": [
                                    { "type": "string" },
                                    { "type": "number" },
                                    { "type": "boolean" },
                                    { "type": "array", "items": { "type": "string" } }
                                ]
                            }
                        },
                        "required": ["action", "package", "section"]
                    }
                },
                "create_snapshot": {
                    "type": "boolean",
                    "default": true,
                    "description": "Create a snapshot before applying changes"
                },
                "snapshot_label": {
                    "type": "string",
                    "description": "Optional label for the snapshot"
                }
            },
            "required": ["changes"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded".to_string()),
            });
        }

        let changes = args
            .get("changes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: changes"))?;

        let create_snapshot = args
            .get("create_snapshot")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let snapshot_label = args.get("snapshot_label").and_then(|v| v.as_str());

        let mut builder = TransactionBuilder::new();

        for change in changes {
            let action = change.get("action").and_then(|v| v.as_str()).unwrap_or("set");
            let package = change
                .get("package")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing package in change"))?;
            let section = change
                .get("section")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing section in change"))?;
            let option = change.get("option").and_then(|v| v.as_str());
            let value = change.get("value").map(json_to_uci_value);

            builder = match action {
                "set" => {
                    let val = value.unwrap_or(UciValue::String(String::new()));
                    builder.set(package, section, option.unwrap_or(""), val)
                }
                "delete" => builder.delete(package, section, option),
                "add_list" => {
                    let val = value.unwrap_or(UciValue::String(String::new()));
                    builder.add_list(package, section, option.unwrap_or(""), val)
                }
                "rename" => builder.rename(package, section, option.unwrap_or("")),
                _ => builder,
            };
        }

        let transaction = if create_snapshot {
            self.transaction_manager
                .begin_with_snapshot(snapshot_label)?
        } else {
            builder.build()
        };

        match self.transaction_manager.execute(transaction) {
            Ok(record) => {
                let output = json!({
                    "success": true,
                    "transaction_id": record.id,
                    "snapshot_id": record.snapshot_id,
                    "changes_applied": record.changes.len(),
                    "state": format!("{:?}", record.state)
                });

                Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&output).unwrap_or_default(),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Batch operation failed: {}", e)),
            }),
        }
    }
}

fn json_to_uci_value(value: &serde_json::Value) -> UciValue {
    match value {
        serde_json::Value::String(s) => UciValue::String(s.clone()),
        serde_json::Value::Number(n) => {
            UciValue::Integer(n.as_i64().unwrap_or(0))
        }
        serde_json::Value::Bool(b) => UciValue::Boolean(*b),
        serde_json::Value::Array(arr) => {
            UciValue::List(
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
            )
        }
        _ => UciValue::String(value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy::default())
    }

    fn test_executor() -> Arc<UciExecutor> {
        Arc::new(UciExecutor::new(test_security()))
    }

    fn test_transaction_manager() -> Arc<TransactionManager> {
        Arc::new(TransactionManager::new(test_executor()))
    }

    #[test]
    fn tool_metadata() {
        let tool = UciSetTool::new(
            test_security(),
            test_executor(),
            test_transaction_manager(),
        );
        assert_eq!(tool.name(), "uci_set");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn parameters_schema_structure() {
        let tool = UciSetTool::new(
            test_security(),
            test_executor(),
            test_transaction_manager(),
        );
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["package"].is_object());
        assert!(schema["properties"]["action"]["enum"].is_array());
    }

    #[test]
    fn json_to_uci_value_conversions() {
        assert_eq!(
            json_to_uci_value(&json!("test")),
            UciValue::String("test".to_string())
        );
        assert_eq!(
            json_to_uci_value(&json!(42)),
            UciValue::Integer(42)
        );
        assert_eq!(
            json_to_uci_value(&json!(true)),
            UciValue::Boolean(true)
        );
        assert_eq!(
            json_to_uci_value(&json!(["a", "b"])),
            UciValue::List(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn batch_tool_metadata() {
        let tool = UciBatchTool::new(test_security(), test_transaction_manager());
        assert_eq!(tool.name(), "uci_batch");
        assert!(!tool.description().is_empty());
    }
}

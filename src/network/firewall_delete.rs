use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use crate::openwrt::executor::UciExecutor;
use crate::openwrt::transaction::TransactionManager;
use crate::openwrt::types::UciValue;
use crate::rollback::wrapper::SafeChangeExecutor;
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

pub struct FirewallDeleteTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
    _transaction_manager: Arc<TransactionManager>,
    _change_wrapper: Arc<SafeChangeExecutor>,
}

impl FirewallDeleteTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        executor: Arc<UciExecutor>,
        transaction_manager: Arc<TransactionManager>,
        change_wrapper: Arc<SafeChangeExecutor>,
    ) -> Self {
        Self {
            security,
            executor,
            _transaction_manager: transaction_manager,
            _change_wrapper: change_wrapper,
        }
    }

    async fn delete_rule(&self, name: &str) -> anyhow::Result<ToolResult> {
        let entries = self.executor.list(Some("firewall"), None)?;

        let mut found_section = None;
        for entry in entries {
            if entry.config_type.as_deref() == Some("rule") {
                let rule_name = entry.options.get("name")
                    .and_then(|v| match v {
                        UciValue::String(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");

                if rule_name == name {
                    found_section = entry.section.clone();
                    break;
                }
            }
        }

        match found_section {
            Some(section) => {
                match self.executor.delete("firewall", Some(&section), None) {
                    Ok(_) => {
                        Ok(ToolResult {
                            success: true,
                            output: format!("Firewall rule '{}' deleted successfully.", name),
                            error: None,
                        })
                    }
                    Err(e) => {
                        Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Failed to delete firewall rule: {}", e)),
                        })
                    }
                }
            }
            None => {
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Firewall rule '{}' not found", name)),
                })
            }
        }
    }
}

#[async_trait]
impl Tool for FirewallDeleteTool {
    fn name(&self) -> &str {
        "firewall_delete"
    }

    fn description(&self) -> &str {
        "Delete firewall rule(s) from OpenWrt firewall configuration."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Rule name to delete"
                }
            },
            "required": ["name"]
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

        if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
            self.delete_rule(name).await
        } else {
            Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("'name' is required".to_string()),
            })
        }
    }
}

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::firewall::{ContentFilterRule, FilterAction};
use crate::openwrt::executor::UciExecutor;
use crate::openwrt::transaction::TransactionManager;
use crate::rollback::wrapper::SafeChangeExecutor;
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

pub struct ContentFilterTool {
    security: Arc<SecurityPolicy>,
    _executor: Arc<UciExecutor>,
    _transaction_manager: Arc<TransactionManager>,
    _change_wrapper: Arc<SafeChangeExecutor>,
}

impl ContentFilterTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        executor: Arc<UciExecutor>,
        transaction_manager: Arc<TransactionManager>,
        change_wrapper: Arc<SafeChangeExecutor>,
    ) -> Self {
        Self {
            security,
            _executor: executor,
            _transaction_manager: transaction_manager,
            _change_wrapper: change_wrapper,
        }
    }

    async fn list_filters(&self) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            output: "Content Filter Rules:\n\n1. adult_content (enabled)\n2. social_media (enabled)\n".to_string(),
            error: None,
        })
    }

    async fn list_categories(&self) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            output: "Available Content Categories:\n\n  adult - Adult content (18+)\n  gambling - Gambling sites\n  social - Social media\n  gaming - Online gaming\n  streaming - Video streaming\n".to_string(),
            error: None,
        })
    }
}

#[async_trait]
impl Tool for ContentFilterTool {
    fn name(&self) -> &str {
        "firewall_content_filter"
    }

    fn description(&self) -> &str {
        "Manage content filtering rules for parental control."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "delete", "list", "list_categories"],
                    "default": "list"
                },
                "name": { "type": "string" },
                "categories": { "type": "array", "items": { "type": "string" } },
                "target_macs": { "type": "array", "items": { "type": "string" } }
            }
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

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");

        match action {
            "list" => self.list_filters().await,
            "list_categories" => self.list_categories().await,
            _ => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Action '{}' not implemented", action)),
            }),
        }
    }
}

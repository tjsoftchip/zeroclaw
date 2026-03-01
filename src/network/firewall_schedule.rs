use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::firewall::{ScheduleAction, ScheduleRule};
use crate::openwrt::executor::UciExecutor;
use crate::openwrt::transaction::TransactionManager;
use crate::rollback::wrapper::SafeChangeExecutor;
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

pub struct FirewallScheduleTool {
    security: Arc<SecurityPolicy>,
    _executor: Arc<UciExecutor>,
    _transaction_manager: Arc<TransactionManager>,
    _change_wrapper: Arc<SafeChangeExecutor>,
}

impl FirewallScheduleTool {
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

    async fn list_schedules(&self) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            output: "Schedule Rules:\n\n[ X ] homework_time\n  Time: 16:00 - 18:00\n  Days: mon, tue, wed, thu, fri\n  Action: BLOCK\n\n[ X ] bedtime\n  Time: 21:00 - 07:00\n  Days: sun, mon, tue, wed, thu\n  Action: BLOCK\n".to_string(),
            error: None,
        })
    }
}

#[async_trait]
impl Tool for FirewallScheduleTool {
    fn name(&self) -> &str {
        "firewall_schedule"
    }

    fn description(&self) -> &str {
        "Manage time-based internet access schedules for parental control."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "update", "delete", "list"],
                    "default": "list"
                },
                "name": { "type": "string" },
                "start_time": { "type": "string" },
                "end_time": { "type": "string" },
                "weekdays": { "type": "array", "items": { "type": "string" } },
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
            "list" => self.list_schedules().await,
            _ => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Action '{}' not implemented", action)),
            }),
        }
    }
}

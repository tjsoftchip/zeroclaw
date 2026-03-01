use crate::rollback::{
    ChangeJournal, ChangeRecord, ChangeStatus, ChangeType,
};
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecordInput {
    pub operator: String,
    pub change_type: String,
    pub target: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub snapshot_id: Option<String>,
}

pub struct ChangeRecordTool {
    journal: Arc<ChangeJournal>,
    security: Arc<SecurityPolicy>,
}

impl ChangeRecordTool {
    pub fn new(journal: Arc<ChangeJournal>, security: Arc<SecurityPolicy>) -> Self {
        Self { journal, security }
    }

    fn parse_change_type(s: &str) -> Result<ChangeType, String> {
        match s.to_lowercase().as_str() {
            "firewall_rule" | "firewall" => Ok(ChangeType::FirewallRule),
            "network_config" | "network" => Ok(ChangeType::NetworkConfig),
            "service_config" | "service" => Ok(ChangeType::ServiceConfig),
            "parental_rule" | "parental" => Ok(ChangeType::ParentalRule),
            "plugin_install" => Ok(ChangeType::PluginInstall),
            "plugin_remove" => Ok(ChangeType::PluginRemove),
            "other" => Ok(ChangeType::Other),
            _ => Err(format!(
                "Unknown change type: {}. Valid types: firewall_rule, network_config, service_config, parental_rule, plugin_install, plugin_remove, other",
                s
            )),
        }
    }
}

#[async_trait]
impl Tool for ChangeRecordTool {
    fn name(&self) -> &str {
        "change_record"
    }

    fn description(&self) -> &str {
        "Record a configuration change to the change journal for audit and rollback purposes. \
         Captures before/after state, operator, change type, and optional snapshot reference."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operator": {
                    "type": "string",
                    "description": "The operator who made the change (user or system identifier)"
                },
                "change_type": {
                    "type": "string",
                    "enum": ["firewall_rule", "network_config", "service_config", "parental_rule", "plugin_install", "plugin_remove", "other"],
                    "description": "Type of configuration change"
                },
                "target": {
                    "type": "string",
                    "description": "Target configuration file or resource path"
                },
                "before": {
                    "type": "string",
                    "description": "Configuration content before the change (optional)"
                },
                "after": {
                    "type": "string",
                    "description": "Configuration content after the change (optional)"
                },
                "snapshot_id": {
                    "type": "string",
                    "description": "Optional snapshot ID to associate with this change"
                }
            },
            "required": ["operator", "change_type", "target"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let input: ChangeRecordInput = serde_json::from_value(args.clone()).map_err(|e| {
            anyhow::anyhow!("Invalid arguments for change_record: {}", e)
        })?;

        if !self.security.allow_file_read(&input.target) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Access denied to target: {}", input.target)),
            });
        }

        let change_type = Self::parse_change_type(&input.change_type).map_err(|e| {
            anyhow::anyhow!("{}", e)
        })?;

        let record = ChangeRecord::new(
            &input.operator,
            change_type,
            &input.target,
            input.before,
            input.after,
            input.snapshot_id,
        );

        let record_id = record.id.clone();
        let timestamp = record.timestamp;
        let target = record.target.clone();

        self.journal.record(&record)?;

        let mut result = format!(
            "Change recorded successfully.\n  ID: {}\n  Target: {}\n  Type: {}\n  Operator: {}\n  Timestamp: {}\n  Status: Pending",
            record_id,
            target,
            change_type,
            input.operator,
            timestamp
        );

        if let Some(ref snapshot_id) = record.snapshot_id {
            result.push_str(&format!("\n  Snapshot: {}", snapshot_id));
        }

        Ok(ToolResult {
            success: true,
            output: result,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Arc<ChangeJournal>, Arc<SecurityPolicy>) {
        let tmp = TempDir::new().unwrap();
        let journal = Arc::new(ChangeJournal::open(tmp.path()).unwrap());
        let security = Arc::new(SecurityPolicy::default());
        (tmp, journal, security)
    }

    #[test]
    fn tool_name_and_description() {
        let (_tmp, journal, security) = setup();
        let tool = ChangeRecordTool::new(journal, security);
        assert_eq!(tool.name(), "change_record");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn parameters_schema_valid() {
        let (_tmp, journal, security) = setup();
        let tool = ChangeRecordTool::new(journal, security);
        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["operator"].is_object());
        assert!(schema["properties"]["change_type"]["enum"].is_array());
    }

    #[tokio::test]
    async fn record_change_success() {
        let (_tmp, journal, security) = setup();
        let tool = ChangeRecordTool::new(journal.clone(), security);

        let args = serde_json::json!({
            "operator": "zeroclaw_user",
            "change_type": "firewall_rule",
            "target": "/etc/firewall/rules.conf",
            "before": "allow 80",
            "after": "allow 80\nallow 443"
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Change recorded successfully"));
        assert!(result.output.contains("firewall_rule"));
        assert!(result.error.is_none());

        assert_eq!(journal.count().unwrap(), 1);
    }

    #[tokio::test]
    async fn record_change_with_snapshot() {
        let (_tmp, journal, security) = setup();
        let tool = ChangeRecordTool::new(journal.clone(), security);

        let args = serde_json::json!({
            "operator": "zeroclaw_user",
            "change_type": "network_config",
            "target": "/etc/network/interfaces",
            "snapshot_id": "snap-123"
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("snap-123"));
    }

    #[tokio::test]
    async fn record_change_invalid_type() {
        let (_tmp, journal, security) = setup();
        let tool = ChangeRecordTool::new(journal.clone(), security);

        let args = serde_json::json!({
            "operator": "zeroclaw_user",
            "change_type": "invalid_type",
            "target": "/etc/test"
        });

        let result = tool.execute(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn record_change_missing_required() {
        let (_tmp, journal, security) = setup();
        let tool = ChangeRecordTool::new(journal.clone(), security);

        let args = serde_json::json!({
            "operator": "zeroclaw_user"
        });

        let result = tool.execute(args).await;
        assert!(result.is_err());
    }

    #[test]
    fn parse_change_type_valid() {
        assert_eq!(ChangeRecordTool::parse_change_type("firewall_rule").unwrap(), ChangeType::FirewallRule);
        assert_eq!(ChangeRecordTool::parse_change_type("firewall").unwrap(), ChangeType::FirewallRule);
        assert_eq!(ChangeRecordTool::parse_change_type("network_config").unwrap(), ChangeType::NetworkConfig);
        assert_eq!(ChangeRecordTool::parse_change_type("network").unwrap(), ChangeType::NetworkConfig);
        assert_eq!(ChangeRecordTool::parse_change_type("service_config").unwrap(), ChangeType::ServiceConfig);
        assert_eq!(ChangeRecordTool::parse_change_type("service").unwrap(), ChangeType::ServiceConfig);
        assert_eq!(ChangeRecordTool::parse_change_type("parental_rule").unwrap(), ChangeType::ParentalRule);
        assert_eq!(ChangeRecordTool::parse_change_type("plugin_install").unwrap(), ChangeType::PluginInstall);
        assert_eq!(ChangeRecordTool::parse_change_type("plugin_remove").unwrap(), ChangeType::PluginRemove);
        assert_eq!(ChangeRecordTool::parse_change_type("other").unwrap(), ChangeType::Other);
    }

    #[test]
    fn parse_change_type_invalid() {
        assert!(ChangeRecordTool::parse_change_type("unknown").is_err());
        assert!(ChangeRecordTool::parse_change_type("").is_err());
    }
}

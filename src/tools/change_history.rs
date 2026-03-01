use crate::rollback::{ChangeJournal, ChangeQuery, ChangeStatus, ChangeType};
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeHistoryInput {
    pub operator: Option<String>,
    pub change_type: Option<String>,
    pub status: Option<String>,
    pub target: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub struct ChangeHistoryTool {
    journal: Arc<ChangeJournal>,
    security: Arc<SecurityPolicy>,
}

impl ChangeHistoryTool {
    pub fn new(journal: Arc<ChangeJournal>, security: Arc<SecurityPolicy>) -> Self {
        Self { journal, security }
    }

    fn parse_change_type(s: &str) -> Option<ChangeType> {
        match s.to_lowercase().as_str() {
            "firewall_rule" | "firewall" => Some(ChangeType::FirewallRule),
            "network_config" | "network" => Some(ChangeType::NetworkConfig),
            "service_config" | "service" => Some(ChangeType::ServiceConfig),
            "parental_rule" | "parental" => Some(ChangeType::ParentalRule),
            "plugin_install" => Some(ChangeType::PluginInstall),
            "plugin_remove" => Some(ChangeType::PluginRemove),
            "other" => Some(ChangeType::Other),
            _ => None,
        }
    }

    fn parse_status(s: &str) -> Option<ChangeStatus> {
        match s.to_lowercase().as_str() {
            "pending" => Some(ChangeStatus::Pending),
            "applied" => Some(ChangeStatus::Applied),
            "rolled_back" | "rolledback" | "rollback" => Some(ChangeStatus::RolledBack),
            "failed" => Some(ChangeStatus::Failed),
            _ => None,
        }
    }

    fn format_timestamp(ts: i64) -> String {
        if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
            dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
        } else {
            ts.to_string()
        }
    }
}

#[async_trait]
impl Tool for ChangeHistoryTool {
    fn name(&self) -> &str {
        "change_history"
    }

    fn description(&self) -> &str {
        "Query change history from the journal. Filter by operator, change type, status, target, or time range. \
         Returns a list of change records with details."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operator": {
                    "type": "string",
                    "description": "Filter by operator name"
                },
                "change_type": {
                    "type": "string",
                    "enum": ["firewall_rule", "network_config", "service_config", "parental_rule", "plugin_install", "plugin_remove", "other"],
                    "description": "Filter by change type"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "applied", "rolled_back", "failed"],
                    "description": "Filter by change status"
                },
                "target": {
                    "type": "string",
                    "description": "Filter by target (supports partial match)"
                },
                "start_time": {
                    "type": "integer",
                    "description": "Start of time range (Unix timestamp)"
                },
                "end_time": {
                    "type": "integer",
                    "description": "End of time range (Unix timestamp)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of records to return (default: 50)",
                    "default": 50
                },
                "offset": {
                    "type": "integer",
                    "description": "Number of records to skip for pagination"
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let input: ChangeHistoryInput = serde_json::from_value(args).map_err(|e| {
            anyhow::anyhow!("Invalid arguments for change_history: {}", e)
        })?;

        let query = ChangeQuery {
            operator: input.operator,
            change_type: input.change_type.and_then(|s| Self::parse_change_type(&s)),
            status: input.status.and_then(|s| Self::parse_status(&s)),
            target: input.target,
            start_time: input.start_time,
            end_time: input.end_time,
            limit: input.limit.or(Some(50)),
            offset: input.offset,
        };

        let records = self.journal.query(&query)?;

        if records.is_empty() {
            return Ok(ToolResult {
                success: true,
                output: "No change records found matching the criteria.".to_string(),
                error: None,
            });
        }

        let mut output = format!("Found {} change record(s):\n\n", records.len());

        for record in &records {
            output.push_str(&format!("ID: {}\n", record.id));
            output.push_str(&format!("  Timestamp: {}\n", Self::format_timestamp(record.timestamp)));
            output.push_str(&format!("  Operator: {}\n", record.operator));
            output.push_str(&format!("  Type: {}\n", record.change_type));
            output.push_str(&format!("  Target: {}\n", record.target));
            output.push_str(&format!("  Status: {}\n", record.status));

            if let Some(ref snapshot_id) = record.snapshot_id {
                output.push_str(&format!("  Snapshot: {}\n", snapshot_id));
            }
            if let Some(ref rollback_id) = record.rollback_id {
                output.push_str(&format!("  Rollback ID: {}\n", rollback_id));
            }

            let before_preview = record.before.as_ref().map(|s| {
                let lines: Vec<&str> = s.lines().take(3).collect();
                let preview = lines.join("\n    ");
                if s.lines().count() > 3 {
                    format!("    {}\n    ...", preview)
                } else {
                    format!("    {}", preview)
                }
            }).unwrap_or_default();

            let after_preview = record.after.as_ref().map(|s| {
                let lines: Vec<&str> = s.lines().take(3).collect();
                let preview = lines.join("\n    ");
                if s.lines().count() > 3 {
                    format!("    {}\n    ...", preview)
                } else {
                    format!("    {}", preview)
                }
            }).unwrap_or_default();

            if !before_preview.is_empty() {
                output.push_str(&format!("  Before:\n{}\n", before_preview));
            }
            if !after_preview.is_empty() {
                output.push_str(&format!("  After:\n{}\n", after_preview));
            }

            output.push('\n');
        }

        Ok(ToolResult {
            success: true,
            output,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rollback::ChangeRecord;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Arc<ChangeJournal>, Arc<SecurityPolicy>) {
        let tmp = TempDir::new().unwrap();
        let journal = Arc::new(ChangeJournal::open(tmp.path()).unwrap());
        let security = Arc::new(SecurityPolicy::default());
        (tmp, journal, security)
    }

    fn populate_journal(journal: &ChangeJournal) {
        for i in 0..5 {
            let mut record = ChangeRecord::new(
                if i % 2 == 0 { "alice" } else { "bob" },
                if i % 2 == 0 { ChangeType::FirewallRule } else { ChangeType::NetworkConfig },
                &format!("/etc/config/{}", i),
                Some(format!("before_{}", i)),
                Some(format!("after_{}", i)),
                None,
            );
            record.status = if i % 2 == 0 { ChangeStatus::Applied } else { ChangeStatus::Pending };
            journal.record(&record).unwrap();
        }
    }

    #[test]
    fn tool_name_and_description() {
        let (_tmp, journal, security) = setup();
        let tool = ChangeHistoryTool::new(journal, security);
        assert_eq!(tool.name(), "change_history");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn parameters_schema_valid() {
        let (_tmp, journal, security) = setup();
        let tool = ChangeHistoryTool::new(journal, security);
        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["operator"].is_object());
        assert!(schema["properties"]["limit"]["default"].is_number());
    }

    #[tokio::test]
    async fn query_all_records() {
        let (_tmp, journal, security) = setup();
        populate_journal(&journal);
        let tool = ChangeHistoryTool::new(journal, security);

        let args = serde_json::json!({});
        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Found 5 change record(s)"));
    }

    #[tokio::test]
    async fn query_by_operator() {
        let (_tmp, journal, security) = setup();
        populate_journal(&journal);
        let tool = ChangeHistoryTool::new(journal, security);

        let args = serde_json::json!({ "operator": "alice" });
        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("alice"));
        assert!(!result.output.contains("bob"));
    }

    #[tokio::test]
    async fn query_by_change_type() {
        let (_tmp, journal, security) = setup();
        populate_journal(&journal);
        let tool = ChangeHistoryTool::new(journal, security);

        let args = serde_json::json!({ "change_type": "firewall_rule" });
        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("firewall_rule"));
    }

    #[tokio::test]
    async fn query_by_status() {
        let (_tmp, journal, security) = setup();
        populate_journal(&journal);
        let tool = ChangeHistoryTool::new(journal, security);

        let args = serde_json::json!({ "status": "applied" });
        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("applied"));
    }

    #[tokio::test]
    async fn query_by_target() {
        let (_tmp, journal, security) = setup();
        populate_journal(&journal);
        let tool = ChangeHistoryTool::new(journal, security);

        let args = serde_json::json!({ "target": "config/1" });
        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("config/1"));
    }

    #[tokio::test]
    async fn query_with_limit() {
        let (_tmp, journal, security) = setup();
        populate_journal(&journal);
        let tool = ChangeHistoryTool::new(journal, security);

        let args = serde_json::json!({ "limit": 2 });
        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Found 2 change record(s)"));
    }

    #[tokio::test]
    async fn query_empty_result() {
        let (_tmp, journal, security) = setup();
        let tool = ChangeHistoryTool::new(journal, security);

        let args = serde_json::json!({ "operator": "nonexistent" });
        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("No change records found"));
    }

    #[test]
    fn parse_change_type_valid() {
        assert!(ChangeHistoryTool::parse_change_type("firewall_rule").is_some());
        assert!(ChangeHistoryTool::parse_change_type("network_config").is_some());
        assert!(ChangeHistoryTool::parse_change_type("service_config").is_some());
    }

    #[test]
    fn parse_change_type_invalid() {
        assert!(ChangeHistoryTool::parse_change_type("invalid").is_none());
        assert!(ChangeHistoryTool::parse_change_type("").is_none());
    }

    #[test]
    fn parse_status_valid() {
        assert_eq!(ChangeHistoryTool::parse_status("pending"), Some(ChangeStatus::Pending));
        assert_eq!(ChangeHistoryTool::parse_status("applied"), Some(ChangeStatus::Applied));
        assert_eq!(ChangeHistoryTool::parse_status("rolled_back"), Some(ChangeStatus::RolledBack));
        assert_eq!(ChangeHistoryTool::parse_status("failed"), Some(ChangeStatus::Failed));
    }

    #[test]
    fn parse_status_invalid() {
        assert!(ChangeHistoryTool::parse_status("invalid").is_none());
    }

    #[test]
    fn format_timestamp_valid() {
        let ts = 1700000000_i64;
        let formatted = ChangeHistoryTool::format_timestamp(ts);
        assert!(formatted.contains("2023"));
        assert!(formatted.contains("UTC"));
    }
}

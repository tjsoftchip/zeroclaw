use crate::rollback::{
    compute_diff_for_record, format_diff_output, ChangeJournal, ChangeRecord, ConfigDiff,
};
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiffInput {
    pub record_id: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub target: Option<String>,
}

pub struct ConfigDiffTool {
    journal: Arc<ChangeJournal>,
    security: Arc<SecurityPolicy>,
}

impl ConfigDiffTool {
    pub fn new(journal: Arc<ChangeJournal>, security: Arc<SecurityPolicy>) -> Self {
        Self { journal, security }
    }

    fn format_json_diff(&self, diff: &ConfigDiff) -> String {
        let mut output = String::new();
        output.push_str(&format!("Target: {}\n", diff.target));
        output.push_str(&format!(
            "Summary: {} added, {} removed, {} modified, {} unchanged\n\n",
            diff.summary.added, diff.summary.removed, diff.summary.modified, diff.summary.unchanged
        ));

        if diff.entries.is_empty() {
            output.push_str("No differences found.\n");
            return output;
        }

        output.push_str("Differences:\n");
        for entry in &diff.entries {
            match entry.diff_type {
                crate::rollback::DiffType::Added => {
                    if let Some(new) = &entry.new_value {
                        output.push_str(&format!("  + {}\n", new));
                    }
                }
                crate::rollback::DiffType::Removed => {
                    if let Some(old) = &entry.old_value {
                        output.push_str(&format!("  - {}\n", old));
                    }
                }
                crate::rollback::DiffType::Modified => {
                    if let Some(old) = &entry.old_value {
                        output.push_str(&format!("  - {}\n", old));
                    }
                    if let Some(new) = &entry.new_value {
                        output.push_str(&format!("  + {}\n", new));
                    }
                }
                crate::rollback::DiffType::Unchanged => {}
            }
        }

        output
    }
}

#[async_trait]
impl Tool for ConfigDiffTool {
    fn name(&self) -> &str {
        "config_diff"
    }

    fn description(&self) -> &str {
        "Compare configuration changes. Either provide a record_id to diff a recorded change, \
         or provide before/after content directly. Shows line-by-line differences with summary statistics."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "record_id": {
                    "type": "string",
                    "description": "ID of a change record to diff (use instead of before/after)"
                },
                "before": {
                    "type": "string",
                    "description": "Configuration content before the change (for direct diff)"
                },
                "after": {
                    "type": "string",
                    "description": "Configuration content after the change (for direct diff)"
                },
                "target": {
                    "type": "string",
                    "description": "Target identifier for the diff (optional, for display purposes)"
                }
            },
            "oneOf": [
                { "required": ["record_id"] },
                { "required": ["before", "after"] }
            ]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let input: ConfigDiffInput = serde_json::from_value(args.clone()).map_err(|e| {
            anyhow::anyhow!("Invalid arguments for config_diff: {}", e)
        })?;

        let diff = if let Some(record_id) = &input.record_id {
            let record = self.journal.get(record_id)?.ok_or_else(|| {
                anyhow::anyhow!("Change record '{}' not found", record_id)
            })?;

            if record.before.is_none() && record.after.is_none() {
                return Ok(ToolResult {
                    success: true,
                    output: format!(
                        "Record {} exists but has no before/after content to compare.",
                        record_id
                    ),
                    error: None,
                });
            }

            compute_diff_for_record(&record)
        } else if input.before.is_some() || input.after.is_some() {
            let mut diff = crate::rollback::compute_diff(
                input.before.as_deref(),
                input.after.as_deref(),
            );
            diff.target = input.target.unwrap_or_else(|| "direct_diff".to_string());
            diff
        } else {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Either record_id or before/after must be provided".to_string()),
            });
        };

        let output = if input.record_id.is_some() {
            format_diff_output(&diff)
        } else {
            self.format_json_diff(&diff)
        };

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
    use crate::rollback::{ChangeType, ChangeStatus};
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
        let tool = ConfigDiffTool::new(journal, security);
        assert_eq!(tool.name(), "config_diff");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn parameters_schema_valid() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal, security);
        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["record_id"].is_object());
        assert!(schema["properties"]["before"].is_object());
        assert!(schema["properties"]["after"].is_object());
    }

    #[tokio::test]
    async fn diff_by_record_id() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal.clone(), security);

        let record = ChangeRecord::new(
            "zeroclaw_user",
            ChangeType::FirewallRule,
            "/etc/firewall/rules.conf",
            Some("allow 80\nallow 443".to_string()),
            Some("allow 80\nallow 443\nallow 8080".to_string()),
            None,
        );
        let record_id = record.id.clone();
        journal.record(&record).unwrap();

        let args = serde_json::json!({ "record_id": record_id });
        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("allow 8080"));
        assert!(result.output.contains("+1"));
    }

    #[tokio::test]
    async fn diff_by_record_id_not_found() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal, security);

        let args = serde_json::json!({ "record_id": "nonexistent-id" });
        let result = tool.execute(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn diff_direct_before_after() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal, security);

        let args = serde_json::json!({
            "before": "line1\nline2\nline3",
            "after": "line1\nmodified\nline3\nline4",
            "target": "/test/config"
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("/test/config"));
        assert!(result.output.contains("modified"));
    }

    #[tokio::test]
    async fn diff_empty_before() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal, security);

        let args = serde_json::json!({
            "before": null,
            "after": "new line 1\nnew line 2"
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("added"));
    }

    #[tokio::test]
    async fn diff_empty_after() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal, security);

        let args = serde_json::json!({
            "before": "old line 1\nold line 2",
            "after": null
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("removed"));
    }

    #[tokio::test]
    async fn diff_no_changes() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal, security);

        let args = serde_json::json!({
            "before": "same content",
            "after": "same content"
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("unchanged"));
    }

    #[tokio::test]
    async fn diff_record_no_content() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal.clone(), security);

        let record = ChangeRecord::new(
            "zeroclaw_user",
            ChangeType::Other,
            "/test/target",
            None,
            None,
            None,
        );
        let record_id = record.id.clone();
        journal.record(&record).unwrap();

        let args = serde_json::json!({ "record_id": record_id });
        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("no before/after content"));
    }

    #[tokio::test]
    async fn diff_missing_arguments() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal, security);

        let args = serde_json::json!({});
        let result = tool.execute(args).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("record_id or before/after"));
    }

    #[test]
    fn format_json_diff_empty() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal, security);

        let diff = ConfigDiff {
            record_id: "test".to_string(),
            target: "/test".to_string(),
            entries: vec![],
            summary: Default::default(),
        };

        let output = tool.format_json_diff(&diff);
        assert!(output.contains("No differences found"));
    }

    #[test]
    fn format_json_diff_with_changes() {
        let (_tmp, journal, security) = setup();
        let tool = ConfigDiffTool::new(journal, security);

        use crate::rollback::{ConfigDiffEntry, DiffType, DiffSummary};

        let diff = ConfigDiff {
            record_id: "test".to_string(),
            target: "/test".to_string(),
            entries: vec![
                ConfigDiffEntry {
                    path: "line1".to_string(),
                    old_value: None,
                    new_value: Some("added line".to_string()),
                    diff_type: DiffType::Added,
                },
                ConfigDiffEntry {
                    path: "line2".to_string(),
                    old_value: Some("removed line".to_string()),
                    new_value: None,
                    diff_type: DiffType::Removed,
                },
            ],
            summary: DiffSummary {
                added: 1,
                removed: 1,
                modified: 0,
                unchanged: 0,
            },
        };

        let output = tool.format_json_diff(&diff);
        assert!(output.contains("+ added line"));
        assert!(output.contains("- removed line"));
    }
}

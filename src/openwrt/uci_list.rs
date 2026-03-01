//! UCI list tool for listing configuration entries.

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::executor::UciExecutor;
use super::types::UciListEntry;
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

pub struct UciListTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
}

impl UciListTool {
    pub fn new(security: Arc<SecurityPolicy>, executor: Arc<UciExecutor>) -> Self {
        Self { security, executor }
    }

    fn format_summary(&self, entries: &[UciListEntry]) -> String {
        let mut output = String::new();

        for entry in entries {
            if entry.section.is_none() {
                output.push_str(&format!("{}\n", entry.package));
            } else if let Some(ref section) = entry.section {
                if let Some(ref config_type) = entry.config_type {
                    output.push_str(&format!(
                        "{}.{} ({})\n",
                        entry.package, section, config_type
                    ));
                } else {
                    output.push_str(&format!("{}.{}\n", entry.package, section));
                }
            }
        }

        output
    }

    fn format_detailed(&self, entries: &[UciListEntry]) -> String {
        let mut output = String::new();

        for entry in entries {
            if entry.section.is_none() {
                output.push_str(&format!("[{}]\n", entry.package));
            } else if let Some(ref section) = entry.section {
                if let Some(ref config_type) = entry.config_type {
                    output.push_str(&format!(
                        "\n[{}.{}]  # type: {}\n",
                        entry.package, section, config_type
                    ));
                } else {
                    output.push_str(&format!("\n[{}.{}]\n", entry.package, section));
                }

                let mut options: Vec<_> = entry.options.iter().collect();
                options.sort_by_key(|(k, _)| *k);

                for (key, value) in options {
                    output.push_str(&format!("    {} = {}\n", key, value));
                }
            }
        }

        output
    }

    fn format_json(&self, entries: &[UciListEntry]) -> String {
        let json_entries: Vec<serde_json::Value> = entries
            .iter()
            .map(|entry| {
                let options: serde_json::Map<String, serde_json::Value> = entry
                    .options
                    .iter()
                    .map(|(k, v)| {
                        let json_val = match v {
                            super::types::UciValue::String(s) => json!(s),
                            super::types::UciValue::List(l) => json!(l),
                            super::types::UciValue::Boolean(b) => json!(b),
                            super::types::UciValue::Integer(i) => json!(i),
                        };
                        (k.clone(), json_val)
                    })
                    .collect();

                json!({
                    "package": entry.package,
                    "section": entry.section,
                    "type": entry.config_type,
                    "options": options
                })
            })
            .collect();

        serde_json::to_string_pretty(&json_entries).unwrap_or_default()
    }
}

#[async_trait]
impl Tool for UciListTool {
    fn name(&self) -> &str {
        "uci_list"
    }

    fn description(&self) -> &str {
        "List UCI configuration packages, sections, or options. Can list all packages, sections within a package, or options within a section."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "UCI package name to list (optional, lists all packages if not specified)"
                },
                "section": {
                    "type": "string",
                    "description": "Section name to list options for (optional, requires package)"
                },
                "format": {
                    "type": "string",
                    "enum": ["summary", "detailed", "json"],
                    "default": "summary",
                    "description": "Output format"
                }
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

        let package = args.get("package").and_then(|v| v.as_str());
        let section = args.get("section").and_then(|v| v.as_str());
        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("summary");

        if section.is_some() && package.is_none() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Section requires package parameter".to_string()),
            });
        }

        if let Some(pkg) = package {
            if !self.executor.config_exists(pkg) {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("UCI package '{}' does not exist", pkg)),
                });
            }
        }

        match self.executor.list(package, section) {
            Ok(entries) => {
                let output = match format {
                    "json" => self.format_json(&entries),
                    "detailed" => self.format_detailed(&entries),
                    _ => self.format_summary(&entries),
                };

                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to list UCI configuration: {}", e)),
            }),
        }
    }
}

pub struct UciChangesTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
}

impl UciChangesTool {
    pub fn new(security: Arc<SecurityPolicy>, executor: Arc<UciExecutor>) -> Self {
        Self { security, executor }
    }
}

#[async_trait]
impl Tool for UciChangesTool {
    fn name(&self) -> &str {
        "uci_changes"
    }

    fn description(&self) -> &str {
        "List pending UCI changes that have not been committed yet."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "UCI package name to check for changes (optional, checks all if not specified)"
                }
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

        let package = args.get("package").and_then(|v| v.as_str());

        match self.executor.changes(package) {
            Ok(changes) => {
                let output = if changes.is_empty() {
                    json!({
                        "has_changes": false,
                        "changes": [],
                        "message": "No pending changes"
                    })
                } else {
                    json!({
                        "has_changes": true,
                        "count": changes.len(),
                        "changes": changes
                    })
                };

                Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&output).unwrap_or_default(),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to list changes: {}", e)),
            }),
        }
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

    #[test]
    fn tool_metadata() {
        let tool = UciListTool::new(test_security(), test_executor());
        assert_eq!(tool.name(), "uci_list");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn parameters_schema_structure() {
        let tool = UciListTool::new(test_security(), test_executor());
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["package"].is_object());
        assert!(schema["properties"]["format"]["enum"].is_array());
    }

    #[test]
    fn changes_tool_metadata() {
        let tool = UciChangesTool::new(test_security(), test_executor());
        assert_eq!(tool.name(), "uci_changes");
        assert!(!tool.description().is_empty());
    }
}

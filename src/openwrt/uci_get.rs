//! UCI get tool for reading configuration values.
//!
//! Provides the `uci_get` tool for reading UCI configuration values
//! from OpenWrt systems.

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::executor::UciExecutor;
use super::types::UciValue;
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

pub struct UciGetTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
}

impl UciGetTool {
    pub fn new(security: Arc<SecurityPolicy>, executor: Arc<UciExecutor>) -> Self {
        Self { security, executor }
    }
}

#[async_trait]
impl Tool for UciGetTool {
    fn name(&self) -> &str {
        "uci_get"
    }

    fn description(&self) -> &str {
        "Read a UCI configuration value from OpenWrt. Returns the value of the specified option or section."
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
                    "description": "Section name within the package (e.g., 'lan', 'wan', '@zone[0]')"
                },
                "option": {
                    "type": "string",
                    "description": "Option name within the section (e.g., 'ipaddr', 'name', 'enabled')"
                }
            },
            "required": ["package"]
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

        let section = args.get("section").and_then(|v| v.as_str());
        let option = args.get("option").and_then(|v| v.as_str());

        if !self.executor.config_exists(package) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("UCI package '{}' does not exist", package)),
            });
        }

        match self.executor.get(package, section, option) {
            Ok(value) => {
                let output = if section.is_some() && option.is_some() {
                    json!({
                        "package": package,
                        "section": section,
                        "option": option,
                        "value": value
                    })
                } else {
                    json!({
                        "package": package,
                        "value": value
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
                error: Some(format!("Failed to get UCI value: {}", e)),
            }),
        }
    }
}

pub struct UciGetBulkTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
}

impl UciGetBulkTool {
    pub fn new(security: Arc<SecurityPolicy>, executor: Arc<UciExecutor>) -> Self {
        Self { security, executor }
    }
}

#[async_trait]
impl Tool for UciGetBulkTool {
    fn name(&self) -> &str {
        "uci_get_bulk"
    }

    fn description(&self) -> &str {
        "Read multiple UCI configuration values in a single call. More efficient than multiple uci_get calls."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "queries": {
                    "type": "array",
                    "description": "Array of query objects",
                    "items": {
                        "type": "object",
                        "properties": {
                            "package": { "type": "string" },
                            "section": { "type": "string" },
                            "option": { "type": "string" }
                        },
                        "required": ["package"]
                    }
                }
            },
            "required": ["queries"]
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

        let queries = args
            .get("queries")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: queries"))?;

        let mut results = Vec::new();
        let mut errors = Vec::new();

        for query in queries {
            let package = query.get("package").and_then(|v| v.as_str());
            let section = query.get("section").and_then(|v| v.as_str());
            let option = query.get("option").and_then(|v| v.as_str());

            if let Some(pkg) = package {
                match self.executor.get(pkg, section, option) {
                    Ok(value) => {
                        results.push(json!({
                            "package": pkg,
                            "section": section,
                            "option": option,
                            "value": value,
                            "success": true
                        }));
                    }
                    Err(e) => {
                        results.push(json!({
                            "package": pkg,
                            "section": section,
                            "option": option,
                            "success": false,
                            "error": e.to_string()
                        }));
                        errors.push(format!("{}.{}.{}: {}", pkg, section.unwrap_or(""), option.unwrap_or(""), e));
                    }
                }
            }
        }

        let output = json!({
            "results": results,
            "total": queries.len(),
            "successful": queries.len() - errors.len(),
            "failed": errors.len()
        });

        Ok(ToolResult {
            success: errors.is_empty(),
            output: serde_json::to_string_pretty(&output).unwrap_or_default(),
            error: if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            },
        })
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
        let tool = UciGetTool::new(test_security(), test_executor());
        assert_eq!(tool.name(), "uci_get");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn parameters_schema_structure() {
        let tool = UciGetTool::new(test_security(), test_executor());
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["package"].is_object());
        assert!(schema["required"].as_array().unwrap().contains(&json!("package")));
    }

    #[tokio::test]
    async fn missing_package_parameter() {
        let tool = UciGetTool::new(test_security(), test_executor());
        let result = tool.execute(json!({})).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bulk_tool_metadata() {
        let tool = UciGetBulkTool::new(test_security(), test_executor());
        assert_eq!(tool.name(), "uci_get_bulk");
        assert!(!tool.description().is_empty());
    }
}

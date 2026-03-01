//! UCI validate tool for configuration validation.
//!
//! Provides the `uci_validate` tool for validating UCI configuration
//! packages against their schemas.

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::executor::UciExecutor;
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

pub struct UciValidateTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
}

impl UciValidateTool {
    pub fn new(security: Arc<SecurityPolicy>, executor: Arc<UciExecutor>) -> Self {
        Self { security, executor }
    }
}

#[async_trait]
impl Tool for UciValidateTool {
    fn name(&self) -> &str {
        "uci_validate"
    }

    fn description(&self) -> &str {
        "Validate UCI configuration package against its schema. Returns validation errors and warnings."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "UCI package name to validate"
                },
                "strict": {
                    "type": "boolean",
                    "default": false,
                    "description": "Enable strict validation mode (treat warnings as errors)"
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

        let strict = args.get("strict").and_then(|v| v.as_bool()).unwrap_or(false);

        if !self.executor.config_exists(package) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("UCI package '{}' does not exist", package)),
            });
        }

        match self.executor.validate(package) {
            Ok(result) => {
                let is_valid = if strict {
                    result.valid && result.warnings.is_empty()
                } else {
                    result.valid
                };

                let output = json!({
                    "package": result.package,
                    "valid": is_valid,
                    "error_count": result.errors.len(),
                    "warning_count": result.warnings.len(),
                    "errors": result.errors.iter().map(|e| json!({
                        "section": e.section,
                        "option": e.option,
                        "message": e.message
                    })).collect::<Vec<_>>(),
                    "warnings": result.warnings.iter().map(|w| json!({
                        "section": w.section,
                        "option": w.option,
                        "message": w.message
                    })).collect::<Vec<_>>()
                });

                Ok(ToolResult {
                    success: is_valid,
                    output: serde_json::to_string_pretty(&output).unwrap_or_default(),
                    error: if is_valid {
                        None
                    } else {
                        Some(format!(
                            "Validation failed with {} errors",
                            result.errors.len()
                        ))
                    },
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Validation failed: {}", e)),
            }),
        }
    }
}

pub struct UciCommitTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
}

impl UciCommitTool {
    pub fn new(security: Arc<SecurityPolicy>, executor: Arc<UciExecutor>) -> Self {
        Self { security, executor }
    }
}

#[async_trait]
impl Tool for UciCommitTool {
    fn name(&self) -> &str {
        "uci_commit"
    }

    fn description(&self) -> &str {
        "Commit pending UCI changes to make them permanent. Changes are written to the configuration files."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "UCI package name to commit (optional, commits all if not specified)"
                },
                "validate": {
                    "type": "boolean",
                    "default": true,
                    "description": "Validate configuration before committing"
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
        let should_validate = args.get("validate").and_then(|v| v.as_bool()).unwrap_or(true);

        if let Some(pkg) = package {
            if !self.executor.config_exists(pkg) {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("UCI package '{}' does not exist", pkg)),
                });
            }
        }

        if should_validate {
            if let Some(pkg) = package {
                match self.executor.validate(pkg) {
                    Ok(result) => {
                        if !result.valid {
                            return Ok(ToolResult {
                                success: false,
                                output: String::new(),
                                error: Some(format!(
                                    "Validation failed for package '{}': {} errors",
                                    pkg,
                                    result.errors.len()
                                )),
                            });
                        }
                    }
                    Err(e) => {
                        return Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Validation failed: {}", e)),
                        });
                    }
                }
            }
        }

        match self.executor.commit(package) {
            Ok(()) => {
                let output = json!({
                    "success": true,
                    "package": package,
                    "message": if package.is_some() {
                        format!("Package '{}' committed successfully", package.unwrap())
                    } else {
                        "All packages committed successfully".to_string()
                    }
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
                error: Some(format!("Commit failed: {}", e)),
            }),
        }
    }
}

pub struct UciRevertTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
}

impl UciRevertTool {
    pub fn new(security: Arc<SecurityPolicy>, executor: Arc<UciExecutor>) -> Self {
        Self { security, executor }
    }
}

#[async_trait]
impl Tool for UciRevertTool {
    fn name(&self) -> &str {
        "uci_revert"
    }

    fn description(&self) -> &str {
        "Revert pending UCI changes. Can revert specific option, section, package, or all changes."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "UCI package name to revert (optional, reverts all if not specified)"
                },
                "section": {
                    "type": "string",
                    "description": "Section name to revert (optional, requires package)"
                },
                "option": {
                    "type": "string",
                    "description": "Option name to revert (optional, requires section)"
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
        let option = args.get("option").and_then(|v| v.as_str());

        if section.is_some() && package.is_none() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Section requires package parameter".to_string()),
            });
        }

        if option.is_some() && section.is_none() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Option requires section parameter".to_string()),
            });
        }

        match self.executor.revert(package, section, option) {
            Ok(()) => {
                let target = match (package, section, option) {
                    (Some(pkg), Some(sec), Some(opt)) => {
                        format!("{}.{}.{}", pkg, sec, opt)
                    }
                    (Some(pkg), Some(sec), None) => {
                        format!("{}.{}", pkg, sec)
                    }
                    (Some(pkg), None, None) => pkg.to_string(),
                    (None, None, None) => "all packages".to_string(),
                    _ => "unknown".to_string(),
                };

                let output = json!({
                    "success": true,
                    "reverted": target,
                    "message": format!("Reverted changes for {}", target)
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
                error: Some(format!("Revert failed: {}", e)),
            }),
        }
    }
}

pub struct UciAddTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
}

impl UciAddTool {
    pub fn new(security: Arc<SecurityPolicy>, executor: Arc<UciExecutor>) -> Self {
        Self { security, executor }
    }
}

#[async_trait]
impl Tool for UciAddTool {
    fn name(&self) -> &str {
        "uci_add"
    }

    fn description(&self) -> &str {
        "Add a new anonymous section to a UCI package. Returns the name of the created section."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "UCI package name"
                },
                "type": {
                    "type": "string",
                    "description": "Section type (e.g., 'interface', 'zone', 'rule')"
                }
            },
            "required": ["package", "type"]
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

        let config_type = args
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: type"))?;

        if !self.executor.config_exists(package) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("UCI package '{}' does not exist", package)),
            });
        }

        match self.executor.add(package, config_type) {
            Ok(section_name) => {
                let output = json!({
                    "success": true,
                    "package": package,
                    "type": config_type,
                    "section": section_name,
                    "message": format!("Created new {} section: {}", config_type, section_name)
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
                error: Some(format!("Failed to add section: {}", e)),
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
    fn validate_tool_metadata() {
        let tool = UciValidateTool::new(test_security(), test_executor());
        assert_eq!(tool.name(), "uci_validate");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn commit_tool_metadata() {
        let tool = UciCommitTool::new(test_security(), test_executor());
        assert_eq!(tool.name(), "uci_commit");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn revert_tool_metadata() {
        let tool = UciRevertTool::new(test_security(), test_executor());
        assert_eq!(tool.name(), "uci_revert");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn add_tool_metadata() {
        let tool = UciAddTool::new(test_security(), test_executor());
        assert_eq!(tool.name(), "uci_add");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn parameters_schema_structure() {
        let tool = UciValidateTool::new(test_security(), test_executor());
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["package"].is_object());
        assert!(schema["required"].as_array().unwrap().contains(&json!("package")));
    }
}

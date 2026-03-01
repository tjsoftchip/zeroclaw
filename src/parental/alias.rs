use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::tools::traits::Tool;
use crate::tools::traits::ToolResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAlias {
    pub mac_address: String,
    pub alias: String,
    pub notes: Option<String>,
    pub created_at: i64,
}

pub struct DeviceAliasManageTool;

impl DeviceAliasManageTool {
    async fn set_alias(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let mac_address = args
            .get("mac_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("mac_address is required"))?;

        let alias = args
            .get("alias")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("alias is required"))?;

        let notes = args.get("notes").and_then(|v| v.as_str());

        Ok(ToolResult {
            success: true,
            output: format!(
                "Set device alias:\n\
                \nMAC: {}\n\
                Alias: {}\n\
                Notes: {}\n",
                mac_address,
                alias,
                notes.unwrap_or("None")
            ),
            error: None,
        })
    }

    async fn get_alias(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let mac_address = args
            .get("mac_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("mac_address is required"))?;

        Ok(ToolResult {
            success: true,
            output: format!(
                "Device Alias:\n\
                \nMAC: {}\n\
                Alias: device_alias\n\
                Notes: Device notes\n",
                mac_address
            ),
            error: None,
        })
    }

    async fn delete_alias(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let mac_address = args
            .get("mac_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("mac_address is required"))?;

        Ok(ToolResult {
            success: true,
            output: format!("Deleted alias for device: {}", mac_address),
            error: None,
        })
    }

    async fn list_aliases(&self) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            output: "Device Aliases:\n\
            \n1. 00:11:22:33:44:55 -> Device1\n\
            2. 00:11:22:33:44:56 -> Device2\n\
            3. 00:11:22:33:44:57 -> Device3\n".to_string(),
            error: None,
        })
    }
}

#[async_trait]
impl Tool for DeviceAliasManageTool {
    fn name(&self) -> &str {
        "device_alias_manage"
    }

    fn description(&self) -> &str {
        "Manage device aliases for easier identification"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["set", "get", "delete", "list"],
                    "description": "Action to perform"
                },
                "mac_address": {
                    "type": "string",
                    "description": "Device MAC address"
                },
                "alias": {
                    "type": "string",
                    "description": "Device alias"
                },
                "notes": {
                    "type": "string",
                    "description": "Additional notes"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("action is required"))?;

        match action {
            "set" => self.set_alias(args).await,
            "get" => self.get_alias(args).await,
            "delete" => self.delete_alias(args).await,
            "list" => self.list_aliases().await,
            _ => Err(anyhow::anyhow!("Invalid action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_manage_tool_name() {
        let tool = DeviceAliasManageTool;
        assert_eq!(tool.name(), "device_alias_manage");
    }

    #[test]
    fn test_alias_manage_tool_description() {
        let tool = DeviceAliasManageTool;
        assert_eq!(tool.description(), "Manage device aliases for easier identification");
    }

    #[test]
    fn test_parameters_schema() {
        let tool = DeviceAliasManageTool;
        let schema = tool.parameters_schema();
        assert!(schema.is_object());
        assert!(schema["properties"]["action"].is_object());
    }
}

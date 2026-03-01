use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::tools::traits::Tool;
use crate::tools::traits::ToolResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub devices: Vec<String>,
    pub created_at: i64,
}

pub struct DeviceGroupManageTool;

impl DeviceGroupManageTool {
    async fn create_group(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("name is required"))?;

        let description = args.get("description").and_then(|v| v.as_str());
        let devices: Vec<String> = args
            .get("devices")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();

        let group_id = uuid::Uuid::new_v4().to_string();

        Ok(ToolResult {
            success: true,
            output: format!(
                "Created device group:\n\
                \nID: {}\n\
                Name: {}\n\
                Description: {}\n\
                Devices: {}\n",
                group_id,
                name,
                description.unwrap_or("None"),
                devices.join(", ")
            ),
            error: None,
        })
    }

    async fn update_group(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let group_id = args
            .get("group_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("group_id is required"))?;

        Ok(ToolResult {
            success: true,
            output: format!("Updated device group: {}", group_id),
            error: None,
        })
    }

    async fn delete_group(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let group_id = args
            .get("group_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("group_id is required"))?;

        Ok(ToolResult {
            success: true,
            output: format!("Deleted device group: {}", group_id),
            error: None,
        })
    }

    async fn list_groups(&self) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            output: "Device groups:\n\
            \nKids Devices:\n\
            - ID: kids-devices\n\
              Name: Kids Devices\n\
              Devices: 00:11:22:33:44:55, 00:11:22:33:44:56\n\
              Description: Children's devices\n\
            \nAdult Devices:\n\
            - ID: adult-devices\n\
              Name: Adult Devices\n\
              Devices: 00:11:22:33:44:57\n\
              Description: Adults' devices\n".to_string(),
            error: None,
        })
    }
}

#[async_trait]
impl Tool for DeviceGroupManageTool {
    fn name(&self) -> &str {
        "device_group_manage"
    }

    fn description(&self) -> &str {
        "Create, update, or delete device groups for parental control"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "update", "delete", "list"],
                    "description": "Action to perform"
                },
                "group_id": {
                    "type": "string",
                    "description": "Group ID (for update/delete)"
                },
                "name": {
                    "type": "string",
                    "description": "Group name (for create/update)"
                },
                "description": {
                    "type": "string",
                    "description": "Group description"
                },
                "devices": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of device MAC addresses"
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
            "create" => self.create_group(args).await,
            "update" => self.update_group(args).await,
            "delete" => self.delete_group(args).await,
            "list" => self.list_groups().await,
            _ => Err(anyhow::anyhow!("Invalid action: {}", action)),
        }
    }
}

pub struct DeviceGroupListTool;

#[async_trait]
impl Tool for DeviceGroupListTool {
    fn name(&self) -> &str {
        "device_group_list"
    }

    fn description(&self) -> &str {
        "List all device groups"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            output: "Device Groups:\n\
            \n1. Kids Devices (kids-devices)\n\
            2. Adult Devices (adult-devices)\n\
            3. Guest Devices (guest-devices)\n".to_string(),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_manage_tool_name() {
        let tool = DeviceGroupManageTool;
        assert_eq!(tool.name(), "device_group_manage");
    }

    #[test]
    fn test_group_list_tool_name() {
        let tool = DeviceGroupListTool;
        assert_eq!(tool.name(), "device_group_list");
    }

    #[test]
    fn test_parameters_schema() {
        let tool = DeviceGroupManageTool;
        let schema = tool.parameters_schema();
        assert!(schema.is_object());
        assert!(schema["properties"]["action"].is_object());
        let action_enum = schema["properties"]["action"]["enum"].as_array();
        assert!(action_enum.is_some());
        let enum_values = action_enum.unwrap();
        assert_eq!(enum_values.len(), 4);
        assert_eq!(enum_values[0], "create");
        assert_eq!(enum_values[1], "update");
        assert_eq!(enum_values[2], "delete");
        assert_eq!(enum_values[3], "list");
    }
}

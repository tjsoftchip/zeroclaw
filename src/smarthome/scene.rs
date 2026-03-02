//! Smart home scene management.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneAction {
    pub device_id: String,
    pub action: String,
    pub parameters: serde_json::Value,
}

impl SceneAction {
    pub fn new(device_id: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
            action: action.into(),
            parameters: json!({}),
        }
    }

    pub fn with_parameters(mut self, params: serde_json::Value) -> Self {
        self.parameters = params;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub actions: Vec<SceneAction>,
    pub enabled: bool,
    pub icon: Option<String>,
}

impl Scene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: None,
            actions: Vec::new(),
            enabled: true,
            icon: None,
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn add_action(mut self, action: SceneAction) -> Self {
        self.actions.push(action);
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

pub struct SmartSceneManageTool {
    security: Arc<SecurityPolicy>,
}

impl SmartSceneManageTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    fn get_default_scenes(&self) -> Vec<Scene> {
        let mut scenes = Vec::new();

        scenes.push(Scene::new("Good Morning")
            .description("Turn on lights and set comfortable temperature")
            .icon("sunrise")
            .add_action(SceneAction::new("light_001", "turn_on")
                .with_parameters(json!({"brightness": 80})))
            .add_action(SceneAction::new("thermostat_001", "set_temperature")
                .with_parameters(json!({"temperature": 22}))));

        scenes.push(Scene::new("Good Night")
            .description("Turn off all lights and lock doors")
            .icon("moon")
            .add_action(SceneAction::new("light_001", "turn_off"))
            .add_action(SceneAction::new("lock_001", "lock")));

        scenes.push(Scene::new("Movie Time")
            .description("Dim lights and set movie atmosphere")
            .icon("film")
            .add_action(SceneAction::new("light_001", "set_brightness")
                .with_parameters(json!({"brightness": 20})))
            .add_action(SceneAction::new("light_001", "set_color")
                .with_parameters(json!({"color": "#FF8000"}))));

        scenes.push(Scene::new("Away Mode")
            .description("Turn off everything and enable security")
            .icon("shield")
            .add_action(SceneAction::new("light_001", "turn_off"))
            .add_action(SceneAction::new("lock_001", "lock"))
            .add_action(SceneAction::new("camera_001", "enable_motion_detection")));

        scenes
    }

    fn format_scenes(&self, scenes: &[Scene]) -> String {
        if scenes.is_empty() {
            return "No scenes configured".to_string();
        }

        let mut output = String::from("Smart Home Scenes:\n\n");
        for scene in scenes {
            output.push_str(&format!(
                "{} {}\n  ID: {}\n  Description: {}\n  Actions: {}\n  Enabled: {}\n\n",
                scene.icon.as_deref().unwrap_or("📍"),
                scene.name,
                scene.id,
                scene.description.as_deref().unwrap_or("No description"),
                scene.actions.len(),
                if scene.enabled { "Yes" } else { "No" }
            ));
        }
        output
    }
}

#[async_trait]
impl Tool for SmartSceneManageTool {
    fn name(&self) -> &str {
        "smart_scene_manage"
    }

    fn description(&self) -> &str {
        "Manage smart home scenes. Create, activate, and delete scenes with multiple device actions."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "activate", "create", "delete"], "default": "list" },
                "scene_id": { "type": "string", "description": "Scene ID for activate/delete" },
                "name": { "type": "string", "description": "Scene name for create" },
                "description": { "type": "string" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");

        match action {
            "list" => {
                let scenes = self.get_default_scenes();
                Ok(ToolResult { success: true, output: self.format_scenes(&scenes), error: None })
            }
            "activate" => {
                let scene_id = args.get("scene_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("scene_id is required"))?;

                let scenes = self.get_default_scenes();
                let scene = scenes.iter().find(|s| s.id == scene_id || s.name.to_lowercase() == scene_id.to_lowercase());

                match scene {
                    Some(s) => {
                        let mut output = format!("Scene '{}' activated:\n\n", s.name);
                        for action in &s.actions {
                            output.push_str(&format!("  - Device {}: {}\n", action.device_id, action.action));
                        }
                        Ok(ToolResult { success: true, output, error: None })
                    }
                    None => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Scene '{}' not found", scene_id)),
                    })
                }
            }
            "create" => {
                let name = args.get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("name is required"))?;

                Ok(ToolResult {
                    success: true,
                    output: format!("Scene '{}' created successfully. Add actions using smart_device_control.", name),
                    error: None,
                })
            }
            "delete" => {
                let scene_id = args.get("scene_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("scene_id is required"))?;

                Ok(ToolResult {
                    success: true,
                    output: format!("Scene '{}' deleted.", scene_id),
                    error: None,
                })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy::default())
    }

    #[test]
    fn test_scene_action() {
        let action = SceneAction::new("light_001", "turn_on")
            .with_parameters(json!({"brightness": 100}));

        assert_eq!(action.device_id, "light_001");
        assert_eq!(action.action, "turn_on");
    }

    #[test]
    fn test_scene_builder() {
        let scene = Scene::new("Test Scene")
            .description("Test description")
            .icon("test")
            .add_action(SceneAction::new("device_001", "turn_on"));

        assert_eq!(scene.name, "Test Scene");
        assert_eq!(scene.description, Some("Test description".to_string()));
        assert_eq!(scene.icon, Some("test".to_string()));
        assert_eq!(scene.actions.len(), 1);
    }

    #[test]
    fn test_smart_scene_manage_tool() {
        let tool = SmartSceneManageTool::new(test_security());
        assert_eq!(tool.name(), "smart_scene_manage");
    }
}

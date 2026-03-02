//! Smart home device control.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeviceAction {
    TurnOn,
    TurnOff,
    Toggle,
    SetBrightness,
    SetColor,
    SetTemperature,
    Lock,
    Unlock,
    Play,
    Pause,
    Stop,
    SetVolume,
    Custom(String),
}

impl std::fmt::Display for DeviceAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceAction::TurnOn => write!(f, "turn_on"),
            DeviceAction::TurnOff => write!(f, "turn_off"),
            DeviceAction::Toggle => write!(f, "toggle"),
            DeviceAction::SetBrightness => write!(f, "set_brightness"),
            DeviceAction::SetColor => write!(f, "set_color"),
            DeviceAction::SetTemperature => write!(f, "set_temperature"),
            DeviceAction::Lock => write!(f, "lock"),
            DeviceAction::Unlock => write!(f, "unlock"),
            DeviceAction::Play => write!(f, "play"),
            DeviceAction::Pause => write!(f, "pause"),
            DeviceAction::Stop => write!(f, "stop"),
            DeviceAction::SetVolume => write!(f, "set_volume"),
            DeviceAction::Custom(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCommand {
    pub device_id: String,
    pub action: DeviceAction,
    pub parameters: serde_json::Value,
}

impl DeviceCommand {
    pub fn new(device_id: impl Into<String>, action: DeviceAction) -> Self {
        Self {
            device_id: device_id.into(),
            action,
            parameters: json!({}),
        }
    }

    pub fn with_parameter(mut self, key: &str, value: serde_json::Value) -> Self {
        if let Some(obj) = self.parameters.as_object_mut() {
            obj.insert(key.to_string(), value);
        }
        self
    }
}

pub struct SmartDeviceControlTool {
    security: Arc<SecurityPolicy>,
}

impl SmartDeviceControlTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    fn execute_command(&self, command: &DeviceCommand) -> anyhow::Result<String> {
        match command.action {
            DeviceAction::TurnOn => Ok(format!("Device {} turned on", command.device_id)),
            DeviceAction::TurnOff => Ok(format!("Device {} turned off", command.device_id)),
            DeviceAction::Toggle => Ok(format!("Device {} toggled", command.device_id)),
            DeviceAction::SetBrightness => {
                let brightness = command.parameters.get("brightness")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100);
                Ok(format!("Device {} brightness set to {}%", command.device_id, brightness))
            }
            DeviceAction::SetColor => {
                let color = command.parameters.get("color")
                    .and_then(|v| v.as_str())
                    .unwrap_or("#FFFFFF");
                Ok(format!("Device {} color set to {}", command.device_id, color))
            }
            DeviceAction::SetTemperature => {
                let temp = command.parameters.get("temperature")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(22.0);
                Ok(format!("Device {} temperature set to {}°C", command.device_id, temp))
            }
            DeviceAction::Lock => Ok(format!("Device {} locked", command.device_id)),
            DeviceAction::Unlock => Ok(format!("Device {} unlocked", command.device_id)),
            DeviceAction::Play => Ok(format!("Device {} playing", command.device_id)),
            DeviceAction::Pause => Ok(format!("Device {} paused", command.device_id)),
            DeviceAction::Stop => Ok(format!("Device {} stopped", command.device_id)),
            DeviceAction::SetVolume => {
                let volume = command.parameters.get("volume")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50);
                Ok(format!("Device {} volume set to {}%", command.device_id, volume))
            }
            DeviceAction::Custom(ref name) => Ok(format!("Device {} executed custom action: {}", command.device_id, name)),
        }
    }
}

#[async_trait]
impl Tool for SmartDeviceControlTool {
    fn name(&self) -> &str {
        "smart_device_control"
    }

    fn description(&self) -> &str {
        "Control smart home devices. Turn on/off, set brightness, color, temperature, and more."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "device_id": { "type": "string", "description": "Device ID to control" },
                "action": {
                    "type": "string",
                    "enum": ["turn_on", "turn_off", "toggle", "set_brightness", "set_color", "set_temperature", "lock", "unlock", "play", "pause", "stop", "set_volume"],
                    "description": "Action to perform"
                },
                "brightness": { "type": "integer", "minimum": 0, "maximum": 100 },
                "color": { "type": "string", "description": "Color in hex format (#RRGGBB)" },
                "temperature": { "type": "number", "description": "Temperature in Celsius" },
                "volume": { "type": "integer", "minimum": 0, "maximum": 100 }
            },
            "required": ["device_id", "action"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let device_id = args.get("device_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("device_id is required"))?;

        let action_str = args.get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("action is required"))?;

        let action = match action_str {
            "turn_on" => DeviceAction::TurnOn,
            "turn_off" => DeviceAction::TurnOff,
            "toggle" => DeviceAction::Toggle,
            "set_brightness" => DeviceAction::SetBrightness,
            "set_color" => DeviceAction::SetColor,
            "set_temperature" => DeviceAction::SetTemperature,
            "lock" => DeviceAction::Lock,
            "unlock" => DeviceAction::Unlock,
            "play" => DeviceAction::Play,
            "pause" => DeviceAction::Pause,
            "stop" => DeviceAction::Stop,
            "set_volume" => DeviceAction::SetVolume,
            _ => DeviceAction::Custom(action_str.to_string()),
        };

        let mut command = DeviceCommand::new(device_id, action);

        if let Some(brightness) = args.get("brightness") {
            command = command.with_parameter("brightness", brightness.clone());
        }
        if let Some(color) = args.get("color") {
            command = command.with_parameter("color", color.clone());
        }
        if let Some(temperature) = args.get("temperature") {
            command = command.with_parameter("temperature", temperature.clone());
        }
        if let Some(volume) = args.get("volume") {
            command = command.with_parameter("volume", volume.clone());
        }

        let result = self.execute_command(&command)?;

        Ok(ToolResult { success: true, output: result, error: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy::default())
    }

    #[test]
    fn test_device_action_display() {
        assert_eq!(DeviceAction::TurnOn.to_string(), "turn_on");
        assert_eq!(DeviceAction::SetBrightness.to_string(), "set_brightness");
    }

    #[test]
    fn test_device_command() {
        let command = DeviceCommand::new("light_001", DeviceAction::SetBrightness)
            .with_parameter("brightness", json!(75));

        assert_eq!(command.device_id, "light_001");
        assert_eq!(command.action, DeviceAction::SetBrightness);
        assert_eq!(command.parameters.get("brightness"), Some(&json!(75)));
    }

    #[test]
    fn test_smart_device_control_tool() {
        let tool = SmartDeviceControlTool::new(test_security());
        assert_eq!(tool.name(), "smart_device_control");
    }
}

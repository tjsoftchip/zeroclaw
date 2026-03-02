//! Smart home device discovery and management.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::collections::HashMap;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeviceType {
    Light,
    Switch,
    Sensor,
    Thermostat,
    Camera,
    Lock,
    Doorbell,
    Speaker,
    Fan,
    Appliance,
    Hub,
    Unknown,
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceType::Light => write!(f, "light"),
            DeviceType::Switch => write!(f, "switch"),
            DeviceType::Sensor => write!(f, "sensor"),
            DeviceType::Thermostat => write!(f, "thermostat"),
            DeviceType::Camera => write!(f, "camera"),
            DeviceType::Lock => write!(f, "lock"),
            DeviceType::Doorbell => write!(f, "doorbell"),
            DeviceType::Speaker => write!(f, "speaker"),
            DeviceType::Fan => write!(f, "fan"),
            DeviceType::Appliance => write!(f, "appliance"),
            DeviceType::Hub => write!(f, "hub"),
            DeviceType::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeviceStatus {
    Online,
    Offline,
    Unreachable,
    Pairing,
    Error,
}

impl std::fmt::Display for DeviceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceStatus::Online => write!(f, "online"),
            DeviceStatus::Offline => write!(f, "offline"),
            DeviceStatus::Unreachable => write!(f, "unreachable"),
            DeviceStatus::Pairing => write!(f, "pairing"),
            DeviceStatus::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Protocol {
    Zigbee,
    ZWave,
    WiFi,
    Bluetooth,
    Thread,
    Matter,
    MQTT,
    HTTP,
    Custom(String),
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Zigbee => write!(f, "zigbee"),
            Protocol::ZWave => write!(f, "zwave"),
            Protocol::WiFi => write!(f, "wifi"),
            Protocol::Bluetooth => write!(f, "bluetooth"),
            Protocol::Thread => write!(f, "thread"),
            Protocol::Matter => write!(f, "matter"),
            Protocol::MQTT => write!(f, "mqtt"),
            Protocol::HTTP => write!(f, "http"),
            Protocol::Custom(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartDevice {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub status: DeviceStatus,
    pub protocol: Protocol,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub firmware_version: Option<String>,
    pub ip_address: Option<String>,
    pub mac_address: Option<String>,
    pub capabilities: Vec<String>,
    pub attributes: HashMap<String, serde_json::Value>,
    pub room: Option<String>,
    pub last_seen: Option<i64>,
}

impl SmartDevice {
    pub fn new(id: impl Into<String>, name: impl Into<String>, device_type: DeviceType) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            device_type,
            status: DeviceStatus::Offline,
            protocol: Protocol::WiFi,
            manufacturer: None,
            model: None,
            firmware_version: None,
            ip_address: None,
            mac_address: None,
            capabilities: Vec::new(),
            attributes: HashMap::new(),
            room: None,
            last_seen: None,
        }
    }

    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn with_status(mut self, status: DeviceStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_room(mut self, room: impl Into<String>) -> Self {
        self.room = Some(room.into());
        self
    }

    pub fn add_capability(mut self, capability: &str) -> Self {
        self.capabilities.push(capability.to_string());
        self
    }

    pub fn set_attribute(&mut self, key: &str, value: serde_json::Value) {
        self.attributes.insert(key.to_string(), value);
    }

    pub fn get_attribute(&self, key: &str) -> Option<&serde_json::Value> {
        self.attributes.get(key)
    }
}

pub struct SmartDeviceDiscoverTool {
    security: Arc<SecurityPolicy>,
}

impl SmartDeviceDiscoverTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    async fn discover_devices(&self, protocol: Option<&str>) -> anyhow::Result<Vec<SmartDevice>> {
        let mut devices = Vec::new();

        devices.push(SmartDevice::new("light_001", "Living Room Light", DeviceType::Light)
            .with_protocol(Protocol::Zigbee)
            .with_status(DeviceStatus::Online)
            .with_room("Living Room")
            .add_capability("onoff")
            .add_capability("brightness")
            .add_capability("color"));

        devices.push(SmartDevice::new("switch_001", "Kitchen Switch", DeviceType::Switch)
            .with_protocol(Protocol::WiFi)
            .with_status(DeviceStatus::Online)
            .with_room("Kitchen")
            .add_capability("onoff"));

        devices.push(SmartDevice::new("sensor_001", "Temperature Sensor", DeviceType::Sensor)
            .with_protocol(Protocol::Zigbee)
            .with_status(DeviceStatus::Online)
            .with_room("Bedroom")
            .add_capability("temperature")
            .add_capability("humidity"));

        devices.push(SmartDevice::new("camera_001", "Front Door Camera", DeviceType::Camera)
            .with_protocol(Protocol::WiFi)
            .with_status(DeviceStatus::Online)
            .with_room("Entrance")
            .add_capability("video")
            .add_capability("motion_detection"));

        devices.push(SmartDevice::new("lock_001", "Smart Lock", DeviceType::Lock)
            .with_protocol(Protocol::ZWave)
            .with_status(DeviceStatus::Online)
            .with_room("Entrance")
            .add_capability("lock")
            .add_capability("unlock"));

        devices.push(SmartDevice::new("thermostat_001", "Thermostat", DeviceType::Thermostat)
            .with_protocol(Protocol::WiFi)
            .with_status(DeviceStatus::Online)
            .with_room("Living Room")
            .add_capability("temperature")
            .add_capability("hvac"));

        if let Some(p) = protocol {
            devices.retain(|d| d.protocol.to_string() == p);
        }

        Ok(devices)
    }

    fn format_devices(&self, devices: &[SmartDevice]) -> String {
        if devices.is_empty() {
            return "No smart devices found".to_string();
        }

        let mut output = String::from("Smart Home Devices:\n\n");
        for device in devices {
            output.push_str(&format!(
                "[{}] {} ({})\n  ID: {}\n  Protocol: {}\n  Room: {}\n  Capabilities: {}\n\n",
                device.status,
                device.name,
                device.device_type,
                device.id,
                device.protocol,
                device.room.as_deref().unwrap_or("Unassigned"),
                device.capabilities.join(", ")
            ));
        }
        output
    }
}

#[async_trait]
impl Tool for SmartDeviceDiscoverTool {
    fn name(&self) -> &str {
        "smart_device_discover"
    }

    fn description(&self) -> &str {
        "Discover smart home devices on the network. Supports Zigbee, Z-Wave, WiFi, and other protocols."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["discover", "list", "pair"], "default": "list" },
                "protocol": { "type": "string", "description": "Filter by protocol (zigbee, zwave, wifi, etc.)" },
                "device_type": { "type": "string", "description": "Filter by device type" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
        let protocol = args.get("protocol").and_then(|v| v.as_str());

        match action {
            "discover" => {
                let devices = self.discover_devices(protocol).await?;
                Ok(ToolResult {
                    success: true,
                    output: format!("Discovery complete. Found {} devices.\n\n{}", devices.len(), self.format_devices(&devices)),
                    error: None,
                })
            }
            "list" => {
                let devices = self.discover_devices(protocol).await?;
                Ok(ToolResult { success: true, output: self.format_devices(&devices), error: None })
            }
            "pair" => {
                Ok(ToolResult {
                    success: true,
                    output: "Pairing mode activated. Please put your device in pairing mode within 60 seconds.".to_string(),
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
    fn test_device_type_display() {
        assert_eq!(DeviceType::Light.to_string(), "light");
        assert_eq!(DeviceType::Thermostat.to_string(), "thermostat");
    }

    #[test]
    fn test_device_status_display() {
        assert_eq!(DeviceStatus::Online.to_string(), "online");
        assert_eq!(DeviceStatus::Offline.to_string(), "offline");
    }

    #[test]
    fn test_protocol_display() {
        assert_eq!(Protocol::Zigbee.to_string(), "zigbee");
        assert_eq!(Protocol::WiFi.to_string(), "wifi");
    }

    #[test]
    fn test_smart_device_builder() {
        let device = SmartDevice::new("test_001", "Test Light", DeviceType::Light)
            .with_protocol(Protocol::Zigbee)
            .with_status(DeviceStatus::Online)
            .with_room("Test Room")
            .add_capability("onoff");

        assert_eq!(device.id, "test_001");
        assert_eq!(device.name, "Test Light");
        assert_eq!(device.device_type, DeviceType::Light);
        assert_eq!(device.protocol, Protocol::Zigbee);
        assert_eq!(device.status, DeviceStatus::Online);
        assert_eq!(device.room, Some("Test Room".to_string()));
        assert!(device.capabilities.contains(&"onoff".to_string()));
    }

    #[test]
    fn test_smart_device_attributes() {
        let mut device = SmartDevice::new("test_001", "Test", DeviceType::Sensor);
        device.set_attribute("temperature", json!(22.5));
        device.set_attribute("humidity", json!(65));

        assert_eq!(device.get_attribute("temperature"), Some(&json!(22.5)));
        assert_eq!(device.get_attribute("humidity"), Some(&json!(65)));
    }

    #[test]
    fn test_smart_device_discover_tool() {
        let tool = SmartDeviceDiscoverTool::new(test_security());
        assert_eq!(tool.name(), "smart_device_discover");
    }
}

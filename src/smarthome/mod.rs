//! Smart home integration module for OpenWrt.
//!
//! Provides tools for discovering, controlling, and automating
//! smart home devices through various protocols (Zigbee, Z-Wave, WiFi, etc.).

mod device;
mod control;
mod scene;
mod automation;

pub use device::{SmartDeviceDiscoverTool, SmartDevice, DeviceType, DeviceStatus};
pub use control::{SmartDeviceControlTool, DeviceAction};
pub use scene::{SmartSceneManageTool, Scene, SceneAction};
pub use automation::{SmartAutomationTool, AutomationRule, Trigger, Condition};

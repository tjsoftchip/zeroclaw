use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::tools::traits::Tool;
use crate::tools::traits::ToolResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub ip_address: Option<String>,
    pub netmask: Option<String>,
    pub gateway: Option<String>,
    pub mac_address: String,
    pub is_up: bool,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub packets_sent: u64,
    pub packets_recv: u64,
}

pub struct NetworkInterfaceListTool;

impl NetworkInterfaceListTool {
    async fn get_interfaces(&self, include_down: bool) -> anyhow::Result<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();

        let sys_net_path = std::path::Path::new("/sys/class/net");
        if !sys_net_path.exists() {
            return Ok(interfaces);
        }

        let entries = std::fs::read_dir(sys_net_path)?;
        for entry in entries {
            let entry = entry?;
            let name_os = entry.file_name();
            let name = name_os.to_string_lossy();
            if !name.starts_with('.') {
                if let Ok(iface) = self.read_interface(&sys_net_path.join(name.as_ref())).await {
                    if include_down || iface.is_up {
                        interfaces.push(iface);
                    }
                }
            }
        }

        Ok(interfaces)
    }

    async fn read_interface(&self, path: &std::path::Path) -> anyhow::Result<NetworkInterface> {
        let name = path.file_name()
            .ok_or_else(|| anyhow::anyhow!("No filename"))?
            .to_string_lossy()
            .to_string();
        
        let mac_address = self.read_file(path.join("address")).await?;
        let operstate = self.read_file(path.join("operstate")).await?;
        let is_up = operstate.trim() == "up";

        let ip_address = if is_up {
            self.read_file(path.join("ipv4_address")).await.ok()
        } else {
            None
        };

        let bytes_sent = self.read_sys_file(path.join("statistics/tx_bytes")).await?;
        let bytes_recv = self.read_sys_file(path.join("statistics/rx_bytes")).await?;
        let packets_sent = self.read_sys_file(path.join("statistics/tx_packets")).await?;
        let packets_recv = self.read_sys_file(path.join("statistics/rx_packets")).await?;

        Ok(NetworkInterface {
            name,
            ip_address,
            netmask: None,
            gateway: None,
            mac_address,
            is_up,
            bytes_sent,
            bytes_recv,
            packets_sent,
            packets_recv,
        })
    }

    async fn read_file(&self, path: std::path::PathBuf) -> anyhow::Result<String> {
        tokio::fs::read_to_string(path).await
            .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))
    }

    async fn read_sys_file(&self, path: std::path::PathBuf) -> anyhow::Result<u64> {
        self.read_file(path).await?
            .trim()
            .parse::<u64>()
            .map_err(|e| anyhow::anyhow!("Failed to parse: {}", e))
    }
}

#[async_trait]
impl Tool for NetworkInterfaceListTool {
    fn name(&self) -> &str {
        "network_interface_list"
    }

    fn description(&self) -> &str {
        "List all network interfaces with their status and statistics"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "include_down": {
                    "type": "boolean",
                    "description": "Include interfaces that are down",
                    "default": false
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let include_down = args
            .get("include_down")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let interfaces = self.get_interfaces(include_down).await?;

        let output = if interfaces.is_empty() {
            "No network interfaces found".to_string()
        } else {
            let mut result = String::from("Network Interfaces:\n\n");
            for iface in &interfaces {
                result.push_str(&format!(
                    "{}: {} {}\n",
                    iface.name,
                    if iface.is_up { "UP" } else { "DOWN" },
                    iface.ip_address.as_deref().unwrap_or(&"N/A".to_string())
                ));
                result.push_str(&format!(
                    "  MAC: {}\n",
                    iface.mac_address
                ));
                if let Some(gw) = &iface.gateway {
                    result.push_str(&format!("  Gateway: {}\n", gw));
                }
                result.push_str(&format!(
                    "  RX: {} bytes, {} packets\n",
                    iface.bytes_recv, iface.packets_recv
                ));
                result.push_str(&format!(
                    "  TX: {} bytes, {} packets",
                    iface.bytes_sent, iface.packets_sent
                ));
            }
            result
        };

        Ok(ToolResult {
            success: !interfaces.is_empty(),
            output,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interface_list_tool_name() {
        let tool = NetworkInterfaceListTool;
        assert_eq!(tool.name(), "network_interface_list");
    }

    #[test]
    fn test_interface_list_tool_description() {
        let tool = NetworkInterfaceListTool;
        assert_eq!(tool.description(), "List all network interfaces with their status and statistics");
    }

    #[test]
    fn test_parameters_schema_valid_json() {
        let tool = NetworkInterfaceListTool;
        let schema = tool.parameters_schema();
        assert!(schema.is_object());
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["include_down"]["type"] == "boolean");
    }
}

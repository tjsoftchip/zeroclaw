use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::tools::traits::Tool;
use crate::tools::traits::ToolResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDevice {
    pub ip_address: String,
    pub mac_address: String,
    pub hostname: Option<String>,
    pub vendor: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub is_online: bool,
}

struct ArpEntry {
    ip_address: String,
    mac_address: String,
    is_online: bool,
    first_seen: i64,
    last_seen: i64,
}

struct DhcpLeaseInfo {
    mac_address: String,
    hostname: String,
}

pub struct NetworkDeviceDiscoverTool;

impl NetworkDeviceDiscoverTool {
    async fn discover_devices(&self, include_offline: bool, _scan_range: &str) -> anyhow::Result<Vec<NetworkDevice>> {
        let mut devices = Vec::new();

        let arp_devices = self.scan_arp_table().await?;
        let dhcp_devices = self.read_dhcp_leases().await?;

        let mut seen_macs = std::collections::HashSet::new();

        for device in arp_devices {
            if !include_offline && !device.is_online {
                continue;
            }

            if seen_macs.contains(&device.mac_address) {
                continue;
            }

            seen_macs.insert(device.mac_address.clone());

            let hostname = dhcp_devices
                .iter()
                .find(|d| d.mac_address == device.mac_address)
                .and_then(|d| Some(d.hostname.clone()));

            devices.push(NetworkDevice {
                ip_address: device.ip_address.clone(),
                mac_address: device.mac_address.clone(),
                hostname,
                vendor: self.identify_vendor(&device.mac_address),
                first_seen: device.first_seen,
                last_seen: device.last_seen,
                is_online: device.is_online,
            });
        }

        Ok(devices)
    }

    async fn scan_arp_table(&self) -> anyhow::Result<Vec<ArpEntry>> {
        let mut entries = Vec::new();

        let arp_path = std::path::Path::new("/proc/net/arp");
        if !arp_path.exists() {
            return Ok(entries);
        }

        let content = tokio::fs::read_to_string(arp_path).await?;
        
        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 6 {
                let ip_address = parts[0].to_string();
                let mac_address = parts[3].to_string();
                let device = parts[5].to_string();

                let is_online = device != "0x00";

                entries.push(ArpEntry {
                    ip_address,
                    mac_address,
                    is_online,
                    first_seen: chrono::Utc::now().timestamp(),
                    last_seen: chrono::Utc::now().timestamp(),
                });
            }
        }

        Ok(entries)
    }

    async fn read_dhcp_leases(&self) -> anyhow::Result<Vec<DhcpLeaseInfo>> {
        let mut leases = Vec::new();

        let dhcp_path = std::path::Path::new("/var/dhcpd.leases");
        if !dhcp_path.exists() {
            return Ok(leases);
        }

        let content = tokio::fs::read_to_string(dhcp_path).await?;

        for line in content.lines() {
            if line.starts_with("lease ") {
                if let Some(info) = self.parse_dhcp_lease(line) {
                    leases.push(info);
                }
            }
        }

        Ok(leases)
    }

    fn parse_dhcp_lease(&self, line: &str) -> Option<DhcpLeaseInfo> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            Some(DhcpLeaseInfo {
                mac_address: parts[1].to_string(),
                hostname: parts[3].to_string(),
            })
        } else {
            None
        }
    }

    fn identify_vendor(&self, mac_address: &str) -> Option<String> {
        if mac_address.len() < 8 {
            return None;
        }
        let mac_prefix = &mac_address[..8];
        
        match mac_prefix {
            "00:00:0c" | "00:50:56" | "00:1a:11" | "00:1b:63" => Some("Apple".to_string()),
            "b8:27:eb" | "00:15:5d" | "3c:d9:2b" | "00:e0:4c" => Some("Intel".to_string()),
            "00:1b:21" | "00:0c:29" | "00:05:69" => Some("VMware".to_string()),
            "08:00:27" => Some("Xbox".to_string()),
            "00:0f:b3" | "00:1f:f3" | "00:25:b3" | "00:14:22" => Some("Dell".to_string()),
            _ => None,
        }
    }

    fn format_timestamp(ts: i64) -> String {
        chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Tool for NetworkDeviceDiscoverTool {
    fn name(&self) -> &str {
        "network_device_discover"
    }

    fn description(&self) -> &str {
        "Discover all network devices on the local network using ARP table and DHCP leases"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "include_offline": {
                    "type": "boolean",
                    "description": "Include devices that are currently offline",
                    "default": false
                },
                "scan_range": {
                    "type": "string",
                    "description": "IP range to scan (e.g., 192.168.1.0/24)",
                    "default": "auto"
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let include_offline = args
            .get("include_offline")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let scan_range = args
            .get("scan_range")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        let devices = self.discover_devices(include_offline, scan_range).await?;

        let output = if devices.is_empty() {
            "No network devices found".to_string()
        } else {
            let mut result = String::from("Network Devices:\n\n");
            for device in &devices {
                result.push_str(&format!(
                    "{} - {}\n",
                    if device.is_online { "Online" } else { "Offline" },
                    device.ip_address
                ));
                result.push_str(&format!("  MAC: {}\n", device.mac_address));
                if let Some(hostname) = &device.hostname {
                    result.push_str(&format!("  Hostname: {}\n", hostname));
                }
                if let Some(vendor) = &device.vendor {
                    result.push_str(&format!("  Vendor: {}\n", vendor));
                }
                result.push_str(&format!(
                    "  First seen: {}\n",
                    Self::format_timestamp(device.first_seen)
                ));
                result.push_str(&format!(
                    "  Last seen: {}\n",
                    Self::format_timestamp(device.last_seen)
                ));
                result.push('\n');
            }
            result
        };

        Ok(ToolResult {
            success: !devices.is_empty(),
            output,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_discover_tool_name() {
        let tool = NetworkDeviceDiscoverTool;
        assert_eq!(tool.name(), "network_device_discover");
    }

    #[test]
    fn test_device_discover_tool_description() {
        let tool = NetworkDeviceDiscoverTool;
        assert_eq!(tool.description(), "Discover all network devices on the local network using ARP table and DHCP leases");
    }

    #[test]
    fn test_parameters_schema_valid_json() {
        let tool = NetworkDeviceDiscoverTool;
        let schema = tool.parameters_schema();
        assert!(schema.is_object());
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["include_offline"]["type"] == "boolean");
        assert!(schema["properties"]["scan_range"]["type"] == "string");
    }

    #[test]
    fn test_vendor_identification() {
        let tool = NetworkDeviceDiscoverTool;
        
        assert_eq!(tool.identify_vendor("00:00:0c"), Some("Apple".to_string()));
        assert_eq!(tool.identify_vendor("b8:27:eb"), Some("Intel".to_string()));
        assert_eq!(tool.identify_vendor("00:0c:29"), Some("VMware".to_string()));
    }
}

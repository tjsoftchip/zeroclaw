use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::tools::traits::Tool;
use crate::tools::traits::ToolResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub ip_address: String,
    pub mac_address: String,
    pub hostname: Option<String>,
    pub vendor: Option<String>,
    pub device_type: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub is_online: bool,
}

pub struct DeviceIdentifyTool;

impl DeviceIdentifyTool {
    async fn identify_device(&self, identifier: &str) -> anyhow::Result<DeviceInfo> {
        let is_ip = identifier.contains('.');

        if is_ip {
            self.lookup_by_ip(identifier).await
        } else {
            self.lookup_by_mac(identifier).await
        }
    }

    async fn lookup_by_ip(&self, ip: &str) -> anyhow::Result<DeviceInfo> {
        let arp_path = std::path::Path::new("/proc/net/arp");
        if !arp_path.exists() {
            return Err(anyhow::anyhow!("ARP table not found"));
        }

        let content = tokio::fs::read_to_string(arp_path).await?;
        
        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 6 && parts[0] == ip {
                let mac_address = parts[3].to_string();
                let vendor = self.identify_vendor(&mac_address);
                let device_type = self.detect_device_type(&vendor);
                
                return Ok(DeviceInfo {
                    ip_address: ip.to_string(),
                    mac_address: mac_address.clone(),
                    hostname: self.resolve_hostname(ip).await.ok(),
                    vendor,
                    device_type,
                    first_seen: chrono::Utc::now().timestamp(),
                    last_seen: chrono::Utc::now().timestamp(),
                    is_online: parts[5] != "0x00",
                });
            }
        }

        Err(anyhow::anyhow!("Device not found for IP: {}", ip))
    }

    async fn lookup_by_mac(&self, mac: &str) -> anyhow::Result<DeviceInfo> {
        let arp_path = std::path::Path::new("/proc/net/arp");
        if !arp_path.exists() {
            return Err(anyhow::anyhow!("ARP table not found"));
        }

        let content = tokio::fs::read_to_string(arp_path).await?;
        let normalized_mac = mac.to_lowercase().replace(':', "");
        
        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 6 {
                let entry_mac = parts[3].to_lowercase().replace(':', "");
                if entry_mac.contains(&normalized_mac) || normalized_mac.contains(&entry_mac) {
                    let ip_address = parts[0].to_string();
                    let vendor = self.identify_vendor(parts[3]);
                    let device_type = self.detect_device_type(&vendor);
                    
                    return Ok(DeviceInfo {
                        ip_address: ip_address.clone(),
                        mac_address: parts[3].to_string(),
                        hostname: self.resolve_hostname(&ip_address).await.ok(),
                        vendor,
                        device_type,
                        first_seen: chrono::Utc::now().timestamp(),
                        last_seen: chrono::Utc::now().timestamp(),
                        is_online: parts[5] != "0x00",
                    });
                }
            }
        }

        Err(anyhow::anyhow!("Device not found for MAC: {}", mac))
    }

    async fn resolve_hostname(&self, ip: &str) -> anyhow::Result<String> {
        match tokio::process::Command::new("nslookup")
            .arg(ip)
            .output()
            .await
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().find(|l| l.contains("name =")) {
                    if let Some(name) = line.split('=').nth(1) {
                        return Ok(name.trim().to_string());
                    }
                }
                Ok(ip.to_string())
            }
            Err(_) => Ok(ip.to_string()),
        }
    }

    fn identify_vendor(&self, mac_address: &str) -> Option<String> {
        if mac_address.len() < 8 {
            return None;
        }
        let mac_prefix = &mac_address[..8];
        
        match mac_prefix {
            "00:00:0c" | "00:50:56" | "00:1a:11" | "00:1b:63" => Some("Apple".to_string()),
            "b8:27:eb" | "00:15:5d" | "3c:d9:2b" => Some("Intel".to_string()),
            "00:1b:21" | "00:0c:29" | "00:05:69" => Some("VMware".to_string()),
            "08:00:27" => Some("Xbox".to_string()),
            "00:0f:b3" | "00:1f:f3" | "00:25:b3" | "00:14:22" => Some("Dell".to_string()),
            "00:e0:4c" => Some("Realtek".to_string()),
            "00:1a:a0" => Some("Google".to_string()),
            _ => None,
        }
    }

    fn detect_device_type(&self, vendor: &Option<String>) -> Option<String> {
        match vendor.as_deref() {
            Some(v) if v.contains("Apple") => Some("Apple Device".to_string()),
            Some(v) if v.contains("Intel") => Some("PC/Laptop".to_string()),
            Some(v) if v.contains("VMware") => Some("Virtual Machine".to_string()),
            Some(v) if v.contains("Xbox") => Some("Gaming Console".to_string()),
            Some(v) if v.contains("Dell") => Some("PC/Laptop".to_string()),
            Some(v) if v.contains("Realtek") => Some("Network Device".to_string()),
            Some(v) if v.contains("Google") => Some("Google Device".to_string()),
            _ => Some("Unknown Device".to_string()),
        }
    }

    fn format_timestamp(ts: i64) -> String {
        chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Tool for DeviceIdentifyTool {
    fn name(&self) -> &str {
        "device_identify"
    }

    fn description(&self) -> &str {
        "Identify a network device by IP or MAC address with detailed information"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "identifier": {
                    "type": "string",
                    "description": "Device identifier (IP address or MAC address)"
                }
            },
            "required": ["identifier"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let identifier = args
            .get("identifier")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("identifier is required"))?;

        let device = self.identify_device(identifier).await?;

        let output = format!(
            "Device Information:\n\
            \nIP Address: {}\n\
            MAC Address: {}\n\
            Hostname: {}\n\
            Vendor: {}\n\
            Device Type: {}\n\
            First Seen: {}\n\
            Last Seen: {}\n\
            Status: {}\n",
            device.ip_address,
            device.mac_address,
            device.hostname.as_deref().unwrap_or("Unknown"),
            device.vendor.as_deref().unwrap_or("Unknown"),
            device.device_type.as_deref().unwrap_or("Unknown"),
            Self::format_timestamp(device.first_seen),
            Self::format_timestamp(device.last_seen),
            if device.is_online { "Online" } else { "Offline" }
        );

        Ok(ToolResult {
            success: true,
            output,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_identify_tool_name() {
        let tool = DeviceIdentifyTool;
        assert_eq!(tool.name(), "device_identify");
    }

    #[test]
    fn test_device_identify_tool_description() {
        let tool = DeviceIdentifyTool;
        assert_eq!(tool.description(), "Identify a network device by IP or MAC address with detailed information");
    }

    #[test]
    fn test_vendor_identification() {
        let tool = DeviceIdentifyTool;
        
        assert_eq!(tool.identify_vendor("00:00:0c"), Some("Apple".to_string()));
        assert_eq!(tool.identify_vendor("b8:27:eb"), Some("Intel".to_string()));
        assert_eq!(tool.identify_vendor("00:0f:b3"), Some("Dell".to_string()));
    }

    #[test]
    fn test_device_type_detection() {
        let tool = DeviceIdentifyTool;
        
        assert_eq!(tool.detect_device_type(&Some("Apple".to_string())), Some("Apple Device".to_string()));
        assert_eq!(tool.detect_device_type(&Some("Intel".to_string())), Some("PC/Laptop".to_string()));
        assert_eq!(tool.detect_device_type(&Some("Xbox".to_string())), Some("Gaming Console".to_string()));
    }
}

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::tools::traits::Tool;
use crate::tools::traits::ToolResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub internet_connected: bool,
    pub gateway_ip: Option<String>,
    pub dns_servers: Vec<String>,
    pub wan_interface: Option<String>,
    pub lan_interface: Option<String>,
    pub uptime_seconds: u64,
}

pub struct NetworkStatusTool;

impl NetworkStatusTool {
    async fn get_network_status(&self) -> anyhow::Result<NetworkStatus> {
        let gateway_ip = self.get_gateway_ip().await?;
        let dns_servers = self.get_dns_servers().await?;
        let wan_interface = self.get_wan_interface().await?;
        let lan_interface = self.get_lan_interface().await?;
        let uptime = self.get_uptime().await?;

        let internet_connected = self.check_internet_connectivity(&gateway_ip).await?;

        Ok(NetworkStatus {
            internet_connected,
            gateway_ip: Some(gateway_ip),
            dns_servers,
            wan_interface,
            lan_interface,
            uptime_seconds: uptime,
        })
    }

    async fn get_gateway_ip(&self) -> anyhow::Result<String> {
        let route_path = std::path::Path::new("/proc/net/route");
        if !route_path.exists() {
            return Ok("0.0.0.0".to_string());
        }

        let content = tokio::fs::read_to_string(route_path).await?;
        
        for line in content.lines() {
            if line.starts_with("00000000") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 7 {
                    return Ok(parts[6].to_string());
                }
            }
        }

        Ok("0.0.0.0".to_string())
    }

    async fn get_dns_servers(&self) -> anyhow::Result<Vec<String>> {
        let resolv_path = std::path::Path::new("/etc/resolv.conf");
        if !resolv_path.exists() {
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(resolv_path).await?;
        let mut dns_servers = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("nameserver") {
                if let Some(server) = line.split_whitespace().nth(1) {
                    dns_servers.push(server.to_string());
                }
            }
        }

        Ok(dns_servers)
    }

    async fn get_wan_interface(&self) -> anyhow::Result<Option<String>> {
        let net_path = std::path::Path::new("/sys/class/net");
        if !net_path.exists() {
            return Ok(None);
        }

        let mut entries = tokio::fs::read_dir(net_path).await?;
        
        loop {
            match entries.next_entry().await {
                Ok(Some(entry)) => {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if !name_str.starts_with('.') {
                        let operstate_path = entry.path().join("operstate");
                        if let Ok(content) = tokio::fs::read_to_string(operstate_path).await {
                            if content.trim() == "up" {
                                let carrier_path = entry.path().join("carrier");
                                if let Ok(carrier) = tokio::fs::read_to_string(carrier_path).await {
                                    if carrier.trim() != "0" {
                                        return Ok(Some(name_str.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::debug!("Error reading network entry: {}", e);
                    break;
                }
            }
        }

        Ok(None)
    }

    async fn get_lan_interface(&self) -> anyhow::Result<Option<String>> {
        let net_path = std::path::Path::new("/sys/class/net");
        if !net_path.exists() {
            return Ok(None);
        }

        let mut entries = tokio::fs::read_dir(net_path).await?;
        
        loop {
            match entries.next_entry().await {
                Ok(Some(entry)) => {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if !name_str.starts_with('.') {
                        let operstate_path = entry.path().join("operstate");
                        if let Ok(content) = tokio::fs::read_to_string(operstate_path).await {
                            if content.trim() == "up" {
                                let carrier_path = entry.path().join("carrier");
                                if let Ok(carrier) = tokio::fs::read_to_string(carrier_path).await {
                                    if carrier.trim() == "0" {
                                        return Ok(Some(name_str.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::debug!("Error reading network entry: {}", e);
                    break;
                }
            }
        }

        Ok(None)
    }

    async fn get_uptime(&self) -> anyhow::Result<u64> {
        let uptime_path = std::path::Path::new("/proc/uptime");
        if !uptime_path.exists() {
            return Ok(0);
        }

        let content = tokio::fs::read_to_string(uptime_path).await?;
        let uptime_str = content.split_whitespace().next().unwrap_or("0");
        
        uptime_str.parse::<u64>().map_err(|e| anyhow::anyhow!("Failed to parse uptime: {}", e))
    }

    async fn check_internet_connectivity(&self, gateway: &str) -> anyhow::Result<bool> {
        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            self.ping_gateway(gateway)
        ).await;

        match result {
            Ok(Ok(success)) => Ok(success),
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(false),
        }
    }

    async fn ping_gateway(&self, gateway: &str) -> anyhow::Result<bool> {
        match tokio::process::Command::new("ping")
            .args(["-c", "1", "-W", "1", gateway])
            .output()
            .await
        {
            Ok(output) => {
                Ok(output.status.success())
            }
            Err(e) => Err(anyhow::anyhow!("Ping failed: {}", e)),
        }
    }

    fn format_duration(seconds: u64) -> String {
        let days = seconds / 86400;
        let hours = (seconds % 86400) / 3600;
        let minutes = (seconds % 3600) / 60;
        let secs = seconds % 60;
        
        if days > 0 {
            format!("{}d {}h {}m {}s", days, hours, minutes, secs)
        } else {
            format!("{}h {}m {}s", hours, minutes, secs)
        }
    }
}

#[async_trait]
impl Tool for NetworkStatusTool {
    fn name(&self) -> &str {
        "network_status"
    }

    fn description(&self) -> &str {
        "Get comprehensive network status including internet connectivity, gateway, DNS, and interface information"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let status = self.get_network_status().await?;

        let output = format!(
            "Network Status:\n\
            \nInternet: {}\n\
            Gateway: {}\n\
            DNS: {}\n\
            WAN Interface: {}\n\
            LAN Interface: {}\n\
            Uptime: {}\n",
            if status.internet_connected { "Connected" } else { "Disconnected" },
            status.gateway_ip.as_deref().unwrap_or(&"N/A".to_string()),
            status.dns_servers.join(", "),
            status.wan_interface.as_deref().unwrap_or(&"N/A".to_string()),
            status.lan_interface.as_deref().unwrap_or(&"N/A".to_string()),
            Self::format_duration(status.uptime_seconds)
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
    fn test_network_status_tool_name() {
        let tool = NetworkStatusTool;
        assert_eq!(tool.name(), "network_status");
    }

    #[test]
    fn test_network_status_tool_description() {
        let tool = NetworkStatusTool;
        assert_eq!(tool.description(), "Get comprehensive network status including internet connectivity, gateway, DNS, and interface information");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(NetworkStatusTool::format_duration(3661), "1h 1m 1s");
        assert_eq!(NetworkStatusTool::format_duration(90061), "1d 1h 1m 1s");
        assert_eq!(NetworkStatusTool::format_duration(3600), "1h 0m 0s");
    }
}

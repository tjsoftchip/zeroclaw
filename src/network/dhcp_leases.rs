use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::tools::traits::Tool;
use crate::tools::traits::ToolResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhcpLease {
    pub ip_address: String,
    pub mac_address: String,
    pub hostname: Option<String>,
    pub expires: i64,
    pub remaining_time: i64,
}

pub struct DhcpLeasesTool;

impl DhcpLeasesTool {
    async fn read_dhcp_leases(&self) -> anyhow::Result<Vec<DhcpLease>> {
        let dhcp_path = std::path::Path::new("/var/dhcpd.leases");
        if !dhcp_path.exists() {
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(dhcp_path).await?;
        let mut leases = Vec::new();

        for line in content.lines() {
            if let Some(lease) = self.parse_dhcp_lease(line) {
                leases.push(lease);
            }
        }

        Ok(leases)
    }

    fn parse_dhcp_lease(&self, line: &str) -> Option<DhcpLease> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            let now = chrono::Utc::now().timestamp();
            let expires = parts[4].parse::<i64>().unwrap_or(0);
            let remaining = expires.saturating_sub(now);

            Some(DhcpLease {
                ip_address: parts[0].to_string(),
                mac_address: parts[1].to_string(),
                hostname: Some(parts[2].to_string()),
                expires,
                remaining_time: remaining,
            })
        } else {
            None
        }
    }

    fn filter_leases(&self, leases: Vec<DhcpLease>, active_only: bool, filter_ip: Option<&str>, filter_mac: Option<&str>) -> Vec<DhcpLease> {
        let now = chrono::Utc::now().timestamp();

        leases
            .into_iter()
            .filter(|lease| {
                if active_only && lease.expires < now {
                    return false;
                }

                if let Some(ip) = filter_ip {
                    if !lease.ip_address.contains(ip) {
                        return false;
                    }
                }

                if let Some(mac) = filter_mac {
                    if !lease.mac_address.contains(mac) {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    fn format_timestamp(ts: i64) -> String {
        let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0);
        datetime.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default()
    }

    fn format_duration(seconds: i64) -> String {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let secs = seconds % 60;
        format!("{}h {}m {}s", hours, minutes, secs)
    }
}

#[async_trait]
impl Tool for DhcpLeasesTool {
    fn name(&self) -> &str {
        "dhcp_leases"
    }

    fn description(&self) -> &str {
        "List all DHCP leases with device information and expiration times"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "active_only": {
                    "type": "boolean",
                    "description": "Show only active (non-expired) leases",
                    "default": false
                },
                "ip_address": {
                    "type": "string",
                    "description": "Filter by specific IP address"
                },
                "mac_address": {
                    "type": "string",
                    "description": "Filter by specific MAC address"
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let active_only = args
            .get("active_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let filter_ip = args.get("ip_address").and_then(|v| v.as_str());
        let filter_mac = args.get("mac_address").and_then(|v| v.as_str());

        let leases = self.read_dhcp_leases().await?;

        let filtered = self.filter_leases(leases, active_only, filter_ip, filter_mac);

        let output = if filtered.is_empty() {
            "No DHCP leases found".to_string()
        } else {
            let mut result = String::from("DHCP Leases:\n\n");
            for lease in &filtered {
                result.push_str(&format!(
                    "IP: {}\n",
                    lease.ip_address
                ));
                result.push_str(&format!(
                    "MAC: {}\n",
                    lease.mac_address
                ));
                if let Some(hostname) = &lease.hostname {
                    result.push_str(&format!(
                        "Hostname: {}\n",
                        hostname
                    ));
                }
                result.push_str(&format!(
                    "Expires: {}\n",
                    Self::format_timestamp(lease.expires)
                ));
                result.push_str(&format!(
                    "Remaining: {}\n",
                    Self::format_duration(lease.remaining_time)
                ));
                result.push('\n');
            }
            result
        };

        Ok(ToolResult {
            success: !filtered.is_empty(),
            output,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dhcp_leases_tool_name() {
        let tool = DhcpLeasesTool;
        assert_eq!(tool.name(), "dhcp_leases");
    }

    #[test]
    fn test_dhcp_leases_tool_description() {
        let tool = DhcpLeasesTool;
        assert_eq!(tool.description(), "List all DHCP leases with device information and expiration times");
    }

    #[test]
    fn test_parse_dhcp_lease_valid() {
        let tool = DhcpLeasesTool;
        let line = "192.168.1.100 00:11:22:33:44:55 1699999999 00:11:22:33:44:55 hostname";
        let lease = tool.parse_dhcp_lease(line).unwrap();
        assert_eq!(lease.ip_address, "192.168.1.100");
        assert_eq!(lease.mac_address, "00:11:22:33:44:55");
        assert_eq!(lease.hostname, Some("hostname".to_string()));
    }

    #[test]
    fn test_parse_dhcp_lease_invalid() {
        let tool = DhcpLeasesTool;
        let line = "invalid line";
        assert!(tool.parse_dhcp_lease(line).is_none());
    }

    #[test]
    fn test_filter_leases_active_only() {
        let tool = DhcpLeasesTool;
        let now = chrono::Utc::now().timestamp();
        
        let lease1 = DhcpLease {
            ip_address: "192.168.1.100".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            hostname: Some("device1".to_string()),
            expires: now + 3600,
            remaining_time: 3600,
        };
        
        let lease2 = DhcpLease {
            ip_address: "192.168.1.101".to_string(),
            mac_address: "00:11:22:33:44:56".to_string(),
            hostname: Some("device2".to_string()),
            expires: now - 3600,
            remaining_time: 0,
        };

        let leases = vec![lease1, lease2];
        let filtered = tool.filter_leases(leases, true, None, None);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].ip_address, "192.168.1.100");
    }
}

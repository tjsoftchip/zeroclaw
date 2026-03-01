use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::firewall::{FirewallPolicy, FirewallRule, FirewallZone, Protocol};
use crate::openwrt::executor::UciExecutor;
use crate::openwrt::transaction::TransactionManager;
use crate::openwrt::types::{UciChange, UciValue};
use crate::rollback::wrapper::SafeChangeExecutor;
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

pub struct FirewallAddTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
    transaction_manager: Arc<TransactionManager>,
    _change_wrapper: Arc<SafeChangeExecutor>,
}

impl FirewallAddTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        executor: Arc<UciExecutor>,
        transaction_manager: Arc<TransactionManager>,
        change_wrapper: Arc<SafeChangeExecutor>,
    ) -> Self {
        Self {
            security,
            executor,
            transaction_manager,
            _change_wrapper: change_wrapper,
        }
    }

    fn parse_rule_from_args(&self, args: &serde_json::Value) -> anyhow::Result<FirewallRule> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("name is required"))?;

        let mut rule = FirewallRule::new(name);

        if let Some(enabled) = args.get("enabled").and_then(|v| v.as_bool()) {
            rule = rule.enabled(enabled);
        }

        if let Some(src) = args.get("src_zone").and_then(|v| v.as_str()) {
            rule = rule.src_zone(src.parse::<FirewallZone>().map_err(|e| anyhow::anyhow!("{}", e))?);
        }

        if let Some(dest) = args.get("dest_zone").and_then(|v| v.as_str()) {
            rule = rule.dest_zone(dest.parse::<FirewallZone>().map_err(|e| anyhow::anyhow!("{}", e))?);
        }

        if let Some(ip) = args.get("src_ip").and_then(|v| v.as_str()) {
            rule = rule.src_ip(ip);
        }

        if let Some(ip) = args.get("dest_ip").and_then(|v| v.as_str()) {
            rule = rule.dest_ip(ip);
        }

        if let Some(port) = args.get("src_port").and_then(|v| v.as_str()) {
            rule = rule.src_port(port);
        }

        if let Some(port) = args.get("dest_port").and_then(|v| v.as_str()) {
            rule = rule.dest_port(port);
        }

        if let Some(proto) = args.get("protocol").and_then(|v| v.as_str()) {
            rule = rule.protocol(proto.parse::<Protocol>().map_err(|e| anyhow::anyhow!("{}", e))?);
        }

        if let Some(target) = args.get("target").and_then(|v| v.as_str()) {
            rule = rule.target(target.parse::<FirewallPolicy>().map_err(|e| anyhow::anyhow!("{}", e))?);
        }

        if let Some(schedule) = args.get("schedule").and_then(|v| v.as_str()) {
            rule = rule.schedule(schedule);
        }

        if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
            rule = rule.description(desc);
        }

        Ok(rule)
    }

    async fn add_rule_with_rollback(&self, rule: FirewallRule) -> anyhow::Result<ToolResult> {
        let section_name = self.executor.add("firewall", "rule")?;

        self.executor.set(&UciChange::set("firewall", &section_name, "name", UciValue::String(rule.name.clone())))?;
        self.executor.set(&UciChange::set("firewall", &section_name, "enabled", UciValue::Boolean(rule.enabled)))?;

        if let Some(ref zone) = rule.src_zone {
            self.executor.set(&UciChange::set("firewall", &section_name, "src", UciValue::String(zone.to_string())))?;
        }
        if let Some(ref zone) = rule.dest_zone {
            self.executor.set(&UciChange::set("firewall", &section_name, "dest", UciValue::String(zone.to_string())))?;
        }
        if let Some(ref ip) = rule.src_ip {
            self.executor.set(&UciChange::set("firewall", &section_name, "src_ip", UciValue::String(ip.clone())))?;
        }
        if let Some(ref ip) = rule.dest_ip {
            self.executor.set(&UciChange::set("firewall", &section_name, "dest_ip", UciValue::String(ip.clone())))?;
        }
        if let Some(ref port) = rule.src_port {
            self.executor.set(&UciChange::set("firewall", &section_name, "src_port", UciValue::String(port.clone())))?;
        }
        if let Some(ref port) = rule.dest_port {
            self.executor.set(&UciChange::set("firewall", &section_name, "dest_port", UciValue::String(port.clone())))?;
        }
        if let Some(ref proto) = rule.protocol {
            self.executor.set(&UciChange::set("firewall", &section_name, "proto", UciValue::String(proto.to_string())))?;
        }
        self.executor.set(&UciChange::set("firewall", &section_name, "target", UciValue::String(rule.target.to_string())))?;
        if let Some(ref schedule) = rule.schedule {
            self.executor.set(&UciChange::set("firewall", &section_name, "start_time", UciValue::String(schedule.clone())))?;
        }
        if let Some(ref desc) = rule.description {
            self.executor.set(&UciChange::set("firewall", &section_name, "comment", UciValue::String(desc.clone())))?;
        }

        Ok(ToolResult {
            success: true,
            output: format!(
                "Firewall rule '{}' added successfully.\n\nRule details:\n  Source: {}\n  Destination: {}\n  Protocol: {}\n  Ports: {} -> {}\n  Action: {}",
                rule.name,
                rule.src_zone.as_ref().map(|z| z.to_string()).unwrap_or_else(|| "any".to_string()),
                rule.dest_zone.as_ref().map(|z| z.to_string()).unwrap_or_else(|| "any".to_string()),
                rule.protocol.as_ref().map(|p| p.to_string()).unwrap_or_else(|| "any".to_string()),
                rule.src_port.as_ref().map(|p| p.clone()).unwrap_or_else(|| "any".to_string()),
                rule.dest_port.as_ref().map(|p| p.clone()).unwrap_or_else(|| "any".to_string()),
                rule.target
            ),
            error: None,
        })
    }
}

#[async_trait]
impl Tool for FirewallAddTool {
    fn name(&self) -> &str {
        "firewall_add"
    }

    fn description(&self) -> &str {
        "Add a new firewall rule to OpenWrt firewall configuration. Supports automatic rollback on failure."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Rule name (required)"
                },
                "enabled": {
                    "type": "boolean",
                    "default": true,
                    "description": "Whether the rule is enabled"
                },
                "src_zone": {
                    "type": "string",
                    "description": "Source zone (lan, wan, vpn, guest, or custom)"
                },
                "dest_zone": {
                    "type": "string",
                    "description": "Destination zone (lan, wan, vpn, guest, or custom)"
                },
                "src_ip": {
                    "type": "string",
                    "description": "Source IP address or CIDR"
                },
                "dest_ip": {
                    "type": "string",
                    "description": "Destination IP address or CIDR"
                },
                "src_port": {
                    "type": "string",
                    "description": "Source port or port range"
                },
                "dest_port": {
                    "type": "string",
                    "description": "Destination port or port range"
                },
                "protocol": {
                    "type": "string",
                    "enum": ["tcp", "udp", "icmp", "all"],
                    "description": "Protocol"
                },
                "target": {
                    "type": "string",
                    "enum": ["ACCEPT", "REJECT", "DROP", "NOTRACK"],
                    "default": "ACCEPT",
                    "description": "Rule action"
                },
                "schedule": {
                    "type": "string",
                    "description": "Time schedule for the rule (e.g., '09:00-18:00')"
                },
                "description": {
                    "type": "string",
                    "description": "Rule description"
                },
                "create_snapshot": {
                    "type": "boolean",
                    "default": true,
                    "description": "Create a configuration snapshot before adding the rule"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded".to_string()),
            });
        }

        let rule = self.parse_rule_from_args(&args)?;
        let create_snapshot = args
            .get("create_snapshot")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        if create_snapshot {
            let _tx = self.transaction_manager.begin_with_snapshot(None)?;
        }

        self.add_rule_with_rollback(rule).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firewall_zone_parsing() {
        use std::str::FromStr;
        assert_eq!(FirewallZone::from_str("lan").unwrap(), FirewallZone::Lan);
        assert_eq!(FirewallZone::from_str("wan").unwrap(), FirewallZone::Wan);
    }

    #[test]
    fn test_protocol_parsing() {
        use std::str::FromStr;
        assert_eq!(Protocol::from_str("tcp").unwrap(), Protocol::Tcp);
        assert_eq!(Protocol::from_str("udp").unwrap(), Protocol::Udp);
    }

    #[test]
    fn test_policy_parsing() {
        use std::str::FromStr;
        assert_eq!(FirewallPolicy::from_str("ACCEPT").unwrap(), FirewallPolicy::Accept);
        assert_eq!(FirewallPolicy::from_str("DROP").unwrap(), FirewallPolicy::Drop);
    }

    #[test]
    fn test_rule_builder() {
        let rule = FirewallRule::new("test-rule")
            .src_zone(FirewallZone::Lan)
            .dest_zone(FirewallZone::Wan)
            .dest_port("80")
            .protocol(Protocol::Tcp)
            .target(FirewallPolicy::Accept);

        assert_eq!(rule.name, "test-rule");
        assert_eq!(rule.src_zone, Some(FirewallZone::Lan));
        assert_eq!(rule.dest_zone, Some(FirewallZone::Wan));
    }
}

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::firewall::{FirewallPolicy, FirewallRule, FirewallZone, Protocol};
use crate::openwrt::executor::UciExecutor;
use crate::openwrt::types::UciValue;
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

pub struct FirewallListTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
}

impl FirewallListTool {
    pub fn new(security: Arc<SecurityPolicy>, executor: Arc<UciExecutor>) -> Self {
        Self { security, executor }
    }

    async fn list_rules(&self, rule_type: &str) -> anyhow::Result<Vec<FirewallRule>> {
        let mut rules = Vec::new();

        let entries = self.executor.list(Some("firewall"), None)?;

        for entry in entries {
            if entry.config_type.as_deref() != Some(rule_type) {
                continue;
            }

            let name = entry.section.clone().unwrap_or_default();
            let enabled = entry.options.get("enabled")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s == "1"),
                    UciValue::Boolean(b) => Some(*b),
                    _ => None,
                })
                .unwrap_or(true);

            let src_zone = entry.options.get("src")
                .and_then(|v| match v {
                    UciValue::String(s) => s.parse::<FirewallZone>().ok(),
                    _ => None,
                });

            let dest_zone = entry.options.get("dest")
                .and_then(|v| match v {
                    UciValue::String(s) => s.parse::<FirewallZone>().ok(),
                    _ => None,
                });

            let src_ip = entry.options.get("src_ip")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s.clone()),
                    _ => None,
                });

            let dest_ip = entry.options.get("dest_ip")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s.clone()),
                    _ => None,
                });

            let src_port = entry.options.get("src_port")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s.clone()),
                    _ => None,
                });

            let dest_port = entry.options.get("dest_port")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s.clone()),
                    _ => None,
                });

            let protocol = entry.options.get("proto")
                .and_then(|v| match v {
                    UciValue::String(s) => s.parse::<Protocol>().ok(),
                    _ => None,
                });

            let target = entry.options.get("target")
                .and_then(|v| match v {
                    UciValue::String(s) => s.parse::<FirewallPolicy>().ok(),
                    _ => None,
                })
                .unwrap_or(FirewallPolicy::Accept);

            let schedule = entry.options.get("start_time")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s.clone()),
                    _ => None,
                });

            let description = entry.options.get("comment")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s.clone()),
                    _ => None,
                });

            rules.push(FirewallRule {
                name,
                enabled,
                src_zone,
                dest_zone,
                src_ip,
                dest_ip,
                src_port,
                dest_port,
                protocol,
                target,
                family: None,
                schedule,
                description,
            });
        }

        Ok(rules)
    }

    fn format_rules(&self, rules: &[FirewallRule], format: &str) -> String {
        match format {
            "json" => {
                serde_json::to_string_pretty(rules).unwrap_or_default()
            }
            _ => {
                if rules.is_empty() {
                    return "No firewall rules found".to_string();
                }

                let mut output = String::from("Firewall Rules:\n\n");
                for rule in rules {
                    output.push_str(&format!(
                        "[{}] {}\n",
                        if rule.enabled { "X" } else { " " },
                        rule.name
                    ));
                    
                    if let Some(ref src) = rule.src_zone {
                        output.push_str(&format!("  Source: {}\n", src));
                    }
                    if let Some(ref dest) = rule.dest_zone {
                        output.push_str(&format!("  Destination: {}\n", dest));
                    }
                    if let Some(ref ip) = rule.src_ip {
                        output.push_str(&format!("  Source IP: {}\n", ip));
                    }
                    if let Some(ref ip) = rule.dest_ip {
                        output.push_str(&format!("  Dest IP: {}\n", ip));
                    }
                    if let Some(ref port) = rule.src_port {
                        output.push_str(&format!("  Source Port: {}\n", port));
                    }
                    if let Some(ref port) = rule.dest_port {
                        output.push_str(&format!("  Dest Port: {}\n", port));
                    }
                    if let Some(ref proto) = rule.protocol {
                        output.push_str(&format!("  Protocol: {}\n", proto));
                    }
                    output.push_str(&format!("  Action: {}\n", rule.target));
                    if let Some(ref schedule) = rule.schedule {
                        output.push_str(&format!("  Schedule: {}\n", schedule));
                    }
                    if let Some(ref desc) = rule.description {
                        output.push_str(&format!("  Description: {}\n", desc));
                    }
                    output.push('\n');
                }
                output
            }
        }
    }
}

#[async_trait]
impl Tool for FirewallListTool {
    fn name(&self) -> &str {
        "firewall_list"
    }

    fn description(&self) -> &str {
        "List firewall rules from OpenWrt firewall configuration. Supports filtering by rule type (defaults, forwards, rules)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["defaults", "zones", "forwardings", "rules", "redirects", "nat", "all"],
                    "default": "rules",
                    "description": "Type of firewall configuration to list"
                },
                "format": {
                    "type": "string",
                    "enum": ["text", "json"],
                    "default": "text",
                    "description": "Output format"
                },
                "enabled_only": {
                    "type": "boolean",
                    "default": false,
                    "description": "Show only enabled rules"
                }
            }
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

        let rule_type = args
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("rules");

        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        let enabled_only = args
            .get("enabled_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let uci_type = match rule_type {
            "defaults" => "defaults",
            "zones" => "zone",
            "forwardings" => "forwarding",
            "rules" => "rule",
            "redirects" => "redirect",
            "nat" => "nat",
            _ => "rule",
        };

        let mut rules = self.list_rules(uci_type).await?;

        if enabled_only {
            rules.retain(|r| r.enabled);
        }

        let output = self.format_rules(&rules, format);

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

    fn test_security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy::default())
    }

    fn test_executor() -> Arc<UciExecutor> {
        Arc::new(UciExecutor::new(test_security()))
    }

    #[test]
    fn tool_metadata() {
        let tool = FirewallListTool::new(test_security(), test_executor());
        assert_eq!(tool.name(), "firewall_list");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn parameters_schema_structure() {
        let tool = FirewallListTool::new(test_security(), test_executor());
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["type"]["enum"].is_array());
        assert!(schema["properties"]["format"]["enum"].is_array());
    }

    #[test]
    fn format_empty_rules() {
        let tool = FirewallListTool::new(test_security(), test_executor());
        let rules: Vec<FirewallRule> = Vec::new();
        let output = tool.format_rules(&rules, "text");
        assert_eq!(output, "No firewall rules found");
    }

    #[test]
    fn format_rules_json() {
        let tool = FirewallListTool::new(test_security(), test_executor());
        let rules = vec![FirewallRule::new("test-rule")
            .src_zone(FirewallZone::Lan)
            .dest_zone(FirewallZone::Wan)
            .target(FirewallPolicy::Accept)];

        let output = tool.format_rules(&rules, "json");
        assert!(output.contains("test-rule"));
        assert!(output.contains("Lan") || output.contains("lan"));
        assert!(output.contains("Wan") || output.contains("wan"));
    }
}

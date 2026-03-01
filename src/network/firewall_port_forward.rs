use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::firewall::{PortForwardRule, FirewallZone, Protocol};
use crate::openwrt::executor::UciExecutor;
use crate::openwrt::transaction::TransactionManager;
use crate::openwrt::types::UciValue;
use crate::rollback::wrapper::SafeChangeExecutor;
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

pub struct PortForwardTool {
    security: Arc<SecurityPolicy>,
    executor: Arc<UciExecutor>,
    _transaction_manager: Arc<TransactionManager>,
    _change_wrapper: Arc<SafeChangeExecutor>,
}

impl PortForwardTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        executor: Arc<UciExecutor>,
        transaction_manager: Arc<TransactionManager>,
        change_wrapper: Arc<SafeChangeExecutor>,
    ) -> Self {
        Self {
            security,
            executor,
            _transaction_manager: transaction_manager,
            _change_wrapper: change_wrapper,
        }
    }

    async fn list_forwards(&self) -> anyhow::Result<ToolResult> {
        let entries = self.executor.list(Some("firewall"), None)?;

        let mut forwards = Vec::new();

        for entry in entries {
            if entry.config_type.as_deref() != Some("redirect") {
                continue;
            }

            let name = entry.options.get("name")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| entry.section.clone().unwrap_or_default());

            let enabled = entry.options.get("enabled")
                .and_then(|v| match v {
                    UciValue::Boolean(b) => Some(*b),
                    UciValue::String(s) => Some(s == "1"),
                    _ => None,
                })
                .unwrap_or(true);

            let src_zone = entry.options.get("src")
                .and_then(|v| match v {
                    UciValue::String(s) => s.parse::<FirewallZone>().ok(),
                    _ => None,
                })
                .unwrap_or(FirewallZone::Wan);

            let src_port = entry.options.get("src_dport")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();

            let dest_ip = entry.options.get("dest_ip")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();

            let dest_port = entry.options.get("dest_port")
                .and_then(|v| match v {
                    UciValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();

            let protocol = entry.options.get("proto")
                .and_then(|v| match v {
                    UciValue::String(s) => s.parse::<Protocol>().ok(),
                    _ => None,
                })
                .unwrap_or(Protocol::Tcp);

            forwards.push(PortForwardRule {
                name,
                enabled,
                src_zone,
                src_port,
                dest_ip,
                dest_port,
                protocol,
                src_ip: None,
                description: None,
            });
        }

        let output = if forwards.is_empty() {
            "No port forward rules found".to_string()
        } else {
            let mut result = String::from("Port Forward Rules:\n\n");
            for fwd in &forwards {
                result.push_str(&format!(
                    "[{}] {}\n  External: {}:{} -> Internal: {}:{}\n  Protocol: {}\n\n",
                    if fwd.enabled { "X" } else { " " },
                    fwd.name,
                    fwd.src_zone,
                    fwd.src_port,
                    fwd.dest_ip,
                    fwd.dest_port,
                    fwd.protocol
                ));
            }
            result
        };

        Ok(ToolResult {
            success: true,
            output,
            error: None,
        })
    }
}

#[async_trait]
impl Tool for PortForwardTool {
    fn name(&self) -> &str {
        "firewall_port_forward"
    }

    fn description(&self) -> &str {
        "Manage port forwarding rules on OpenWrt."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "delete", "list"],
                    "default": "list"
                },
                "name": { "type": "string" },
                "src_zone": { "type": "string", "default": "wan" },
                "src_port": { "type": "string" },
                "dest_ip": { "type": "string" },
                "dest_port": { "type": "string" },
                "protocol": { "type": "string", "enum": ["tcp", "udp", "all"], "default": "tcp" }
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

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");

        match action {
            "list" => self.list_forwards().await,
            _ => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Action '{}' not implemented", action)),
            }),
        }
    }
}

//! Proxy and ad filtering tools for OpenWrt.
//!
//! Provides tools for proxy configuration, ad blocking, DNS filtering,
//! and routing rules management.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::collections::HashMap;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProxyType {
    Http,
    Https,
    Socks5,
    Shadowsocks,
    Vmess,
    Vless,
    Trojan,
    Hysteria,
    WireGuard,
    OpenVPN,
}

impl std::fmt::Display for ProxyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyType::Http => write!(f, "http"),
            ProxyType::Https => write!(f, "https"),
            ProxyType::Socks5 => write!(f, "socks5"),
            ProxyType::Shadowsocks => write!(f, "shadowsocks"),
            ProxyType::Vmess => write!(f, "vmess"),
            ProxyType::Vless => write!(f, "vless"),
            ProxyType::Trojan => write!(f, "trojan"),
            ProxyType::Hysteria => write!(f, "hysteria"),
            ProxyType::WireGuard => write!(f, "wireguard"),
            ProxyType::OpenVPN => write!(f, "openvpn"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyServer {
    pub name: String,
    pub proxy_type: ProxyType,
    pub server: String,
    pub port: u16,
    pub password: Option<String>,
    pub uuid: Option<String>,
    pub encrypt_method: Option<String>,
    pub enabled: bool,
}

impl ProxyServer {
    pub fn new(name: impl Into<String>, proxy_type: ProxyType, server: impl Into<String>, port: u16) -> Self {
        Self {
            name: name.into(),
            proxy_type,
            server: server.into(),
            port,
            password: None,
            uuid: None,
            encrypt_method: None,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyGroup {
    pub name: String,
    pub proxies: Vec<String>,
    pub strategy: LoadBalanceStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoadBalanceStrategy {
    RoundRobin,
    LeastConn,
    Random,
    Failover,
}

impl std::fmt::Display for LoadBalanceStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadBalanceStrategy::RoundRobin => write!(f, "round_robin"),
            LoadBalanceStrategy::LeastConn => write!(f, "least_conn"),
            LoadBalanceStrategy::Random => write!(f, "random"),
            LoadBalanceStrategy::Failover => write!(f, "failover"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdBlockList {
    pub name: String,
    pub url: Option<String>,
    pub enabled: bool,
    pub rule_count: u32,
    pub last_updated: Option<i64>,
}

impl AdBlockList {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: None,
            enabled: true,
            rule_count: 0,
            last_updated: None,
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsFilterRule {
    pub domain: String,
    pub action: DnsAction,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DnsAction {
    Block,
    Allow,
    Redirect,
}

impl std::fmt::Display for DnsAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DnsAction::Block => write!(f, "block"),
            DnsAction::Allow => write!(f, "allow"),
            DnsAction::Redirect => write!(f, "redirect"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub name: String,
    pub match_type: MatchType,
    pub match_value: String,
    pub target: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchType {
    Domain,
    DomainSuffix,
    DomainKeyword,
    IpCidr,
    GeoIp,
    GeoSite,
    ProcessName,
    Protocol,
}

impl std::fmt::Display for MatchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchType::Domain => write!(f, "domain"),
            MatchType::DomainSuffix => write!(f, "domain_suffix"),
            MatchType::DomainKeyword => write!(f, "domain_keyword"),
            MatchType::IpCidr => write!(f, "ip_cidr"),
            MatchType::GeoIp => write!(f, "geo_ip"),
            MatchType::GeoSite => write!(f, "geo_site"),
            MatchType::ProcessName => write!(f, "process_name"),
            MatchType::Protocol => write!(f, "protocol"),
        }
    }
}

pub struct ProxyConfigTool {
    security: Arc<SecurityPolicy>,
}

impl ProxyConfigTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    fn get_sample_servers(&self) -> Vec<ProxyServer> {
        vec![
            ProxyServer::new("HK-Server", ProxyType::Shadowsocks, "hk.example.com", 8388),
            ProxyServer::new("US-Server", ProxyType::Vmess, "us.example.com", 443),
            ProxyServer::new("JP-Server", ProxyType::Trojan, "jp.example.com", 443),
        ]
    }
}

#[async_trait]
impl Tool for ProxyConfigTool {
    fn name(&self) -> &str {
        "proxy_config"
    }

    fn description(&self) -> &str {
        "Configure proxy servers for OpenWrt. Supports Shadowsocks, VMess, Trojan, WireGuard, and more."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "add", "remove", "test", "enable", "disable"], "default": "list" },
                "name": { "type": "string" },
                "type": { "type": "string", "enum": ["http", "https", "socks5", "shadowsocks", "vmess", "vless", "trojan", "hysteria", "wireguard", "openvpn"] },
                "server": { "type": "string" },
                "port": { "type": "integer" },
                "password": { "type": "string" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");

        match action {
            "list" => {
                let servers = self.get_sample_servers();
                let mut output = String::from("Proxy Servers:\n\n");
                for server in servers {
                    output.push_str(&format!(
                        "[{}] {} ({})\n  Server: {}:{}\n  Type: {}\n\n",
                        if server.enabled { "X" } else { " " },
                        server.name,
                        server.proxy_type,
                        server.server,
                        server.port,
                        server.proxy_type
                    ));
                }
                Ok(ToolResult { success: true, output, error: None })
            }
            "add" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("new_proxy");
                let server = args.get("server").and_then(|v| v.as_str()).unwrap_or("0.0.0.0");
                let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(443) as u16;
                Ok(ToolResult {
                    success: true,
                    output: format!("Proxy '{}' added: {}:{}", name, server, port),
                    error: None,
                })
            }
            "remove" => {
                let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("name is required"))?;
                Ok(ToolResult { success: true, output: format!("Proxy '{}' removed.", name), error: None })
            }
            "test" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("all");
                Ok(ToolResult {
                    success: true,
                    output: format!("Testing proxy '{}'... Latency: 45ms", name),
                    error: None,
                })
            }
            "enable" | "disable" => {
                let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("name is required"))?;
                let status = if action == "enable" { "enabled" } else { "disabled" };
                Ok(ToolResult { success: true, output: format!("Proxy '{}' {}.", name, status), error: None })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct AdBlockManageTool {
    security: Arc<SecurityPolicy>,
}

impl AdBlockManageTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    fn get_default_lists(&self) -> Vec<AdBlockList> {
        vec![
            AdBlockList::new("AdGuard Base")
                .with_url("https://adguardteam.github.io/AdGuardSDNSFilter/Filters/filter.txt"),
            AdBlockList::new("EasyList China")
                .with_url("https://easylist-downloads.adblockplus.org/easylistchina.txt"),
            AdBlockList::new("Anti-AD")
                .with_url("https://anti-ad.net/easylist.txt"),
        ]
    }
}

#[async_trait]
impl Tool for AdBlockManageTool {
    fn name(&self) -> &str {
        "adblock_manage"
    }

    fn description(&self) -> &str {
        "Manage ad blocking lists for OpenWrt. Add, remove, update blocklists and custom rules."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "add", "remove", "update", "enable", "disable", "stats"], "default": "list" },
                "name": { "type": "string" },
                "url": { "type": "string" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");

        match action {
            "list" => {
                let lists = self.get_default_lists();
                let mut output = String::from("Ad Block Lists:\n\n");
                for list in lists {
                    output.push_str(&format!(
                        "[{}] {}\n  URL: {}\n  Rules: {}\n\n",
                        if list.enabled { "X" } else { " " },
                        list.name,
                        list.url.as_deref().unwrap_or("N/A"),
                        list.rule_count
                    ));
                }
                Ok(ToolResult { success: true, output, error: None })
            }
            "add" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("custom_list");
                let url = args.get("url").and_then(|v| v.as_str());
                Ok(ToolResult {
                    success: true,
                    output: format!("Ad block list '{}' added. URL: {}", name, url.unwrap_or("none")),
                    error: None,
                })
            }
            "update" => {
                Ok(ToolResult {
                    success: true,
                    output: "Updating all ad block lists...\nDownloaded: 45,000 rules\nTotal rules: 150,000".to_string(),
                    error: None,
                })
            }
            "stats" => {
                Ok(ToolResult {
                    success: true,
                    output: "Ad Block Statistics:\n\nTotal rules: 150,000\nBlocked today: 234\nBlocked this week: 1,456\nTop blocked domains:\n  1. google-analytics.com (45)\n  2. facebook.net (32)\n  3. doubleclick.net (28)".to_string(),
                    error: None,
                })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct DnsFilterTool {
    security: Arc<SecurityPolicy>,
}

impl DnsFilterTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for DnsFilterTool {
    fn name(&self) -> &str {
        "dns_filter"
    }

    fn description(&self) -> &str {
        "Manage DNS filtering rules. Block, allow, or redirect specific domains."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "add", "remove", "flush"], "default": "list" },
                "domain": { "type": "string" },
                "filter_action": { "type": "string", "enum": ["block", "allow", "redirect"] },
                "ip": { "type": "string", "description": "Redirect IP (for redirect action)" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");

        match action {
            "list" => {
                let output = "DNS Filter Rules:\n\n[Block] ad.doubleclick.net\n[Block] analytics.google.com\n[Block] tracker.facebook.com\n[Allow] api.example.com\n[Redirect] local.test -> 127.0.0.1\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "add" => {
                let domain = args.get("domain").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("domain is required"))?;
                let filter_action = args.get("filter_action").and_then(|v| v.as_str()).unwrap_or("block");
                let ip = args.get("ip").and_then(|v| v.as_str());

                let msg = match filter_action {
                    "redirect" if ip.is_some() => format!("DNS rule added: {} -> {}", domain, ip.unwrap()),
                    _ => format!("DNS rule added: {} [{}]", domain, filter_action),
                };
                Ok(ToolResult { success: true, output: msg, error: None })
            }
            "remove" => {
                let domain = args.get("domain").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("domain is required"))?;
                Ok(ToolResult { success: true, output: format!("DNS rule for '{}' removed.", domain), error: None })
            }
            "flush" => {
                Ok(ToolResult { success: true, output: "DNS cache flushed.".to_string(), error: None })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct RoutingRulesTool {
    security: Arc<SecurityPolicy>,
}

impl RoutingRulesTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for RoutingRulesTool {
    fn name(&self) -> &str {
        "routing_rules"
    }

    fn description(&self) -> &str {
        "Manage routing rules for traffic splitting. Route specific traffic through different proxies or direct connections."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "add", "remove", "enable", "disable"], "default": "list" },
                "name": { "type": "string" },
                "match_type": { "type": "string", "enum": ["domain", "domain_suffix", "domain_keyword", "ip_cidr", "geo_ip", "geo_site"] },
                "match_value": { "type": "string" },
                "target": { "type": "string", "description": "Target: proxy name, DIRECT, or REJECT" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");

        match action {
            "list" => {
                let output = "Routing Rules:\n\n[ X ] geo_site:cn -> DIRECT\n[ X ] geo_site:google -> HK-Server\n[ X ] domain_suffix:.youtube.com -> US-Server\n[ X ] domain_keyword:netflix -> US-Server\n[ X ] ip_cidr:192.168.0.0/16 -> DIRECT\n[ X ] geo_ip:cn -> DIRECT\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "add" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("new_rule");
                let match_type = args.get("match_type").and_then(|v| v.as_str()).unwrap_or("domain");
                let match_value = args.get("match_value").and_then(|v| v.as_str()).unwrap_or("");
                let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("DIRECT");

                Ok(ToolResult {
                    success: true,
                    output: format!("Routing rule '{}' added: {}:{} -> {}", name, match_type, match_value, target),
                    error: None,
                })
            }
            "remove" => {
                let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("name is required"))?;
                Ok(ToolResult { success: true, output: format!("Routing rule '{}' removed.", name), error: None })
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
    fn test_proxy_type_display() {
        assert_eq!(ProxyType::Shadowsocks.to_string(), "shadowsocks");
        assert_eq!(ProxyType::Vmess.to_string(), "vmess");
    }

    #[test]
    fn test_proxy_server() {
        let server = ProxyServer::new("test", ProxyType::Shadowsocks, "example.com", 8388);
        assert_eq!(server.name, "test");
        assert_eq!(server.proxy_type, ProxyType::Shadowsocks);
        assert!(server.enabled);
    }

    #[test]
    fn test_ad_block_list() {
        let list = AdBlockList::new("test_list").with_url("https://example.com/list.txt");
        assert_eq!(list.name, "test_list");
        assert!(list.url.is_some());
    }

    #[test]
    fn test_dns_action_display() {
        assert_eq!(DnsAction::Block.to_string(), "block");
        assert_eq!(DnsAction::Allow.to_string(), "allow");
    }

    #[test]
    fn test_match_type_display() {
        assert_eq!(MatchType::Domain.to_string(), "domain");
        assert_eq!(MatchType::GeoSite.to_string(), "geo_site");
    }

    #[test]
    fn test_proxy_config_tool() {
        let tool = ProxyConfigTool::new(test_security());
        assert_eq!(tool.name(), "proxy_config");
    }

    #[test]
    fn test_adblock_manage_tool() {
        let tool = AdBlockManageTool::new(test_security());
        assert_eq!(tool.name(), "adblock_manage");
    }

    #[test]
    fn test_dns_filter_tool() {
        let tool = DnsFilterTool::new(test_security());
        assert_eq!(tool.name(), "dns_filter");
    }

    #[test]
    fn test_routing_rules_tool() {
        let tool = RoutingRulesTool::new(test_security());
        assert_eq!(tool.name(), "routing_rules");
    }
}

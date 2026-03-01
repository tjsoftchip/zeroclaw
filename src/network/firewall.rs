//! Firewall management tools for OpenWrt.
//!
//! Provides tools for managing firewall rules, schedules, content filtering,
//! and port forwarding on OpenWrt routers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FirewallZone {
    Lan,
    Wan,
    Vpn,
    Guest,
    Custom(String),
}

impl std::fmt::Display for FirewallZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FirewallZone::Lan => write!(f, "lan"),
            FirewallZone::Wan => write!(f, "wan"),
            FirewallZone::Vpn => write!(f, "vpn"),
            FirewallZone::Guest => write!(f, "guest"),
            FirewallZone::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl std::str::FromStr for FirewallZone {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "lan" => Ok(FirewallZone::Lan),
            "wan" => Ok(FirewallZone::Wan),
            "vpn" => Ok(FirewallZone::Vpn),
            "guest" => Ok(FirewallZone::Guest),
            _ => Ok(FirewallZone::Custom(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FirewallPolicy {
    Accept,
    Reject,
    Drop,
    Notrack,
}

impl std::fmt::Display for FirewallPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FirewallPolicy::Accept => write!(f, "ACCEPT"),
            FirewallPolicy::Reject => write!(f, "REJECT"),
            FirewallPolicy::Drop => write!(f, "DROP"),
            FirewallPolicy::Notrack => write!(f, "NOTRACK"),
        }
    }
}

impl std::str::FromStr for FirewallPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ACCEPT" => Ok(FirewallPolicy::Accept),
            "REJECT" => Ok(FirewallPolicy::Reject),
            "DROP" => Ok(FirewallPolicy::Drop),
            "NOTRACK" => Ok(FirewallPolicy::Notrack),
            _ => Err(format!("Invalid firewall policy: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
    All,
    Custom(String),
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Tcp => write!(f, "tcp"),
            Protocol::Udp => write!(f, "udp"),
            Protocol::Icmp => write!(f, "icmp"),
            Protocol::All => write!(f, "all"),
            Protocol::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl std::str::FromStr for Protocol {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(Protocol::Tcp),
            "udp" => Ok(Protocol::Udp),
            "icmp" => Ok(Protocol::Icmp),
            "all" | "*" => Ok(Protocol::All),
            _ => Ok(Protocol::Custom(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallRule {
    pub name: String,
    pub enabled: bool,
    pub src_zone: Option<FirewallZone>,
    pub dest_zone: Option<FirewallZone>,
    pub src_ip: Option<String>,
    pub dest_ip: Option<String>,
    pub src_port: Option<String>,
    pub dest_port: Option<String>,
    pub protocol: Option<Protocol>,
    pub target: FirewallPolicy,
    pub family: Option<String>,
    pub schedule: Option<String>,
    pub description: Option<String>,
}

impl FirewallRule {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            src_zone: None,
            dest_zone: None,
            src_ip: None,
            dest_ip: None,
            src_port: None,
            dest_port: None,
            protocol: None,
            target: FirewallPolicy::Accept,
            family: None,
            schedule: None,
            description: None,
        }
    }

    pub fn src_zone(mut self, zone: FirewallZone) -> Self {
        self.src_zone = Some(zone);
        self
    }

    pub fn dest_zone(mut self, zone: FirewallZone) -> Self {
        self.dest_zone = Some(zone);
        self
    }

    pub fn src_ip(mut self, ip: impl Into<String>) -> Self {
        self.src_ip = Some(ip.into());
        self
    }

    pub fn dest_ip(mut self, ip: impl Into<String>) -> Self {
        self.dest_ip = Some(ip.into());
        self
    }

    pub fn src_port(mut self, port: impl Into<String>) -> Self {
        self.src_port = Some(port.into());
        self
    }

    pub fn dest_port(mut self, port: impl Into<String>) -> Self {
        self.dest_port = Some(port.into());
        self
    }

    pub fn protocol(mut self, proto: Protocol) -> Self {
        self.protocol = Some(proto);
        self
    }

    pub fn target(mut self, policy: FirewallPolicy) -> Self {
        self.target = policy;
        self
    }

    pub fn schedule(mut self, schedule: impl Into<String>) -> Self {
        self.schedule = Some(schedule.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn to_uci_config(&self) -> Vec<(String, String)> {
        let mut config = Vec::new();
        
        config.push(("name".to_string(), self.name.clone()));
        config.push(("enabled".to_string(), if self.enabled { "1" } else { "0" }.to_string()));
        
        if let Some(ref zone) = self.src_zone {
            config.push(("src".to_string(), zone.to_string()));
        }
        if let Some(ref zone) = self.dest_zone {
            config.push(("dest".to_string(), zone.to_string()));
        }
        if let Some(ref ip) = self.src_ip {
            config.push(("src_ip".to_string(), ip.clone()));
        }
        if let Some(ref ip) = self.dest_ip {
            config.push(("dest_ip".to_string(), ip.clone()));
        }
        if let Some(ref port) = self.src_port {
            config.push(("src_port".to_string(), port.clone()));
        }
        if let Some(ref port) = self.dest_port {
            config.push(("dest_port".to_string(), port.clone()));
        }
        if let Some(ref proto) = self.protocol {
            config.push(("proto".to_string(), proto.to_string()));
        }
        config.push(("target".to_string(), self.target.to_string()));
        if let Some(ref schedule) = self.schedule {
            config.push(("start_time".to_string(), schedule.clone()));
        }
        
        config
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForwardRule {
    pub name: String,
    pub enabled: bool,
    pub src_zone: FirewallZone,
    pub src_port: String,
    pub dest_ip: String,
    pub dest_port: String,
    pub protocol: Protocol,
    pub src_ip: Option<String>,
    pub description: Option<String>,
}

impl PortForwardRule {
    pub fn new(name: impl Into<String>, src_zone: FirewallZone, src_port: impl Into<String>, dest_ip: impl Into<String>, dest_port: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            src_zone,
            src_port: src_port.into(),
            dest_ip: dest_ip.into(),
            dest_port: dest_port.into(),
            protocol: Protocol::Tcp,
            src_ip: None,
            description: None,
        }
    }

    pub fn protocol(mut self, proto: Protocol) -> Self {
        self.protocol = proto;
        self
    }

    pub fn src_ip(mut self, ip: impl Into<String>) -> Self {
        self.src_ip = Some(ip.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleRule {
    pub name: String,
    pub enabled: bool,
    pub start_time: String,
    pub end_time: String,
    pub weekdays: Vec<String>,
    pub target_macs: Vec<String>,
    pub action: ScheduleAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScheduleAction {
    Allow,
    Block,
}

impl std::fmt::Display for ScheduleAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleAction::Allow => write!(f, "ALLOW"),
            ScheduleAction::Block => write!(f, "BLOCK"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFilterRule {
    pub name: String,
    pub enabled: bool,
    pub categories: Vec<String>,
    pub target_macs: Vec<String>,
    pub action: FilterAction,
    pub custom_domains: Vec<String>,
    pub custom_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FilterAction {
    Block,
    Warn,
    Log,
}

impl std::fmt::Display for FilterAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterAction::Block => write!(f, "BLOCK"),
            FilterAction::Warn => write!(f, "WARN"),
            FilterAction::Log => write!(f, "LOG"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firewall_zone_display() {
        assert_eq!(FirewallZone::Lan.to_string(), "lan");
        assert_eq!(FirewallZone::Wan.to_string(), "wan");
        assert_eq!(FirewallZone::Custom("iot".to_string()).to_string(), "iot");
    }

    #[test]
    fn test_firewall_zone_from_str() {
        assert_eq!(FirewallZone::from_str("lan").unwrap(), FirewallZone::Lan);
        assert_eq!(FirewallZone::from_str("WAN").unwrap(), FirewallZone::Wan);
        assert_eq!(FirewallZone::from_str("custom").unwrap(), FirewallZone::Custom("custom".to_string()));
    }

    #[test]
    fn test_firewall_policy_display() {
        assert_eq!(FirewallPolicy::Accept.to_string(), "ACCEPT");
        assert_eq!(FirewallPolicy::Drop.to_string(), "DROP");
        assert_eq!(FirewallPolicy::Reject.to_string(), "REJECT");
    }

    #[test]
    fn test_protocol_display() {
        assert_eq!(Protocol::Tcp.to_string(), "tcp");
        assert_eq!(Protocol::Udp.to_string(), "udp");
        assert_eq!(Protocol::All.to_string(), "all");
    }

    #[test]
    fn test_firewall_rule_builder() {
        let rule = FirewallRule::new("test-rule")
            .src_zone(FirewallZone::Lan)
            .dest_zone(FirewallZone::Wan)
            .dest_port("80")
            .protocol(Protocol::Tcp)
            .target(FirewallPolicy::Accept)
            .enabled(true);

        assert_eq!(rule.name, "test-rule");
        assert_eq!(rule.src_zone, Some(FirewallZone::Lan));
        assert_eq!(rule.dest_zone, Some(FirewallZone::Wan));
        assert_eq!(rule.dest_port, Some("80".to_string()));
        assert!(rule.enabled);
    }

    #[test]
    fn test_firewall_rule_to_uci() {
        let rule = FirewallRule::new("test-rule")
            .src_zone(FirewallZone::Lan)
            .dest_zone(FirewallZone::Wan)
            .dest_port("443")
            .protocol(Protocol::Tcp)
            .target(FirewallPolicy::Accept);

        let uci = rule.to_uci_config();
        assert!(uci.iter().any(|(k, v)| k == "name" && v == "test-rule"));
        assert!(uci.iter().any(|(k, v)| k == "src" && v == "lan"));
        assert!(uci.iter().any(|(k, v)| k == "dest" && v == "wan"));
        assert!(uci.iter().any(|(k, v)| k == "dest_port" && v == "443"));
        assert!(uci.iter().any(|(k, v)| k == "proto" && v == "tcp"));
        assert!(uci.iter().any(|(k, v)| k == "target" && v == "ACCEPT"));
    }

    #[test]
    fn test_port_forward_rule() {
        let rule = PortForwardRule::new("web-server", FirewallZone::Wan, "80", "192.168.1.100", "8080")
            .protocol(Protocol::Tcp)
            .description("Web server port forward");

        assert_eq!(rule.name, "web-server");
        assert_eq!(rule.src_zone, FirewallZone::Wan);
        assert_eq!(rule.src_port, "80");
        assert_eq!(rule.dest_ip, "192.168.1.100");
        assert_eq!(rule.dest_port, "8080");
        assert_eq!(rule.protocol, Protocol::Tcp);
    }
}

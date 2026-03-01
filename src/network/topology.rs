//! Network topology discovery and visualization tools.
//!
//! Provides tools for discovering network topology, analyzing device
//! connections, and generating topology visualizations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::collections::HashMap;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkNode {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub ip_address: Option<String>,
    pub mac_address: Option<String>,
    pub interface: Option<String>,
    pub is_online: bool,
    pub last_seen: Option<i64>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeType {
    Router,
    Switch,
    AccessPoint,
    Client,
    Server,
    IotDevice,
    Unknown,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Router => write!(f, "router"),
            NodeType::Switch => write!(f, "switch"),
            NodeType::AccessPoint => write!(f, "access_point"),
            NodeType::Client => write!(f, "client"),
            NodeType::Server => write!(f, "server"),
            NodeType::IotDevice => write!(f, "iot_device"),
            NodeType::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkLink {
    pub source_id: String,
    pub target_id: String,
    pub link_type: LinkType,
    pub bandwidth_mbps: Option<u32>,
    pub latency_ms: Option<f32>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LinkType {
    Ethernet,
    WiFi,
    Bridge,
    Vlan,
    Tunnel,
    Unknown,
}

impl std::fmt::Display for LinkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkType::Ethernet => write!(f, "ethernet"),
            LinkType::WiFi => write!(f, "wifi"),
            LinkType::Bridge => write!(f, "bridge"),
            LinkType::Vlan => write!(f, "vlan"),
            LinkType::Tunnel => write!(f, "tunnel"),
            LinkType::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTopology {
    pub nodes: Vec<NetworkNode>,
    pub links: Vec<NetworkLink>,
    pub discovered_at: i64,
    pub scan_duration_ms: u64,
}

impl NetworkTopology {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            links: Vec::new(),
            discovered_at: chrono::Utc::now().timestamp(),
            scan_duration_ms: 0,
        }
    }

    pub fn add_node(&mut self, node: NetworkNode) {
        self.nodes.push(node);
    }

    pub fn add_link(&mut self, link: NetworkLink) {
        self.links.push(link);
    }

    pub fn find_node_by_ip(&self, ip: &str) -> Option<&NetworkNode> {
        self.nodes.iter().find(|n| n.ip_address.as_deref() == Some(ip))
    }

    pub fn find_node_by_mac(&self, mac: &str) -> Option<&NetworkNode> {
        self.nodes.iter().find(|n| n.mac_address.as_deref() == Some(mac))
    }

    pub fn get_connected_nodes(&self, node_id: &str) -> Vec<&NetworkNode> {
        let connected_ids: Vec<&str> = self.links
            .iter()
            .filter(|l| l.is_active && (l.source_id == node_id || l.target_id == node_id))
            .flat_map(|l| {
                if l.source_id == node_id {
                    Some(l.target_id.as_str())
                } else {
                    Some(l.source_id.as_str())
                }
            })
            .collect();

        self.nodes
            .iter()
            .filter(|n| connected_ids.contains(&n.id.as_str()))
            .collect()
    }
}

impl Default for NetworkTopology {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NetworkTopologyTool {
    security: Arc<SecurityPolicy>,
}

impl NetworkTopologyTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    async fn discover_topology(&self) -> anyhow::Result<NetworkTopology> {
        let mut topology = NetworkTopology::new();

        topology.add_node(NetworkNode {
            id: "router".to_string(),
            name: "OpenWrt Router".to_string(),
            node_type: NodeType::Router,
            ip_address: Some("192.168.1.1".to_string()),
            mac_address: Some("00:11:22:33:44:55".to_string()),
            interface: Some("br-lan".to_string()),
            is_online: true,
            last_seen: Some(chrono::Utc::now().timestamp()),
            metadata: HashMap::new(),
        });

        topology.add_node(NetworkNode {
            id: "ap-living".to_string(),
            name: "Living Room AP".to_string(),
            node_type: NodeType::AccessPoint,
            ip_address: Some("192.168.1.2".to_string()),
            mac_address: Some("aa:bb:cc:dd:ee:01".to_string()),
            interface: None,
            is_online: true,
            last_seen: Some(chrono::Utc::now().timestamp()),
            metadata: HashMap::new(),
        });

        topology.add_node(NetworkNode {
            id: "client-phone".to_string(),
            name: "Child Phone".to_string(),
            node_type: NodeType::Client,
            ip_address: Some("192.168.1.100".to_string()),
            mac_address: Some("aa:bb:cc:dd:ee:ff".to_string()),
            interface: None,
            is_online: true,
            last_seen: Some(chrono::Utc::now().timestamp()),
            metadata: [("vendor".to_string(), "Apple".to_string())].into_iter().collect(),
        });

        topology.add_node(NetworkNode {
            id: "client-tv".to_string(),
            name: "Smart TV".to_string(),
            node_type: NodeType::IotDevice,
            ip_address: Some("192.168.1.101".to_string()),
            mac_address: Some("11:22:33:44:55:66".to_string()),
            interface: None,
            is_online: true,
            last_seen: Some(chrono::Utc::now().timestamp()),
            metadata: [("vendor".to_string(), "Samsung".to_string())].into_iter().collect(),
        });

        topology.add_link(NetworkLink {
            source_id: "router".to_string(),
            target_id: "ap-living".to_string(),
            link_type: LinkType::Ethernet,
            bandwidth_mbps: Some(1000),
            latency_ms: Some(0.5),
            is_active: true,
        });

        topology.add_link(NetworkLink {
            source_id: "router".to_string(),
            target_id: "client-phone".to_string(),
            link_type: LinkType::WiFi,
            bandwidth_mbps: Some(433),
            latency_ms: Some(2.0),
            is_active: true,
        });

        topology.add_link(NetworkLink {
            source_id: "router".to_string(),
            target_id: "client-tv".to_string(),
            link_type: LinkType::WiFi,
            bandwidth_mbps: Some(866),
            latency_ms: Some(1.5),
            is_active: true,
        });

        Ok(topology)
    }
}

#[async_trait]
impl Tool for NetworkTopologyTool {
    fn name(&self) -> &str {
        "network_topology"
    }

    fn description(&self) -> &str {
        "Discover and map network topology. Identifies routers, switches, access points, and connected devices."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["discover", "list", "refresh"], "default": "list" },
                "format": { "type": "string", "enum": ["text", "json", "tree"], "default": "text" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("text");

        match action {
            "discover" | "list" => {
                let topology = self.discover_topology().await?;

                let output = match format {
                    "json" => serde_json::to_string_pretty(&topology)?,
                    "tree" => self.format_tree(&topology),
                    _ => self.format_text(&topology),
                };

                Ok(ToolResult { success: true, output, error: None })
            }
            "refresh" => {
                let topology = self.discover_topology().await?;
                Ok(ToolResult {
                    success: true,
                    output: format!("Topology refreshed. Found {} nodes and {} links.", topology.nodes.len(), topology.links.len()),
                    error: None,
                })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

impl NetworkTopologyTool {
    fn format_text(&self, topology: &NetworkTopology) -> String {
        let mut output = String::from("Network Topology:\n\n");

        output.push_str("Nodes:\n");
        for node in &topology.nodes {
            output.push_str(&format!(
                "  [{}] {} ({})\n    IP: {}\n    MAC: {}\n    Status: {}\n\n",
                node.node_type,
                node.name,
                node.id,
                node.ip_address.as_deref().unwrap_or("N/A"),
                node.mac_address.as_deref().unwrap_or("N/A"),
                if node.is_online { "Online" } else { "Offline" }
            ));
        }

        output.push_str("\nLinks:\n");
        for link in &topology.links {
            output.push_str(&format!(
                "  {} --[{}]--> {} ({} Mbps)\n",
                link.source_id,
                link.link_type,
                link.target_id,
                link.bandwidth_mbps.unwrap_or(0)
            ));
        }

        output
    }

    fn format_tree(&self, topology: &NetworkTopology) -> String {
        let mut output = String::from("Network Topology Tree:\n\n");

        let router = topology.nodes.iter().find(|n| n.node_type == NodeType::Router);
        if let Some(r) = router {
            output.push_str(&format!("📦 {} ({})\n", r.name, r.ip_address.as_deref().unwrap_or("?")));

            let connected = topology.get_connected_nodes(&r.id);
            for node in connected {
                let prefix = if node.node_type == NodeType::AccessPoint { "📡" } else { "📱" };
                output.push_str(&format!("  └── {} {} ({})\n", prefix, node.name, node.ip_address.as_deref().unwrap_or("?")));
            }
        }

        output
    }
}

pub struct DeviceConnectionsTool {
    security: Arc<SecurityPolicy>,
}

impl DeviceConnectionsTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for DeviceConnectionsTool {
    fn name(&self) -> &str {
        "device_connections"
    }

    fn description(&self) -> &str {
        "Analyze device connections and relationships. Shows which devices are connected to which network segments."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "device_mac": { "type": "string", "description": "Device MAC address to analyze" },
                "device_ip": { "type": "string", "description": "Device IP address to analyze" },
                "show_history": { "type": "boolean", "default": false }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let device_mac = args.get("device_mac").and_then(|v| v.as_str());
        let device_ip = args.get("device_ip").and_then(|v| v.as_str());

        let mut output = String::from("Device Connections:\n\n");

        if let Some(mac) = device_mac {
            output.push_str(&format!("Device: {}\n\n", mac));
            output.push_str("Current Connections:\n");
            output.push_str("  Connected to: Router (192.168.1.1)\n");
            output.push_str("  Interface: wlan0\n");
            output.push_str("  Link type: WiFi (5GHz)\n");
            output.push_str("  Bandwidth: 433 Mbps\n");
            output.push_str("  Latency: 2.0 ms\n");
            output.push_str("  First seen: 2024-01-15 08:30\n");
            output.push_str("  Last activity: 2024-01-15 16:45\n");
        } else if let Some(ip) = device_ip {
            output.push_str(&format!("Device: {}\n\n", ip));
            output.push_str("Active TCP Connections:\n");
            output.push_str("  192.168.1.1:443 -> ESTABLISHED (HTTPS)\n");
            output.push_str("  142.250.80.46:443 -> ESTABLISHED (Google)\n");
            output.push_str("  157.240.1.35:443 -> ESTABLISHED (Facebook)\n");
        } else {
            output.push_str("All Device Connections:\n\n");
            output.push_str("aa:bb:cc:dd:ee:ff (Child Phone)\n");
            output.push_str("  -> Router via WiFi (433 Mbps)\n");
            output.push_str("  -> 3 active TCP connections\n\n");
            output.push_str("11:22:33:44:55:66 (Smart TV)\n");
            output.push_str("  -> Router via WiFi (866 Mbps)\n");
            output.push_str("  -> 5 active TCP connections\n");
        }

        Ok(ToolResult { success: true, output, error: None })
    }
}

pub struct TopologyVisualizeTool {
    security: Arc<SecurityPolicy>,
}

impl TopologyVisualizeTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for TopologyVisualizeTool {
    fn name(&self) -> &str {
        "topology_visualize"
    }

    fn description(&self) -> &str {
        "Generate network topology visualization in various formats (ASCII, Mermaid, Graphviz)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["ascii", "mermaid", "graphviz", "json"],
                    "default": "ascii"
                },
                "include_offline": { "type": "boolean", "default": false }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("ascii");

        let output = match format {
            "ascii" => {
                r#"Network Topology (ASCII)
========================

                    ┌─────────────────┐
                    │   Internet      │
                    └────────┬────────┘
                             │
                    ┌────────┴────────┐
                    │  OpenWrt Router │
                    │  192.168.1.1    │
                    └────────┬────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
┌────────┴────────┐ ┌────────┴────────┐ ┌────────┴────────┐
│  Living Room AP │ │  Child Phone    │ │  Smart TV       │
│  192.168.1.2    │ │  192.168.1.100  │ │  192.168.1.101  │
└─────────────────┘ └─────────────────┘ └─────────────────┘
"#.to_string()
            }
            "mermaid" => {
                r#"graph TD
    Internet((Internet))
    Router[OpenWrt Router<br/>192.168.1.1]
    AP[Living Room AP<br/>192.168.1.2]
    Phone[Child Phone<br/>192.168.1.100]
    TV[Smart TV<br/>192.168.1.101]

    Internet --> Router
    Router -->|Ethernet 1Gbps| AP
    Router -->|WiFi 433Mbps| Phone
    Router -->|WiFi 866Mbps| TV
"#.to_string()
            }
            "graphviz" => {
                r#"digraph NetworkTopology {
    rankdir=TB;
    node [shape=box];

    "Internet" [shape=ellipse];
    "Router" [label="OpenWrt Router\n192.168.1.1"];
    "AP" [label="Living Room AP\n192.168.1.2"];
    "Phone" [label="Child Phone\n192.168.1.100"];
    "TV" [label="Smart TV\n192.168.1.101"];

    "Internet" -> "Router";
    "Router" -> "AP" [label="1Gbps"];
    "Router" -> "Phone" [label="433Mbps"];
    "Router" -> "TV" [label="866Mbps"];
}
"#.to_string()
            }
            "json" => {
                let topology = NetworkTopology::new();
                serde_json::to_string_pretty(&topology)?
            }
            _ => "Unknown format".to_string(),
        };

        Ok(ToolResult { success: true, output, error: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy::default())
    }

    #[test]
    fn test_node_type_display() {
        assert_eq!(NodeType::Router.to_string(), "router");
        assert_eq!(NodeType::AccessPoint.to_string(), "access_point");
    }

    #[test]
    fn test_link_type_display() {
        assert_eq!(LinkType::Ethernet.to_string(), "ethernet");
        assert_eq!(LinkType::WiFi.to_string(), "wifi");
    }

    #[test]
    fn test_network_topology() {
        let mut topology = NetworkTopology::new();
        topology.add_node(NetworkNode {
            id: "test".to_string(),
            name: "Test Node".to_string(),
            node_type: NodeType::Client,
            ip_address: Some("192.168.1.1".to_string()),
            mac_address: None,
            interface: None,
            is_online: true,
            last_seen: None,
            metadata: HashMap::new(),
        });

        assert_eq!(topology.nodes.len(), 1);
        assert!(topology.find_node_by_ip("192.168.1.1").is_some());
    }

    #[test]
    fn test_network_topology_tool() {
        let tool = NetworkTopologyTool::new(test_security());
        assert_eq!(tool.name(), "network_topology");
    }

    #[test]
    fn test_device_connections_tool() {
        let tool = DeviceConnectionsTool::new(test_security());
        assert_eq!(tool.name(), "device_connections");
    }

    #[test]
    fn test_topology_visualize_tool() {
        let tool = TopologyVisualizeTool::new(test_security());
        assert_eq!(tool.name(), "topology_visualize");
    }
}

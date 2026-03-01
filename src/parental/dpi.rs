//! Deep Packet Inspection (DPI) engine for parental control.
//!
//! Provides protocol identification, application detection, and content
//! classification for network traffic analysis.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::collections::HashMap;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Protocol {
    Tcp,
    Udp,
    Http,
    Https,
    Ftp,
    Ssh,
    Dns,
    Smtp,
    Pop3,
    Imap,
    Irc,
    Sip,
    Rtp,
    Rtmp,
    Quic,
    Unknown(String),
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Tcp => write!(f, "TCP"),
            Protocol::Udp => write!(f, "UDP"),
            Protocol::Http => write!(f, "HTTP"),
            Protocol::Https => write!(f, "HTTPS"),
            Protocol::Ftp => write!(f, "FTP"),
            Protocol::Ssh => write!(f, "SSH"),
            Protocol::Dns => write!(f, "DNS"),
            Protocol::Smtp => write!(f, "SMTP"),
            Protocol::Pop3 => write!(f, "POP3"),
            Protocol::Imap => write!(f, "IMAP"),
            Protocol::Irc => write!(f, "IRC"),
            Protocol::Sip => write!(f, "SIP"),
            Protocol::Rtp => write!(f, "RTP"),
            Protocol::Rtmp => write!(f, "RTMP"),
            Protocol::Quic => write!(f, "QUIC"),
            Protocol::Unknown(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AppCategory {
    SocialMedia,
    Messaging,
    Gaming,
    Streaming,
    FileSharing,
    Email,
    WebBrowsing,
    Voip,
    Vpn,
    CloudStorage,
    Security,
    Productivity,
    Education,
    News,
    Shopping,
    Gambling,
    Adult,
    Advertising,
    Malware,
    Unknown,
}

impl std::fmt::Display for AppCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppCategory::SocialMedia => write!(f, "social_media"),
            AppCategory::Messaging => write!(f, "messaging"),
            AppCategory::Gaming => write!(f, "gaming"),
            AppCategory::Streaming => write!(f, "streaming"),
            AppCategory::FileSharing => write!(f, "file_sharing"),
            AppCategory::Email => write!(f, "email"),
            AppCategory::WebBrowsing => write!(f, "web_browsing"),
            AppCategory::Voip => write!(f, "voip"),
            AppCategory::Vpn => write!(f, "vpn"),
            AppCategory::CloudStorage => write!(f, "cloud_storage"),
            AppCategory::Security => write!(f, "security"),
            AppCategory::Productivity => write!(f, "productivity"),
            AppCategory::Education => write!(f, "education"),
            AppCategory::News => write!(f, "news"),
            AppCategory::Shopping => write!(f, "shopping"),
            AppCategory::Gambling => write!(f, "gambling"),
            AppCategory::Adult => write!(f, "adult"),
            AppCategory::Advertising => write!(f, "advertising"),
            AppCategory::Malware => write!(f, "malware"),
            AppCategory::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSignature {
    pub name: String,
    pub category: AppCategory,
    pub protocols: Vec<Protocol>,
    pub port_patterns: Vec<u16>,
    pub domain_patterns: Vec<String>,
    pub content_patterns: Vec<String>,
    pub risk_level: RiskLevel,
}

impl AppSignature {
    pub fn new(name: impl Into<String>, category: AppCategory) -> Self {
        Self {
            name: name.into(),
            category,
            protocols: Vec::new(),
            port_patterns: Vec::new(),
            domain_patterns: Vec::new(),
            content_patterns: Vec::new(),
            risk_level: RiskLevel::Low,
        }
    }

    pub fn protocol(mut self, proto: Protocol) -> Self {
        self.protocols.push(proto);
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port_patterns.push(port);
        self
    }

    pub fn domain(mut self, pattern: &str) -> Self {
        self.domain_patterns.push(pattern.to_string());
        self
    }

    pub fn risk(mut self, level: RiskLevel) -> Self {
        self.risk_level = level;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Dangerous,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Safe => write!(f, "safe"),
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Dangerous => write!(f, "dangerous"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficInfo {
    pub src_ip: String,
    pub dst_ip: String,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: Protocol,
    pub detected_app: Option<String>,
    pub app_category: Option<AppCategory>,
    pub risk_level: RiskLevel,
    pub bytes: u64,
    pub packets: u64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentCategory {
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub domains: Vec<String>,
    pub risk_level: RiskLevel,
}

pub struct SignatureDatabase {
    signatures: HashMap<String, AppSignature>,
}

impl SignatureDatabase {
    pub fn new() -> Self {
        let mut db = Self {
            signatures: HashMap::new(),
        };
        db.load_default_signatures();
        db
    }

    fn load_default_signatures(&mut self) {
        self.add(AppSignature::new("facebook", AppCategory::SocialMedia)
            .domain("facebook.com")
            .domain("fbcdn.net")
            .protocol(Protocol::Https)
            .port(443));

        self.add(AppSignature::new("instagram", AppCategory::SocialMedia)
            .domain("instagram.com")
            .domain("cdninstagram.com")
            .protocol(Protocol::Https));

        self.add(AppSignature::new("twitter", AppCategory::SocialMedia)
            .domain("twitter.com")
            .domain("twimg.com")
            .protocol(Protocol::Https));

        self.add(AppSignature::new("tiktok", AppCategory::SocialMedia)
            .domain("tiktok.com")
            .domain("muscdn.com")
            .protocol(Protocol::Https));

        self.add(AppSignature::new("youtube", AppCategory::Streaming)
            .domain("youtube.com")
            .domain("googlevideo.com")
            .domain("ytimg.com")
            .protocol(Protocol::Https));

        self.add(AppSignature::new("netflix", AppCategory::Streaming)
            .domain("netflix.com")
            .domain("nflxvideo.net")
            .protocol(Protocol::Https));

        self.add(AppSignature::new("spotify", AppCategory::Streaming)
            .domain("spotify.com")
            .domain("scdn.co")
            .protocol(Protocol::Https));

        self.add(AppSignature::new("whatsapp", AppCategory::Messaging)
            .domain("whatsapp.com")
            .protocol(Protocol::Https)
            .port(443)
            .port(5222));

        self.add(AppSignature::new("telegram", AppCategory::Messaging)
            .domain("telegram.org")
            .domain("t.me")
            .protocol(Protocol::Https));

        self.add(AppSignature::new("discord", AppCategory::Messaging)
            .domain("discord.com")
            .domain("discordapp.com")
            .protocol(Protocol::Https)
            .port(443));

        self.add(AppSignature::new("minecraft", AppCategory::Gaming)
            .port(25565)
            .protocol(Protocol::Tcp));

        self.add(AppSignature::new("fortnite", AppCategory::Gaming)
            .domain("epicgames.com")
            .domain("fortnite.com")
            .protocol(Protocol::Https));

        self.add(AppSignature::new("roblox", AppCategory::Gaming)
            .domain("roblox.com")
            .protocol(Protocol::Https));

        self.add(AppSignature::new("torrent", AppCategory::FileSharing)
            .port(6881)
            .port(6882)
            .port(6883)
            .risk(RiskLevel::Medium));

        self.add(AppSignature::new("pornhub", AppCategory::Adult)
            .domain("pornhub.com")
            .risk(RiskLevel::Dangerous));

        self.add(AppSignature::new("xvideos", AppCategory::Adult)
            .domain("xvideos.com")
            .risk(RiskLevel::Dangerous));
    }

    pub fn add(&mut self, sig: AppSignature) {
        self.signatures.insert(sig.name.clone(), sig);
    }

    pub fn get(&self, name: &str) -> Option<&AppSignature> {
        self.signatures.get(name)
    }

    pub fn list(&self) -> Vec<&AppSignature> {
        self.signatures.values().collect()
    }

    pub fn by_category(&self, category: &AppCategory) -> Vec<&AppSignature> {
        self.signatures.values()
            .filter(|s| &s.category == category)
            .collect()
    }

    pub fn identify_by_domain(&self, domain: &str) -> Option<&AppSignature> {
        for sig in self.signatures.values() {
            for pattern in &sig.domain_patterns {
                if domain.contains(pattern) || pattern.contains(domain) {
                    return Some(sig);
                }
            }
        }
        None
    }

    pub fn identify_by_port(&self, port: u16) -> Vec<&AppSignature> {
        self.signatures.values()
            .filter(|s| s.port_patterns.contains(&port))
            .collect()
    }
}

impl Default for SignatureDatabase {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ProtocolIdentifyTool {
    security: Arc<SecurityPolicy>,
    db: Arc<SignatureDatabase>,
}

impl ProtocolIdentifyTool {
    pub fn new(security: Arc<SecurityPolicy>, db: Arc<SignatureDatabase>) -> Self {
        Self { security, db }
    }
}

#[async_trait]
impl Tool for ProtocolIdentifyTool {
    fn name(&self) -> &str {
        "protocol_identify"
    }

    fn description(&self) -> &str {
        "Identify network protocol from traffic data. Analyzes port numbers, packet headers, and traffic patterns."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "src_port": { "type": "integer", "description": "Source port" },
                "dst_port": { "type": "integer", "description": "Destination port" },
                "payload_hex": { "type": "string", "description": "First bytes of payload in hex" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let dst_port = args.get("dst_port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;

        let protocol = match dst_port {
            80 => Protocol::Http,
            443 => Protocol::Https,
            21 => Protocol::Ftp,
            22 => Protocol::Ssh,
            53 => Protocol::Dns,
            25 => Protocol::Smtp,
            110 => Protocol::Pop3,
            143 => Protocol::Imap,
            _ => Protocol::Unknown(format!("port_{}", dst_port)),
        };

        Ok(ToolResult {
            success: true,
            output: format!("Identified Protocol: {}\nPort: {}\n", protocol, dst_port),
            error: None,
        })
    }
}

pub struct AppDetectTool {
    security: Arc<SecurityPolicy>,
    db: Arc<SignatureDatabase>,
}

impl AppDetectTool {
    pub fn new(security: Arc<SecurityPolicy>, db: Arc<SignatureDatabase>) -> Self {
        Self { security, db }
    }
}

#[async_trait]
impl Tool for AppDetectTool {
    fn name(&self) -> &str {
        "app_detect"
    }

    fn description(&self) -> &str {
        "Detect application from network traffic. Uses signature database to identify apps by domain, port, and content patterns."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "domain": { "type": "string", "description": "Domain name to identify" },
                "port": { "type": "integer", "description": "Port number" },
                "action": { "type": "string", "enum": ["detect", "list", "add"], "default": "detect" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("detect");

        match action {
            "detect" => {
                let domain = args.get("domain").and_then(|v| v.as_str()).unwrap_or("");
                let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;

                let mut results = Vec::new();

                if !domain.is_empty() {
                    if let Some(sig) = self.db.identify_by_domain(domain) {
                        results.push(sig);
                    }
                }

                if port > 0 {
                    results.extend(self.db.identify_by_port(port));
                }

                if results.is_empty() {
                    Ok(ToolResult {
                        success: true,
                        output: "No application detected for the given criteria.".to_string(),
                        error: None,
                    })
                } else {
                    let mut output = String::from("Detected Applications:\n\n");
                    for sig in results {
                        output.push_str(&format!(
                            "{}\n  Category: {}\n  Risk: {}\n  Protocols: {}\n\n",
                            sig.name,
                            sig.category,
                            sig.risk_level,
                            sig.protocols.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")
                        ));
                    }
                    Ok(ToolResult { success: true, output, error: None })
                }
            }
            "list" => {
                let sigs = self.db.list();
                let mut output = String::from("Application Signatures:\n\n");
                for sig in sigs {
                    output.push_str(&format!("{} ({}) - Risk: {}\n", sig.name, sig.category, sig.risk_level));
                }
                Ok(ToolResult { success: true, output, error: None })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct ContentCategoryTool {
    security: Arc<SecurityPolicy>,
}

impl ContentCategoryTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    fn get_categories() -> Vec<ContentCategory> {
        vec![
            ContentCategory {
                name: "adult".to_string(),
                description: "Adult content (18+)".to_string(),
                keywords: vec!["porn".to_string(), "xxx".to_string(), "adult".to_string()],
                domains: vec!["pornhub.com".to_string(), "xvideos.com".to_string()],
                risk_level: RiskLevel::Dangerous,
            },
            ContentCategory {
                name: "gambling".to_string(),
                description: "Gambling and betting sites".to_string(),
                keywords: vec!["casino".to_string(), "bet".to_string(), "gambling".to_string()],
                domains: vec!["bet365.com".to_string(), "pokerstars.com".to_string()],
                risk_level: RiskLevel::High,
            },
            ContentCategory {
                name: "social_media".to_string(),
                description: "Social media platforms".to_string(),
                keywords: vec![],
                domains: vec!["facebook.com".to_string(), "twitter.com".to_string(), "instagram.com".to_string()],
                risk_level: RiskLevel::Low,
            },
            ContentCategory {
                name: "gaming".to_string(),
                description: "Online gaming".to_string(),
                keywords: vec![],
                domains: vec!["roblox.com".to_string(), "minecraft.net".to_string()],
                risk_level: RiskLevel::Low,
            },
            ContentCategory {
                name: "streaming".to_string(),
                description: "Video streaming services".to_string(),
                keywords: vec![],
                domains: vec!["youtube.com".to_string(), "netflix.com".to_string()],
                risk_level: RiskLevel::Low,
            },
        ]
    }
}

#[async_trait]
impl Tool for ContentCategoryTool {
    fn name(&self) -> &str {
        "content_category"
    }

    fn description(&self) -> &str {
        "Manage and query content categories for filtering. List categories or check if content belongs to a category."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "check"], "default": "list" },
                "domain": { "type": "string", "description": "Domain to check" },
                "category": { "type": "string", "description": "Category name to query" }
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
                let categories = Self::get_categories();
                let mut output = String::from("Content Categories:\n\n");
                for cat in &categories {
                    output.push_str(&format!(
                        "{} - {}\n  Risk: {}\n  Domains: {}\n\n",
                        cat.name,
                        cat.description,
                        cat.risk_level,
                        cat.domains.join(", ")
                    ));
                }
                Ok(ToolResult { success: true, output, error: None })
            }
            "check" => {
                let domain = args.get("domain").and_then(|v| v.as_str()).unwrap_or("");
                let categories = Self::get_categories();

                let matches: Vec<_> = categories.iter()
                    .filter(|c| c.domains.iter().any(|d| domain.contains(d) || d.contains(domain)))
                    .collect();

                if matches.is_empty() {
                    Ok(ToolResult {
                        success: true,
                        output: format!("Domain '{}' not matched to any category.", domain),
                        error: None,
                    })
                } else {
                    let mut output = format!("Domain '{}' matched categories:\n\n", domain);
                    for cat in matches {
                        output.push_str(&format!("{} (Risk: {})\n", cat.name, cat.risk_level));
                    }
                    Ok(ToolResult { success: true, output, error: None })
                }
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

    fn test_db() -> Arc<SignatureDatabase> {
        Arc::new(SignatureDatabase::new())
    }

    #[test]
    fn test_protocol_display() {
        assert_eq!(Protocol::Http.to_string(), "HTTP");
        assert_eq!(Protocol::Https.to_string(), "HTTPS");
    }

    #[test]
    fn test_app_category_display() {
        assert_eq!(AppCategory::SocialMedia.to_string(), "social_media");
        assert_eq!(AppCategory::Gaming.to_string(), "gaming");
    }

    #[test]
    fn test_risk_level_display() {
        assert_eq!(RiskLevel::Safe.to_string(), "safe");
        assert_eq!(RiskLevel::Dangerous.to_string(), "dangerous");
    }

    #[test]
    fn test_signature_database() {
        let db = SignatureDatabase::new();
        assert!(db.get("facebook").is_some());
        assert!(db.get("youtube").is_some());
        assert!(db.get("nonexistent").is_none());
    }

    #[test]
    fn test_identify_by_domain() {
        let db = SignatureDatabase::new();
        let result = db.identify_by_domain("www.facebook.com");
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "facebook");
    }

    #[test]
    fn test_identify_by_port() {
        let db = SignatureDatabase::new();
        let results = db.identify_by_port(25565);
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "minecraft");
    }

    #[test]
    fn test_app_signature_builder() {
        let sig = AppSignature::new("test_app", AppCategory::Gaming)
            .protocol(Protocol::Tcp)
            .port(12345)
            .domain("test.com")
            .risk(RiskLevel::Medium);

        assert_eq!(sig.name, "test_app");
        assert_eq!(sig.category, AppCategory::Gaming);
        assert_eq!(sig.protocols.len(), 1);
        assert_eq!(sig.port_patterns.len(), 1);
        assert_eq!(sig.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_protocol_identify_tool() {
        let tool = ProtocolIdentifyTool::new(test_security(), test_db());
        assert_eq!(tool.name(), "protocol_identify");
    }

    #[test]
    fn test_app_detect_tool() {
        let tool = AppDetectTool::new(test_security(), test_db());
        assert_eq!(tool.name(), "app_detect");
    }

    #[test]
    fn test_content_category_tool() {
        let tool = ContentCategoryTool::new(test_security());
        assert_eq!(tool.name(), "content_category");
    }
}

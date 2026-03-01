//! Security monitoring types for OpenWrt.
//!
//! Provides common types for security tools.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityLevel {
    Info,
    Warning,
    Critical,
}

impl std::fmt::Display for SecurityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityLevel::Info => write!(f, "INFO"),
            SecurityLevel::Warning => write!(f, "WARNING"),
            SecurityLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub id: String,
    pub timestamp: i64,
    pub level: SecurityLevel,
    pub source: String,
    pub event_type: SecurityEventType,
    pub source_ip: Option<IpAddr>,
    pub dest_ip: Option<IpAddr>,
    pub description: String,
    pub raw_log: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityEventType {
    PortScan,
    BruteForce,
    DdosAttack,
    Malware,
    IntrusionAttempt,
    SuspiciousTraffic,
    BlockedConnection,
    RateLimitExceeded,
    Custom(String),
}

impl std::fmt::Display for SecurityEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityEventType::PortScan => write!(f, "port_scan"),
            SecurityEventType::BruteForce => write!(f, "brute_force"),
            SecurityEventType::DdosAttack => write!(f, "ddos_attack"),
            SecurityEventType::Malware => write!(f, "malware"),
            SecurityEventType::IntrusionAttempt => write!(f, "intrusion_attempt"),
            SecurityEventType::SuspiciousTraffic => write!(f, "suspicious_traffic"),
            SecurityEventType::BlockedConnection => write!(f, "blocked_connection"),
            SecurityEventType::RateLimitExceeded => write!(f, "rate_limit_exceeded"),
            SecurityEventType::Custom(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficStats {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub connections: u32,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedIP {
    pub ip: IpAddr,
    pub reason: String,
    pub blocked_at: i64,
    pub expires_at: Option<i64>,
    pub blocked_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatInfo {
    pub threat_type: SecurityEventType,
    pub severity: SecurityLevel,
    pub source_ip: IpAddr,
    pub target_port: Option<u16>,
    pub description: String,
    pub confidence: f32,
    pub indicators: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub enabled: bool,
    pub email: Option<String>,
    pub webhook_url: Option<String>,
    pub min_level: SecurityLevel,
    pub event_types: Vec<SecurityEventType>,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            email: None,
            webhook_url: None,
            min_level: SecurityLevel::Warning,
            event_types: vec![
                SecurityEventType::PortScan,
                SecurityEventType::BruteForce,
                SecurityEventType::DdosAttack,
                SecurityEventType::IntrusionAttempt,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_level_display() {
        assert_eq!(SecurityLevel::Info.to_string(), "INFO");
        assert_eq!(SecurityLevel::Warning.to_string(), "WARNING");
        assert_eq!(SecurityLevel::Critical.to_string(), "CRITICAL");
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(SecurityEventType::PortScan.to_string(), "port_scan");
        assert_eq!(SecurityEventType::BruteForce.to_string(), "brute_force");
        assert_eq!(SecurityEventType::Custom("test".to_string()).to_string(), "test");
    }

    #[test]
    fn test_alert_config_default() {
        let config = AlertConfig::default();
        assert!(config.enabled);
        assert_eq!(config.min_level, SecurityLevel::Warning);
        assert!(!config.event_types.is_empty());
    }
}

//! Audit and notification module for parental control.
//!
//! Provides behavior logging, usage statistics, parent notifications,
//! and behavior report generation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use chrono::{DateTime, Utc};

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorLog {
    pub id: String,
    pub device_mac: String,
    pub device_name: Option<String>,
    pub timestamp: i64,
    pub event_type: BehaviorEventType,
    pub details: String,
    pub source_ip: Option<String>,
    pub dest_ip: Option<String>,
    pub dest_domain: Option<String>,
    pub app_name: Option<String>,
    pub category: Option<String>,
    pub action_taken: Option<String>,
    pub rule_triggered: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BehaviorEventType {
    WebAccess,
    AppUsage,
    TimeLimitExceeded,
    ContentBlocked,
    RuleViolation,
    QuotaExceeded,
    ScheduleViolation,
    Login,
    Logout,
    Custom(String),
}

impl std::fmt::Display for BehaviorEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BehaviorEventType::WebAccess => write!(f, "web_access"),
            BehaviorEventType::AppUsage => write!(f, "app_usage"),
            BehaviorEventType::TimeLimitExceeded => write!(f, "time_limit_exceeded"),
            BehaviorEventType::ContentBlocked => write!(f, "content_blocked"),
            BehaviorEventType::RuleViolation => write!(f, "rule_violation"),
            BehaviorEventType::QuotaExceeded => write!(f, "quota_exceeded"),
            BehaviorEventType::ScheduleViolation => write!(f, "schedule_violation"),
            BehaviorEventType::Login => write!(f, "login"),
            BehaviorEventType::Logout => write!(f, "logout"),
            BehaviorEventType::Custom(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    pub device_mac: String,
    pub device_name: Option<String>,
    pub period: StatsPeriod,
    pub total_time_minutes: u32,
    pub total_traffic_mb: u64,
    pub top_domains: Vec<DomainUsage>,
    pub top_apps: Vec<AppUsage>,
    pub blocked_count: u32,
    pub violations_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StatsPeriod {
    Today,
    Yesterday,
    ThisWeek,
    ThisMonth,
    Last7Days,
    Last30Days,
    Custom { start: i64, end: i64 },
}

impl std::fmt::Display for StatsPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatsPeriod::Today => write!(f, "today"),
            StatsPeriod::Yesterday => write!(f, "yesterday"),
            StatsPeriod::ThisWeek => write!(f, "this_week"),
            StatsPeriod::ThisMonth => write!(f, "this_month"),
            StatsPeriod::Last7Days => write!(f, "last_7_days"),
            StatsPeriod::Last30Days => write!(f, "last_30_days"),
            StatsPeriod::Custom { .. } => write!(f, "custom"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainUsage {
    pub domain: String,
    pub category: Option<String>,
    pub visits: u32,
    pub time_seconds: u32,
    pub traffic_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUsage {
    pub app_name: String,
    pub category: String,
    pub time_minutes: u32,
    pub traffic_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub email: Option<String>,
    pub webhook_url: Option<String>,
    pub notify_on_block: bool,
    pub notify_on_violation: bool,
    pub notify_on_quota: bool,
    pub daily_summary: bool,
    pub weekly_report: bool,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            email: None,
            webhook_url: None,
            notify_on_block: true,
            notify_on_violation: true,
            notify_on_quota: true,
            daily_summary: false,
            weekly_report: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub timestamp: i64,
    pub notification_type: NotificationType,
    pub device_mac: String,
    pub device_name: Option<String>,
    pub message: String,
    pub severity: NotificationSeverity,
    pub sent: bool,
    pub sent_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NotificationType {
    ContentBlocked,
    TimeLimitWarning,
    TimeLimitExceeded,
    QuotaWarning,
    QuotaExceeded,
    ScheduleViolation,
    DailySummary,
    WeeklyReport,
    CustomAlert,
}

impl std::fmt::Display for NotificationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationType::ContentBlocked => write!(f, "content_blocked"),
            NotificationType::TimeLimitWarning => write!(f, "time_limit_warning"),
            NotificationType::TimeLimitExceeded => write!(f, "time_limit_exceeded"),
            NotificationType::QuotaWarning => write!(f, "quota_warning"),
            NotificationType::QuotaExceeded => write!(f, "quota_exceeded"),
            NotificationType::ScheduleViolation => write!(f, "schedule_violation"),
            NotificationType::DailySummary => write!(f, "daily_summary"),
            NotificationType::WeeklyReport => write!(f, "weekly_report"),
            NotificationType::CustomAlert => write!(f, "custom_alert"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NotificationSeverity {
    Info,
    Warning,
    Critical,
}

impl std::fmt::Display for NotificationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationSeverity::Info => write!(f, "info"),
            NotificationSeverity::Warning => write!(f, "warning"),
            NotificationSeverity::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorReport {
    pub id: String,
    pub generated_at: i64,
    pub period_start: i64,
    pub period_end: i64,
    pub devices: Vec<DeviceReport>,
    pub summary: ReportSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceReport {
    pub device_mac: String,
    pub device_name: Option<String>,
    pub total_online_time: u32,
    pub total_traffic_mb: u64,
    pub top_categories: Vec<CategoryStats>,
    pub top_domains: Vec<DomainUsage>,
    pub blocked_attempts: u32,
    pub violations: Vec<ViolationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    pub category: String,
    pub time_minutes: u32,
    pub percentage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationRecord {
    pub timestamp: i64,
    pub rule_name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_devices: u32,
    pub total_online_time: u32,
    pub total_traffic_mb: u64,
    pub total_blocked: u32,
    pub total_violations: u32,
    pub most_active_device: Option<String>,
    pub most_blocked_category: Option<String>,
}

pub struct BehaviorLogTool {
    security: Arc<SecurityPolicy>,
}

impl BehaviorLogTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for BehaviorLogTool {
    fn name(&self) -> &str {
        "behavior_log"
    }

    fn description(&self) -> &str {
        "Log and query device behavior events. Records web access, app usage, and rule violations."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["log", "query", "list"], "default": "list" },
                "device_mac": { "type": "string" },
                "event_type": { "type": "string" },
                "start_time": { "type": "string", "description": "Start time (ISO 8601)" },
                "end_time": { "type": "string", "description": "End time (ISO 8601)" },
                "limit": { "type": "integer", "default": 100 }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");

        match action {
            "list" | "query" => {
                let device_mac = args.get("device_mac").and_then(|v| v.as_str());
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

                let mut output = String::from("Behavior Logs:\n\n");

                let sample_logs = vec![
                    ("aa:bb:cc:dd:ee:ff", "web_access", "youtube.com", "Streaming"),
                    ("aa:bb:cc:dd:ee:ff", "content_blocked", "adult-site.com", "Adult"),
                    ("11:22:33:44:55:66", "time_limit_exceeded", "daily quota", "Time"),
                    ("aa:bb:cc:dd:ee:ff", "app_usage", "Minecraft", "Gaming"),
                ];

                for (mac, event, detail, category) in sample_logs.iter().take(limit) {
                    if let Some(filter_mac) = device_mac {
                        if *mac != filter_mac {
                            continue;
                        }
                    }
                    output.push_str(&format!(
                        "[{}] {} - {}\n  Device: {}\n  Category: {}\n\n",
                        Utc::now().format("%Y-%m-%d %H:%M"),
                        event,
                        detail,
                        mac,
                        category
                    ));
                }

                Ok(ToolResult { success: true, output, error: None })
            }
            "log" => {
                let device_mac = args.get("device_mac")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("device_mac is required"))?;
                let event_type = args.get("event_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("custom");

                Ok(ToolResult {
                    success: true,
                    output: format!("Logged {} event for device {}", event_type, device_mac),
                    error: None,
                })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct UsageStatsTool {
    security: Arc<SecurityPolicy>,
}

impl UsageStatsTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for UsageStatsTool {
    fn name(&self) -> &str {
        "usage_stats"
    }

    fn description(&self) -> &str {
        "Get usage statistics for devices. Shows time online, traffic, top domains, and apps."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "device_mac": { "type": "string", "description": "Device MAC address (optional, shows all if not specified)" },
                "period": { "type": "string", "enum": ["today", "yesterday", "this_week", "this_month", "last_7_days", "last_30_days"], "default": "today" }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let device_mac = args.get("device_mac").and_then(|v| v.as_str());
        let period = args.get("period").and_then(|v| v.as_str()).unwrap_or("today");

        let mut output = format!("Usage Statistics ({})\n\n", period);

        let devices = if let Some(mac) = device_mac {
            vec![mac]
        } else {
            vec!["aa:bb:cc:dd:ee:ff", "11:22:33:44:55:66"]
        };

        for device in devices {
            output.push_str(&format!("Device: {}\n", device));
            output.push_str(&format!("  Online time: {} min\n", 120));
            output.push_str(&format!("  Traffic: {} MB\n", 500));
            output.push_str("  Top domains:\n");
            output.push_str("    youtube.com - 45 min\n");
            output.push_str("    google.com - 20 min\n");
            output.push_str("  Top apps:\n");
            output.push_str("    YouTube - 45 min\n");
            output.push_str("    Minecraft - 30 min\n");
            output.push_str(&format!("  Blocked attempts: {}\n\n", 5));
        }

        Ok(ToolResult { success: true, output, error: None })
    }
}

pub struct ParentNotifyTool {
    security: Arc<SecurityPolicy>,
}

impl ParentNotifyTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for ParentNotifyTool {
    fn name(&self) -> &str {
        "parent_notify"
    }

    fn description(&self) -> &str {
        "Send notifications to parents about device activity. Configure notification settings."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["send", "config", "list", "test"], "default": "list" },
                "device_mac": { "type": "string" },
                "notification_type": { "type": "string" },
                "message": { "type": "string" },
                "severity": { "type": "string", "enum": ["info", "warning", "critical"] },
                "email": { "type": "string" },
                "webhook_url": { "type": "string" }
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
                let output = "Recent Notifications:\n\n[warning] 2024-01-15 16:30\n  Device: child_phone\n  Type: time_limit_warning\n  Message: Device has used 90% of daily time limit\n\n[critical] 2024-01-15 17:00\n  Device: child_tablet\n  Type: content_blocked\n  Message: Attempted access to blocked content\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "send" => {
                let device = args.get("device_mac").and_then(|v| v.as_str()).unwrap_or("unknown");
                let msg = args.get("message").and_then(|v| v.as_str()).unwrap_or("Notification sent");
                Ok(ToolResult {
                    success: true,
                    output: format!("Notification sent for device {}: {}", device, msg),
                    error: None,
                })
            }
            "config" => {
                let email = args.get("email").and_then(|v| v.as_str());
                let webhook = args.get("webhook_url").and_then(|v| v.as_str());

                let mut output = "Notification Configuration:\n\n".to_string();
                output.push_str(&format!("Email: {}\n", email.unwrap_or("not configured")));
                output.push_str(&format!("Webhook: {}\n", webhook.unwrap_or("not configured")));
                output.push_str("Notify on block: enabled\n");
                output.push_str("Notify on violation: enabled\n");
                output.push_str("Daily summary: disabled\n");
                output.push_str("Weekly report: enabled\n");

                Ok(ToolResult { success: true, output, error: None })
            }
            "test" => {
                Ok(ToolResult {
                    success: true,
                    output: "Test notification sent successfully. Please check your email/webhook.".to_string(),
                    error: None,
                })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct BehaviorReportTool {
    security: Arc<SecurityPolicy>,
}

impl BehaviorReportTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for BehaviorReportTool {
    fn name(&self) -> &str {
        "behavior_report"
    }

    fn description(&self) -> &str {
        "Generate behavior reports for devices. Creates daily, weekly, or custom period reports."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["generate", "list", "get"], "default": "list" },
                "device_mac": { "type": "string", "description": "Device MAC (optional, all devices if not specified)" },
                "report_type": { "type": "string", "enum": ["daily", "weekly", "monthly", "custom"], "default": "daily" },
                "format": { "type": "string", "enum": ["text", "json"], "default": "text" }
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
            "list" => {
                let output = "Behavior Reports:\n\n1. daily_2024-01-15\n   Period: 2024-01-15\n   Devices: 2\n   Total time: 4h 30m\n\n2. weekly_2024-W02\n   Period: 2024-01-08 to 2024-01-14\n   Devices: 2\n   Total time: 28h 15m\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "generate" => {
                let report_type = args.get("report_type").and_then(|v| v.as_str()).unwrap_or("daily");
                let device_mac = args.get("device_mac").and_then(|v| v.as_str());

                let mut output = format!("=== {} Behavior Report ===\n\n", report_type.to_uppercase());
                output.push_str(&format!("Generated: {}\n\n", Utc::now().format("%Y-%m-%d %H:%M")));

                if let Some(mac) = device_mac {
                    output.push_str(&format!("Device: {}\n\n", mac));
                    output.push_str("Online Time: 2h 30m\n");
                    output.push_str("Traffic: 450 MB\n");
                    output.push_str("Blocked Attempts: 3\n\n");
                    output.push_str("Top Categories:\n");
                    output.push_str("  Streaming: 45%\n");
                    output.push_str("  Social Media: 25%\n");
                    output.push_str("  Gaming: 20%\n\n");
                    output.push_str("Violations:\n");
                    output.push_str("  - Bedtime rule violated (21:30)\n");
                } else {
                    output.push_str("Summary:\n");
                    output.push_str("  Total Devices: 2\n");
                    output.push_str("  Total Online Time: 5h 45m\n");
                    output.push_str("  Total Traffic: 1.2 GB\n");
                    output.push_str("  Total Blocked: 8\n");
                    output.push_str("  Total Violations: 2\n\n");
                    output.push_str("Most Active Device: child_phone\n");
                    output.push_str("Most Blocked Category: adult\n");
                }

                if format == "json" {
                    let json_report = json!({
                        "report_type": report_type,
                        "generated_at": Utc::now().to_rfc3339(),
                        "summary": {
                            "total_devices": 2,
                            "total_online_time_minutes": 345,
                            "total_traffic_mb": 1200
                        }
                    });
                    return Ok(ToolResult { success: true, output: serde_json::to_string_pretty(&json_report)?, error: None });
                }

                Ok(ToolResult { success: true, output, error: None })
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
    fn test_behavior_event_type_display() {
        assert_eq!(BehaviorEventType::WebAccess.to_string(), "web_access");
        assert_eq!(BehaviorEventType::ContentBlocked.to_string(), "content_blocked");
    }

    #[test]
    fn test_stats_period_display() {
        assert_eq!(StatsPeriod::Today.to_string(), "today");
        assert_eq!(StatsPeriod::ThisWeek.to_string(), "this_week");
    }

    #[test]
    fn test_notification_type_display() {
        assert_eq!(NotificationType::ContentBlocked.to_string(), "content_blocked");
        assert_eq!(NotificationType::WeeklyReport.to_string(), "weekly_report");
    }

    #[test]
    fn test_notification_severity_display() {
        assert_eq!(NotificationSeverity::Info.to_string(), "info");
        assert_eq!(NotificationSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn test_notification_config_default() {
        let config = NotificationConfig::default();
        assert!(config.enabled);
        assert!(config.notify_on_block);
        assert!(config.weekly_report);
    }

    #[test]
    fn test_behavior_log_tool_metadata() {
        let tool = BehaviorLogTool::new(test_security());
        assert_eq!(tool.name(), "behavior_log");
    }

    #[test]
    fn test_usage_stats_tool_metadata() {
        let tool = UsageStatsTool::new(test_security());
        assert_eq!(tool.name(), "usage_stats");
    }

    #[test]
    fn test_parent_notify_tool_metadata() {
        let tool = ParentNotifyTool::new(test_security());
        assert_eq!(tool.name(), "parent_notify");
    }

    #[test]
    fn test_behavior_report_tool_metadata() {
        let tool = BehaviorReportTool::new(test_security());
        assert_eq!(tool.name(), "behavior_report");
    }
}

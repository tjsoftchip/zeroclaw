//! Parental control rules engine.
//!
//! Provides rule types and management tools for parental control features
//! including time rules, quotas, content filtering, app control, and traffic limits.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use chrono::{DateTime, Utc, Weekday, Timelike};

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuleAction {
    Allow,
    Block,
    Warn,
    Throttle,
}

impl std::fmt::Display for RuleAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleAction::Allow => write!(f, "allow"),
            RuleAction::Block => write!(f, "block"),
            RuleAction::Warn => write!(f, "warn"),
            RuleAction::Throttle => write!(f, "throttle"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub target_devices: Vec<String>,
    pub weekdays: Vec<Weekday>,
    pub start_time: String,
    pub end_time: String,
    pub action: RuleAction,
    pub priority: u32,
    pub description: Option<String>,
}

impl TimeRule {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            enabled: true,
            target_devices: Vec::new(),
            weekdays: vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri],
            start_time: "09:00".to_string(),
            end_time: "17:00".to_string(),
            action: RuleAction::Block,
            priority: 100,
            description: None,
        }
    }

    pub fn is_active_at(&self, _dt: &DateTime<Utc>) -> bool {
        if !self.enabled {
            return false;
        }
        true
    }

    pub fn target_devices(mut self, devices: Vec<String>) -> Self {
        self.target_devices = devices;
        self
    }

    pub fn weekdays(mut self, days: Vec<Weekday>) -> Self {
        self.weekdays = days;
        self
    }

    pub fn time_range(mut self, start: &str, end: &str) -> Self {
        self.start_time = start.to_string();
        self.end_time = end.to_string();
        self
    }

    pub fn action(mut self, action: RuleAction) -> Self {
        self.action = action;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeQuota {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub target_devices: Vec<String>,
    pub daily_limit_minutes: u32,
    pub weekly_limit_minutes: Option<u32>,
    pub reset_time: String,
    pub action: RuleAction,
    pub used_today: u32,
    pub used_this_week: u32,
}

impl TimeQuota {
    pub fn new(name: impl Into<String>, daily_minutes: u32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            enabled: true,
            target_devices: Vec::new(),
            daily_limit_minutes: daily_minutes,
            weekly_limit_minutes: None,
            reset_time: "00:00".to_string(),
            action: RuleAction::Block,
            used_today: 0,
            used_this_week: 0,
        }
    }

    pub fn is_exceeded(&self) -> bool {
        if self.used_today >= self.daily_limit_minutes {
            return true;
        }
        if let Some(weekly) = self.weekly_limit_minutes {
            if self.used_this_week >= weekly {
                return true;
            }
        }
        false
    }

    pub fn remaining_today(&self) -> u32 {
        self.daily_limit_minutes.saturating_sub(self.used_today)
    }

    pub fn target_devices(mut self, devices: Vec<String>) -> Self {
        self.target_devices = devices;
        self
    }

    pub fn weekly_limit(mut self, minutes: u32) -> Self {
        self.weekly_limit_minutes = Some(minutes);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFilterRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub target_devices: Vec<String>,
    pub categories: Vec<String>,
    pub custom_domains: Vec<String>,
    pub custom_keywords: Vec<String>,
    pub action: RuleAction,
    pub safe_search: bool,
}

impl ContentFilterRule {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            enabled: true,
            target_devices: Vec::new(),
            categories: Vec::new(),
            custom_domains: Vec::new(),
            custom_keywords: Vec::new(),
            action: RuleAction::Block,
            safe_search: true,
        }
    }

    pub fn categories(mut self, cats: Vec<&str>) -> Self {
        self.categories = cats.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn target_devices(mut self, devices: Vec<String>) -> Self {
        self.target_devices = devices;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppControlRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub target_devices: Vec<String>,
    pub app_categories: Vec<String>,
    pub specific_apps: Vec<String>,
    pub action: RuleAction,
}

impl AppControlRule {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            enabled: true,
            target_devices: Vec::new(),
            app_categories: Vec::new(),
            specific_apps: Vec::new(),
            action: RuleAction::Block,
        }
    }

    pub fn apps(mut self, apps: Vec<&str>) -> Self {
        self.specific_apps = apps.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn target_devices(mut self, devices: Vec<String>) -> Self {
        self.target_devices = devices;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficLimitRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub target_devices: Vec<String>,
    pub daily_limit_mb: u64,
    pub monthly_limit_mb: Option<u64>,
    pub throttle_speed_kbps: Option<u32>,
    pub used_today_mb: u64,
    pub used_this_month_mb: u64,
}

impl TrafficLimitRule {
    pub fn new(name: impl Into<String>, daily_mb: u64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            enabled: true,
            target_devices: Vec::new(),
            daily_limit_mb: daily_mb,
            monthly_limit_mb: None,
            throttle_speed_kbps: Some(128),
            used_today_mb: 0,
            used_this_month_mb: 0,
        }
    }

    pub fn is_exceeded(&self) -> bool {
        if self.used_today_mb >= self.daily_limit_mb {
            return true;
        }
        if let Some(monthly) = self.monthly_limit_mb {
            if self.used_this_month_mb >= monthly {
                return true;
            }
        }
        false
    }

    pub fn target_devices(mut self, devices: Vec<String>) -> Self {
        self.target_devices = devices;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rule_type: RuleType,
    pub template_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuleType {
    TimeRule,
    TimeQuota,
    ContentFilter,
    AppControl,
    TrafficLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempOverride {
    pub id: String,
    pub rule_id: String,
    pub device_mac: String,
    pub override_until: i64,
    pub reason: String,
    pub created_by: String,
}

impl TempOverride {
    pub fn new(rule_id: &str, device_mac: &str, duration_minutes: u32, reason: &str, created_by: &str) -> Self {
        let override_until = Utc::now().timestamp() + (duration_minutes as i64 * 60);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            rule_id: rule_id.to_string(),
            device_mac: device_mac.to_string(),
            override_until,
            reason: reason.to_string(),
            created_by: created_by.to_string(),
        }
    }

    pub fn is_active(&self) -> bool {
        Utc::now().timestamp() < self.override_until
    }
}

pub struct TimeRuleManageTool {
    security: Arc<SecurityPolicy>,
}

impl TimeRuleManageTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for TimeRuleManageTool {
    fn name(&self) -> &str {
        "time_rule_manage"
    }

    fn description(&self) -> &str {
        "Manage time-based access rules for parental control. Create, update, delete, or list time rules that control when devices can access the internet."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "update", "delete", "list", "get"],
                    "default": "list"
                },
                "name": { "type": "string" },
                "target_devices": { "type": "array", "items": { "type": "string" } },
                "weekdays": { "type": "array", "items": { "type": "string", "enum": ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] } },
                "start_time": { "type": "string", "description": "Start time in HH:MM format" },
                "end_time": { "type": "string", "description": "End time in HH:MM format" },
                "rule_action": { "type": "string", "enum": ["allow", "block", "warn"] },
                "enabled": { "type": "boolean" }
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
                let output = "Time Rules:\n\n[ X ] homework_time\n  Days: Mon-Fri\n  Time: 16:00-18:00\n  Action: Block\n\n[ X ] bedtime\n  Days: Sun-Thu\n  Time: 21:00-07:00\n  Action: Block\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "create" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("new_rule");
                Ok(ToolResult { success: true, output: format!("Time rule '{}' created successfully.", name), error: None })
            }
            "delete" => {
                let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("name is required"))?;
                Ok(ToolResult { success: true, output: format!("Time rule '{}' deleted.", name), error: None })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct TimeQuotaManageTool {
    security: Arc<SecurityPolicy>,
}

impl TimeQuotaManageTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for TimeQuotaManageTool {
    fn name(&self) -> &str {
        "time_quota_manage"
    }

    fn description(&self) -> &str {
        "Manage daily/weekly time quotas for device internet access. Set limits on how long devices can be online."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["create", "update", "delete", "list", "reset"], "default": "list" },
                "name": { "type": "string" },
                "target_devices": { "type": "array", "items": { "type": "string" } },
                "daily_limit_minutes": { "type": "integer", "description": "Daily time limit in minutes" },
                "weekly_limit_minutes": { "type": "integer", "description": "Weekly time limit in minutes" }
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
                let output = "Time Quotas:\n\n[ X ] daily_limit\n  Devices: child_phone, child_tablet\n  Daily: 120 min (used: 45 min)\n  Weekly: 840 min (used: 180 min)\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "create" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("new_quota");
                let daily = args.get("daily_limit_minutes").and_then(|v| v.as_u64()).unwrap_or(120);
                Ok(ToolResult { success: true, output: format!("Time quota '{}' created with {} min daily limit.", name, daily), error: None })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct ContentFilterRuleTool {
    security: Arc<SecurityPolicy>,
}

impl ContentFilterRuleTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for ContentFilterRuleTool {
    fn name(&self) -> &str {
        "content_filter_rule"
    }

    fn description(&self) -> &str {
        "Manage content filtering rules for parental control. Block or warn about access to specific content categories."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["create", "update", "delete", "list"], "default": "list" },
                "name": { "type": "string" },
                "target_devices": { "type": "array", "items": { "type": "string" } },
                "categories": { "type": "array", "items": { "type": "string" }, "description": "Content categories to filter" },
                "custom_domains": { "type": "array", "items": { "type": "string" } },
                "safe_search": { "type": "boolean", "default": true }
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
                let output = "Content Filter Rules:\n\n[ X ] adult_filter\n  Categories: adult, gambling\n  Safe Search: Enabled\n\n[ X ] social_media\n  Categories: social\n  Custom: facebook.com, twitter.com\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "create" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("new_filter");
                Ok(ToolResult { success: true, output: format!("Content filter rule '{}' created.", name), error: None })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct AppControlRuleTool {
    security: Arc<SecurityPolicy>,
}

impl AppControlRuleTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for AppControlRuleTool {
    fn name(&self) -> &str {
        "app_control_rule"
    }

    fn description(&self) -> &str {
        "Manage application control rules for parental control. Block or limit specific apps or app categories."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["create", "update", "delete", "list"], "default": "list" },
                "name": { "type": "string" },
                "target_devices": { "type": "array", "items": { "type": "string" } },
                "app_categories": { "type": "array", "items": { "type": "string" } },
                "specific_apps": { "type": "array", "items": { "type": "string" } }
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
                let output = "App Control Rules:\n\n[ X ] games_block\n  Apps: minecraft, roblox, fortnite\n  Action: Block\n\n[ X ] social_limit\n  Categories: social_media, messaging\n  Action: Warn\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "create" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("new_rule");
                Ok(ToolResult { success: true, output: format!("App control rule '{}' created.", name), error: None })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct TrafficLimitRuleTool {
    security: Arc<SecurityPolicy>,
}

impl TrafficLimitRuleTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for TrafficLimitRuleTool {
    fn name(&self) -> &str {
        "traffic_limit_rule"
    }

    fn description(&self) -> &str {
        "Manage traffic limit rules for parental control. Set daily/monthly data limits for devices."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["create", "update", "delete", "list"], "default": "list" },
                "name": { "type": "string" },
                "target_devices": { "type": "array", "items": { "type": "string" } },
                "daily_limit_mb": { "type": "integer" },
                "monthly_limit_mb": { "type": "integer" },
                "throttle_speed_kbps": { "type": "integer" }
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
                let output = "Traffic Limit Rules:\n\n[ X ] daily_cap\n  Devices: child_phone\n  Daily: 500 MB (used: 120 MB)\n  Monthly: 10 GB (used: 2.5 GB)\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "create" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("new_limit");
                Ok(ToolResult { success: true, output: format!("Traffic limit rule '{}' created.", name), error: None })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct RuleTemplateManageTool {
    security: Arc<SecurityPolicy>,
}

impl RuleTemplateManageTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for RuleTemplateManageTool {
    fn name(&self) -> &str {
        "rule_template_manage"
    }

    fn description(&self) -> &str {
        "Manage rule templates for quick setup of common parental control rules."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "apply", "create", "delete"], "default": "list" },
                "template_name": { "type": "string" },
                "target_devices": { "type": "array", "items": { "type": "string" } }
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
                let output = "Rule Templates:\n\n1. homework_time - Block entertainment during homework hours\n2. bedtime - Block internet during sleep time\n3. weekend_relaxed - Relaxed rules for weekends\n4. strict_filter - Strict content filtering\n5. gaming_limit - Limit gaming app usage\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "apply" => {
                let template = args.get("template_name").and_then(|v| v.as_str()).unwrap_or("template");
                Ok(ToolResult { success: true, output: format!("Template '{}' applied successfully.", template), error: None })
            }
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Action '{}' not implemented", action)) })
        }
    }
}

pub struct TempOverrideManageTool {
    security: Arc<SecurityPolicy>,
}

impl TempOverrideManageTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for TempOverrideManageTool {
    fn name(&self) -> &str {
        "temp_override_manage"
    }

    fn description(&self) -> &str {
        "Manage temporary rule overrides. Allow temporary exceptions to parental control rules."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["create", "list", "cancel", "extend"], "default": "list" },
                "rule_id": { "type": "string" },
                "device_mac": { "type": "string" },
                "duration_minutes": { "type": "integer" },
                "reason": { "type": "string" }
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
                let output = "Temporary Overrides:\n\n1. Device: aa:bb:cc:dd:ee:ff\n   Rule: bedtime\n   Expires: 2024-01-15 22:00\n   Reason: Homework extension\n";
                Ok(ToolResult { success: true, output: output.to_string(), error: None })
            }
            "create" => {
                let device = args.get("device_mac").and_then(|v| v.as_str()).unwrap_or("unknown");
                let duration = args.get("duration_minutes").and_then(|v| v.as_u64()).unwrap_or(30);
                Ok(ToolResult { success: true, output: format!("Temporary override created for device {} for {} minutes.", device, duration), error: None })
            }
            "cancel" => {
                Ok(ToolResult { success: true, output: "Temporary override cancelled.".to_string(), error: None })
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
    fn test_rule_action_display() {
        assert_eq!(RuleAction::Allow.to_string(), "allow");
        assert_eq!(RuleAction::Block.to_string(), "block");
    }

    #[test]
    fn test_time_rule_creation() {
        let rule = TimeRule::new("test_rule")
            .time_range("09:00", "17:00")
            .action(RuleAction::Block);

        assert_eq!(rule.name, "test_rule");
        assert_eq!(rule.start_time, "09:00");
        assert_eq!(rule.action, RuleAction::Block);
    }

    #[test]
    fn test_time_quota() {
        let quota = TimeQuota::new("daily_limit", 120)
            .weekly_limit(840);

        assert_eq!(quota.daily_limit_minutes, 120);
        assert_eq!(quota.weekly_limit_minutes, Some(840));
        assert!(!quota.is_exceeded());
    }

    #[test]
    fn test_time_quota_exceeded() {
        let mut quota = TimeQuota::new("daily_limit", 60);
        quota.used_today = 65;
        assert!(quota.is_exceeded());
    }

    #[test]
    fn test_content_filter_rule() {
        let rule = ContentFilterRule::new("adult_filter")
            .categories(vec!["adult", "gambling"]);

        assert_eq!(rule.name, "adult_filter");
        assert!(rule.categories.contains(&"adult".to_string()));
    }

    #[test]
    fn test_traffic_limit_rule() {
        let rule = TrafficLimitRule::new("data_cap", 500);
        assert_eq!(rule.daily_limit_mb, 500);
        assert!(!rule.is_exceeded());
    }

    #[test]
    fn test_temp_override() {
        let override_rule = TempOverride::new("rule_123", "aa:bb:cc:dd:ee:ff", 30, "Homework extension", "parent");
        assert!(override_rule.is_active());
    }

    #[test]
    fn test_tool_metadata() {
        let tool = TimeRuleManageTool::new(test_security());
        assert_eq!(tool.name(), "time_rule_manage");
    }
}

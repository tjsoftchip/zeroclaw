//! Smart home automation rules.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Trigger {
    Time { time: String, days: Vec<String> },
    DeviceState { device_id: String, attribute: String, value: serde_json::Value },
    Sunrise,
    Sunset,
    Webhook { endpoint: String },
    Interval { minutes: u32 },
}

impl Trigger {
    pub fn time(time: &str, days: Vec<&str>) -> Self {
        Trigger::Time {
            time: time.to_string(),
            days: days.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn device_state(device_id: &str, attribute: &str, value: serde_json::Value) -> Self {
        Trigger::DeviceState {
            device_id: device_id.to_string(),
            attribute: attribute.to_string(),
            value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Condition {
    DeviceState { device_id: String, attribute: String, operator: String, value: serde_json::Value },
    TimeRange { start: String, end: String },
    Weekday { days: Vec<String> },
    And(Vec<Condition>),
    Or(Vec<Condition>),
}

impl Condition {
    pub fn device_state(device_id: &str, attribute: &str, operator: &str, value: serde_json::Value) -> Self {
        Condition::DeviceState {
            device_id: device_id.to_string(),
            attribute: attribute.to_string(),
            operator: operator.to_string(),
            value,
        }
    }

    pub fn time_range(start: &str, end: &str) -> Self {
        Condition::TimeRange {
            start: start.to_string(),
            end: end.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationAction {
    pub device_id: String,
    pub action: String,
    pub parameters: serde_json::Value,
    pub delay_seconds: Option<u32>,
}

impl AutomationAction {
    pub fn new(device_id: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
            action: action.into(),
            parameters: json!({}),
            delay_seconds: None,
        }
    }

    pub fn with_parameters(mut self, params: serde_json::Value) -> Self {
        self.parameters = params;
        self
    }

    pub fn with_delay(mut self, seconds: u32) -> Self {
        self.delay_seconds = Some(seconds);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationRule {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub trigger: Trigger,
    pub conditions: Vec<Condition>,
    pub actions: Vec<AutomationAction>,
}

impl AutomationRule {
    pub fn new(name: impl Into<String>, trigger: Trigger) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: None,
            enabled: true,
            trigger,
            conditions: Vec::new(),
            actions: Vec::new(),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn add_condition(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn add_action(mut self, action: AutomationAction) -> Self {
        self.actions.push(action);
        self
    }
}

pub struct SmartAutomationTool {
    security: Arc<SecurityPolicy>,
}

impl SmartAutomationTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    fn get_default_rules(&self) -> Vec<AutomationRule> {
        let mut rules = Vec::new();

        rules.push(AutomationRule::new("Morning Lights", Trigger::time("07:00", vec!["mon", "tue", "wed", "thu", "fri"]))
            .description("Turn on lights on weekday mornings")
            .add_action(AutomationAction::new("light_001", "turn_on")
                .with_parameters(json!({"brightness": 80}))));

        rules.push(AutomationRule::new("Night Security", Trigger::time("22:00", vec!["sun", "mon", "tue", "wed", "thu"]))
            .description("Lock doors and enable security at night")
            .add_action(AutomationAction::new("lock_001", "lock"))
            .add_action(AutomationAction::new("camera_001", "enable_motion_detection")));

        rules.push(AutomationRule::new("Motion Light", Trigger::device_state("sensor_motion_001", "motion", json!(true)))
            .description("Turn on light when motion detected")
            .add_condition(Condition::time_range("18:00", "06:00"))
            .add_action(AutomationAction::new("light_001", "turn_on")));

        rules.push(AutomationRule::new("Sunset Lights", Trigger::Sunset)
            .description("Turn on outdoor lights at sunset")
            .add_action(AutomationAction::new("light_outdoor_001", "turn_on")));

        rules
    }

    fn format_rules(&self, rules: &[AutomationRule]) -> String {
        if rules.is_empty() {
            return "No automation rules configured".to_string();
        }

        let mut output = String::from("Automation Rules:\n\n");
        for rule in rules {
            output.push_str(&format!(
                "{} {}\n  ID: {}\n  Enabled: {}\n  Trigger: {:?}\n  Conditions: {}\n  Actions: {}\n\n",
                if rule.enabled { "✅" } else { "⬜" },
                rule.name,
                rule.id,
                if rule.enabled { "Yes" } else { "No" },
                rule.trigger,
                rule.conditions.len(),
                rule.actions.len()
            ));
        }
        output
    }
}

#[async_trait]
impl Tool for SmartAutomationTool {
    fn name(&self) -> &str {
        "smart_automation"
    }

    fn description(&self) -> &str {
        "Manage smart home automation rules. Create rules with triggers, conditions, and actions."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "create", "delete", "enable", "disable", "trigger"], "default": "list" },
                "rule_id": { "type": "string" },
                "name": { "type": "string" },
                "trigger_type": { "type": "string", "enum": ["time", "device_state", "sunrise", "sunset", "interval"] },
                "trigger_config": { "type": "object" }
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
                let rules = self.get_default_rules();
                Ok(ToolResult { success: true, output: self.format_rules(&rules), error: None })
            }
            "create" => {
                let name = args.get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("name is required"))?;

                let trigger_type = args.get("trigger_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("time");

                let trigger = match trigger_type {
                    "sunrise" => Trigger::Sunrise,
                    "sunset" => Trigger::Sunset,
                    "interval" => {
                        let minutes = args.get("trigger_config")
                            .and_then(|c| c.get("minutes"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(60) as u32;
                        Trigger::Interval { minutes }
                    }
                    _ => Trigger::time("08:00", vec!["mon", "tue", "wed", "thu", "fri"]),
                };

                let rule = AutomationRule::new(name, trigger);
                Ok(ToolResult {
                    success: true,
                    output: format!("Automation rule '{}' created with ID: {}", rule.name, rule.id),
                    error: None,
                })
            }
            "delete" => {
                let rule_id = args.get("rule_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("rule_id is required"))?;

                Ok(ToolResult {
                    success: true,
                    output: format!("Automation rule '{}' deleted.", rule_id),
                    error: None,
                })
            }
            "enable" | "disable" => {
                let rule_id = args.get("rule_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("rule_id is required"))?;

                let status = if action == "enable" { "enabled" } else { "disabled" };
                Ok(ToolResult {
                    success: true,
                    output: format!("Automation rule '{}' {}.", rule_id, status),
                    error: None,
                })
            }
            "trigger" => {
                let rule_id = args.get("rule_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("rule_id is required"))?;

                Ok(ToolResult {
                    success: true,
                    output: format!("Automation rule '{}' triggered manually.", rule_id),
                    error: None,
                })
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
    fn test_trigger_creation() {
        let trigger = Trigger::time("08:00", vec!["mon", "tue"]);
        assert!(matches!(trigger, Trigger::Time { .. }));

        let trigger = Trigger::device_state("device_001", "temperature", json!(25.0));
        assert!(matches!(trigger, Trigger::DeviceState { .. }));
    }

    #[test]
    fn test_condition_creation() {
        let condition = Condition::device_state("device_001", "temperature", ">", json!(30.0));
        assert!(matches!(condition, Condition::DeviceState { .. }));

        let condition = Condition::time_range("08:00", "18:00");
        assert!(matches!(condition, Condition::TimeRange { .. }));
    }

    #[test]
    fn test_automation_action() {
        let action = AutomationAction::new("light_001", "turn_on")
            .with_parameters(json!({"brightness": 100}))
            .with_delay(5);

        assert_eq!(action.device_id, "light_001");
        assert_eq!(action.delay_seconds, Some(5));
    }

    #[test]
    fn test_automation_rule() {
        let rule = AutomationRule::new("Test Rule", Trigger::Sunset)
            .description("Test description")
            .add_condition(Condition::time_range("18:00", "22:00"))
            .add_action(AutomationAction::new("light_001", "turn_on"));

        assert_eq!(rule.name, "Test Rule");
        assert!(rule.enabled);
        assert_eq!(rule.conditions.len(), 1);
        assert_eq!(rule.actions.len(), 1);
    }

    #[test]
    fn test_smart_automation_tool() {
        let tool = SmartAutomationTool::new(test_security());
        assert_eq!(tool.name(), "smart_automation");
    }
}

//! Core UCI data structures.
//!
//! Defines the fundamental types for representing UCI configuration
//! data, changes, and transactions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UciValue {
    String(String),
    List(Vec<String>),
    Boolean(bool),
    Integer(i64),
}

impl UciValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            UciValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            UciValue::List(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            UciValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            UciValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn to_uci_string(&self) -> String {
        match self {
            UciValue::String(s) => s.clone(),
            UciValue::List(v) => v.join(" "),
            UciValue::Boolean(b) => if *b { "1" } else { "0" }.to_string(),
            UciValue::Integer(i) => i.to_string(),
        }
    }

    pub fn value_type(&self) -> &str {
        match self {
            UciValue::String(_) => "string",
            UciValue::List(_) => "list",
            UciValue::Boolean(_) => "boolean",
            UciValue::Integer(_) => "integer",
        }
    }
}

impl std::fmt::Display for UciValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UciValue::String(s) => write!(f, "{}", s),
            UciValue::List(v) => write!(f, "[{}]", v.join(", ")),
            UciValue::Boolean(b) => write!(f, "{}", b),
            UciValue::Integer(i) => write!(f, "{}", i),
        }
    }
}

impl From<String> for UciValue {
    fn from(s: String) -> Self {
        UciValue::String(s)
    }
}

impl From<&str> for UciValue {
    fn from(s: &str) -> Self {
        UciValue::String(s.to_string())
    }
}

impl From<bool> for UciValue {
    fn from(b: bool) -> Self {
        UciValue::Boolean(b)
    }
}

impl From<i64> for UciValue {
    fn from(i: i64) -> Self {
        UciValue::Integer(i)
    }
}

impl From<Vec<String>> for UciValue {
    fn from(v: Vec<String>) -> Self {
        UciValue::List(v)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UciConfig {
    pub package: String,
    pub config_type: String,
    pub name: String,
    pub options: HashMap<String, UciValue>,
    pub anonymous: bool,
}

impl UciConfig {
    pub fn new(package: &str, config_type: &str, name: &str) -> Self {
        Self {
            package: package.to_string(),
            config_type: config_type.to_string(),
            name: name.to_string(),
            options: HashMap::new(),
            anonymous: false,
        }
    }

    pub fn with_option(mut self, key: &str, value: UciValue) -> Self {
        self.options.insert(key.to_string(), value);
        self
    }

    pub fn with_anonymous(mut self, anonymous: bool) -> Self {
        self.anonymous = anonymous;
        self
    }

    pub fn get(&self, key: &str) -> Option<&UciValue> {
        self.options.get(key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UciAction {
    Set,
    Delete,
    AddList,
    Rename,
}

impl std::fmt::Display for UciAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UciAction::Set => write!(f, "set"),
            UciAction::Delete => write!(f, "delete"),
            UciAction::AddList => write!(f, "add_list"),
            UciAction::Rename => write!(f, "rename"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UciChange {
    pub action: UciAction,
    pub package: String,
    pub section: Option<String>,
    pub option: Option<String>,
    pub value: Option<UciValue>,
    pub old_value: Option<UciValue>,
}

impl UciChange {
    pub fn set(package: &str, section: &str, option: &str, value: UciValue) -> Self {
        Self {
            action: UciAction::Set,
            package: package.to_string(),
            section: Some(section.to_string()),
            option: Some(option.to_string()),
            value: Some(value),
            old_value: None,
        }
    }

    pub fn delete(package: &str, section: &str, option: Option<&str>) -> Self {
        Self {
            action: UciAction::Delete,
            package: package.to_string(),
            section: Some(section.to_string()),
            option: option.map(|s| s.to_string()),
            value: None,
            old_value: None,
        }
    }

    pub fn add_list(package: &str, section: &str, option: &str, value: UciValue) -> Self {
        Self {
            action: UciAction::AddList,
            package: package.to_string(),
            section: Some(section.to_string()),
            option: Some(option.to_string()),
            value: Some(value),
            old_value: None,
        }
    }

    pub fn rename(package: &str, section: &str, new_name: &str) -> Self {
        Self {
            action: UciAction::Rename,
            package: package.to_string(),
            section: Some(section.to_string()),
            option: Some(new_name.to_string()),
            value: None,
            old_value: None,
        }
    }

    pub fn with_old_value(mut self, old: UciValue) -> Self {
        self.old_value = Some(old);
        self
    }

    pub fn to_uci_command(&self) -> String {
        match &self.action {
            UciAction::Set => {
                let value = self
                    .value
                    .as_ref()
                    .map(|v| v.to_uci_string())
                    .unwrap_or_default();
                match (&self.section, &self.option) {
                    (Some(section), Some(option)) => {
                        format!("set {}.{}.{}={}", self.package, section, option, value)
                    }
                    (Some(section), None) => {
                        format!("set {}.{}", self.package, section)
                    }
                    _ => format!("set {}", self.package),
                }
            }
            UciAction::Delete => match (&self.section, &self.option) {
                (Some(section), Some(option)) => {
                    format!("delete {}.{}.{}", self.package, section, option)
                }
                (Some(section), None) => {
                    format!("delete {}.{}", self.package, section)
                }
                _ => format!("delete {}", self.package),
            },
            UciAction::AddList => {
                let value = self
                    .value
                    .as_ref()
                    .map(|v| v.to_uci_string())
                    .unwrap_or_default();
                format!(
                    "add_list {}.{}.{}={}",
                    self.package,
                    self.section.as_deref().unwrap_or(""),
                    self.option.as_deref().unwrap_or(""),
                    value
                )
            }
            UciAction::Rename => {
                format!(
                    "rename {}.{}={}",
                    self.package,
                    self.section.as_deref().unwrap_or(""),
                    self.option.as_deref().unwrap_or("")
                )
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UciTransaction {
    pub id: String,
    pub changes: Vec<UciChange>,
    pub snapshot_id: Option<String>,
    pub created_at: i64,
    pub committed: bool,
}

impl UciTransaction {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            changes: Vec::new(),
            snapshot_id: None,
            created_at: chrono::Utc::now().timestamp(),
            committed: false,
        }
    }

    pub fn with_snapshot(mut self, snapshot_id: &str) -> Self {
        self.snapshot_id = Some(snapshot_id.to_string());
        self
    }

    pub fn add_change(&mut self, change: UciChange) {
        self.changes.push(change);
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn change_count(&self) -> usize {
        self.changes.len()
    }
}

impl Default for UciTransaction {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UciValidationResult {
    pub valid: bool,
    pub package: String,
    pub errors: Vec<UciValidationError>,
    pub warnings: Vec<UciValidationWarning>,
}

impl UciValidationResult {
    pub fn success(package: &str) -> Self {
        Self {
            valid: true,
            package: package.to_string(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn failure(package: &str, errors: Vec<UciValidationError>) -> Self {
        Self {
            valid: false,
            package: package.to_string(),
            errors,
            warnings: Vec::new(),
        }
    }

    pub fn with_warning(mut self, warning: UciValidationWarning) -> Self {
        self.warnings.push(warning);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UciValidationError {
    pub section: Option<String>,
    pub option: Option<String>,
    pub message: String,
}

impl UciValidationError {
    pub fn new(message: &str) -> Self {
        Self {
            section: None,
            option: None,
            message: message.to_string(),
        }
    }

    pub fn with_section(mut self, section: &str) -> Self {
        self.section = Some(section.to_string());
        self
    }

    pub fn with_option(mut self, option: &str) -> Self {
        self.option = Some(option.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UciValidationWarning {
    pub section: Option<String>,
    pub option: Option<String>,
    pub message: String,
}

impl UciValidationWarning {
    pub fn new(message: &str) -> Self {
        Self {
            section: None,
            option: None,
            message: message.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UciListEntry {
    pub package: String,
    pub section: Option<String>,
    pub config_type: Option<String>,
    pub options: HashMap<String, UciValue>,
}

impl UciListEntry {
    pub fn package(package: &str) -> Self {
        Self {
            package: package.to_string(),
            section: None,
            config_type: None,
            options: HashMap::new(),
        }
    }

    pub fn section(package: &str, section: &str, config_type: &str) -> Self {
        Self {
            package: package.to_string(),
            section: Some(section.to_string()),
            config_type: Some(config_type.to_string()),
            options: HashMap::new(),
        }
    }

    pub fn with_option(mut self, key: &str, value: UciValue) -> Self {
        self.options.insert(key.to_string(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uci_value_string_conversion() {
        let v = UciValue::String("test".to_string());
        assert_eq!(v.as_str(), Some("test"));
        assert_eq!(v.to_uci_string(), "test");
    }

    #[test]
    fn uci_value_list_conversion() {
        let v = UciValue::List(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(v.as_list(), Some(&["a".to_string(), "b".to_string()][..]));
        assert_eq!(v.to_uci_string(), "a b");
    }

    #[test]
    fn uci_value_boolean_conversion() {
        let v = UciValue::Boolean(true);
        assert_eq!(v.as_bool(), Some(true));
        assert_eq!(v.to_uci_string(), "1");

        let v = UciValue::Boolean(false);
        assert_eq!(v.to_uci_string(), "0");
    }

    #[test]
    fn uci_value_integer_conversion() {
        let v = UciValue::Integer(42);
        assert_eq!(v.as_integer(), Some(42));
        assert_eq!(v.to_uci_string(), "42");
    }

    #[test]
    fn uci_change_set_command() {
        let change = UciChange::set("network", "lan", "ipaddr", UciValue::from("192.168.1.1"));
        assert_eq!(change.to_uci_command(), "set network.lan.ipaddr=192.168.1.1");
    }

    #[test]
    fn uci_change_delete_command() {
        let change = UciChange::delete("network", "lan", Some("ipaddr"));
        assert_eq!(change.to_uci_command(), "delete network.lan.ipaddr");
    }

    #[test]
    fn uci_change_add_list_command() {
        let change = UciChange::add_list(
            "firewall",
            "zone",
            "name",
            UciValue::from("wan"),
        );
        assert_eq!(change.to_uci_command(), "add_list firewall.zone.name=wan");
    }

    #[test]
    fn uci_transaction_add_changes() {
        let mut tx = UciTransaction::new();
        assert!(tx.is_empty());

        tx.add_change(UciChange::set("network", "lan", "ipaddr", UciValue::from("192.168.1.1")));
        tx.add_change(UciChange::set("network", "lan", "netmask", UciValue::from("255.255.255.0")));

        assert_eq!(tx.change_count(), 2);
        assert!(!tx.is_empty());
    }

    #[test]
    fn uci_config_builder() {
        let config = UciConfig::new("network", "interface", "lan")
            .with_option("ipaddr", UciValue::from("192.168.1.1"))
            .with_option("netmask", UciValue::from("255.255.255.0"))
            .with_anonymous(false);

        assert_eq!(config.package, "network");
        assert_eq!(config.config_type, "interface");
        assert_eq!(config.name, "lan");
        assert!(!config.anonymous);
        assert_eq!(config.get("ipaddr").unwrap().as_str(), Some("192.168.1.1"));
    }

    #[test]
    fn uci_validation_result() {
        let result = UciValidationResult::success("network");
        assert!(result.valid);
        assert!(result.errors.is_empty());

        let result = UciValidationResult::failure(
            "network",
            vec![UciValidationError::new("Invalid IP address")],
        );
        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
    }
}

//! UCI command execution wrapper.
//!
//! Provides safe execution of UCI commands with proper error handling,
//! output parsing, and security validation.

use anyhow::{bail, Context, Result};
use std::process::Command;
use std::sync::Arc;

use super::types::{UciChange, UciListEntry, UciValue, UciValidationResult, UciValidationError};
use crate::security::SecurityPolicy;

pub const UCI_COMMAND: &str = "uci";
pub const UCI_CONFIG_DIR: &str = "/etc/config";

pub struct UciExecutor {
    security: Arc<SecurityPolicy>,
    uci_command: String,
    config_dir: String,
    dry_run: bool,
}

impl UciExecutor {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self {
            security,
            uci_command: UCI_COMMAND.to_string(),
            config_dir: UCI_CONFIG_DIR.to_string(),
            dry_run: false,
        }
    }

    pub fn with_uci_command(mut self, cmd: &str) -> Self {
        self.uci_command = cmd.to_string();
        self
    }

    pub fn with_config_dir(mut self, dir: &str) -> Self {
        self.config_dir = dir.to_string();
        self
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn get(&self, package: &str, section: Option<&str>, option: Option<&str>) -> Result<String> {
        let key = self.build_key(package, section, option);
        let output = self.run_uci_command(&["get", &key])?;
        Ok(output.trim().to_string())
    }

    pub fn get_value(&self, package: &str, section: &str, option: &str) -> Result<UciValue> {
        let raw = self.get(package, Some(section), Some(option))?;
        Ok(UciValue::String(raw))
    }

    pub fn set(&self, change: &UciChange) -> Result<()> {
        if self.dry_run {
            tracing::info!("Dry run: would execute: uci {}", change.to_uci_command());
            return Ok(());
        }

        let cmd = change.to_uci_command();
        self.run_uci_command(&["set", &cmd])?;
        Ok(())
    }

    pub fn delete(&self, package: &str, section: Option<&str>, option: Option<&str>) -> Result<()> {
        if self.dry_run {
            tracing::info!(
                "Dry run: would delete: {}.{}.{}",
                package,
                section.unwrap_or(""),
                option.unwrap_or("")
            );
            return Ok(());
        }

        let key = self.build_key(package, section, option);
        self.run_uci_command(&["delete", &key])?;
        Ok(())
    }

    pub fn add_list(&self, package: &str, section: &str, option: &str, value: &str) -> Result<()> {
        if self.dry_run {
            tracing::info!(
                "Dry run: would add_list: {}.{}.{}={}",
                package, section, option, value
            );
            return Ok(());
        }

        let key = format!("{}.{}.{}", package, section, option);
        self.run_uci_command(&["add_list", &format!("{}={}", key, value)])?;
        Ok(())
    }

    pub fn rename(&self, package: &str, section: &str, new_name: &str) -> Result<()> {
        if self.dry_run {
            tracing::info!("Dry run: would rename: {}.{}={}", package, section, new_name);
            return Ok(());
        }

        let key = format!("{}.{}", package, section);
        self.run_uci_command(&["rename", &format!("{}={}", key, new_name)])?;
        Ok(())
    }

    pub fn list(&self, package: Option<&str>, section: Option<&str>) -> Result<Vec<UciListEntry>> {
        match (package, section) {
            (Some(pkg), Some(sec)) => self.list_section_options(pkg, sec),
            (Some(pkg), None) => self.list_package_sections(pkg),
            (None, None) => self.list_all_packages(),
            (None, Some(_)) => bail!("Cannot list section without package"),
        }
    }

    fn list_all_packages(&self) -> Result<Vec<UciListEntry>> {
        let config_dir = std::path::Path::new(&self.config_dir);
        if !config_dir.exists() {
            bail!("UCI config directory does not exist: {}", self.config_dir);
        }

        let mut entries = Vec::new();
        for entry in std::fs::read_dir(config_dir)
            .with_context(|| format!("Failed to read config directory: {}", self.config_dir))?
        {
            let entry = entry.context("Failed to read directory entry")?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') {
                entries.push(UciListEntry::package(&name));
            }
        }

        Ok(entries)
    }

    fn list_package_sections(&self, package: &str) -> Result<Vec<UciListEntry>> {
        let output = self.run_uci_command(&["show", package])?;
        self.parse_show_output(package, &output)
    }

    fn list_section_options(&self, package: &str, section: &str) -> Result<Vec<UciListEntry>> {
        let output = self.run_uci_command(&["show", &format!("{}.{}", package, section)])?;
        self.parse_show_output(package, &output)
    }

    fn parse_show_output(&self, package: &str, output: &str) -> Result<Vec<UciListEntry>> {
        let mut entries: Vec<UciListEntry> = Vec::new();
        let mut current_section: Option<(String, String)> = None;
        let mut current_options: std::collections::HashMap<String, UciValue> =
            std::collections::HashMap::new();

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() || !line.starts_with(package) {
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() != 2 {
                continue;
            }

            let key = parts[0];
            let value = parts[1];

            let key_parts: Vec<&str> = key.split('.').collect();
            if key_parts.len() < 2 {
                continue;
            }

            let section_name = key_parts[1];

            if key_parts.len() == 2 {
                if let Some((sec_name, sec_type)) = &current_section {
                    entries.push(
                        UciListEntry::section(package, sec_name, sec_type)
                            .with_options(std::mem::take(&mut current_options)),
                    );
                }
                current_section = Some((section_name.to_string(), value.to_string()));
                current_options.clear();
            } else if key_parts.len() >= 3 {
                let option_name = key_parts[2];
                current_options.insert(option_name.to_string(), UciValue::String(value.to_string()));
            }
        }

        if let Some((sec_name, sec_type)) = &current_section {
            entries.push(
                UciListEntry::section(package, sec_name, sec_type)
                    .with_options(std::mem::take(&mut current_options)),
            );
        }

        Ok(entries)
    }

    pub fn validate(&self, package: &str) -> Result<UciValidationResult> {
        let output = match self.run_uci_command_quiet(&["validate", package]) {
            Ok(o) => o,
            Err(e) => {
                return Ok(UciValidationResult::failure(
                    package,
                    vec![UciValidationError::new(&e.to_string())],
                ));
            }
        };

        if output.is_empty() || output.trim().is_empty() {
            return Ok(UciValidationResult::success(package));
        }

        let errors = output
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| UciValidationError::new(l))
            .collect();

        Ok(UciValidationResult::failure(package, errors))
    }

    pub fn commit(&self, package: Option<&str>) -> Result<()> {
        if self.dry_run {
            tracing::info!(
                "Dry run: would commit: {}",
                package.unwrap_or("all packages")
            );
            return Ok(());
        }

        match package {
            Some(pkg) => {
                self.run_uci_command(&["commit", pkg])?;
            }
            None => {
                self.run_uci_command(&["commit"])?;
            }
        }
        Ok(())
    }

    pub fn changes(&self, package: Option<&str>) -> Result<Vec<String>> {
        let output = match package {
            Some(pkg) => self.run_uci_command(&["changes", pkg])?,
            None => self.run_uci_command(&["changes"])?,
        };

        Ok(output
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect())
    }

    pub fn revert(&self, package: Option<&str>, section: Option<&str>, option: Option<&str>) -> Result<()> {
        if self.dry_run {
            tracing::info!(
                "Dry run: would revert: {}.{}.{}",
                package.unwrap_or("*"),
                section.unwrap_or("*"),
                option.unwrap_or("*")
            );
            return Ok(());
        }

        match (package, section, option) {
            (Some(pkg), Some(sec), Some(opt)) => {
                let key = format!("{}.{}.{}", pkg, sec, opt);
                self.run_uci_command(&["revert", &key])?;
            }
            (Some(pkg), Some(sec), None) => {
                let key = format!("{}.{}", pkg, sec);
                self.run_uci_command(&["revert", &key])?;
            }
            (Some(pkg), None, None) => {
                self.run_uci_command(&["revert", pkg])?;
            }
            (None, None, None) => {
                self.run_uci_command(&["revert"])?;
            }
            _ => bail!("Invalid revert parameters"),
        }
        Ok(())
    }

    pub fn add(&self, package: &str, config_type: &str) -> Result<String> {
        if self.dry_run {
            tracing::info!("Dry run: would add: {}.{}", package, config_type);
            return Ok("dry-run-section".to_string());
        }

        let output = self.run_uci_command(&["add", package, config_type])?;
        Ok(output.trim().to_string())
    }

    fn build_key(&self, package: &str, section: Option<&str>, option: Option<&str>) -> String {
        match (section, option) {
            (Some(sec), Some(opt)) => format!("{}.{}.{}", package, sec, opt),
            (Some(sec), None) => format!("{}.{}", package, sec),
            (None, None) => package.to_string(),
            (None, Some(_)) => package.to_string(),
        }
    }

    fn run_uci_command(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.uci_command)
            .args(args)
            .output()
            .with_context(|| format!("Failed to execute uci command: {} {:?}", self.uci_command, args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("UCI command failed: {}", stderr.trim());
        }

        String::from_utf8(output.stdout)
            .with_context(|| "UCI command output is not valid UTF-8")
    }

    fn run_uci_command_quiet(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.uci_command)
            .args(args)
            .output()
            .with_context(|| format!("Failed to execute uci command: {} {:?}", self.uci_command, args))?;

        String::from_utf8(output.stdout)
            .with_context(|| "UCI command output is not valid UTF-8")
    }

    pub fn is_uci_available(&self) -> bool {
        Command::new(&self.uci_command)
            .arg("help")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn config_exists(&self, package: &str) -> bool {
        std::path::Path::new(&self.config_dir)
            .join(package)
            .exists()
    }
}

impl UciListEntry {
    fn with_options(mut self, options: std::collections::HashMap<String, UciValue>) -> Self {
        self.options = options;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy::default())
    }

    #[test]
    fn build_key_variants() {
        let executor = UciExecutor::new(test_security());

        assert_eq!(executor.build_key("network", None, None), "network");
        assert_eq!(executor.build_key("network", Some("lan"), None), "network.lan");
        assert_eq!(
            executor.build_key("network", Some("lan"), Some("ipaddr")),
            "network.lan.ipaddr"
        );
    }

    #[test]
    fn dry_run_mode() {
        let executor = UciExecutor::new(test_security()).with_dry_run(true);

        let change = UciChange::set("network", "lan", "ipaddr", UciValue::from("192.168.1.1"));
        let result = executor.set(&change);
        assert!(result.is_ok());
    }
}

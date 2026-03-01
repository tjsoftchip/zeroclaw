//! Package management tools for OpenWrt.
//!
//! Provides tools for opkg package management and service control.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub maintainer: Option<String>,
    pub description: Option<String>,
    pub depends: Vec<String>,
    pub status: PackageStatus,
    pub size: Option<u64>,
    pub installed_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PackageStatus {
    Installed,
    NotInstalled,
    Upgradable,
}

impl std::fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageStatus::Installed => write!(f, "installed"),
            PackageStatus::NotInstalled => write!(f, "not-installed"),
            PackageStatus::Upgradable => write!(f, "upgradable"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub enabled: bool,
    pub running: bool,
    pub description: Option<String>,
}

pub struct OpkgListTool {
    security: Arc<SecurityPolicy>,
}

impl OpkgListTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    async fn list_packages(&self, filter: Option<&str>) -> anyhow::Result<Vec<PackageInfo>> {
        let output = tokio::process::Command::new("opkg")
            .args(["list", "--size"])
            .output()
            .await;

        let output = match output {
            Ok(o) => o,
            Err(_) => return Ok(Vec::new()),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            if let Some(pkg) = self.parse_package_line(line) {
                if let Some(f) = filter {
                    if !pkg.name.contains(f) {
                        continue;
                    }
                }
                packages.push(pkg);
            }
        }

        Ok(packages)
    }

    fn parse_package_line(&self, line: &str) -> Option<PackageInfo> {
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() < 2 {
            return None;
        }

        Some(PackageInfo {
            name: parts[0].to_string(),
            version: parts[1].to_string(),
            architecture: "all".to_string(),
            maintainer: None,
            description: parts.get(2).map(|s| s.to_string()),
            depends: Vec::new(),
            status: PackageStatus::NotInstalled,
            size: None,
            installed_size: None,
        })
    }

    fn format_packages(&self, packages: &[PackageInfo]) -> String {
        if packages.is_empty() {
            return "No packages found".to_string();
        }

        let mut output = String::from("Packages:\n\n");
        for pkg in packages {
            output.push_str(&format!(
                "{} ({})\n  Status: {}\n  Description: {}\n\n",
                pkg.name,
                pkg.version,
                pkg.status,
                pkg.description.as_deref().unwrap_or("N/A")
            ));
        }
        output
    }
}

#[async_trait]
impl Tool for OpkgListTool {
    fn name(&self) -> &str {
        "opkg_list"
    }

    fn description(&self) -> &str {
        "List available or installed packages on OpenWrt using opkg."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "filter": {
                    "type": "string",
                    "description": "Filter packages by name pattern"
                },
                "installed_only": {
                    "type": "boolean",
                    "default": false,
                    "description": "Show only installed packages"
                },
                "upgradable_only": {
                    "type": "boolean",
                    "default": false,
                    "description": "Show only upgradable packages"
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded".to_string()),
            });
        }

        let filter = args.get("filter").and_then(|v| v.as_str());
        let packages = self.list_packages(filter).await?;

        let output = self.format_packages(&packages);

        Ok(ToolResult {
            success: true,
            output,
            error: None,
        })
    }
}

pub struct OpkgInstallTool {
    security: Arc<SecurityPolicy>,
}

impl OpkgInstallTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for OpkgInstallTool {
    fn name(&self) -> &str {
        "opkg_install"
    }

    fn description(&self) -> &str {
        "Install packages on OpenWrt using opkg."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "Package name to install"
                },
                "force_depends": {
                    "type": "boolean",
                    "default": false,
                    "description": "Force installation even if dependencies are not met"
                },
                "force_overwrite": {
                    "type": "boolean",
                    "default": false,
                    "description": "Force overwrite existing files"
                }
            },
            "required": ["package"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded".to_string()),
            });
        }

        let package = args
            .get("package")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("package is required"))?;

        let mut cmd_args = vec!["install", package];

        let output = tokio::process::Command::new("opkg")
            .args(&cmd_args)
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                Ok(ToolResult {
                    success: true,
                    output: format!("Package '{}' installed successfully.", package),
                    error: None,
                })
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to install package: {}", stderr)),
                })
            }
            Err(e) => {
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to execute opkg: {}", e)),
                })
            }
        }
    }
}

pub struct OpkgRemoveTool {
    security: Arc<SecurityPolicy>,
}

impl OpkgRemoveTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for OpkgRemoveTool {
    fn name(&self) -> &str {
        "opkg_remove"
    }

    fn description(&self) -> &str {
        "Remove packages from OpenWrt using opkg."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "Package name to remove"
                },
                "force_depends": {
                    "type": "boolean",
                    "default": false,
                    "description": "Force removal even if other packages depend on it"
                },
                "purge": {
                    "type": "boolean",
                    "default": false,
                    "description": "Remove configuration files as well"
                }
            },
            "required": ["package"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded".to_string()),
            });
        }

        let package = args
            .get("package")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("package is required"))?;

        let output = tokio::process::Command::new("opkg")
            .args(["remove", package])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                Ok(ToolResult {
                    success: true,
                    output: format!("Package '{}' removed successfully.", package),
                    error: None,
                })
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to remove package: {}", stderr)),
                })
            }
            Err(e) => {
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to execute opkg: {}", e)),
                })
            }
        }
    }
}

pub struct OpkgUpdateTool {
    security: Arc<SecurityPolicy>,
}

impl OpkgUpdateTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for OpkgUpdateTool {
    fn name(&self) -> &str {
        "opkg_update"
    }

    fn description(&self) -> &str {
        "Update package lists on OpenWrt using opkg."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded".to_string()),
            });
        }

        let output = tokio::process::Command::new("opkg")
            .arg("update")
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                Ok(ToolResult {
                    success: true,
                    output: "Package lists updated successfully.".to_string(),
                    error: None,
                })
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to update package lists: {}", stderr)),
                })
            }
            Err(e) => {
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to execute opkg: {}", e)),
                })
            }
        }
    }
}

pub struct ServiceManageTool {
    security: Arc<SecurityPolicy>,
}

impl ServiceManageTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    async fn list_services(&self) -> anyhow::Result<Vec<ServiceInfo>> {
        let initd_path = std::path::Path::new("/etc/init.d");
        if !initd_path.exists() {
            return Ok(Vec::new());
        }

        let mut services = Vec::new();
        let mut entries = tokio::fs::read_dir(initd_path).await?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }

            let enabled = self.is_service_enabled(&name).await.unwrap_or(false);
            let running = self.is_service_running(&name).await.unwrap_or(false);

            services.push(ServiceInfo {
                name,
                enabled,
                running,
                description: None,
            });
        }

        Ok(services)
    }

    async fn is_service_enabled(&self, name: &str) -> anyhow::Result<bool> {
        let rc_path = format!("/etc/rc.d/S*{}", name);
        let pattern = glob::glob(&rc_path)?;
        Ok(pattern.count() > 0)
    }

    async fn is_service_running(&self, name: &str) -> anyhow::Result<bool> {
        let output = tokio::process::Command::new("/etc/init.d")
            .arg(name)
            .arg("status")
            .output()
            .await;

        match output {
            Ok(o) => Ok(o.status.success()),
            Err(_) => Ok(false),
        }
    }

    fn format_services(&self, services: &[ServiceInfo]) -> String {
        if services.is_empty() {
            return "No services found".to_string();
        }

        let mut output = String::from("Services:\n\n");
        for svc in services {
            output.push_str(&format!(
                "{}\n  Enabled: {}\n  Running: {}\n\n",
                svc.name,
                if svc.enabled { "Yes" } else { "No" },
                if svc.running { "Yes" } else { "No" }
            ));
        }
        output
    }
}

#[async_trait]
impl Tool for ServiceManageTool {
    fn name(&self) -> &str {
        "service_manage"
    }

    fn description(&self) -> &str {
        "Manage OpenWrt services (start, stop, restart, enable, disable, status)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "start", "stop", "restart", "enable", "disable", "status"],
                    "default": "list"
                },
                "service": {
                    "type": "string",
                    "description": "Service name (required for all actions except list)"
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded".to_string()),
            });
        }

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");

        match action {
            "list" => {
                let services = self.list_services().await?;
                let output = self.format_services(&services);
                Ok(ToolResult { success: true, output, error: None })
            }
            "start" | "stop" | "restart" | "enable" | "disable" => {
                let service = args
                    .get("service")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("service is required for this action"))?;

                let output = tokio::process::Command::new("/etc/init.d")
                    .arg(service)
                    .arg(action)
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => {
                        Ok(ToolResult {
                            success: true,
                            output: format!("Service '{}' {} successfully.", service, action),
                            error: None,
                        })
                    }
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Failed to {} service: {}", action, stderr)),
                        })
                    }
                    Err(e) => {
                        Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Failed to execute: {}", e)),
                        })
                    }
                }
            }
            "status" => {
                let service = args
                    .get("service")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("service is required for status action"))?;

                let running = self.is_service_running(service).await.unwrap_or(false);
                let enabled = self.is_service_enabled(service).await.unwrap_or(false);

                Ok(ToolResult {
                    success: true,
                    output: format!(
                        "Service '{}'\n  Enabled: {}\n  Running: {}",
                        service,
                        if enabled { "Yes" } else { "No" },
                        if running { "Yes" } else { "No" }
                    ),
                    error: None,
                })
            }
            _ => Err(anyhow::anyhow!("Invalid action: {}", action)),
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
    fn test_package_status_display() {
        assert_eq!(PackageStatus::Installed.to_string(), "installed");
        assert_eq!(PackageStatus::NotInstalled.to_string(), "not-installed");
        assert_eq!(PackageStatus::Upgradable.to_string(), "upgradable");
    }

    #[test]
    fn test_opkg_list_tool_metadata() {
        let tool = OpkgListTool::new(test_security());
        assert_eq!(tool.name(), "opkg_list");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_opkg_install_tool_metadata() {
        let tool = OpkgInstallTool::new(test_security());
        assert_eq!(tool.name(), "opkg_install");
    }

    #[test]
    fn test_service_manage_tool_metadata() {
        let tool = ServiceManageTool::new(test_security());
        assert_eq!(tool.name(), "service_manage");
    }
}

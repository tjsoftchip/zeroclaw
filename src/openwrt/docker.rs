//! Docker management tools for OpenWrt.
//!
//! Provides tools for managing Docker containers, images, networks, and volumes.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub state: String,
    pub ports: Vec<PortMapping>,
    pub created: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContainerStatus {
    Running,
    Paused,
    Exited,
    Created,
    Dead,
}

impl std::fmt::Display for ContainerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerStatus::Running => write!(f, "running"),
            ContainerStatus::Paused => write!(f, "paused"),
            ContainerStatus::Exited => write!(f, "exited"),
            ContainerStatus::Created => write!(f, "created"),
            ContainerStatus::Dead => write!(f, "dead"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub container_port: u16,
    pub host_port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub subnet: Option<String>,
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created: i64,
}

pub struct DockerListTool {
    security: Arc<SecurityPolicy>,
}

impl DockerListTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    async fn list_containers(&self, all: bool) -> anyhow::Result<Vec<ContainerInfo>> {
        let mut args = vec!["ps", "--format", "{{json .}}"];
        if all {
            args.push("-a");
        }

        let output = tokio::process::Command::new("docker")
            .args(&args)
            .output()
            .await;

        let output = match output {
            Ok(o) if o.status.success() => o,
            _ => return Ok(Vec::new()),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut containers = Vec::new();

        for line in stdout.lines() {
            if line.is_empty() {
                continue;
            }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(container) = self.parse_container(&json) {
                    containers.push(container);
                }
            }
        }

        Ok(containers)
    }

    fn parse_container(&self, json: &serde_json::Value) -> Option<ContainerInfo> {
        Some(ContainerInfo {
            id: json.get("ID")?.as_str()?.to_string(),
            name: json.get("Names")?.as_str().unwrap_or("unknown").to_string(),
            image: json.get("Image")?.as_str().unwrap_or("unknown").to_string(),
            status: self.parse_status(json.get("State")?.as_str().unwrap_or("")),
            state: json.get("Status")?.as_str().unwrap_or("").to_string(),
            ports: Vec::new(),
            created: 0,
        })
    }

    fn parse_status(&self, state: &str) -> ContainerStatus {
        match state.to_lowercase().as_str() {
            "running" => ContainerStatus::Running,
            "paused" => ContainerStatus::Paused,
            "exited" => ContainerStatus::Exited,
            "created" => ContainerStatus::Created,
            "dead" => ContainerStatus::Dead,
            _ => ContainerStatus::Exited,
        }
    }

    fn format_containers(&self, containers: &[ContainerInfo]) -> String {
        if containers.is_empty() {
            return "No containers found".to_string();
        }

        let mut output = String::from("Containers:\n\n");
        for c in containers {
            output.push_str(&format!(
                "{} ({})\n  Image: {}\n  Status: {}\n  State: {}\n\n",
                c.name, c.id, c.image, c.status, c.state
            ));
        }
        output
    }
}

#[async_trait]
impl Tool for DockerListTool {
    fn name(&self) -> &str {
        "docker_list"
    }

    fn description(&self) -> &str {
        "List Docker containers on OpenWrt."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "all": {
                    "type": "boolean",
                    "default": false,
                    "description": "Show all containers (including stopped)"
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

        let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
        let containers = self.list_containers(all).await?;
        let output = self.format_containers(&containers);

        Ok(ToolResult { success: true, output, error: None })
    }
}

pub struct DockerManageTool {
    security: Arc<SecurityPolicy>,
}

impl DockerManageTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for DockerManageTool {
    fn name(&self) -> &str {
        "docker_manage"
    }

    fn description(&self) -> &str {
        "Manage Docker containers (start, stop, restart, remove, logs)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start", "stop", "restart", "remove", "logs", "exec"],
                    "description": "Action to perform"
                },
                "container": {
                    "type": "string",
                    "description": "Container name or ID"
                },
                "command": {
                    "type": "string",
                    "description": "Command to execute (for exec action)"
                }
            },
            "required": ["action", "container"]
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

        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("action is required"))?;

        let container = args
            .get("container")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("container is required"))?;

        let output = match action {
            "start" | "stop" | "restart" | "rm" => {
                tokio::process::Command::new("docker")
                    .args([action, container])
                    .output()
                    .await
            }
            "remove" => {
                tokio::process::Command::new("docker")
                    .args(["rm", container])
                    .output()
                    .await
            }
            "logs" => {
                tokio::process::Command::new("docker")
                    .args(["logs", "--tail", "100", container])
                    .output()
                    .await
            }
            "exec" => {
                let command = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("command is required for exec action"))?;

                tokio::process::Command::new("docker")
                    .args(["exec", container, "sh", "-c", command])
                    .output()
                    .await
            }
            _ => return Err(anyhow::anyhow!("Invalid action: {}", action)),
        };

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                Ok(ToolResult {
                    success: true,
                    output: if stdout.is_empty() {
                        format!("Container '{}' {} successfully.", container, action)
                    } else {
                        stdout.to_string()
                    },
                    error: None,
                })
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed: {}", stderr)),
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to execute docker: {}", e)),
            }),
        }
    }
}

pub struct DockerImageTool {
    security: Arc<SecurityPolicy>,
}

impl DockerImageTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for DockerImageTool {
    fn name(&self) -> &str {
        "docker_image"
    }

    fn description(&self) -> &str {
        "Manage Docker images (list, pull, remove, prune)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "pull", "remove", "prune"],
                    "default": "list"
                },
                "image": {
                    "type": "string",
                    "description": "Image name (for pull/remove)"
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
                let output = tokio::process::Command::new("docker")
                    .args(["images", "--format", "{{.Repository}}:{{.Tag}}\t{{.Size}}"])
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => {
                        let stdout = String::from_utf8_lossy(&o.stdout);
                        Ok(ToolResult {
                            success: true,
                            output: format!("Images:\n\n{}", stdout),
                            error: None,
                        })
                    }
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Failed: {}", stderr)),
                        })
                    }
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed: {}", e)),
                    }),
                }
            }
            "pull" => {
                let image = args
                    .get("image")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("image is required for pull"))?;

                let output = tokio::process::Command::new("docker")
                    .args(["pull", image])
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => Ok(ToolResult {
                        success: true,
                        output: format!("Image '{}' pulled successfully.", image),
                        error: None,
                    }),
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Failed to pull: {}", stderr)),
                        })
                    }
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed: {}", e)),
                    }),
                }
            }
            "remove" => {
                let image = args
                    .get("image")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("image is required for remove"))?;

                let output = tokio::process::Command::new("docker")
                    .args(["rmi", image])
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => Ok(ToolResult {
                        success: true,
                        output: format!("Image '{}' removed.", image),
                        error: None,
                    }),
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Failed: {}", stderr)),
                        })
                    }
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed: {}", e)),
                    }),
                }
            }
            "prune" => {
                let output = tokio::process::Command::new("docker")
                    .args(["image", "prune", "-f"])
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => Ok(ToolResult {
                        success: true,
                        output: "Unused images pruned.".to_string(),
                        error: None,
                    }),
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Failed: {}", stderr)),
                        })
                    }
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed: {}", e)),
                    }),
                }
            }
            _ => Err(anyhow::anyhow!("Invalid action: {}", action)),
        }
    }
}

pub struct DockerNetworkTool {
    security: Arc<SecurityPolicy>,
}

impl DockerNetworkTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for DockerNetworkTool {
    fn name(&self) -> &str {
        "docker_network"
    }

    fn description(&self) -> &str {
        "Manage Docker networks (list, create, remove)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "create", "remove"],
                    "default": "list"
                },
                "name": {
                    "type": "string",
                    "description": "Network name (for create/remove)"
                },
                "driver": {
                    "type": "string",
                    "default": "bridge",
                    "description": "Network driver (for create)"
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
                let output = tokio::process::Command::new("docker")
                    .args(["network", "ls", "--format", "{{.Name}}\t{{.Driver}}"])
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => {
                        let stdout = String::from_utf8_lossy(&o.stdout);
                        Ok(ToolResult {
                            success: true,
                            output: format!("Networks:\n\n{}", stdout),
                            error: None,
                        })
                    }
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Failed: {}", stderr)),
                        })
                    }
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed: {}", e)),
                    }),
                }
            }
            "create" => {
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("name is required for create"))?;

                let driver = args.get("driver").and_then(|v| v.as_str()).unwrap_or("bridge");

                let output = tokio::process::Command::new("docker")
                    .args(["network", "create", "-d", driver, name])
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => Ok(ToolResult {
                        success: true,
                        output: format!("Network '{}' created.", name),
                        error: None,
                    }),
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Failed: {}", stderr)),
                        })
                    }
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed: {}", e)),
                    }),
                }
            }
            "remove" => {
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("name is required for remove"))?;

                let output = tokio::process::Command::new("docker")
                    .args(["network", "rm", name])
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => Ok(ToolResult {
                        success: true,
                        output: format!("Network '{}' removed.", name),
                        error: None,
                    }),
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!("Failed: {}", stderr)),
                        })
                    }
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed: {}", e)),
                    }),
                }
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
    fn test_container_status_display() {
        assert_eq!(ContainerStatus::Running.to_string(), "running");
        assert_eq!(ContainerStatus::Exited.to_string(), "exited");
    }

    #[test]
    fn test_docker_list_tool_metadata() {
        let tool = DockerListTool::new(test_security());
        assert_eq!(tool.name(), "docker_list");
    }

    #[test]
    fn test_docker_manage_tool_metadata() {
        let tool = DockerManageTool::new(test_security());
        assert_eq!(tool.name(), "docker_manage");
    }

    #[test]
    fn test_docker_image_tool_metadata() {
        let tool = DockerImageTool::new(test_security());
        assert_eq!(tool.name(), "docker_image");
    }

    #[test]
    fn test_docker_network_tool_metadata() {
        let tool = DockerNetworkTool::new(test_security());
        assert_eq!(tool.name(), "docker_network");
    }
}

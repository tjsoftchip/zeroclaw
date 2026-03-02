use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeBase {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub document_count: usize,
    pub total_tokens: usize,
    pub embedding_model: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeStoreConfig {
    pub data_dir: PathBuf,
    pub max_bases: usize,
    pub max_documents_per_base: usize,
    pub default_embedding_model: String,
}

impl Default for KnowledgeStoreConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("/var/lib/zeroclaw/knowledge"),
            max_bases: 100,
            max_documents_per_base: 10000,
            default_embedding_model: "text-embedding-ada-002".to_string(),
        }
    }
}

pub struct KnowledgeStoreTool {
    config: KnowledgeStoreConfig,
    security: Arc<SecurityPolicy>,
}

impl KnowledgeStoreTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self::with_config(KnowledgeStoreConfig::default(), security)
    }

    pub fn with_config(config: KnowledgeStoreConfig, security: Arc<SecurityPolicy>) -> Self {
        Self { config, security }
    }

    async fn ensure_data_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.config.data_dir).await?;
        fs::create_dir_all(self.config.data_dir.join("bases")).await?;
        Ok(())
    }

    fn bases_file(&self) -> PathBuf {
        self.config.data_dir.join("bases.json")
    }

    fn base_dir(&self, id: &str) -> PathBuf {
        self.config.data_dir.join("bases").join(id)
    }

    fn base_meta_file(&self, id: &str) -> PathBuf {
        self.base_dir(id).join("meta.json")
    }

    async fn load_bases(&self) -> Result<Vec<KnowledgeBase>> {
        let file = self.bases_file();
        if !file.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&file).await?;
        let bases: Vec<KnowledgeBase> = serde_json::from_str(&content)?;
        Ok(bases)
    }

    async fn save_bases(&self, bases: &[KnowledgeBase]) -> Result<()> {
        self.ensure_data_dir().await?;
        let file = self.bases_file();
        let content = serde_json::to_string_pretty(bases)?;
        let mut f = fs::File::create(&file).await?;
        f.write_all(content.as_bytes()).await?;
        Ok(())
    }

    async fn create_base(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let name = args["name"].as_str()
            .ok_or_else(|| anyhow!("name is required"))?
            .to_string();
        let description = args["description"].as_str().unwrap_or("").to_string();
        let embedding_model = args["embedding_model"].as_str()
            .unwrap_or(&self.config.default_embedding_model)
            .to_string();

        let mut bases = self.load_bases().await?;
        if bases.len() >= self.config.max_bases {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Maximum number of knowledge bases reached".to_string()) });
        }

        let id = format!("kb_{}", chrono::Utc::now().timestamp());
        let now = chrono::Utc::now().timestamp();

        let base = KnowledgeBase {
            id: id.clone(),
            name,
            description,
            created_at: now,
            updated_at: now,
            document_count: 0,
            total_tokens: 0,
            embedding_model,
            metadata: HashMap::new(),
        };

        fs::create_dir_all(self.base_dir(&id)).await?;
        let meta_content = serde_json::to_string_pretty(&base)?;
        fs::write(self.base_meta_file(&id), &meta_content).await?;

        bases.push(base.clone());
        self.save_bases(&bases).await?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&base)?, error: None })
    }

    async fn list_bases(&self) -> Result<ToolResult> {
        let bases = self.load_bases().await?;
        Ok(ToolResult { success: true, output: serde_json::to_string(&bases)?, error: None })
    }

    async fn get_base(&self, id: &str) -> Result<ToolResult> {
        let meta_file = self.base_meta_file(id);
        if !meta_file.exists() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Knowledge base {} not found", id)) });
        }
        let content = fs::read_to_string(&meta_file).await?;
        let base: KnowledgeBase = serde_json::from_str(&content)?;
        Ok(ToolResult { success: true, output: serde_json::to_string(&base)?, error: None })
    }

    async fn delete_base(&self, id: &str) -> Result<ToolResult> {
        let mut bases = self.load_bases().await?;
        let initial_len = bases.len();
        bases.retain(|b| b.id != id);

        if bases.len() == initial_len {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Knowledge base {} not found", id)) });
        }

        self.save_bases(&bases).await?;

        let base_dir = self.base_dir(id);
        if base_dir.exists() {
            fs::remove_dir_all(&base_dir).await?;
        }

        Ok(ToolResult { success: true, output: serde_json::to_string(&json!({"deleted": true, "id": id}))?, error: None })
    }

    async fn update_base(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let id = args["id"].as_str()
            .ok_or_else(|| anyhow!("id is required"))?;

        let meta_file = self.base_meta_file(id);
        if !meta_file.exists() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Knowledge base {} not found", id)) });
        }

        let content = fs::read_to_string(&meta_file).await?;
        let mut base: KnowledgeBase = serde_json::from_str(&content)?;

        if let Some(name) = args["name"].as_str() {
            base.name = name.to_string();
        }
        if let Some(description) = args["description"].as_str() {
            base.description = description.to_string();
        }
        base.updated_at = chrono::Utc::now().timestamp();

        let updated_content = serde_json::to_string_pretty(&base)?;
        fs::write(&meta_file, &updated_content).await?;

        let mut bases = self.load_bases().await?;
        if let Some(existing) = bases.iter_mut().find(|b| b.id == id) {
            *existing = base.clone();
        }
        self.save_bases(&bases).await?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&base)?, error: None })
    }
}

#[async_trait]
impl Tool for KnowledgeStoreTool {
    fn name(&self) -> &str {
        "knowledge_store"
    }

    fn description(&self) -> &str {
        "Manage knowledge bases for private knowledge management. Supports creating, listing, getting, and deleting knowledge bases."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "get", "delete", "update"],
                    "description": "Action to perform"
                },
                "id": { "type": "string", "description": "Knowledge base ID (required for get, delete, update)" },
                "name": { "type": "string", "description": "Knowledge base name (required for create)" },
                "description": { "type": "string", "description": "Knowledge base description" },
                "embedding_model": { "type": "string", "description": "Embedding model to use" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args["action"].as_str()
            .ok_or_else(|| anyhow!("action is required"))?;

        self.ensure_data_dir().await?;

        match action {
            "create" => self.create_base(&args).await,
            "list" => self.list_bases().await,
            "get" => {
                let id = args["id"].as_str()
                    .ok_or_else(|| anyhow!("id is required for get action"))?;
                self.get_base(id).await
            }
            "delete" => {
                let id = args["id"].as_str()
                    .ok_or_else(|| anyhow!("id is required for delete action"))?;
                self.delete_base(id).await
            }
            "update" => self.update_base(&args).await,
            _ => Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Unknown action: {}", action)) }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy::default())
    }

    #[tokio::test]
    async fn test_knowledge_store_create() {
        let dir = tempdir().unwrap();
        let config = KnowledgeStoreConfig {
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let tool = KnowledgeStoreTool::with_config(config, test_security());

        let params = json!({
            "action": "create",
            "name": "Test Knowledge Base",
            "description": "A test knowledge base"
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.success);
        let output: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert!(output["id"].as_str().unwrap().starts_with("kb_"));
    }

    #[tokio::test]
    async fn test_knowledge_store_list() {
        let dir = tempdir().unwrap();
        let config = KnowledgeStoreConfig {
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let tool = KnowledgeStoreTool::with_config(config, test_security());

        let create_params = json!({
            "action": "create",
            "name": "Test KB 1",
            "description": "First test KB"
        });
        tool.execute(create_params).await.unwrap();

        let list_params = json!({ "action": "list" });
        let result = tool.execute(list_params).await.unwrap();
        assert!(result.success);
        let output: Vec<serde_json::Value> = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output.len(), 1);
    }

    #[tokio::test]
    async fn test_knowledge_store_delete() {
        let dir = tempdir().unwrap();
        let config = KnowledgeStoreConfig {
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let tool = KnowledgeStoreTool::with_config(config, test_security());

        let create_params = json!({
            "action": "create",
            "name": "Test KB to Delete",
            "description": "Will be deleted"
        });
        let create_result = tool.execute(create_params).await.unwrap();
        let output: serde_json::Value = serde_json::from_str(&create_result.output).unwrap();
        let kb_id = output["id"].as_str().unwrap();

        let delete_params = json!({
            "action": "delete",
            "id": kb_id
        });
        let result = tool.execute(delete_params).await.unwrap();
        assert!(result.success);
        let output: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert!(output["deleted"].as_bool().unwrap());
    }
}

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
pub struct EmbeddingConfig {
    pub model: String,
    pub dimensions: usize,
    pub batch_size: usize,
    pub max_tokens: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "text-embedding-ada-002".to_string(),
            dimensions: 1536,
            batch_size: 100,
            max_tokens: 8191,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingEntry {
    pub id: String,
    pub text_hash: String,
    pub embedding: Vec<f32>,
    pub model: String,
    pub created_at: i64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub id: String,
    pub score: f32,
    pub content: Option<String>,
    pub metadata: HashMap<String, String>,
}

pub struct EmbeddingTool {
    config: EmbeddingConfig,
    data_dir: PathBuf,
    security: Arc<SecurityPolicy>,
}

impl EmbeddingTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self::with_config(EmbeddingConfig::default(), PathBuf::from("/var/lib/zeroclaw/knowledge"), security)
    }

    pub fn with_config(config: EmbeddingConfig, data_dir: PathBuf, security: Arc<SecurityPolicy>) -> Self {
        Self { config, data_dir, security }
    }

    fn base_dir(&self, kb_id: &str) -> PathBuf {
        self.data_dir.join("bases").join(kb_id)
    }

    fn embeddings_file(&self, kb_id: &str) -> PathBuf {
        self.base_dir(kb_id).join("embeddings.json")
    }

    async fn load_embeddings(&self, kb_id: &str) -> Result<Vec<EmbeddingEntry>> {
        let file = self.embeddings_file(kb_id);
        if !file.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&file).await?;
        let embeddings: Vec<EmbeddingEntry> = serde_json::from_str(&content)?;
        Ok(embeddings)
    }

    async fn save_embeddings(&self, kb_id: &str, embeddings: &[EmbeddingEntry]) -> Result<()> {
        let file = self.embeddings_file(kb_id);
        let content = serde_json::to_string_pretty(embeddings)?;
        let mut f = fs::File::create(&file).await?;
        f.write_all(content.as_bytes()).await?;
        Ok(())
    }

    fn simple_hash(text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    fn mock_embedding(text: &str, dimensions: usize) -> Vec<f32> {
        let bytes = text.as_bytes();
        let mut embedding = vec![0.0f32; dimensions];
        
        for (i, byte) in bytes.iter().cycle().take(dimensions).enumerate() {
            embedding[i] = (*byte as f32 / 255.0 - 0.5) * 2.0;
        }
        
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }
        
        embedding
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }
        
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    async fn generate_embedding(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let texts = if let Some(texts) = args.get("texts").and_then(|t| t.as_array()) {
            texts.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect::<Vec<_>>()
        } else if let Some(text) = args["text"].as_str() {
            vec![text.to_string()]
        } else {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("text or texts is required for generate action".to_string()) });
        };

        let mut results = Vec::new();
        for text in texts {
            let embedding = Self::mock_embedding(&text, self.config.dimensions);
            let text_hash = Self::simple_hash(&text);
            
            results.push(json!({
                "text_hash": text_hash,
                "embedding": embedding,
                "dimensions": embedding.len(),
                "model": self.config.model
            }));
        }

        Ok(ToolResult { success: true, output: serde_json::to_string(&results)?, error: None })
    }

    async fn store_embedding(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;

        let base_dir = self.base_dir(kb_id);
        if !base_dir.exists() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Knowledge base {} not found", kb_id)) });
        }

        let text = args["text"].as_str()
            .ok_or_else(|| anyhow!("text is required for store action"))?;

        let embedding = if let Some(emb) = args.get("embedding").and_then(|e| e.as_array()) {
            emb.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect()
        } else {
            Self::mock_embedding(text, self.config.dimensions)
        };

        let id = format!("emb_{}", chrono::Utc::now().timestamp_millis());
        let text_hash = Self::simple_hash(text);

        let entry = EmbeddingEntry {
            id: id.clone(),
            text_hash: text_hash.clone(),
            embedding,
            model: self.config.model.clone(),
            created_at: chrono::Utc::now().timestamp(),
            metadata: HashMap::new(),
        };

        let mut embeddings = self.load_embeddings(kb_id).await?;
        embeddings.push(entry.clone());
        self.save_embeddings(kb_id, &embeddings).await?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&json!({"id": id, "text_hash": text_hash, "dimensions": entry.embedding.len(), "model": entry.model}))?, error: None })
    }

    async fn search_embeddings(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;

        let query_embedding: Vec<f32> = if let Some(emb) = args.get("query_embedding").and_then(|e| e.as_array()) {
            emb.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect()
        } else if let Some(text) = args["text"].as_str() {
            Self::mock_embedding(text, self.config.dimensions)
        } else {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("query_embedding or text is required for search action".to_string()) });
        };

        let top_k = args["top_k"].as_u64().unwrap_or(10) as usize;
        let threshold = args["threshold"].as_f64().unwrap_or(0.0) as f32;

        let embeddings = self.load_embeddings(kb_id).await?;
        
        let mut results: Vec<VectorSearchResult> = embeddings
            .iter()
            .map(|e| {
                let score = Self::cosine_similarity(&query_embedding, &e.embedding);
                VectorSearchResult {
                    id: e.id.clone(),
                    score,
                    content: None,
                    metadata: e.metadata.clone(),
                }
            })
            .filter(|r| r.score >= threshold)
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);

        Ok(ToolResult { success: true, output: serde_json::to_string(&results)?, error: None })
    }

    async fn delete_embedding(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;
        let emb_id = args["embedding_id"].as_str()
            .ok_or_else(|| anyhow!("embedding_id is required for delete action"))?;

        let mut embeddings = self.load_embeddings(kb_id).await?;
        let initial_len = embeddings.len();
        embeddings.retain(|e| e.id != emb_id);

        if embeddings.len() == initial_len {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Embedding {} not found", emb_id)) });
        }

        self.save_embeddings(kb_id, &embeddings).await?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&json!({"deleted": true, "id": emb_id}))?, error: None })
    }

    async fn get_stats(&self, kb_id: &str) -> Result<ToolResult> {
        let embeddings = self.load_embeddings(kb_id).await?;
        
        let total_embeddings = embeddings.len();
        let total_dimensions = if !embeddings.is_empty() {
            embeddings[0].embedding.len()
        } else {
            self.config.dimensions
        };

        let models: std::collections::HashSet<String> = embeddings
            .iter()
            .map(|e| e.model.clone())
            .collect();

        Ok(ToolResult { success: true, output: serde_json::to_string(&json!({
            "total_embeddings": total_embeddings,
            "dimensions": total_dimensions,
            "models": models,
            "config": {
                "default_model": self.config.model,
                "default_dimensions": self.config.dimensions,
                "batch_size": self.config.batch_size,
                "max_tokens": self.config.max_tokens
            }
        }))?, error: None })
    }
}

#[async_trait]
impl Tool for EmbeddingTool {
    fn name(&self) -> &str {
        "embedding_manage"
    }

    fn description(&self) -> &str {
        "Manage embeddings for semantic search. Supports generating, storing, and searching vector embeddings."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["generate", "store", "search", "delete", "stats"],
                    "description": "Action to perform"
                },
                "knowledge_base_id": { "type": "string", "description": "Knowledge base ID" },
                "text": { "type": "string", "description": "Text to generate embedding for" },
                "texts": { "type": "array", "items": { "type": "string" }, "description": "Array of texts for batch mode" },
                "embedding_id": { "type": "string", "description": "Embedding ID (required for delete)" },
                "query_embedding": { "type": "array", "items": { "type": "number" }, "description": "Query embedding vector" },
                "top_k": { "type": "integer", "description": "Number of top results (default: 10)" },
                "threshold": { "type": "number", "description": "Minimum similarity threshold (default: 0.0)" }
            },
            "required": ["action", "knowledge_base_id"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("Rate limit exceeded".to_string()) });
        }

        let action = args["action"].as_str()
            .ok_or_else(|| anyhow!("action is required"))?;

        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;

        match action {
            "generate" => self.generate_embedding(&args).await,
            "store" => self.store_embedding(&args).await,
            "search" => self.search_embeddings(&args).await,
            "delete" => self.delete_embedding(&args).await,
            "stats" => self.get_stats(kb_id).await,
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
    async fn test_embedding_generate() {
        let dir = tempdir().unwrap();
        let tool = EmbeddingTool::with_config(EmbeddingConfig::default(), dir.path().to_path_buf(), test_security());

        let params = json!({
            "action": "generate",
            "knowledge_base_id": "kb_test",
            "text": "Hello world"
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.success);
        let output: Vec<serde_json::Value> = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output.len(), 1);
    }

    #[tokio::test]
    async fn test_embedding_store_and_search() {
        let dir = tempdir().unwrap();
        let tool = EmbeddingTool::with_config(EmbeddingConfig::default(), dir.path().to_path_buf(), test_security());

        let kb_id = "kb_embed_test";
        fs::create_dir_all(tool.base_dir(kb_id)).await.unwrap();

        let store_params = json!({
            "action": "store",
            "knowledge_base_id": kb_id,
            "text": "Rust programming language"
        });
        let store_result = tool.execute(store_params).await.unwrap();
        assert!(store_result.success);

        let search_params = json!({
            "action": "search",
            "knowledge_base_id": kb_id,
            "text": "Rust programming",
            "top_k": 5
        });
        let search_result = tool.execute(search_params).await.unwrap();
        assert!(search_result.success);
        let output: Vec<serde_json::Value> = serde_json::from_str(&search_result.output).unwrap();
        assert!(!output.is_empty());
    }

    #[tokio::test]
    async fn test_embedding_stats() {
        let dir = tempdir().unwrap();
        let tool = EmbeddingTool::with_config(EmbeddingConfig::default(), dir.path().to_path_buf(), test_security());

        let kb_id = "kb_stats_test";
        fs::create_dir_all(tool.base_dir(kb_id)).await.unwrap();

        let store_params = json!({
            "action": "store",
            "knowledge_base_id": kb_id,
            "text": "Test document for stats"
        });
        tool.execute(store_params).await.unwrap();

        let stats_params = json!({
            "action": "stats",
            "knowledge_base_id": kb_id
        });
        let result = tool.execute(stats_params).await.unwrap();
        assert!(result.success);
        let output: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output["total_embeddings"].as_u64().unwrap(), 1);
    }
}

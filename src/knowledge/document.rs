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
pub struct Document {
    pub id: String,
    pub knowledge_base_id: String,
    pub title: String,
    pub content: String,
    pub content_type: String,
    pub source: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub token_count: usize,
    pub chunk_count: usize,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub id: String,
    pub document_id: String,
    pub content: String,
    pub token_count: usize,
    pub embedding: Option<Vec<f32>>,
    pub metadata: HashMap<String, String>,
}

pub struct DocumentManageTool {
    data_dir: PathBuf,
    security: Arc<SecurityPolicy>,
}

impl DocumentManageTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self::with_path(PathBuf::from("/var/lib/zeroclaw/knowledge"), security)
    }

    pub fn with_path(data_dir: PathBuf, security: Arc<SecurityPolicy>) -> Self {
        Self { data_dir, security }
    }

    fn base_dir(&self, kb_id: &str) -> PathBuf {
        self.data_dir.join("bases").join(kb_id)
    }

    fn documents_file(&self, kb_id: &str) -> PathBuf {
        self.base_dir(kb_id).join("documents.json")
    }

    fn document_dir(&self, kb_id: &str, doc_id: &str) -> PathBuf {
        self.base_dir(kb_id).join("documents").join(doc_id)
    }

    fn chunks_file(&self, kb_id: &str, doc_id: &str) -> PathBuf {
        self.document_dir(kb_id, doc_id).join("chunks.json")
    }

    async fn load_documents(&self, kb_id: &str) -> Result<Vec<Document>> {
        let file = self.documents_file(kb_id);
        if !file.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&file).await?;
        let docs: Vec<Document> = serde_json::from_str(&content)?;
        Ok(docs)
    }

    async fn save_documents(&self, kb_id: &str, docs: &[Document]) -> Result<()> {
        let file = self.documents_file(kb_id);
        let content = serde_json::to_string_pretty(docs)?;
        let mut f = fs::File::create(&file).await?;
        f.write_all(content.as_bytes()).await?;
        Ok(())
    }

    fn estimate_tokens(text: &str) -> usize {
        text.split_whitespace().count()
    }

    fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let mut start = 0;

        while start < words.len() {
            let end = (start + chunk_size).min(words.len());
            let chunk = words[start..end].join(" ");
            chunks.push(chunk);
            start += chunk_size - overlap;
        }

        chunks
    }

    async fn add_document(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;

        let base_dir = self.base_dir(kb_id);
        if !base_dir.exists() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Knowledge base {} not found", kb_id)) });
        }

        let title = args["title"].as_str()
            .ok_or_else(|| anyhow!("title is required"))?
            .to_string();
        let content = args["content"].as_str()
            .ok_or_else(|| anyhow!("content is required"))?
            .to_string();
        let content_type = args["content_type"].as_str().unwrap_or("text").to_string();
        let source = args["source"].as_str().unwrap_or("").to_string();

        let doc_id = format!("doc_{}", chrono::Utc::now().timestamp_millis());
        let now = chrono::Utc::now().timestamp();
        let token_count = Self::estimate_tokens(&content);

        let chunks = Self::chunk_text(&content, 500, 50);
        let chunk_count = chunks.len();

        let doc = Document {
            id: doc_id.clone(),
            knowledge_base_id: kb_id.to_string(),
            title,
            content,
            content_type,
            source,
            created_at: now,
            updated_at: now,
            token_count,
            chunk_count,
            metadata: HashMap::new(),
        };

        let doc_dir = self.document_dir(kb_id, &doc_id);
        fs::create_dir_all(&doc_dir).await?;

        let doc_chunks: Vec<DocumentChunk> = chunks
            .into_iter()
            .enumerate()
            .map(|(i, content)| DocumentChunk {
                id: format!("{}_chunk_{}", doc_id, i),
                document_id: doc_id.clone(),
                content,
                token_count: Self::estimate_tokens(&content),
                embedding: None,
                metadata: HashMap::new(),
            })
            .collect();

        let chunks_content = serde_json::to_string_pretty(&doc_chunks)?;
        fs::write(self.chunks_file(kb_id, &doc_id), &chunks_content).await?;

        let mut docs = self.load_documents(kb_id).await?;
        docs.push(doc.clone());
        self.save_documents(kb_id, &docs).await?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&doc)?, error: None })
    }

    async fn list_documents(&self, kb_id: &str) -> Result<ToolResult> {
        let docs = self.load_documents(kb_id).await?;
        let summaries: Vec<serde_json::Value> = docs
            .iter()
            .map(|d| {
                json!({
                    "id": d.id,
                    "title": d.title,
                    "content_type": d.content_type,
                    "source": d.source,
                    "token_count": d.token_count,
                    "chunk_count": d.chunk_count,
                    "created_at": d.created_at,
                    "updated_at": d.updated_at
                })
            })
            .collect();
        Ok(ToolResult { success: true, output: serde_json::to_string(&summaries)?, error: None })
    }

    async fn get_document(&self, kb_id: &str, doc_id: &str) -> Result<ToolResult> {
        let docs = self.load_documents(kb_id).await?;
        let doc = docs
            .iter()
            .find(|d| d.id == doc_id)
            .ok_or_else(|| anyhow!("Document {} not found", doc_id))?;

        let chunks_file = self.chunks_file(kb_id, doc_id);
        let chunks: Vec<DocumentChunk> = if chunks_file.exists() {
            let content = fs::read_to_string(&chunks_file).await?;
            serde_json::from_str(&content)?
        } else {
            Vec::new()
        };

        Ok(ToolResult { success: true, output: serde_json::to_string(&json!({"document": doc, "chunks": chunks}))?, error: None })
    }

    async fn delete_document(&self, kb_id: &str, doc_id: &str) -> Result<ToolResult> {
        let mut docs = self.load_documents(kb_id).await?;
        let initial_len = docs.len();
        docs.retain(|d| d.id != doc_id);

        if docs.len() == initial_len {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Document {} not found", doc_id)) });
        }

        self.save_documents(kb_id, &docs).await?;

        let doc_dir = self.document_dir(kb_id, doc_id);
        if doc_dir.exists() {
            fs::remove_dir_all(&doc_dir).await?;
        }

        Ok(ToolResult { success: true, output: serde_json::to_string(&json!({"deleted": true, "id": doc_id}))?, error: None })
    }

    async fn search_documents(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;
        let query = args["query"].as_str()
            .ok_or_else(|| anyhow!("query is required for search"))?;
        let limit = args["limit"].as_u64().unwrap_or(10) as usize;

        let docs = self.load_documents(kb_id).await?;
        let query_lower = query.to_lowercase();
        let mut results: Vec<serde_json::Value> = Vec::new();

        for doc in &docs {
            if results.len() >= limit {
                break;
            }

            let title_match = doc.title.to_lowercase().contains(&query_lower);
            let content_match = doc.content.to_lowercase().contains(&query_lower);

            if title_match || content_match {
                let score = if title_match { 1.0 } else { 0.5 };
                results.push(json!({
                    "document": {
                        "id": doc.id,
                        "title": doc.title,
                        "content_type": doc.content_type,
                        "source": doc.source,
                        "token_count": doc.token_count
                    },
                    "score": score,
                    "match_type": if title_match { "title" } else { "content" }
                }));
            }
        }

        Ok(ToolResult { success: true, output: serde_json::to_string(&results)?, error: None })
    }

    async fn update_document(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;
        let doc_id = args["document_id"].as_str()
            .ok_or_else(|| anyhow!("document_id is required"))?;

        let mut docs = self.load_documents(kb_id).await?;
        let doc = docs
            .iter_mut()
            .find(|d| d.id == doc_id)
            .ok_or_else(|| anyhow!("Document {} not found", doc_id))?;

        if let Some(title) = args["title"].as_str() {
            doc.title = title.to_string();
        }
        if let Some(content) = args["content"].as_str() {
            doc.content = content.to_string();
            doc.token_count = Self::estimate_tokens(&doc.content);

            let chunks = Self::chunk_text(&doc.content, 500, 50);
            doc.chunk_count = chunks.len();

            let doc_chunks: Vec<DocumentChunk> = chunks
                .into_iter()
                .enumerate()
                .map(|(i, content)| DocumentChunk {
                    id: format!("{}_chunk_{}", doc_id, i),
                    document_id: doc_id.to_string(),
                    content,
                    token_count: Self::estimate_tokens(&content),
                    embedding: None,
                    metadata: HashMap::new(),
                })
                .collect();

            let chunks_content = serde_json::to_string_pretty(&doc_chunks)?;
            fs::write(self.chunks_file(kb_id, doc_id), &chunks_content).await?;
        }
        doc.updated_at = chrono::Utc::now().timestamp();

        self.save_documents(kb_id, &docs).await?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&doc)?, error: None })
    }
}

#[async_trait]
impl Tool for DocumentManageTool {
    fn name(&self) -> &str {
        "document_manage"
    }

    fn description(&self) -> &str {
        "Manage documents in knowledge bases. Supports adding, listing, getting, deleting, and searching documents."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "list", "get", "delete", "search", "update"],
                    "description": "Action to perform"
                },
                "knowledge_base_id": { "type": "string", "description": "Knowledge base ID" },
                "document_id": { "type": "string", "description": "Document ID (required for get, delete, update)" },
                "title": { "type": "string", "description": "Document title" },
                "content": { "type": "string", "description": "Document content" },
                "content_type": { "type": "string", "description": "Content type: text, markdown, html, pdf" },
                "source": { "type": "string", "description": "Document source URL or path" },
                "query": { "type": "string", "description": "Search query (required for search action)" },
                "limit": { "type": "integer", "description": "Maximum number of results (default: 10)" }
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
            "add" => self.add_document(&args).await,
            "list" => self.list_documents(kb_id).await,
            "get" => {
                let doc_id = args["document_id"].as_str()
                    .ok_or_else(|| anyhow!("document_id is required for get action"))?;
                self.get_document(kb_id, doc_id).await
            }
            "delete" => {
                let doc_id = args["document_id"].as_str()
                    .ok_or_else(|| anyhow!("document_id is required for delete action"))?;
                self.delete_document(kb_id, doc_id).await
            }
            "search" => self.search_documents(&args).await,
            "update" => self.update_document(&args).await,
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
    async fn test_document_add_and_list() {
        let dir = tempdir().unwrap();
        let tool = DocumentManageTool::with_path(dir.path().to_path_buf(), test_security());

        let kb_id = "kb_test";
        fs::create_dir_all(tool.base_dir(kb_id)).await.unwrap();
        fs::write(tool.documents_file(kb_id), "[]").await.unwrap();

        let add_params = json!({
            "action": "add",
            "knowledge_base_id": kb_id,
            "title": "Test Document",
            "content": "This is a test document content.",
            "content_type": "text"
        });

        let result = tool.execute(add_params).await.unwrap();
        assert!(result.success);
        let output: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert!(output["id"].as_str().unwrap().starts_with("doc_"));

        let list_params = json!({
            "action": "list",
            "knowledge_base_id": kb_id
        });
        let list_result = tool.execute(list_params).await.unwrap();
        assert!(list_result.success);
        let output: Vec<serde_json::Value> = serde_json::from_str(&list_result.output).unwrap();
        assert_eq!(output.len(), 1);
    }

    #[tokio::test]
    async fn test_document_search() {
        let dir = tempdir().unwrap();
        let tool = DocumentManageTool::with_path(dir.path().to_path_buf(), test_security());

        let kb_id = "kb_search_test";
        fs::create_dir_all(tool.base_dir(kb_id)).await.unwrap();
        fs::write(tool.documents_file(kb_id), "[]").await.unwrap();

        let add_params = json!({
            "action": "add",
            "knowledge_base_id": kb_id,
            "title": "Rust Programming Guide",
            "content": "Learn Rust programming language basics and advanced concepts."
        });
        tool.execute(add_params).await.unwrap();

        let search_params = json!({
            "action": "search",
            "knowledge_base_id": kb_id,
            "query": "Rust"
        });
        let result = tool.execute(search_params).await.unwrap();
        assert!(result.success);
        let output: Vec<serde_json::Value> = serde_json::from_str(&result.output).unwrap();
        assert!(!output.is_empty());
    }
}

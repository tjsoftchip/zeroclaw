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
pub struct KnowledgeNode {
    pub id: String,
    pub label: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub weight: f32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    pub nodes: Vec<KnowledgeNode>,
    pub edges: Vec<KnowledgeEdge>,
    pub total_weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub node_labels: HashMap<String, usize>,
    pub edge_relations: HashMap<String, usize>,
}

pub struct KnowledgeGraphTool {
    data_dir: PathBuf,
    security: Arc<SecurityPolicy>,
}

impl KnowledgeGraphTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self::with_path(PathBuf::from("/var/lib/zeroclaw/knowledge"), security)
    }

    pub fn with_path(data_dir: PathBuf, security: Arc<SecurityPolicy>) -> Self {
        Self { data_dir, security }
    }

    fn base_dir(&self, kb_id: &str) -> PathBuf {
        self.data_dir.join("bases").join(kb_id)
    }

    fn nodes_file(&self, kb_id: &str) -> PathBuf {
        self.base_dir(kb_id).join("graph_nodes.json")
    }

    fn edges_file(&self, kb_id: &str) -> PathBuf {
        self.base_dir(kb_id).join("graph_edges.json")
    }

    async fn load_nodes(&self, kb_id: &str) -> Result<Vec<KnowledgeNode>> {
        let file = self.nodes_file(kb_id);
        if !file.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&file).await?;
        let nodes: Vec<KnowledgeNode> = serde_json::from_str(&content)?;
        Ok(nodes)
    }

    async fn save_nodes(&self, kb_id: &str, nodes: &[KnowledgeNode]) -> Result<()> {
        let file = self.nodes_file(kb_id);
        let content = serde_json::to_string_pretty(nodes)?;
        let mut f = fs::File::create(&file).await?;
        f.write_all(content.as_bytes()).await?;
        Ok(())
    }

    async fn load_edges(&self, kb_id: &str) -> Result<Vec<KnowledgeEdge>> {
        let file = self.edges_file(kb_id);
        if !file.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&file).await?;
        let edges: Vec<KnowledgeEdge> = serde_json::from_str(&content)?;
        Ok(edges)
    }

    async fn save_edges(&self, kb_id: &str, edges: &[KnowledgeEdge]) -> Result<()> {
        let file = self.edges_file(kb_id);
        let content = serde_json::to_string_pretty(edges)?;
        let mut f = fs::File::create(&file).await?;
        f.write_all(content.as_bytes()).await?;
        Ok(())
    }

    async fn add_node(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;

        let base_dir = self.base_dir(kb_id);
        if !base_dir.exists() {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Knowledge base {} not found", kb_id)) });
        }

        let label = args["label"].as_str()
            .ok_or_else(|| anyhow!("label is required"))?
            .to_string();

        let properties: HashMap<String, serde_json::Value> = args
            .get("properties")
            .and_then(|p| p.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let id = format!("node_{}", chrono::Utc::now().timestamp_millis());
        let now = chrono::Utc::now().timestamp();

        let node = KnowledgeNode {
            id: id.clone(),
            label,
            properties,
            created_at: now,
            updated_at: now,
        };

        let mut nodes = self.load_nodes(kb_id).await?;
        nodes.push(node.clone());
        self.save_nodes(kb_id, &nodes).await?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&node)?, error: None })
    }

    async fn get_node(&self, kb_id: &str, node_id: &str) -> Result<ToolResult> {
        let nodes = self.load_nodes(kb_id).await?;
        let node = nodes
            .iter()
            .find(|n| n.id == node_id)
            .ok_or_else(|| anyhow!("Node {} not found", node_id))?;

        let edges = self.load_edges(kb_id).await?;
        let outgoing: Vec<&KnowledgeEdge> = edges.iter().filter(|e| e.source_id == node_id).collect();
        let incoming: Vec<&KnowledgeEdge> = edges.iter().filter(|e| e.target_id == node_id).collect();

        Ok(ToolResult { success: true, output: serde_json::to_string(&json!({
            "node": node,
            "outgoing_edges": outgoing.len(),
            "incoming_edges": incoming.len()
        }))?, error: None })
    }

    async fn delete_node(&self, kb_id: &str, node_id: &str) -> Result<ToolResult> {
        let mut nodes = self.load_nodes(kb_id).await?;
        let initial_len = nodes.len();
        nodes.retain(|n| n.id != node_id);

        if nodes.len() == initial_len {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Node {} not found", node_id)) });
        }

        self.save_nodes(kb_id, &nodes).await?;

        let mut edges = self.load_edges(kb_id).await?;
        let edges_before = edges.len();
        edges.retain(|e| e.source_id != node_id && e.target_id != node_id);
        self.save_edges(kb_id, &edges).await?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&json!({
            "deleted": true,
            "id": node_id,
            "edges_removed": edges_before - edges.len()
        }))?, error: None })
    }

    async fn add_edge(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;

        let source_id = args["source_id"].as_str()
            .ok_or_else(|| anyhow!("source_id is required"))?
            .to_string();
        let target_id = args["target_id"].as_str()
            .ok_or_else(|| anyhow!("target_id is required"))?
            .to_string();
        let relation = args["relation"].as_str()
            .ok_or_else(|| anyhow!("relation is required"))?
            .to_string();

        let nodes = self.load_nodes(kb_id).await?;
        if !nodes.iter().any(|n| n.id == source_id) {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Source node {} not found", source_id)) });
        }
        if !nodes.iter().any(|n| n.id == target_id) {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Target node {} not found", target_id)) });
        }

        let properties: HashMap<String, serde_json::Value> = args
            .get("properties")
            .and_then(|p| p.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let weight = args["weight"].as_f64().unwrap_or(1.0) as f32;

        let id = format!("edge_{}", chrono::Utc::now().timestamp_millis());

        let edge = KnowledgeEdge {
            id: id.clone(),
            source_id,
            target_id,
            relation,
            properties,
            weight,
            created_at: chrono::Utc::now().timestamp(),
        };

        let mut edges = self.load_edges(kb_id).await?;
        edges.push(edge.clone());
        self.save_edges(kb_id, &edges).await?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&edge)?, error: None })
    }

    async fn get_edge(&self, kb_id: &str, edge_id: &str) -> Result<ToolResult> {
        let edges = self.load_edges(kb_id).await?;
        let edge = edges
            .iter()
            .find(|e| e.id == edge_id)
            .ok_or_else(|| anyhow!("Edge {} not found", edge_id))?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&edge)?, error: None })
    }

    async fn delete_edge(&self, kb_id: &str, edge_id: &str) -> Result<ToolResult> {
        let mut edges = self.load_edges(kb_id).await?;
        let initial_len = edges.len();
        edges.retain(|e| e.id != edge_id);

        if edges.len() == initial_len {
            return Ok(ToolResult { success: false, output: String::new(), error: Some(format!("Edge {} not found", edge_id)) });
        }

        self.save_edges(kb_id, &edges).await?;

        Ok(ToolResult { success: true, output: serde_json::to_string(&json!({"deleted": true, "id": edge_id}))?, error: None })
    }

    async fn find_path(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;
        let source_id = args["source_id"].as_str()
            .ok_or_else(|| anyhow!("source_id is required"))?;
        let target_id = args["target_id"].as_str()
            .ok_or_else(|| anyhow!("target_id is required"))?;
        let max_depth = args["max_depth"].as_u64().unwrap_or(5) as usize;

        let nodes = self.load_nodes(kb_id).await?;
        let edges = self.load_edges(kb_id).await?;

        let mut adjacency: HashMap<String, Vec<(String, String, f32)>> = HashMap::new();
        for edge in &edges {
            adjacency
                .entry(edge.source_id.clone())
                .or_default()
                .push((edge.target_id.clone(), edge.id.clone(), edge.weight));
        }

        let mut visited: HashMap<String, (String, String)> = HashMap::new();
        let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();
        queue.push_back((source_id.to_string(), 0));

        while let Some((current, depth)) = queue.pop_front() {
            if current == target_id {
                break;
            }
            if depth >= max_depth {
                continue;
            }

            if let Some(neighbors) = adjacency.get(&current) {
                for (next, edge_id, _) in neighbors {
                    if !visited.contains_key(next) {
                        visited.insert(next.clone(), (current.clone(), edge_id.clone()));
                        queue.push_back((next.clone(), depth + 1));
                    }
                }
            }
        }

        if !visited.contains_key(target_id) && source_id != target_id {
            return Ok(ToolResult { success: false, output: String::new(), error: Some("No path found".to_string()) });
        }

        let mut path_nodes = Vec::new();
        let mut path_edges = Vec::new();
        let mut current = target_id.to_string();
        let mut total_weight = 0.0f32;

        while current != source_id {
            if let Some(node) = nodes.iter().find(|n| n.id == current) {
                path_nodes.push(node.clone());
            }

            if let Some((prev, edge_id)) = visited.get(&current) {
                if let Some(edge) = edges.iter().find(|e| e.id == edge_id) {
                    total_weight += edge.weight;
                    path_edges.push(edge.clone());
                }
                current = prev.clone();
            } else {
                break;
            }
        }

        if let Some(node) = nodes.iter().find(|n| n.id == source_id) {
            path_nodes.push(node.clone());
        }

        path_nodes.reverse();
        path_edges.reverse();

        Ok(ToolResult { success: true, output: serde_json::to_string(&GraphPath {
            nodes: path_nodes,
            edges: path_edges,
            total_weight,
        })?, error: None })
    }

    async fn query_graph(&self, args: &serde_json::Value) -> Result<ToolResult> {
        let kb_id = args["knowledge_base_id"].as_str()
            .ok_or_else(|| anyhow!("knowledge_base_id is required"))?;

        let nodes = self.load_nodes(kb_id).await?;
        let edges = self.load_edges(kb_id).await?;

        let filtered_nodes: Vec<&KnowledgeNode> = if let Some(label) = args["query_label"].as_str() {
            nodes.iter().filter(|n| n.label == label).collect()
        } else {
            nodes.iter().collect()
        };

        let filtered_edges: Vec<&KnowledgeEdge> = if let Some(relation) = args["query_relation"].as_str() {
            edges.iter().filter(|e| e.relation == relation).collect()
        } else {
            edges.iter().collect()
        };

        Ok(ToolResult { success: true, output: serde_json::to_string(&json!({
            "nodes": filtered_nodes,
            "edges": filtered_edges
        }))?, error: None })
    }

    async fn get_stats(&self, kb_id: &str) -> Result<ToolResult> {
        let nodes = self.load_nodes(kb_id).await?;
        let edges = self.load_edges(kb_id).await?;

        let mut node_labels: HashMap<String, usize> = HashMap::new();
        for node in &nodes {
            *node_labels.entry(node.label.clone()).or_default() += 1;
        }

        let mut edge_relations: HashMap<String, usize> = HashMap::new();
        for edge in &edges {
            *edge_relations.entry(edge.relation.clone()).or_default() += 1;
        }

        let stats = GraphStats {
            node_count: nodes.len(),
            edge_count: edges.len(),
            node_labels,
            edge_relations,
        };

        Ok(ToolResult { success: true, output: serde_json::to_string(&stats)?, error: None })
    }
}

#[async_trait]
impl Tool for KnowledgeGraphTool {
    fn name(&self) -> &str {
        "knowledge_graph"
    }

    fn description(&self) -> &str {
        "Manage knowledge graph for entity relationships. Supports nodes, edges, paths, and graph queries."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add_node", "get_node", "delete_node", "add_edge", "get_edge", "delete_edge", "find_path", "query", "stats"],
                    "description": "Action to perform"
                },
                "knowledge_base_id": { "type": "string", "description": "Knowledge base ID" },
                "node_id": { "type": "string", "description": "Node ID" },
                "edge_id": { "type": "string", "description": "Edge ID" },
                "label": { "type": "string", "description": "Node label/type" },
                "source_id": { "type": "string", "description": "Source node ID for edge" },
                "target_id": { "type": "string", "description": "Target node ID for edge or path" },
                "relation": { "type": "string", "description": "Edge relation type" },
                "properties": { "type": "object", "description": "Node or edge properties" },
                "weight": { "type": "number", "description": "Edge weight (default: 1.0)" },
                "max_depth": { "type": "integer", "description": "Maximum path depth (default: 5)" },
                "query_label": { "type": "string", "description": "Filter nodes by label" },
                "query_relation": { "type": "string", "description": "Filter edges by relation" }
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
            "add_node" => self.add_node(&args).await,
            "get_node" => {
                let node_id = args["node_id"].as_str()
                    .ok_or_else(|| anyhow!("node_id is required for get_node action"))?;
                self.get_node(kb_id, node_id).await
            }
            "delete_node" => {
                let node_id = args["node_id"].as_str()
                    .ok_or_else(|| anyhow!("node_id is required for delete_node action"))?;
                self.delete_node(kb_id, node_id).await
            }
            "add_edge" => self.add_edge(&args).await,
            "get_edge" => {
                let edge_id = args["edge_id"].as_str()
                    .ok_or_else(|| anyhow!("edge_id is required for get_edge action"))?;
                self.get_edge(kb_id, edge_id).await
            }
            "delete_edge" => {
                let edge_id = args["edge_id"].as_str()
                    .ok_or_else(|| anyhow!("edge_id is required for delete_edge action"))?;
                self.delete_edge(kb_id, edge_id).await
            }
            "find_path" => self.find_path(&args).await,
            "query" => self.query_graph(&args).await,
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
    async fn test_graph_add_node() {
        let dir = tempdir().unwrap();
        let tool = KnowledgeGraphTool::with_path(dir.path().to_path_buf(), test_security());

        let kb_id = "kb_graph_test";
        fs::create_dir_all(tool.base_dir(kb_id)).await.unwrap();

        let params = json!({
            "action": "add_node",
            "knowledge_base_id": kb_id,
            "label": "Person",
            "properties": {
                "name": "Alice",
                "age": 30
            }
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.success);
        let output: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert!(output["id"].as_str().unwrap().starts_with("node_"));
    }

    #[tokio::test]
    async fn test_graph_add_edge_and_find_path() {
        let dir = tempdir().unwrap();
        let tool = KnowledgeGraphTool::with_path(dir.path().to_path_buf(), test_security());

        let kb_id = "kb_path_test";
        fs::create_dir_all(tool.base_dir(kb_id)).await.unwrap();

        let node1_params = json!({
            "action": "add_node",
            "knowledge_base_id": kb_id,
            "label": "Person",
            "properties": {"name": "Alice"}
        });
        let node1_result = tool.execute(node1_params).await.unwrap();
        let node1_output: serde_json::Value = serde_json::from_str(&node1_result.output).unwrap();
        let node1_id = node1_output["id"].as_str().unwrap();

        let node2_params = json!({
            "action": "add_node",
            "knowledge_base_id": kb_id,
            "label": "Person",
            "properties": {"name": "Bob"}
        });
        let node2_result = tool.execute(node2_params).await.unwrap();
        let node2_output: serde_json::Value = serde_json::from_str(&node2_result.output).unwrap();
        let node2_id = node2_output["id"].as_str().unwrap();

        let edge_params = json!({
            "action": "add_edge",
            "knowledge_base_id": kb_id,
            "source_id": node1_id,
            "target_id": node2_id,
            "relation": "knows"
        });
        let edge_result = tool.execute(edge_params).await.unwrap();
        assert!(edge_result.success);

        let path_params = json!({
            "action": "find_path",
            "knowledge_base_id": kb_id,
            "source_id": node1_id,
            "target_id": node2_id
        });
        let path_result = tool.execute(path_params).await.unwrap();
        assert!(path_result.success);
        let path_output: serde_json::Value = serde_json::from_str(&path_result.output).unwrap();
        assert_eq!(path_output["nodes"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_graph_stats() {
        let dir = tempdir().unwrap();
        let tool = KnowledgeGraphTool::with_path(dir.path().to_path_buf(), test_security());

        let kb_id = "kb_stats_test";
        fs::create_dir_all(tool.base_dir(kb_id)).await.unwrap();

        let node_params = json!({
            "action": "add_node",
            "knowledge_base_id": kb_id,
            "label": "Person"
        });
        tool.execute(node_params).await.unwrap();

        let stats_params = json!({
            "action": "stats",
            "knowledge_base_id": kb_id
        });
        let result = tool.execute(stats_params).await.unwrap();
        assert!(result.success);
        let output: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output["node_count"].as_u64().unwrap(), 1);
    }
}

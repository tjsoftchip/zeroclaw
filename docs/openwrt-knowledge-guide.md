# Knowledge Management Guide

This guide covers the private knowledge management features in ZeroClaw.

**Last Updated**: March 2026  
**Module**: `src/knowledge/`

## Table of Contents

- [Overview](#overview)
- [Knowledge Bases](#knowledge-bases)
- [Document Management](#document-management)
- [Embeddings & Semantic Search](#embeddings--semantic-search)
- [Knowledge Graph](#knowledge-graph)
- [Use Cases](#use-cases)

## Overview

The knowledge management module provides tools for building and querying private knowledge bases:

```
┌─────────────────────────────────────────────────────────────┐
│                   Knowledge Management                       │
├─────────────────────────────────────────────────────────────┤
│  Storage Layer                                              │
│  ├── Knowledge Bases (isolated containers)                  │
│  ├── Documents (with auto-chunking)                         │
│  └── Embeddings (vector representations)                    │
├─────────────────────────────────────────────────────────────┤
│  Search Layer                                               │
│  ├── Keyword Search                                         │
│  ├── Semantic Search (vector similarity)                    │
│  └── Hybrid Search                                          │
├─────────────────────────────────────────────────────────────┤
│  Graph Layer                                                │
│  ├── Entity Nodes                                           │
│  ├── Relationship Edges                                     │
│  └── Path Finding                                           │
└─────────────────────────────────────────────────────────────┘
```

---

## Knowledge Bases

Knowledge bases are isolated containers for documents, embeddings, and graphs.

### Create Knowledge Base

```json
{
    "tool": "knowledge_store",
    "params": {
        "action": "create",
        "name": "Product Documentation",
        "description": "Technical documentation and manuals",
        "embedding_model": "text-embedding-ada-002"
    }
}
```

**Response:**
```json
{
    "id": "kb_1709318400",
    "name": "Product Documentation",
    "description": "Technical documentation and manuals",
    "created_at": 1709318400,
    "updated_at": 1709318400,
    "document_count": 0,
    "total_tokens": 0,
    "embedding_model": "text-embedding-ada-002"
}
```

### List Knowledge Bases

```json
{
    "tool": "knowledge_store",
    "params": {
        "action": "list"
    }
}
```

**Response:**
```json
[
    {
        "id": "kb_1709318400",
        "name": "Product Documentation",
        "document_count": 15,
        "total_tokens": 45000
    },
    {
        "id": "kb_1709318500",
        "name": "Support Knowledge",
        "document_count": 32,
        "total_tokens": 128000
    }
]
```

### Get Knowledge Base

```json
{
    "tool": "knowledge_store",
    "params": {
        "action": "get",
        "id": "kb_1709318400"
    }
}
```

### Update Knowledge Base

```json
{
    "tool": "knowledge_store",
    "params": {
        "action": "update",
        "id": "kb_1709318400",
        "name": "Product Docs v2",
        "description": "Updated product documentation"
    }
}
```

### Delete Knowledge Base

```json
{
    "tool": "knowledge_store",
    "params": {
        "action": "delete",
        "id": "kb_1709318400"
    }
}
```

---

## Document Management

### Add Document

Documents are automatically chunked for optimal retrieval:

```json
{
    "tool": "document_manage",
    "params": {
        "action": "add",
        "knowledge_base_id": "kb_1709318400",
        "title": "API Reference Guide",
        "content": "# API Reference\n\nThis document provides comprehensive API documentation...\n\n## Authentication\n\nAll API requests require authentication...",
        "content_type": "markdown",
        "source": "https://docs.example.com/api"
    }
}
```

**Response:**
```json
{
    "id": "doc_1709318400000",
    "knowledge_base_id": "kb_1709318400",
    "title": "API Reference Guide",
    "content_type": "markdown",
    "source": "https://docs.example.com/api",
    "token_count": 1500,
    "chunk_count": 4,
    "created_at": 1709318400
}
```

### Document Chunking

Documents are automatically split into chunks:
- **Chunk size**: 500 tokens
- **Overlap**: 50 tokens
- **Purpose**: Maintain context across chunk boundaries

### List Documents

```json
{
    "tool": "document_manage",
    "params": {
        "action": "list",
        "knowledge_base_id": "kb_1709318400"
    }
}
```

**Response:**
```json
[
    {
        "id": "doc_1709318400000",
        "title": "API Reference Guide",
        "content_type": "markdown",
        "token_count": 1500,
        "chunk_count": 4,
        "created_at": 1709318400
    },
    {
        "id": "doc_1709318500000",
        "title": "Installation Guide",
        "content_type": "markdown",
        "token_count": 800,
        "chunk_count": 2,
        "created_at": 1709318500
    }
]
```

### Get Document

```json
{
    "tool": "document_manage",
    "params": {
        "action": "get",
        "knowledge_base_id": "kb_1709318400",
        "document_id": "doc_1709318400000"
    }
}
```

**Response:**
```json
{
    "document": {
        "id": "doc_1709318400000",
        "title": "API Reference Guide",
        "content": "# API Reference\n\n...",
        "token_count": 1500
    },
    "chunks": [
        {
            "id": "doc_1709318400000_chunk_0",
            "content": "# API Reference\n\nThis document provides...",
            "token_count": 450
        },
        {
            "id": "doc_1709318400000_chunk_1",
            "content": "...authentication using API keys...",
            "token_count": 420
        }
    ]
}
```

### Search Documents

Keyword-based search:

```json
{
    "tool": "document_manage",
    "params": {
        "action": "search",
        "knowledge_base_id": "kb_1709318400",
        "query": "authentication API",
        "limit": 10
    }
}
```

**Response:**
```json
[
    {
        "document": {
            "id": "doc_1709318400000",
            "title": "API Reference Guide",
            "token_count": 1500
        },
        "score": 1.0,
        "match_type": "title"
    },
    {
        "document": {
            "id": "doc_1709318500000",
            "title": "Security Guide",
            "token_count": 600
        },
        "score": 0.5,
        "match_type": "content"
    }
]
```

### Update Document

```json
{
    "tool": "document_manage",
    "params": {
        "action": "update",
        "knowledge_base_id": "kb_1709318400",
        "document_id": "doc_1709318400000",
        "title": "API Reference Guide v2",
        "content": "Updated content..."
    }
}
```

### Delete Document

```json
{
    "tool": "document_manage",
    "params": {
        "action": "delete",
        "knowledge_base_id": "kb_1709318400",
        "document_id": "doc_1709318400000"
    }
}
```

---

## Embeddings & Semantic Search

Embeddings enable semantic search - finding content by meaning, not just keywords.

### Generate Embeddings

Generate embeddings for text:

```json
{
    "tool": "embedding_manage",
    "params": {
        "action": "generate",
        "knowledge_base_id": "kb_1709318400",
        "text": "How do I authenticate API requests?"
    }
}
```

**Response:**
```json
[
    {
        "text_hash": "a1b2c3d4",
        "embedding": [0.1, -0.2, 0.3, ...],
        "dimensions": 1536,
        "model": "text-embedding-ada-002"
    }
]
```

### Store Embeddings

Store embeddings for later search:

```json
{
    "tool": "embedding_manage",
    "params": {
        "action": "store",
        "knowledge_base_id": "kb_1709318400",
        "text": "API authentication requires Bearer tokens in the Authorization header"
    }
}
```

**Response:**
```json
{
    "id": "emb_1709318400000",
    "text_hash": "e5f6g7h8",
    "dimensions": 1536,
    "model": "text-embedding-ada-002"
}
```

### Semantic Search

Search by meaning:

```json
{
    "tool": "embedding_manage",
    "params": {
        "action": "search",
        "knowledge_base_id": "kb_1709318400",
        "text": "How do I secure my API calls?",
        "top_k": 5,
        "threshold": 0.7
    }
}
```

**Response:**
```json
[
    {
        "id": "emb_001",
        "score": 0.95,
        "metadata": {
            "document_id": "doc_001",
            "chunk_id": "chunk_0"
        }
    },
    {
        "id": "emb_002",
        "score": 0.88,
        "metadata": {
            "document_id": "doc_002",
            "chunk_id": "chunk_1"
        }
    }
]
```

### Get Embedding Statistics

```json
{
    "tool": "embedding_manage",
    "params": {
        "action": "stats",
        "knowledge_base_id": "kb_1709318400"
    }
}
```

**Response:**
```json
{
    "total_embeddings": 150,
    "dimensions": 1536,
    "models": ["text-embedding-ada-002"],
    "config": {
        "default_model": "text-embedding-ada-002",
        "default_dimensions": 1536,
        "batch_size": 100,
        "max_tokens": 8191
    }
}
```

### Delete Embedding

```json
{
    "tool": "embedding_manage",
    "params": {
        "action": "delete",
        "knowledge_base_id": "kb_1709318400",
        "embedding_id": "emb_1709318400000"
    }
}
```

---

## Knowledge Graph

The knowledge graph stores entities and their relationships.

### Add Node

Create an entity node:

```json
{
    "tool": "knowledge_graph",
    "params": {
        "action": "add_node",
        "knowledge_base_id": "kb_1709318400",
        "label": "Person",
        "properties": {
            "name": "John Doe",
            "role": "Developer",
            "email": "john@example.com"
        }
    }
}
```

**Response:**
```json
{
    "id": "node_1709318400000",
    "label": "Person",
    "properties": {
        "name": "John Doe",
        "role": "Developer"
    },
    "created_at": 1709318400
}
```

### Node Labels

Common node labels:
- `Person` - People
- `Organization` - Companies, teams
- `Project` - Projects, products
- `Document` - Documents, articles
- `Concept` - Abstract concepts
- `Technology` - Technologies, tools
- `Location` - Physical locations

### Add Edge

Create a relationship:

```json
{
    "tool": "knowledge_graph",
    "params": {
        "action": "add_edge",
        "knowledge_base_id": "kb_1709318400",
        "source_id": "node_001",
        "target_id": "node_002",
        "relation": "works_on",
        "weight": 1.0,
        "properties": {
            "since": "2024-01-01",
            "role": "lead"
        }
    }
}
```

**Response:**
```json
{
    "id": "edge_1709318400000",
    "source_id": "node_001",
    "target_id": "node_002",
    "relation": "works_on",
    "weight": 1.0,
    "created_at": 1709318400
}
```

### Common Relations

| Relation | Description |
|----------|-------------|
| `works_for` | Person → Organization |
| `works_on` | Person → Project |
| `manages` | Person → Person/Project |
| `uses` | Person/Project → Technology |
| `depends_on` | Project → Project |
| `related_to` | Generic relationship |
| `part_of` | Component → Whole |
| `located_in` | Entity → Location |

### Get Node

```json
{
    "tool": "knowledge_graph",
    "params": {
        "action": "get_node",
        "knowledge_base_id": "kb_1709318400",
        "node_id": "node_001"
    }
}
```

**Response:**
```json
{
    "node": {
        "id": "node_001",
        "label": "Person",
        "properties": {"name": "John Doe"}
    },
    "outgoing_edges": 3,
    "incoming_edges": 2
}
```

### Find Path

Find relationship path between nodes:

```json
{
    "tool": "knowledge_graph",
    "params": {
        "action": "find_path",
        "knowledge_base_id": "kb_1709318400",
        "source_id": "node_001",
        "target_id": "node_010",
        "max_depth": 5
    }
}
```

**Response:**
```json
{
    "nodes": [
        {"id": "node_001", "label": "Person", "properties": {"name": "Alice"}},
        {"id": "node_005", "label": "Project", "properties": {"name": "ZeroClaw"}},
        {"id": "node_010", "label": "Person", "properties": {"name": "Bob"}}
    ],
    "edges": [
        {"id": "edge_001", "source_id": "node_001", "target_id": "node_005", "relation": "works_on"},
        {"id": "edge_002", "source_id": "node_005", "target_id": "node_010", "relation": "has_contributor"}
    ],
    "total_weight": 2.0
}
```

### Query Graph

Filter by label or relation:

```json
{
    "tool": "knowledge_graph",
    "params": {
        "action": "query",
        "knowledge_base_id": "kb_1709318400",
        "query_label": "Person",
        "query_relation": "works_on"
    }
}
```

### Get Statistics

```json
{
    "tool": "knowledge_graph",
    "params": {
        "action": "stats",
        "knowledge_base_id": "kb_1709318400"
    }
}
```

**Response:**
```json
{
    "node_count": 50,
    "edge_count": 120,
    "node_labels": {
        "Person": 20,
        "Project": 10,
        "Technology": 15,
        "Document": 5
    },
    "edge_relations": {
        "works_on": 30,
        "uses": 40,
        "depends_on": 25,
        "related_to": 25
    }
}
```

### Delete Operations

Delete node (and connected edges):

```json
{
    "tool": "knowledge_graph",
    "params": {
        "action": "delete_node",
        "knowledge_base_id": "kb_1709318400",
        "node_id": "node_001"
    }
}
```

Delete edge:

```json
{
    "tool": "knowledge_graph",
    "params": {
        "action": "delete_edge",
        "knowledge_base_id": "kb_1709318400",
        "edge_id": "edge_001"
    }
}
```

---

## Use Cases

### Use Case 1: Product Documentation

Create a knowledge base for product docs:

```json
// 1. Create knowledge base
{
    "action": "create",
    "name": "Product Documentation",
    "embedding_model": "text-embedding-ada-002"
}

// 2. Add documents
{
    "action": "add",
    "knowledge_base_id": "kb_xxx",
    "title": "Getting Started",
    "content": "...",
    "content_type": "markdown"
}

// 3. Search
{
    "action": "search",
    "knowledge_base_id": "kb_xxx",
    "text": "How do I install the agent?"
}
```

### Use Case 2: Team Knowledge Graph

Build a graph of team relationships:

```json
// 1. Create nodes
{"action": "add_node", "label": "Person", "properties": {"name": "Alice", "role": "Lead"}}
{"action": "add_node", "label": "Person", "properties": {"name": "Bob", "role": "Developer"}}
{"action": "add_node", "label": "Project", "properties": {"name": "ZeroClaw"}}
{"action": "add_node", "label": "Technology", "properties": {"name": "Rust"}}

// 2. Create relationships
{"action": "add_edge", "source_id": "alice", "target_id": "zeroclaw", "relation": "leads"}
{"action": "add_edge", "source_id": "bob", "target_id": "zeroclaw", "relation": "works_on"}
{"action": "add_edge", "source_id": "zeroclaw", "target_id": "rust", "relation": "uses"}

// 3. Find connections
{"action": "find_path", "source_id": "alice", "target_id": "rust"}
```

### Use Case 3: Support Knowledge Base

Build a support knowledge base with semantic search:

```json
// 1. Create knowledge base
{"action": "create", "name": "Support KB"}

// 2. Add FAQ documents
{"action": "add", "title": "How to reset password", "content": "..."}
{"action": "add", "title": "API rate limits", "content": "..."}

// 3. Store embeddings for common queries
{"action": "store", "text": "I forgot my password and need to reset it"}

// 4. Semantic search for user query
{
    "action": "search",
    "text": "I can't log into my account",
    "top_k": 3,
    "threshold": 0.7
}
```

---

## See Also

- [OpenWrt API Reference](openwrt-api-reference.md)
- [Smart Home Integration](openwrt-smarthome-guide.md)

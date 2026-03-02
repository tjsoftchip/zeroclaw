# OpenWrt Smart Gateway API Reference

This document provides comprehensive API reference for all OpenWrt smart gateway tools.

**Last Updated**: March 2026  
**Module Version**: 1.0.0  
**Branch**: `feature/openwrt-smart-gateway`

## Table of Contents

- [Overview](#overview)
- [Authentication](#authentication)
- [Common Parameters](#common-parameters)
- [Configuration Rollback](#configuration-rollback)
- [UCI Configuration](#uci-configuration)
- [Network Management](#network-management)
- [Firewall Management](#firewall-management)
- [Package Management](#package-management)
- [Docker Management](#docker-management)
- [Parental Control](#parental-control)
- [Smart Home Integration](#smart-home-integration)
- [Proxy & Ad Filtering](#proxy--ad-filtering)
- [Knowledge Management](#knowledge-management)

## Overview

ZeroClaw provides 70+ tools for OpenWrt smart gateway management. All tools follow the same interface pattern:

```rust
trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult>;
}
```

### Tool Result Format

```json
{
    "success": true,
    "output": "{...}",
    "error": null
}
```

## Authentication

All tools integrate with ZeroClaw's security policy:

- Rate limiting checks
- Permission validation
- Audit logging

## Common Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `action` | string | Action to perform (create, list, get, update, delete) |
| `id` | string | Resource identifier |
| `enabled` | boolean | Enable/disable flag |

---

## Configuration Rollback

### config_snapshot_create

Create a configuration snapshot before making changes.

**Parameters:**
```json
{
    "name": "string (optional)",
    "description": "string (optional)",
    "tags": ["string"] // optional
}
```

**Example:**
```json
{
    "name": "pre-update-snapshot",
    "description": "Before firmware update",
    "tags": ["important", "pre-update"]
}
```

**Response:**
```json
{
    "id": "snap_1709318400",
    "name": "pre-update-snapshot",
    "created_at": 1709318400,
    "file_count": 42,
    "total_size": 102400
}
```

### config_snapshot_list

List all configuration snapshots.

**Parameters:**
```json
{
    "limit": 50,
    "offset": 0,
    "tag": "string (optional)"
}
```

### config_rollback

Rollback to a specific snapshot.

**Parameters:**
```json
{
    "snapshot_id": "string (required)",
    "dry_run": false,
    "force": false
}
```

### change_record

Record a configuration change.

**Parameters:**
```json
{
    "action": "create",
    "target": "string (required)",
    "operation": "string (required)",
    "before": {},
    "after": {},
    "snapshot_id": "string (optional)"
}
```

### change_history

Query change history.

**Parameters:**
```json
{
    "target": "string (optional)",
    "start_time": 1709318400,
    "end_time": 1709404800,
    "limit": 100
}
```

### config_diff

Compare configurations between snapshots.

**Parameters:**
```json
{
    "snapshot_id_a": "string (required)",
    "snapshot_id_b": "string (required)"
}
```

---

## UCI Configuration

### uci_get

Read UCI configuration value.

**Parameters:**
```json
{
    "config": "string (required)",
    "section": "string (optional)",
    "option": "string (optional)"
}
```

**Example:**
```json
{
    "config": "network",
    "section": "lan",
    "option": "ipaddr"
}
```

**Response:**
```json
{
    "value": "192.168.1.1",
    "config": "network",
    "section": "lan",
    "option": "ipaddr"
}
```

### uci_get_bulk

Bulk read multiple UCI values.

**Parameters:**
```json
{
    "queries": [
        {"config": "network", "section": "lan"},
        {"config": "wireless", "section": "radio0"}
    ]
}
```

### uci_set

Write UCI configuration value.

**Parameters:**
```json
{
    "config": "string (required)",
    "section": "string (required)",
    "option": "string (optional)",
    "value": "string | array | object",
    "commit": true
}
```

**Example:**
```json
{
    "config": "network",
    "section": "lan",
    "option": "ipaddr",
    "value": "192.168.10.1",
    "commit": true
}
```

### uci_list

List UCI configuration sections.

**Parameters:**
```json
{
    "config": "string (required)",
    "type": "string (optional)"
}
```

### uci_validate

Validate UCI configuration.

**Parameters:**
```json
{
    "config": "string (required)",
    "schema": {}
}
```

### uci_batch

Execute multiple UCI operations atomically.

**Parameters:**
```json
{
    "operations": [
        {"action": "set", "config": "network", "section": "lan", "option": "ipaddr", "value": "192.168.10.1"},
        {"action": "set", "config": "network", "section": "lan", "option": "netmask", "value": "255.255.255.0"}
    ],
    "commit": true,
    "create_snapshot": true
}
```

---

## Network Management

### network_interface_list

List all network interfaces.

**Parameters:**
```json
{
    "include_disabled": false
}
```

**Response:**
```json
[
    {
        "name": "lan",
        "type": "bridge",
        "device": "br-lan",
        "proto": "static",
        "ipaddr": "192.168.1.1",
        "netmask": "255.255.255.0",
        "up": true,
        "mac": "00:11:22:33:44:55"
    }
]
```

### network_device_discover

Discover devices on the network.

**Parameters:**
```json
{
    "scan_type": "arp | mdns | dhcp",
    "timeout_ms": 5000
}
```

**Response:**
```json
[
    {
        "mac": "AA:BB:CC:DD:EE:FF",
        "ip": "192.168.1.100",
        "hostname": "iphone-john",
        "vendor": "Apple",
        "interface": "lan",
        "first_seen": 1709318400,
        "last_seen": 1709404800
    }
]
```

### network_status

Get network connection status.

**Parameters:**
```json
{
    "interface": "string (optional)"
}
```

### dhcp_leases

Query DHCP leases.

**Parameters:**
```json
{
    "active_only": true
}
```

### network_topology

Discover network topology.

**Parameters:**
```json
{
    "action": "discover | get | analyze",
    "include_offline": false
}
```

### topology_visualize

Generate topology visualization.

**Parameters:**
```json
{
    "format": "json | dot | mermaid",
    "include_labels": true
}
```

---

## Firewall Management

### firewall_list

List firewall rules.

**Parameters:**
```json
{
    "zone": "string (optional)",
    "chain": "string (optional)"
}
```

### firewall_add

Add firewall rule.

**Parameters:**
```json
{
    "name": "string (required)",
    "zone": "lan | wan | guest",
    "src": "string",
    "dest": "string",
    "proto": "tcp | udp | icmp | all",
    "src_ip": "string (optional)",
    "dest_ip": "string (optional)",
    "src_port": "string (optional)",
    "dest_port": "string (optional)",
    "target": "ACCEPT | DROP | REJECT",
    "enabled": true
}
```

**Example:**
```json
{
    "name": "allow-http",
    "zone": "wan",
    "src": "wan",
    "dest": "lan",
    "proto": "tcp",
    "dest_ip": "192.168.1.10",
    "dest_port": "80",
    "target": "ACCEPT"
}
```

### firewall_delete

Delete firewall rule.

**Parameters:**
```json
{
    "name": "string (required)"
}
```

### firewall_schedule

Manage internet access schedules.

**Parameters:**
```json
{
    "action": "create | update | delete | list",
    "name": "string",
    "weekdays": ["mon", "tue", "wed", "thu", "fri"],
    "start_time": "22:00",
    "end_time": "07:00",
    "target_devices": ["mac:AA:BB:CC:DD:EE:FF"],
    "action_type": "block | allow"
}
```

### firewall_content_filter

Manage content filtering rules.

**Parameters:**
```json
{
    "action": "create | update | delete | list",
    "name": "string",
    "categories": ["adult", "gambling", "social-media"],
    "custom_domains": ["example.com"],
    "target_devices": [],
    "mode": "block | warn"
}
```

### firewall_port_forward

Manage port forwarding rules.

**Parameters:**
```json
{
    "action": "create | update | delete | list",
    "name": "string",
    "proto": "tcp | udp",
    "src_dport": 8080,
    "dest_ip": "192.168.1.10",
    "dest_port": 80,
    "src_ip": "string (optional)"
}
```

---

## Package Management

### opkg_list

List available/installed packages.

**Parameters:**
```json
{
    "installed_only": false,
    "upgradable_only": false,
    "name": "string (optional)"
}
```

### opkg_install

Install package.

**Parameters:**
```json
{
    "package": "string (required)",
    "force_overwrite": false
}
```

### opkg_remove

Remove package.

**Parameters:**
```json
{
    "package": "string (required)",
    "force_depends": false,
    "autoremove": true
}
```

### opkg_update

Update package lists.

**Parameters:** None

### service_manage

Manage system services.

**Parameters:**
```json
{
    "action": "start | stop | restart | enable | disable | status",
    "service": "string (required)"
}
```

---

## Docker Management

### docker_list

List Docker containers.

**Parameters:**
```json
{
    "all": false,
    "filters": {}
}
```

### docker_manage

Manage Docker containers.

**Parameters:**
```json
{
    "action": "start | stop | restart | pause | unpause | remove",
    "container": "string (required)",
    "timeout": 10
}
```

### docker_image

Manage Docker images.

**Parameters:**
```json
{
    "action": "list | pull | remove | prune",
    "image": "string (optional)",
    "tag": "latest"
}
```

### docker_network

Manage Docker networks.

**Parameters:**
```json
{
    "action": "list | create | remove | inspect",
    "name": "string (optional)",
    "driver": "bridge"
}
```

---

## Parental Control

### Device Management

#### device_identify

Identify device type and capabilities.

**Parameters:**
```json
{
    "mac": "string (required)",
    "ip": "string (optional)"
}
```

**Response:**
```json
{
    "mac": "AA:BB:CC:DD:EE:FF",
    "device_type": "smartphone",
    "os": "iOS 17.4",
    "vendor": "Apple",
    "hostname": "iPhone-John",
    "capabilities": ["wifi", "bluetooth"]
}
```

#### device_group_manage

Manage device groups.

**Parameters:**
```json
{
    "action": "create | update | delete | list | add_device | remove_device",
    "name": "string",
    "description": "string",
    "devices": ["mac:AA:BB:CC:DD:EE:FF"],
    "schedule": {}
}
```

#### device_alias_manage

Manage device aliases.

**Parameters:**
```json
{
    "action": "create | update | delete | list",
    "mac": "string (required)",
    "alias": "string",
    "icon": "phone | laptop | tablet | desktop | tv | gaming"
}
```

### Rules Engine

#### time_rule_manage

Manage time-based access rules.

**Parameters:**
```json
{
    "action": "create | update | delete | list | get",
    "name": "string",
    "target_devices": ["mac:AA:BB:CC:DD:EE:FF"],
    "weekdays": ["mon", "tue", "wed", "thu", "fri"],
    "start_time": "22:00",
    "end_time": "07:00",
    "rule_action": "allow | block | warn | throttle",
    "enabled": true,
    "priority": 100
}
```

#### time_quota_manage

Manage daily/weekly time quotas.

**Parameters:**
```json
{
    "action": "create | update | delete | list | reset",
    "name": "string",
    "target_devices": [],
    "daily_limit_minutes": 120,
    "weekly_limit_minutes": 840,
    "reset_day": "sunday"
}
```

#### content_filter_rule

Manage content filtering rules.

**Parameters:**
```json
{
    "action": "create | update | delete | list",
    "name": "string",
    "target_devices": [],
    "categories": ["adult", "violence", "gambling"],
    "custom_domains": [],
    "safe_search": true
}
```

#### app_control_rule

Manage application control rules.

**Parameters:**
```json
{
    "action": "create | update | delete | list",
    "name": "string",
    "target_devices": [],
    "apps": ["tiktok", "instagram", "youtube"],
    "action": "block | warn | limit",
    "time_limit_minutes": 60
}
```

#### traffic_limit_rule

Manage traffic limit rules.

**Parameters:**
```json
{
    "action": "create | update | delete | list",
    "name": "string",
    "target_devices": [],
    "daily_limit_mb": 500,
    "monthly_limit_mb": 10000,
    "throttle_after_limit": true,
    "throttle_speed_kbps": 256
}
```

### DPI Engine

#### protocol_identify

Identify network protocols.

**Parameters:**
```json
{
    "packet_data": "base64 encoded",
    "src_ip": "string",
    "src_port": 12345,
    "dst_ip": "string",
    "dst_port": 443
}
```

**Response:**
```json
{
    "protocol": "HTTPS",
    "confidence": 0.98,
    "details": {
        "tls_version": "1.3",
        "sni": "example.com"
    }
}
```

#### app_detect

Detect applications in traffic.

**Parameters:**
```json
{
    "flow_data": "base64 encoded",
    "protocol": "tcp"
}
```

**Response:**
```json
{
    "app_name": "YouTube",
    "category": "streaming",
    "confidence": 0.95,
    "risk_level": "low"
}
```

#### content_category

Categorize content.

**Parameters:**
```json
{
    "url": "string (optional)",
    "content": "string (optional)",
    "keywords": []
}
```

**Response:**
```json
{
    "categories": ["entertainment", "streaming"],
    "confidence": 0.92,
    "safe_for_children": true
}
```

### Audit & Notifications

#### behavior_log

Log user behavior events.

**Parameters:**
```json
{
    "action": "log | query | export",
    "device_mac": "string",
    "event_type": "access_blocked | time_limit | content_filtered",
    "details": {}
}
```

#### usage_stats

Get usage statistics.

**Parameters:**
```json
{
    "device_mac": "string (optional)",
    "period": "daily | weekly | monthly",
    "start_date": "2024-03-01",
    "end_date": "2024-03-31"
}
```

**Response:**
```json
{
    "total_time_minutes": 3600,
    "total_traffic_mb": 5120,
    "top_apps": [
        {"app": "YouTube", "time_minutes": 1200, "traffic_mb": 2048},
        {"app": "Instagram", "time_minutes": 800, "traffic_mb": 512}
    ],
    "top_categories": [
        {"category": "streaming", "time_minutes": 1500},
        {"category": "social", "time_minutes": 900}
    ]
}
```

#### parent_notify

Send notification to parents.

**Parameters:**
```json
{
    "action": "send | list | acknowledge",
    "type": "alert | summary | warning",
    "device_mac": "string",
    "message": "string",
    "channels": ["telegram", "email"]
}
```

#### behavior_report

Generate behavior report.

**Parameters:**
```json
{
    "action": "generate | list | get",
    "device_mac": "string (optional)",
    "report_type": "daily | weekly | monthly",
    "format": "json | pdf | html"
}
```

---

## Smart Home Integration

### smarthome_device

Manage smart home devices.

**Parameters:**
```json
{
    "action": "discover | list | get | add | remove | update",
    "device_id": "string (optional)",
    "protocol": "zigbee | z-wave | wifi | mqtt | matter",
    "name": "string",
    "type": "light | switch | sensor | thermostat | camera | lock",
    "room": "string",
    "config": {}
}
```

**Example - Add Zigbee Device:**
```json
{
    "action": "add",
    "protocol": "zigbee",
    "name": "Living Room Light",
    "type": "light",
    "room": "living_room",
    "config": {
        "ieee_address": "0x00158D0001234567",
        "endpoint": 1
    }
}
```

### smarthome_control

Control smart home devices.

**Parameters:**
```json
{
    "action": "set | get | toggle | dim | color",
    "device_id": "string (required)",
    "state": "on | off",
    "brightness": 80,
    "color": {"h": 180, "s": 100, "v": 80},
    "temperature": 22.5
}
```

### smarthome_scene

Manage smart home scenes.

**Parameters:**
```json
{
    "action": "create | update | delete | list | activate",
    "name": "string",
    "icon": "string",
    "devices": [
        {
            "device_id": "light_001",
            "state": "on",
            "brightness": 50
        }
    ],
    "triggers": [
        {"type": "time", "value": "22:00"},
        {"type": "device", "device_id": "sensor_001", "condition": "motion_detected"}
    ]
}
```

### smarthome_automation

Manage automation rules.

**Parameters:**
```json
{
    "action": "create | update | delete | list | enable | disable",
    "name": "string",
    "enabled": true,
    "triggers": [
        {
            "type": "time",
            "schedule": {"weekdays": ["mon", "tue", "wed", "thu", "fri"], "time": "07:00"}
        },
        {
            "type": "device",
            "device_id": "sensor_motion",
            "condition": "state == 'motion'"
        }
    ],
    "conditions": [
        {"type": "time_range", "start": "07:00", "end": "23:00"}
    ],
    "actions": [
        {"type": "device", "device_id": "light_001", "action": "turn_on"},
        {"type": "notification", "message": "Motion detected"}
    ]
}
```

---

## Proxy & Ad Filtering

### proxy_config

Configure proxy settings.

**Parameters:**
```json
{
    "action": "set | get | test | status",
    "enabled": true,
    "type": "shadowsocks | vmess | trojan | wireguard | socks5 | http",
    "server": {
        "address": "proxy.example.com",
        "port": 443,
        "password": "secret",
        "method": "aes-256-gcm"
    },
    "mode": "global | rule | direct",
    "auto_switch": true
}
```

### adblock_manage

Manage ad blocking.

**Parameters:**
```json
{
    "action": "enable | disable | status | update | list",
    "enabled": true,
    "lists": [
        {
            "name": "EasyList",
            "url": "https://easylist-downloads.adblockplus.org/easylist.txt",
            "enabled": true
        }
    ],
    "whitelist": ["example.com"],
    "blacklist": ["ads.example.com"]
}
```

### dns_filter

Configure DNS filtering.

**Parameters:**
```json
{
    "action": "set | get | flush",
    "enabled": true,
    "upstream_dns": ["8.8.8.8", "1.1.1.1"],
    "block_lists": ["adguard-dns"],
    "custom_rules": [
        {"domain": "ads.example.com", "action": "block"},
        {"domain": "safe.example.com", "action": "allow"}
    ]
}
```

### routing_rules

Manage routing rules.

**Parameters:**
```json
{
    "action": "set | get | list | add | remove",
    "rules": [
        {
            "name": "streaming-direct",
            "match": {"domain_suffix": [".netflix.com", ".youtube.com"]},
            "action": "direct"
        },
        {
            "name": "cn-direct",
            "match": {"geoip": "CN"},
            "action": "direct"
        },
        {
            "name": "default-proxy",
            "match": {"all": true},
            "action": "proxy",
            "proxy_group": "default"
        }
    ]
}
```

---

## Knowledge Management

### knowledge_store

Manage knowledge bases.

**Parameters:**
```json
{
    "action": "create | list | get | delete | update",
    "id": "string (for get/delete/update)",
    "name": "string (for create)",
    "description": "string",
    "embedding_model": "text-embedding-ada-002"
}
```

**Example - Create Knowledge Base:**
```json
{
    "action": "create",
    "name": "Technical Documentation",
    "description": "Product technical docs and manuals",
    "embedding_model": "text-embedding-ada-002"
}
```

**Response:**
```json
{
    "id": "kb_1709318400",
    "name": "Technical Documentation",
    "description": "Product technical docs and manuals",
    "created_at": 1709318400,
    "document_count": 0,
    "total_tokens": 0,
    "embedding_model": "text-embedding-ada-002"
}
```

### document_manage

Manage documents in knowledge bases.

**Parameters:**
```json
{
    "action": "add | list | get | delete | search | update",
    "knowledge_base_id": "string (required)",
    "document_id": "string (for get/delete/update)",
    "title": "string",
    "content": "string",
    "content_type": "text | markdown | html | pdf",
    "source": "string",
    "query": "string (for search)",
    "limit": 10
}
```

**Example - Add Document:**
```json
{
    "action": "add",
    "knowledge_base_id": "kb_1709318400",
    "title": "API Reference Guide",
    "content": "# API Reference\n\nThis document describes...",
    "content_type": "markdown",
    "source": "https://docs.example.com/api"
}
```

**Response:**
```json
{
    "id": "doc_1709318400000",
    "title": "API Reference Guide",
    "token_count": 1500,
    "chunk_count": 4
}
```

### embedding_manage

Manage embeddings for semantic search.

**Parameters:**
```json
{
    "action": "generate | store | search | delete | stats",
    "knowledge_base_id": "string (required)",
    "text": "string",
    "texts": ["string"],
    "embedding_id": "string (for delete)",
    "query_embedding": [0.1, 0.2, ...],
    "top_k": 10,
    "threshold": 0.0
}
```

**Example - Semantic Search:**
```json
{
    "action": "search",
    "knowledge_base_id": "kb_1709318400",
    "text": "How to configure firewall rules?",
    "top_k": 5,
    "threshold": 0.7
}
```

**Response:**
```json
[
    {
        "id": "emb_001",
        "score": 0.95,
        "metadata": {"document_id": "doc_001"}
    },
    {
        "id": "emb_002",
        "score": 0.88,
        "metadata": {"document_id": "doc_002"}
    }
]
```

### knowledge_graph

Manage knowledge graph for entity relationships.

**Parameters:**
```json
{
    "action": "add_node | get_node | delete_node | add_edge | get_edge | delete_edge | find_path | query | stats",
    "knowledge_base_id": "string (required)",
    "node_id": "string",
    "edge_id": "string",
    "label": "string",
    "source_id": "string",
    "target_id": "string",
    "relation": "string",
    "properties": {},
    "weight": 1.0,
    "max_depth": 5,
    "query_label": "string",
    "query_relation": "string"
}
```

**Example - Add Node:**
```json
{
    "action": "add_node",
    "knowledge_base_id": "kb_1709318400",
    "label": "Person",
    "properties": {
        "name": "John Doe",
        "role": "Developer"
    }
}
```

**Example - Find Path:**
```json
{
    "action": "find_path",
    "knowledge_base_id": "kb_1709318400",
    "source_id": "node_001",
    "target_id": "node_010",
    "max_depth": 5
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

---

## Error Handling

All tools return errors in a consistent format:

```json
{
    "success": false,
    "output": "",
    "error": "Error message describing what went wrong"
}
```

Common error codes:
- `Rate limit exceeded` - Too many requests
- `Permission denied` - Insufficient permissions
- `Resource not found` - Requested resource doesn't exist
- `Invalid parameters` - Missing or invalid parameters
- `Operation failed` - Operation could not be completed

---

## Rate Limits

| Tool Category | Rate Limit |
|--------------|------------|
| Read operations | 100/minute |
| Write operations | 30/minute |
| Search operations | 50/minute |
| Bulk operations | 10/minute |

---

## See Also

- [OpenWrt Development Summary](openwrt-development-summary.md)
- [Commands Reference](commands-reference.md)
- [Config Reference](config-reference.md)
- [Operations Runbook](operations-runbook.md)

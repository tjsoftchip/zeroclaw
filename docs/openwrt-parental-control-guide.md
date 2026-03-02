# Parental Control Module Guide

This guide provides detailed instructions for using the parental control features in ZeroClaw's OpenWrt smart gateway.

**Last Updated**: March 2026  
**Module**: `src/parental/`

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Device Management](#device-management)
- [Time-Based Rules](#time-based-rules)
- [Time Quotas](#time-quotas)
- [Content Filtering](#content-filtering)
- [Application Control](#application-control)
- [Traffic Limits](#traffic-limits)
- [DPI Engine](#dpi-engine)
- [Audit & Notifications](#audit--notifications)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Overview

The parental control module provides comprehensive tools for managing children's internet access:

```
┌─────────────────────────────────────────────────────────────┐
│                    Parental Control System                   │
├─────────────────────────────────────────────────────────────┤
│  Device Management                                          │
│  ├── Device Identification (OS, type, vendor)               │
│  ├── Device Groups (children, adults, guests)               │
│  └── Device Aliases (friendly names)                        │
├─────────────────────────────────────────────────────────────┤
│  Access Control                                             │
│  ├── Time Rules (bedtime, study time)                       │
│  ├── Time Quotas (daily/weekly limits)                      │
│  ├── Content Filtering (adult, violence, gambling)          │
│  ├── App Control (TikTok, YouTube, games)                   │
│  └── Traffic Limits (data caps)                             │
├─────────────────────────────────────────────────────────────┤
│  Deep Packet Inspection                                     │
│  ├── Protocol Identification                                │
│  ├── Application Detection                                  │
│  └── Content Categorization                                 │
├─────────────────────────────────────────────────────────────┤
│  Monitoring & Alerts                                        │
│  ├── Behavior Logging                                       │
│  ├── Usage Statistics                                       │
│  ├── Parent Notifications                                   │
│  └── Behavior Reports                                       │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### 1. Identify Devices

First, identify all devices on your network:

```json
{
    "tool": "network_device_discover",
    "params": {
        "scan_type": "arp"
    }
}
```

### 2. Create Device Groups

Create groups for different family members:

```json
{
    "tool": "device_group_manage",
    "params": {
        "action": "create",
        "name": "Children",
        "description": "Devices belonging to children",
        "devices": []
    }
}
```

### 3. Add Devices to Groups

```json
{
    "tool": "device_group_manage",
    "params": {
        "action": "add_device",
        "name": "Children",
        "devices": ["mac:AA:BB:CC:DD:EE:FF", "mac:11:22:33:44:55:66"]
    }
}
```

### 4. Set Up Bedtime Rule

```json
{
    "tool": "time_rule_manage",
    "params": {
        "action": "create",
        "name": "Bedtime",
        "target_devices": ["group:Children"],
        "weekdays": ["sun", "mon", "tue", "wed", "thu"],
        "start_time": "21:00",
        "end_time": "07:00",
        "rule_action": "block"
    }
}
```

### 5. Enable Content Filtering

```json
{
    "tool": "content_filter_rule",
    "params": {
        "action": "create",
        "name": "Adult Content Block",
        "target_devices": ["group:Children"],
        "categories": ["adult", "violence", "gambling"],
        "safe_search": true
    }
}
```

---

## Device Management

### Device Identification

Automatically identify device type, OS, and capabilities:

```json
{
    "tool": "device_identify",
    "params": {
        "mac": "AA:BB:CC:DD:EE:FF",
        "ip": "192.168.1.100"
    }
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
    "model": "iPhone 15 Pro",
    "capabilities": ["wifi", "bluetooth", "nfc"]
}
```

### Device Groups

Organize devices into logical groups:

**Create a Group:**
```json
{
    "tool": "device_group_manage",
    "params": {
        "action": "create",
        "name": "Teenagers",
        "description": "Teenagers' devices",
        "devices": [],
        "schedule": {
            "default_action": "allow",
            "rules": []
        }
    }
}
```

**Add Devices:**
```json
{
    "tool": "device_group_manage",
    "params": {
        "action": "add_device",
        "name": "Teenagers",
        "devices": ["mac:AA:BB:CC:DD:EE:FF"]
    }
}
```

**List Groups:**
```json
{
    "tool": "device_group_manage",
    "params": {
        "action": "list"
    }
}
```

### Device Aliases

Assign friendly names and icons:

```json
{
    "tool": "device_alias_manage",
    "params": {
        "action": "create",
        "mac": "AA:BB:CC:DD:EE:FF",
        "alias": "John's iPhone",
        "icon": "phone"
    }
}
```

**Available Icons:**
- `phone` - Smartphone
- `tablet` - Tablet
- `laptop` - Laptop
- `desktop` - Desktop computer
- `tv` - Smart TV
- `gaming` - Gaming console
- `speaker` - Smart speaker
- `watch` - Smart watch
- `other` - Other devices

---

## Time-Based Rules

### Basic Time Rule

Block internet during specific hours:

```json
{
    "tool": "time_rule_manage",
    "params": {
        "action": "create",
        "name": "Study Time",
        "target_devices": ["group:Children"],
        "weekdays": ["mon", "tue", "wed", "thu", "fri"],
        "start_time": "09:00",
        "end_time": "15:00",
        "rule_action": "block",
        "priority": 100
    }
}
```

### Multiple Time Rules

Create different rules for different times:

**Weeknight Bedtime:**
```json
{
    "name": "Weeknight Bedtime",
    "weekdays": ["sun", "mon", "tue", "wed", "thu"],
    "start_time": "21:00",
    "end_time": "07:00",
    "rule_action": "block"
}
```

**Weekend Extended Hours:**
```json
{
    "name": "Weekend Bedtime",
    "weekdays": ["fri", "sat"],
    "start_time": "23:00",
    "end_time": "09:00",
    "rule_action": "block"
}
```

### Rule Actions

| Action | Description |
|--------|-------------|
| `allow` | Explicitly allow access |
| `block` | Block all internet access |
| `warn` | Allow but log and notify parents |
| `throttle` | Reduce bandwidth speed |

### Rule Priority

Rules are evaluated by priority (higher first). Use priority to handle overlapping rules:

```json
{
    "name": "Allow Educational Sites",
    "target_devices": ["group:Children"],
    "weekdays": ["mon", "tue", "wed", "thu", "fri"],
    "start_time": "09:00",
    "end_time": "15:00",
    "rule_action": "allow",
    "priority": 200
}
```

---

## Time Quotas

### Daily Time Limit

Limit total daily internet time:

```json
{
    "tool": "time_quota_manage",
    "params": {
        "action": "create",
        "name": "Daily Screen Time",
        "target_devices": ["group:Children"],
        "daily_limit_minutes": 120,
        "reset_at": "00:00"
    }
}
```

### Weekly Time Limit

Set weekly limits with daily caps:

```json
{
    "tool": "time_quota_manage",
    "params": {
        "action": "create",
        "name": "Weekly Screen Time",
        "target_devices": ["group:Children"],
        "daily_limit_minutes": 120,
        "weekly_limit_minutes": 840,
        "reset_day": "sunday"
    }
}
```

### Check Quota Status

```json
{
    "tool": "time_quota_manage",
    "params": {
        "action": "list"
    }
}
```

**Response:**
```json
[
    {
        "id": "quota_001",
        "name": "Daily Screen Time",
        "daily_limit_minutes": 120,
        "used_today_minutes": 45,
        "remaining_today_minutes": 75,
        "weekly_limit_minutes": 840,
        "used_this_week_minutes": 320
    }
]
```

### Reset Quota

Manually reset quota (e.g., as reward):

```json
{
    "tool": "time_quota_manage",
    "params": {
        "action": "reset",
        "name": "Daily Screen Time",
        "device_mac": "AA:BB:CC:DD:EE:FF"
    }
}
```

---

## Content Filtering

### Category-Based Filtering

Block content by category:

```json
{
    "tool": "content_filter_rule",
    "params": {
        "action": "create",
        "name": "Adult Content Filter",
        "target_devices": ["group:Children"],
        "categories": [
            "adult",
            "violence",
            "gambling",
            "drugs",
            "weapons"
        ],
        "mode": "block"
    }
}
```

**Available Categories:**
| Category | Description |
|----------|-------------|
| `adult` | Adult content |
| `violence` | Violent content |
| `gambling` | Gambling sites |
| `drugs` | Drug-related content |
| `weapons` | Weapons content |
| `hate-speech` | Hate speech |
| `social-media` | Social networks |
| `streaming` | Video streaming |
| `gaming` | Online games |
| `file-sharing` | P2P/torrents |

### Custom Domain Blocking

Block specific domains:

```json
{
    "tool": "content_filter_rule",
    "params": {
        "action": "create",
        "name": "Block Specific Sites",
        "target_devices": ["group:Children"],
        "custom_domains": [
            "tiktok.com",
            "snapchat.com",
            "discord.com"
        ],
        "mode": "block"
    }
}
```

### Safe Search

Force safe search on search engines:

```json
{
    "tool": "content_filter_rule",
    "params": {
        "action": "create",
        "name": "Safe Search",
        "target_devices": ["group:Children"],
        "safe_search": true
    }
}
```

This enforces safe search on:
- Google
- Bing
- YouTube
- DuckDuckGo

### Warning Mode

Warn instead of block:

```json
{
    "tool": "content_filter_rule",
    "params": {
        "action": "create",
        "name": "Teen Content Warning",
        "target_devices": ["group:Teenagers"],
        "categories": ["adult", "violence"],
        "mode": "warn"
    }
}
```

---

## Application Control

### Block Specific Apps

```json
{
    "tool": "app_control_rule",
    "params": {
        "action": "create",
        "name": "Block Social Media",
        "target_devices": ["group:Children"],
        "apps": [
            "tiktok",
            "instagram",
            "snapchat",
            "facebook"
        ],
        "action": "block"
    }
}
```

**Supported Apps:**
| App | Identifier |
|-----|------------|
| TikTok | `tiktok` |
| Instagram | `instagram` |
| YouTube | `youtube` |
| Facebook | `facebook` |
| Snapchat | `snapchat` |
| Twitter/X | `twitter` |
| Discord | `discord` |
| WhatsApp | `whatsapp` |
| Telegram | `telegram` |
| Netflix | `netflix` |
| Spotify | `spotify` |
| Roblox | `roblox` |
| Minecraft | `minecraft` |
| Fortnite | `fortnite` |

### Time-Limited Apps

Allow apps for limited time:

```json
{
    "tool": "app_control_rule",
    "params": {
        "action": "create",
        "name": "Limited Gaming",
        "target_devices": ["group:Children"],
        "apps": ["roblox", "minecraft"],
        "action": "limit",
        "time_limit_minutes": 60
    }
}
```

### App Categories

Block by category:

```json
{
    "tool": "app_control_rule",
    "params": {
        "action": "create",
        "name": "Block Games",
        "target_devices": ["group:Children"],
        "categories": ["gaming", "social"],
        "action": "block"
    }
}
```

---

## Traffic Limits

### Daily Data Cap

```json
{
    "tool": "traffic_limit_rule",
    "params": {
        "action": "create",
        "name": "Daily Data Limit",
        "target_devices": ["group:Children"],
        "daily_limit_mb": 500,
        "throttle_after_limit": true,
        "throttle_speed_kbps": 256
    }
}
```

### Monthly Data Cap

```json
{
    "tool": "traffic_limit_rule",
    "params": {
        "action": "create",
        "name": "Monthly Data Limit",
        "target_devices": ["group:Teenagers"],
        "monthly_limit_mb": 10000,
        "reset_day": 1
    }
}
```

### Check Usage

```json
{
    "tool": "traffic_limit_rule",
    "params": {
        "action": "list"
    }
}
```

---

## DPI Engine

### Protocol Identification

Identify network protocols:

```json
{
    "tool": "protocol_identify",
    "params": {
        "src_ip": "192.168.1.100",
        "src_port": 54321,
        "dst_ip": "142.250.185.78",
        "dst_port": 443,
        "protocol": "tcp"
    }
}
```

**Response:**
```json
{
    "protocol": "HTTPS",
    "confidence": 0.98,
    "details": {
        "tls_version": "1.3",
        "sni": "www.google.com",
        "cipher_suite": "TLS_AES_256_GCM_SHA384"
    }
}
```

### Application Detection

Detect applications in traffic:

```json
{
    "tool": "app_detect",
    "params": {
        "flow_data": "...",
        "protocol": "tcp"
    }
}
```

**Response:**
```json
{
    "app_name": "YouTube",
    "category": "streaming",
    "confidence": 0.95,
    "risk_level": "low",
    "bandwidth_usage": "high"
}
```

### Content Categorization

Categorize web content:

```json
{
    "tool": "content_category",
    "params": {
        "url": "https://example.com/page"
    }
}
```

**Response:**
```json
{
    "categories": ["entertainment", "streaming"],
    "confidence": 0.92,
    "safe_for_children": false,
    "reason": "Contains mature content indicators"
}
```

---

## Audit & Notifications

### Behavior Logging

Query behavior logs:

```json
{
    "tool": "behavior_log",
    "params": {
        "action": "query",
        "device_mac": "AA:BB:CC:DD:EE:FF",
        "start_time": 1709318400,
        "end_time": 1709404800,
        "event_types": ["access_blocked", "time_limit", "content_filtered"]
    }
}
```

**Response:**
```json
[
    {
        "timestamp": 1709352000,
        "device_mac": "AA:BB:CC:DD:EE:FF",
        "event_type": "content_filtered",
        "details": {
            "url": "https://blocked-site.com",
            "category": "adult",
            "action_taken": "blocked"
        }
    }
]
```

### Usage Statistics

Get detailed usage stats:

```json
{
    "tool": "usage_stats",
    "params": {
        "device_mac": "AA:BB:CC:DD:EE:FF",
        "period": "weekly"
    }
}
```

**Response:**
```json
{
    "total_time_minutes": 1680,
    "total_traffic_mb": 4096,
    "top_apps": [
        {"app": "YouTube", "time_minutes": 600, "traffic_mb": 2048},
        {"app": "Roblox", "time_minutes": 400, "traffic_mb": 512},
        {"app": "Instagram", "time_minutes": 300, "traffic_mb": 256}
    ],
    "top_categories": [
        {"category": "streaming", "time_minutes": 700},
        {"category": "gaming", "time_minutes": 500},
        {"category": "social", "time_minutes": 400}
    ],
    "blocked_attempts": 45,
    "daily_breakdown": [
        {"date": "2024-03-01", "time_minutes": 240},
        {"date": "2024-03-02", "time_minutes": 280}
    ]
}
```

### Parent Notifications

Send notifications to parents:

```json
{
    "tool": "parent_notify",
    "params": {
        "action": "send",
        "type": "alert",
        "device_mac": "AA:BB:CC:DD:EE:FF",
        "message": "Blocked site access attempt detected",
        "channels": ["telegram", "email"]
    }
}
```

### Behavior Reports

Generate reports:

```json
{
    "tool": "behavior_report",
    "params": {
        "action": "generate",
        "report_type": "weekly",
        "device_mac": "AA:BB:CC:DD:EE:FF",
        "format": "html"
    }
}
```

---

## Best Practices

### 1. Age-Appropriate Rules

**Young Children (Under 10):**
```json
{
    "daily_limit_minutes": 60,
    "categories": ["adult", "violence", "gambling", "social-media"],
    "apps": ["tiktok", "instagram", "snapchat"],
    "safe_search": true
}
```

**Pre-Teens (10-13):**
```json
{
    "daily_limit_minutes": 120,
    "categories": ["adult", "violence", "gambling"],
    "apps": ["tiktok"],
    "safe_search": true
}
```

**Teenagers (14-17):**
```json
{
    "daily_limit_minutes": 180,
    "categories": ["adult"],
    "mode": "warn",
    "safe_search": true
}
```

### 2. Gradual Relaxation

Create rules that automatically adjust:

```json
{
    "name": "Weekend Bonus",
    "weekdays": ["sat", "sun"],
    "target_devices": ["group:Children"],
    "rule_action": "allow",
    "priority": 50
}
```

### 3. Reward System

Use temporary overrides as rewards:

```json
{
    "tool": "temp_override_manage",
    "params": {
        "action": "create",
        "name": "Homework Complete Bonus",
        "target_devices": ["mac:AA:BB:CC:DD:EE:FF"],
        "extra_minutes": 30,
        "expires_at": 1709404800
    }
}
```

### 4. Communication

Enable warning mode before blocking:

```json
{
    "categories": ["adult"],
    "mode": "warn",
    "notify_parent": true
}
```

---

## Troubleshooting

### Device Not Identified

**Problem:** Device shows as "Unknown"

**Solution:**
1. Enable more scan types:
```json
{
    "scan_type": "arp",
    "deep_scan": true
}
```

2. Check DHCP leases for hostname

3. Manually set alias:
```json
{
    "action": "create",
    "mac": "AA:BB:CC:DD:EE:FF",
    "alias": "Living Room TV",
    "device_type": "tv"
}
```

### Rules Not Applying

**Problem:** Rules don't seem to work

**Solution:**
1. Check rule priority
2. Verify device is in target group
3. Check rule is enabled:
```json
{
    "action": "get",
    "name": "Bedtime Rule"
}
```

### False Positives

**Problem:** Legitimate sites blocked

**Solution:**
1. Add to whitelist:
```json
{
    "action": "update",
    "name": "Content Filter",
    "whitelist": ["educational-site.com"]
}
```

2. Lower confidence threshold

### Quota Not Resetting

**Problem:** Quota shows incorrect values

**Solution:**
1. Manual reset:
```json
{
    "action": "reset",
    "name": "Daily Screen Time"
}
```

2. Check timezone settings

---

## See Also

- [OpenWrt API Reference](openwrt-api-reference.md)
- [Network Management Guide](openwrt-network-guide.md)
- [Smart Home Integration](openwrt-smarthome-guide.md)

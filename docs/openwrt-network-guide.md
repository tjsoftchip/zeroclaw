# Network Management Guide

This guide covers network management features for ZeroClaw's OpenWrt smart gateway.

**Last Updated**: March 2026  
**Module**: `src/network/`

## Table of Contents

- [Overview](#overview)
- [Interface Management](#interface-management)
- [Device Discovery](#device-discovery)
- [Network Topology](#network-topology)
- [Firewall Configuration](#firewall-configuration)
- [DHCP Management](#dhcp-management)
- [Bandwidth Monitoring](#bandwidth-monitoring)
- [Network Diagnostics](#network-diagnostics)

## Overview

The network management module provides comprehensive tools for managing OpenWrt network infrastructure:

```
┌─────────────────────────────────────────────────────────────┐
│                   Network Management                         │
├─────────────────────────────────────────────────────────────┤
│  Interface Layer                                            │
│  ├── Interface List & Status                                │
│  ├── Protocol Configuration                                 │
│  └── Bridge Management                                      │
├─────────────────────────────────────────────────────────────┤
│  Device Layer                                               │
│  ├── Device Discovery (ARP, mDNS, DHCP)                     │
│  ├── Device Tracking                                        │
│  └── Vendor Identification                                  │
├─────────────────────────────────────────────────────────────┤
│  Topology Layer                                             │
│  ├── Network Mapping                                        │
│  ├── Connection Analysis                                    │
│  └── Visualization                                          │
├─────────────────────────────────────────────────────────────┤
│  Security Layer                                             │
│  ├── Firewall Rules                                         │
│  ├── Port Forwarding                                        │
│  ├── Content Filtering                                      │
│  └── Access Schedules                                       │
└─────────────────────────────────────────────────────────────┘
```

---

## Interface Management

### List Network Interfaces

Get all network interfaces with status:

```json
{
    "tool": "network_interface_list",
    "params": {
        "include_disabled": true
    }
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
        "mac": "00:11:22:33:44:55",
        "rx_bytes": 1073741824,
        "tx_bytes": 536870912,
        "rx_packets": 1000000,
        "tx_packets": 500000
    },
    {
        "name": "wan",
        "type": "ethernet",
        "device": "eth0",
        "proto": "dhcp",
        "ipaddr": "203.0.113.100",
        "netmask": "255.255.255.0",
        "gateway": "203.0.113.1",
        "dns": ["8.8.8.8", "8.8.4.4"],
        "up": true
    },
    {
        "name": "guest",
        "type": "bridge",
        "device": "br-guest",
        "proto": "static",
        "ipaddr": "192.168.2.1",
        "netmask": "255.255.255.0",
        "up": true,
        "isolated": true
    }
]
```

### Get Network Status

Check connectivity status:

```json
{
    "tool": "network_status",
    "params": {
        "interface": "wan"
    }
}
```

**Response:**
```json
{
    "interface": "wan",
    "up": true,
    "connected": true,
    "ipaddr": "203.0.113.100",
    "gateway": "203.0.113.1",
    "dns_servers": ["8.8.8.8", "8.8.4.4"],
    "uptime_seconds": 86400,
    "latency_ms": 15,
    "packet_loss": 0.0,
    "wan_ip": "203.0.113.100",
    "wan_gateway": "203.0.113.1"
}
```

### Configure Interface

Set interface parameters:

```json
{
    "tool": "uci_set",
    "params": {
        "config": "network",
        "section": "lan",
        "values": {
            "ipaddr": "192.168.10.1",
            "netmask": "255.255.255.0"
        },
        "commit": true
    }
}
```

---

## Device Discovery

### Scan Network Devices

Discover devices using multiple methods:

```json
{
    "tool": "network_device_discover",
    "params": {
        "scan_type": "arp",
        "timeout_ms": 5000
    }
}
```

**Scan Types:**
| Type | Description | Speed | Accuracy |
|------|-------------|-------|----------|
| `arp` | ARP table scan | Fast | High |
| `mdns` | mDNS/Bonjour | Medium | Medium |
| `dhcp` | DHCP leases | Fast | High |
| `ping` | Ping sweep | Slow | Medium |

**Response:**
```json
[
    {
        "mac": "AA:BB:CC:DD:EE:FF",
        "ip": "192.168.1.100",
        "hostname": "iPhone-John",
        "vendor": "Apple, Inc.",
        "interface": "lan",
        "first_seen": 1709318400,
        "last_seen": 1709404800,
        "online": true,
        "device_type": "smartphone"
    },
    {
        "mac": "11:22:33:44:55:66",
        "ip": "192.168.1.101",
        "hostname": "SmartTV-LivingRoom",
        "vendor": "Samsung Electronics",
        "interface": "lan",
        "online": true,
        "device_type": "tv"
    }
]
```

### Track Device History

Get device connection history:

```json
{
    "tool": "device_connections",
    "params": {
        "mac": "AA:BB:CC:DD:EE:FF",
        "period": "weekly"
    }
}
```

**Response:**
```json
{
    "mac": "AA:BB:CC:DD:EE:FF",
    "total_sessions": 45,
    "total_online_time_minutes": 3600,
    "avg_session_minutes": 80,
    "sessions": [
        {
            "start_time": 1709318400,
            "end_time": 1709325600,
            "ip": "192.168.1.100",
            "traffic_rx_mb": 512,
            "traffic_tx_mb": 128
        }
    ]
}
```

---

## Network Topology

### Discover Topology

Map network topology:

```json
{
    "tool": "network_topology",
    "params": {
        "action": "discover",
        "include_offline": false
    }
}
```

**Response:**
```json
{
    "nodes": [
        {
            "id": "router",
            "type": "router",
            "name": "OpenWrt Gateway",
            "ip": "192.168.1.1",
            "mac": "00:11:22:33:44:55"
        },
        {
            "id": "switch_1",
            "type": "switch",
            "name": "Living Room Switch",
            "ip": "192.168.1.2",
            "mac": "AA:BB:CC:11:22:33"
        },
        {
            "id": "ap_1",
            "type": "access_point",
            "name": "Bedroom AP",
            "ip": "192.168.1.3",
            "mac": "AA:BB:CC:44:55:66"
        },
        {
            "id": "device_1",
            "type": "device",
            "name": "iPhone-John",
            "ip": "192.168.1.100",
            "mac": "AA:BB:CC:DD:EE:FF"
        }
    ],
    "links": [
        {
            "source": "router",
            "target": "switch_1",
            "type": "ethernet",
            "speed_mbps": 1000
        },
        {
            "source": "switch_1",
            "target": "ap_1",
            "type": "ethernet",
            "speed_mbps": 1000
        },
        {
            "source": "ap_1",
            "target": "device_1",
            "type": "wifi",
            "speed_mbps": 433,
            "signal_dbm": -45
        }
    ]
}
```

### Analyze Connections

Analyze device connections:

```json
{
    "tool": "device_connections",
    "params": {
        "action": "analyze",
        "device_mac": "AA:BB:CC:DD:EE:FF"
    }
}
```

**Response:**
```json
{
    "device": {
        "mac": "AA:BB:CC:DD:EE:FF",
        "name": "iPhone-John"
    },
    "connection_path": [
        {"node": "router", "interface": "br-lan"},
        {"node": "switch_1", "interface": "eth1"},
        {"node": "ap_1", "interface": "wlan0"}
    ],
    "connection_quality": {
        "latency_ms": 2,
        "packet_loss": 0,
        "wifi_signal_dbm": -45,
        "wifi_snr": 45
    }
}
```

### Visualize Topology

Generate visualization:

```json
{
    "tool": "topology_visualize",
    "params": {
        "format": "mermaid",
        "include_labels": true
    }
}
```

**Response (Mermaid format):**
```
graph TD
    R[OpenWrt Gateway<br/>192.168.1.1]
    S[Living Room Switch<br/>192.168.1.2]
    A[Bedroom AP<br/>192.168.1.3]
    D[iPhone-John<br/>192.168.1.100]
    
    R -->|1Gbps| S
    S -->|1Gbps| A
    A -->|WiFi 433Mbps| D
```

---

## Firewall Configuration

### List Firewall Rules

```json
{
    "tool": "firewall_list",
    "params": {
        "zone": "lan",
        "chain": "forward"
    }
}
```

**Response:**
```json
[
    {
        "name": "Allow-DHCP-Renew",
        "src": "wan",
        "dest": "lan",
        "proto": "udp",
        "dest_port": 68,
        "target": "ACCEPT",
        "enabled": true
    },
    {
        "name": "Allow-Ping",
        "src": "wan",
        "dest": "lan",
        "proto": "icmp",
        "icmp_type": "echo-request",
        "target": "ACCEPT",
        "enabled": true
    }
]
```

### Add Firewall Rule

```json
{
    "tool": "firewall_add",
    "params": {
        "name": "Allow-Web-Server",
        "src": "wan",
        "dest": "lan",
        "proto": "tcp",
        "dest_ip": "192.168.1.10",
        "dest_port": 80,
        "target": "ACCEPT",
        "enabled": true
    }
}
```

### Port Forwarding

Set up port forwarding:

```json
{
    "tool": "firewall_port_forward",
    "params": {
        "action": "create",
        "name": "Web-Server-Forward",
        "proto": "tcp",
        "src_dport": 8080,
        "dest_ip": "192.168.1.10",
        "dest_port": 80,
        "src_ip": "",
        "enabled": true
    }
}
```

### Access Schedule

Time-based access control:

```json
{
    "tool": "firewall_schedule",
    "params": {
        "action": "create",
        "name": "Kids-Bedtime",
        "target_devices": ["mac:AA:BB:CC:DD:EE:FF"],
        "weekdays": ["sun", "mon", "tue", "wed", "thu"],
        "start_time": "21:00",
        "end_time": "07:00",
        "action_type": "block"
    }
}
```

### Content Filter

Configure content filtering:

```json
{
    "tool": "firewall_content_filter",
    "params": {
        "action": "create",
        "name": "Adult-Block",
        "categories": ["adult", "violence", "gambling"],
        "target_devices": [],
        "mode": "block",
        "safe_search": true
    }
}
```

---

## DHCP Management

### List DHCP Leases

```json
{
    "tool": "dhcp_leases",
    "params": {
        "active_only": true
    }
}
```

**Response:**
```json
[
    {
        "hostname": "iPhone-John",
        "ip": "192.168.1.100",
        "mac": "AA:BB:CC:DD:EE:FF",
        "expires": 1709404800,
        "remaining_seconds": 86400
    },
    {
        "hostname": "SmartTV-LivingRoom",
        "ip": "192.168.1.101",
        "mac": "11:22:33:44:55:66",
        "expires": 1709404800,
        "remaining_seconds": 86400
    }
]
```

### Static Lease

Add static DHCP lease:

```json
{
    "tool": "uci_set",
    "params": {
        "config": "dhcp",
        "section": "@host[-1]",
        "values": {
            "name": "server",
            "mac": "AA:BB:CC:DD:EE:FF",
            "ip": "192.168.1.10"
        },
        "commit": true
    }
}
```

---

## Bandwidth Monitoring

### Real-time Bandwidth

```json
{
    "tool": "bandwidth_monitor",
    "params": {
        "action": "realtime",
        "interface": "wan"
    }
}
```

**Response:**
```json
{
    "interface": "wan",
    "rx_bytes_per_sec": 1048576,
    "tx_bytes_per_sec": 524288,
    "rx_kbps": 8192,
    "tx_kbps": 4096,
    "connections": 45,
    "timestamp": 1709404800
}
```

### Historical Statistics

```json
{
    "tool": "bandwidth_monitor",
    "params": {
        "action": "history",
        "period": "daily",
        "interface": "all"
    }
}
```

**Response:**
```json
{
    "period": "daily",
    "data": [
        {
            "timestamp": 1709318400,
            "wan_rx_mb": 5120,
            "wan_tx_mb": 2048,
            "lan_rx_mb": 2048,
            "lan_tx_mb": 5120
        }
    ]
}
```

### Per-Device Bandwidth

```json
{
    "tool": "bandwidth_monitor",
    "params": {
        "action": "per_device",
        "top_n": 10
    }
}
```

**Response:**
```json
[
    {
        "mac": "AA:BB:CC:DD:EE:FF",
        "name": "iPhone-John",
        "rx_mb": 2048,
        "tx_mb": 512,
        "percentage": 35
    },
    {
        "mac": "11:22:33:44:55:66",
        "name": "SmartTV-LivingRoom",
        "rx_mb": 3072,
        "tx_mb": 128,
        "percentage": 45
    }
]
```

---

## Network Diagnostics

### Ping Test

```json
{
    "tool": "network_diagnose",
    "params": {
        "action": "ping",
        "target": "8.8.8.8",
        "count": 5
    }
}
```

**Response:**
```json
{
    "target": "8.8.8.8",
    "packets_sent": 5,
    "packets_received": 5,
    "packet_loss": 0,
    "min_ms": 10.5,
    "avg_ms": 12.3,
    "max_ms": 15.1,
    "jitter_ms": 2.1
}
```

### DNS Lookup

```json
{
    "tool": "network_diagnose",
    "params": {
        "action": "dns_lookup",
        "hostname": "google.com",
        "dns_server": "8.8.8.8"
    }
}
```

**Response:**
```json
{
    "hostname": "google.com",
    "addresses": ["142.250.185.78", "2607:f8b0:4004:800::200e"],
    "query_time_ms": 15,
    "dns_server": "8.8.8.8"
}
```

### Traceroute

```json
{
    "tool": "network_diagnose",
    "params": {
        "action": "traceroute",
        "target": "google.com",
        "max_hops": 30
    }
}
```

**Response:**
```json
{
    "target": "google.com",
    "hops": [
        {"hop": 1, "ip": "192.168.1.1", "rtt_ms": 1.2, "hostname": "gateway"},
        {"hop": 2, "ip": "10.0.0.1", "rtt_ms": 5.3, "hostname": "isp-gateway"},
        {"hop": 3, "ip": "203.0.113.1", "rtt_ms": 10.1, "hostname": ""},
        {"hop": 4, "ip": "142.250.185.78", "rtt_ms": 15.2, "hostname": "google.com"}
    ]
}
```

### Port Scan

```json
{
    "tool": "network_diagnose",
    "params": {
        "action": "port_scan",
        "target": "192.168.1.10",
        "ports": [22, 80, 443, 8080]
    }
}
```

**Response:**
```json
{
    "target": "192.168.1.10",
    "ports": [
        {"port": 22, "state": "open", "service": "ssh"},
        {"port": 80, "state": "open", "service": "http"},
        {"port": 443, "state": "closed", "service": "https"},
        {"port": 8080, "state": "filtered", "service": "http-proxy"}
    ]
}
```

---

## See Also

- [OpenWrt API Reference](openwrt-api-reference.md)
- [Parental Control Guide](openwrt-parental-control-guide.md)
- [Smart Home Integration](openwrt-smarthome-guide.md)
- [Proxy & Ad Filtering](openwrt-proxy-guide.md)

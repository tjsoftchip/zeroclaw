# Smart Home Integration Guide

This guide covers smart home integration features for ZeroClaw's OpenWrt smart gateway.

**Last Updated**: March 2026  
**Module**: `src/smarthome/`

## Table of Contents

- [Overview](#overview)
- [Supported Protocols](#supported-protocols)
- [Device Management](#device-management)
- [Device Control](#device-control)
- [Scenes](#scenes)
- [Automation](#automation)
- [Examples](#examples)

## Overview

The smart home module provides unified control over various smart home protocols:

```
┌─────────────────────────────────────────────────────────────┐
│                   Smart Home Integration                     │
├─────────────────────────────────────────────────────────────┤
│  Protocol Layer                                             │
│  ├── Zigbee (via Zigbee2MQTT)                               │
│  ├── Z-Wave (via OpenZWave)                                 │
│  ├── WiFi (native HTTP/MQTT)                                │
│  ├── MQTT (generic broker)                                  │
│  └── Matter (future support)                                │
├─────────────────────────────────────────────────────────────┤
│  Device Layer                                               │
│  ├── Lights (on/off, dimming, color)                        │
│  ├── Switches & Plugs                                       │
│  ├── Sensors (motion, temperature, humidity)                │
│  ├── Thermostats                                            │
│  ├── Cameras                                                │
│  └── Locks                                                  │
├─────────────────────────────────────────────────────────────┤
│  Automation Layer                                           │
│  ├── Scenes (preset configurations)                         │
│  ├── Triggers (time, device, event)                         │
│  ├── Conditions (time range, device state)                  │
│  └── Actions (device control, notifications)                │
└─────────────────────────────────────────────────────────────┘
```

---

## Supported Protocols

### Zigbee

Most popular IoT protocol, low power, mesh networking.

**Supported Devices:**
- Philips Hue
- IKEA Tradfri
- Xiaomi/Aqara
- Tuya Zigbee
- Sonoff Zigbee

**Configuration:**
```json
{
    "protocol": "zigbee",
    "config": {
        "coordinator": "/dev/ttyUSB0",
        "channel": 11,
        "pan_id": 6754
    }
}
```

### Z-Wave

Reliable sub-GHz protocol, strong security.

**Supported Devices:**
- Fibaro
- Aeotec
- SmartThings
- Zooz

**Configuration:**
```json
{
    "protocol": "z-wave",
    "config": {
        "controller": "/dev/ttyACM0",
        "network_key": "auto"
    }
}
```

### WiFi

Direct WiFi devices using HTTP/HTTPS or MQTT.

**Supported Devices:**
- Sonoff WiFi
- Shelly
- Tuya WiFi
- TP-Link Kasa
- Custom ESPHome/Tasmota

### MQTT

Generic MQTT broker integration.

**Configuration:**
```json
{
    "protocol": "mqtt",
    "config": {
        "broker": "mqtt://localhost:1883",
        "username": "user",
        "password": "pass",
        "client_id": "zeroclaw"
    }
}
```

---

## Device Management

### Discover Devices

Scan for new devices:

```json
{
    "tool": "smarthome_device",
    "params": {
        "action": "discover",
        "protocol": "zigbee",
        "timeout_seconds": 60
    }
}
```

**Response:**
```json
[
    {
        "device_id": "zigbee_0x00158D0001234567",
        "protocol": "zigbee",
        "type": "light",
        "name": "Living Room Light",
        "model": "Philips LCT001",
        "manufacturer": "Philips",
        "capabilities": ["onoff", "brightness", "color_xy"],
        "available": true
    },
    {
        "device_id": "zigbee_0x00158D0007654321",
        "protocol": "zigbee",
        "type": "sensor",
        "name": "Motion Sensor",
        "model": "Aqara RTCGQ11LM",
        "manufacturer": "Xiaomi",
        "capabilities": ["occupancy", "illuminance"],
        "available": true
    }
]
```

### Add Device Manually

Add a device with specific configuration:

```json
{
    "tool": "smarthome_device",
    "params": {
        "action": "add",
        "protocol": "zigbee",
        "name": "Bedroom Light",
        "type": "light",
        "room": "bedroom",
        "config": {
            "ieee_address": "0x00158D0001234567",
            "endpoint": 1
        }
    }
}
```

### List Devices

Get all registered devices:

```json
{
    "tool": "smarthome_device",
    "params": {
        "action": "list",
        "protocol": "all",
        "type": "all",
        "room": "living_room"
    }
}
```

**Response:**
```json
[
    {
        "device_id": "light_001",
        "name": "Living Room Light",
        "type": "light",
        "protocol": "zigbee",
        "room": "living_room",
        "state": {
            "on": true,
            "brightness": 80,
            "color": {"h": 180, "s": 100, "v": 80}
        },
        "available": true,
        "last_seen": 1709404800
    }
]
```

### Get Device Details

```json
{
    "tool": "smarthome_device",
    "params": {
        "action": "get",
        "device_id": "light_001"
    }
}
```

**Response:**
```json
{
    "device_id": "light_001",
    "name": "Living Room Light",
    "type": "light",
    "protocol": "zigbee",
    "model": "Philips LCT001",
    "manufacturer": "Philips",
    "room": "living_room",
    "capabilities": ["onoff", "brightness", "color_xy", "color_temp"],
    "state": {
        "on": true,
        "brightness": 80,
        "color_temp": 4000,
        "color": {"h": 180, "s": 100, "v": 80}
    },
    "config": {
        "ieee_address": "0x00158D0001234567",
        "endpoint": 1
    },
    "available": true,
    "last_seen": 1709404800,
    "battery": null,
    "linkquality": 120
}
```

### Update Device

```json
{
    "tool": "smarthome_device",
    "params": {
        "action": "update",
        "device_id": "light_001",
        "name": "Main Living Room Light",
        "room": "living_room",
        "icon": "ceiling-light"
    }
}
```

### Remove Device

```json
{
    "tool": "smarthome_device",
    "params": {
        "action": "remove",
        "device_id": "light_001",
        "force": false
    }
}
```

---

## Device Control

### Basic Control

Turn device on/off:

```json
{
    "tool": "smarthome_control",
    "params": {
        "action": "set",
        "device_id": "light_001",
        "state": "on"
    }
}
```

### Brightness Control

Set brightness level (0-100):

```json
{
    "tool": "smarthome_control",
    "params": {
        "action": "dim",
        "device_id": "light_001",
        "brightness": 50
    }
}
```

### Color Control

Set color using HSV:

```json
{
    "tool": "smarthome_control",
    "params": {
        "action": "color",
        "device_id": "light_001",
        "color": {
            "h": 180,
            "s": 100,
            "v": 80
        }
    }
}
```

Or using color temperature (Kelvin):

```json
{
    "tool": "smarthome_control",
    "params": {
        "action": "color",
        "device_id": "light_001",
        "color_temp": 4000
    }
}
```

### Get State

```json
{
    "tool": "smarthome_control",
    "params": {
        "action": "get",
        "device_id": "light_001"
    }
}
```

**Response:**
```json
{
    "device_id": "light_001",
    "state": {
        "on": true,
        "brightness": 50,
        "color_temp": 4000
    }
}
```

### Toggle

```json
{
    "tool": "smarthome_control",
    "params": {
        "action": "toggle",
        "device_id": "light_001"
    }
}
```

### Thermostat Control

```json
{
    "tool": "smarthome_control",
    "params": {
        "action": "set",
        "device_id": "thermostat_001",
        "temperature": 22.5,
        "mode": "heat"
    }
}
```

---

## Scenes

Scenes are preset configurations for multiple devices.

### Create Scene

```json
{
    "tool": "smarthome_scene",
    "params": {
        "action": "create",
        "name": "Movie Night",
        "icon": "film",
        "room": "living_room",
        "devices": [
            {
                "device_id": "light_001",
                "state": "on",
                "brightness": 20,
                "color_temp": 2700
            },
            {
                "device_id": "light_002",
                "state": "off"
            },
            {
                "device_id": "switch_001",
                "state": "on"
            }
        ]
    }
}
```

### List Scenes

```json
{
    "tool": "smarthome_scene",
    "params": {
        "action": "list"
    }
}
```

**Response:**
```json
[
    {
        "id": "scene_001",
        "name": "Movie Night",
        "icon": "film",
        "room": "living_room",
        "device_count": 3,
        "created_at": 1709318400
    },
    {
        "id": "scene_002",
        "name": "Good Morning",
        "icon": "sun",
        "room": "all",
        "device_count": 5,
        "created_at": 1709318500
    }
]
```

### Activate Scene

```json
{
    "tool": "smarthome_scene",
    "params": {
        "action": "activate",
        "name": "Movie Night"
    }
}
```

### Scene with Triggers

Create a scene with automatic triggers:

```json
{
    "tool": "smarthome_scene",
    "params": {
        "action": "create",
        "name": "Bedtime",
        "icon": "moon",
        "devices": [
            {"device_id": "light_001", "state": "off"},
            {"device_id": "light_002", "state": "off"},
            {"device_id": "thermostat_001", "temperature": 19}
        ],
        "triggers": [
            {"type": "time", "value": "22:00"}
        ]
    }
}
```

### Update Scene

```json
{
    "tool": "smarthome_scene",
    "params": {
        "action": "update",
        "name": "Movie Night",
        "devices": [
            {"device_id": "light_001", "state": "on", "brightness": 15}
        ]
    }
}
```

### Delete Scene

```json
{
    "tool": "smarthome_scene",
    "params": {
        "action": "delete",
        "name": "Movie Night"
    }
}
```

---

## Automation

Automations allow complex rule-based control.

### Create Automation

```json
{
    "tool": "smarthome_automation",
    "params": {
        "action": "create",
        "name": "Morning Routine",
        "enabled": true,
        "triggers": [
            {
                "type": "time",
                "schedule": {
                    "weekdays": ["mon", "tue", "wed", "thu", "fri"],
                    "time": "07:00"
                }
            },
            {
                "type": "device",
                "device_id": "sensor_motion_001",
                "condition": "occupancy == true"
            }
        ],
        "conditions": [
            {
                "type": "time_range",
                "start": "06:00",
                "end": "09:00"
            }
        ],
        "actions": [
            {
                "type": "device",
                "device_id": "light_001",
                "action": "turn_on",
                "brightness": 80
            },
            {
                "type": "scene",
                "name": "Good Morning"
            },
            {
                "type": "notification",
                "message": "Good morning! Starting your day."
            }
        ]
    }
}
```

### Trigger Types

| Type | Description | Example |
|------|-------------|---------|
| `time` | Scheduled time | `{"type": "time", "schedule": {"time": "07:00"}}` |
| `sunrise` | Sunrise event | `{"type": "sunrise", "offset_minutes": -30}` |
| `sunset` | Sunset event | `{"type": "sunset", "offset_minutes": 30}` |
| `device` | Device state change | `{"type": "device", "device_id": "sensor_001", "condition": "occupancy == true"}` |
| `webhook` | HTTP webhook | `{"type": "webhook", "path": "/api/trigger/morning"}` |

### Condition Types

| Type | Description | Example |
|------|-------------|---------|
| `time_range` | Time window | `{"type": "time_range", "start": "18:00", "end": "23:00"}` |
| `device_state` | Device condition | `{"type": "device_state", "device_id": "light_001", "state": "on"}` |
| `presence` | Home/away status | `{"type": "presence", "status": "home"}` |
| `day_type` | Weekday/weekend | `{"type": "day_type", "days": ["weekday"]}` |

### Action Types

| Type | Description | Example |
|------|-------------|---------|
| `device` | Control device | `{"type": "device", "device_id": "light_001", "action": "turn_on"}` |
| `scene` | Activate scene | `{"type": "scene", "name": "Movie Night"}` |
| `notification` | Send notification | `{"type": "notification", "message": "Motion detected"}` |
| `delay` | Wait before next | `{"type": "delay", "seconds": 30}` |
| `webhook` | HTTP request | `{"type": "webhook", "url": "https://example.com/hook"}` |

### List Automations

```json
{
    "tool": "smarthome_automation",
    "params": {
        "action": "list"
    }
}
```

### Enable/Disable Automation

```json
{
    "tool": "smarthome_automation",
    "params": {
        "action": "disable",
        "name": "Morning Routine"
    }
}
```

### Delete Automation

```json
{
    "tool": "smarthome_automation",
    "params": {
        "action": "delete",
        "name": "Morning Routine"
    }
}
```

---

## Examples

### Example 1: Motion-Activated Lighting

Automatically turn on lights when motion is detected:

```json
{
    "tool": "smarthome_automation",
    "params": {
        "action": "create",
        "name": "Hallway Motion Light",
        "triggers": [
            {
                "type": "device",
                "device_id": "sensor_motion_hallway",
                "condition": "occupancy == true"
            }
        ],
        "conditions": [
            {
                "type": "time_range",
                "start": "18:00",
                "end": "07:00"
            }
        ],
        "actions": [
            {
                "type": "device",
                "device_id": "light_hallway",
                "action": "turn_on",
                "brightness": 50
            },
            {
                "type": "delay",
                "seconds": 120
            },
            {
                "type": "device",
                "device_id": "light_hallway",
                "action": "turn_off"
            }
        ]
    }
}
```

### Example 2: Away Mode

Turn off all lights when everyone leaves:

```json
{
    "tool": "smarthome_automation",
    "params": {
        "action": "create",
        "name": "Away Mode",
        "triggers": [
            {
                "type": "presence",
                "status": "away"
            }
        ],
        "actions": [
            {
                "type": "device",
                "device_id": "all_lights",
                "action": "turn_off"
            },
            {
                "type": "device",
                "device_id": "thermostat_main",
                "action": "set_mode",
                "mode": "eco"
            }
        ]
    }
}
```

### Example 3: Sunrise Wake-up

Gradually brighten lights at sunrise:

```json
{
    "tool": "smarthome_automation",
    "params": {
        "action": "create",
        "name": "Sunrise Wake-up",
        "triggers": [
            {
                "type": "sunrise",
                "offset_minutes": -30
            }
        ],
        "conditions": [
            {
                "type": "presence",
                "status": "home"
            }
        ],
        "actions": [
            {
                "type": "device",
                "device_id": "light_bedroom",
                "action": "turn_on",
                "brightness": 1,
                "transition_seconds": 1800
            }
        ]
    }
}
```

### Example 4: Temperature-Based Fan Control

Turn on fan when temperature exceeds threshold:

```json
{
    "tool": "smarthome_automation",
    "params": {
        "action": "create",
        "name": "Auto Fan Control",
        "triggers": [
            {
                "type": "device",
                "device_id": "sensor_temp_living",
                "condition": "temperature > 25"
            }
        ],
        "actions": [
            {
                "type": "device",
                "device_id": "fan_living",
                "action": "turn_on"
            }
        ]
    }
}
```

### Example 5: Door Lock Notification

Notify when door is unlocked:

```json
{
    "tool": "smarthome_automation",
    "params": {
        "action": "create",
        "name": "Door Unlock Alert",
        "triggers": [
            {
                "type": "device",
                "device_id": "lock_front",
                "condition": "state == 'unlocked'"
            }
        ],
        "conditions": [
            {
                "type": "presence",
                "status": "away"
            }
        ],
        "actions": [
            {
                "type": "notification",
                "message": "Front door has been unlocked!",
                "channels": ["telegram", "push"]
            }
        ]
    }
}
```

---

## See Also

- [OpenWrt API Reference](openwrt-api-reference.md)
- [Parental Control Guide](openwrt-parental-control-guide.md)
- [Network Management Guide](openwrt-network-guide.md)

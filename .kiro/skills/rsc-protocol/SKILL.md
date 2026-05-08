---
name: rsc-protocol
description: "Bluetooth Running Speed and Cadence (RSC) Service protocol implementation guide. Covers GATT service structure, RSC Measurement characteristic parsing/building, RSC Feature bits, SC Control Point procedures, and sensor location handling. Use when implementing RSC peripheral or central, parsing RSC measurement data, building notification payloads, working with 0x1814 service or 0x2A53/0x2A54/0x2A5D characteristics, or converting treadmill data to RSC format."
globs: ["**/peripheral.rs", "**/protocol.rs", "**/converter.rs"]
user-invocable: true
---

# RSC Protocol

> Bluetooth Running Speed and Cadence Service V1.0 (RSCS_SPEC)

## UUIDs

| Item | UUID |
|------|------|
| RSC Service | `0x1814` |
| RSC Measurement | `0x2A53` |
| RSC Feature | `0x2A54` |
| Sensor Location | `0x2A5D` |
| SC Control Point | `0x2A55` |

## RSC Measurement (0x2A53) — Notify

Variable-length, little-endian. Mandatory fields always present; optional fields controlled by flags.

```
Byte 0:       Flags (UINT8)
Bytes 1–2:    Instantaneous Speed (UINT16) — units: 1/256 m/s
Byte 3:       Instantaneous Cadence (UINT8) — steps/min
[Bytes 4–5]:  Instantaneous Stride Length (UINT16) — units: 1/100 m (centimeters)
[Bytes 6–9]:  Total Distance (UINT32) — units: 1/10 m (decimeters)
```

### Flags byte

| Bit | Name | Effect |
|-----|------|--------|
| 0 | Stride Length Present | If 1, stride length field (2 bytes) follows cadence |
| 1 | Total Distance Present | If 1, total distance field (4 bytes) follows |
| 2 | Walking or Running | 0 = walking, 1 = running |
| 3–7 | RFU | Set to 0 |

### Payload sizes

| Flags | Size (bytes) |
|-------|-------------|
| 0b000 | 4 (flags + speed + cadence) |
| 0b001 | 6 (+stride length) |
| 0b010 | 8 (+total distance) |
| 0b011 | 10 (+both) |

### Speed conversion

```
speed_m_per_s = raw_u16 / 256.0
speed_km_per_h = speed_m_per_s * 3.6
speed_mph = speed_m_per_s * 2.23694
```

## RSC Feature (0x2A54) — Read

UINT16, little-endian. Static for device lifetime.

| Bit | Feature |
|-----|---------|
| 0 | Instantaneous Stride Length Measurement Supported |
| 1 | Total Distance Measurement Supported |
| 2 | Walking or Running Status Supported |
| 3 | Sensor Calibration Procedure Supported |
| 4 | Multiple Sensor Locations Supported |
| 5–15 | RFU (set to 0) |

## SC Control Point (0x2A55) — Write + Indicate

Op Codes: 0x01=Set Cumulative Value (UINT32), 0x02=Start Calibration, 0x03=Update Sensor Location (UINT8), 0x04=Request Supported Locations, 0x10=Response Code.

Response Values: 0x01=Success, 0x02=Op Code Not Supported, 0x03=Invalid Parameter, 0x04=Operation Failed.

## Sensor Location (0x2A5D) — Read

UINT8 enum: 0=Other, 1=Top of shoe, 2=In shoe, 3=Hip, 4–16=cycling positions (see REFERENCE.md)

## Implementation checklist (peripheral)

- [ ] Register service 0x1814 as Primary Service
- [ ] Add RSC Measurement char (Notify) + CCCD
- [ ] Add RSC Feature char (Read) with supported bits
- [ ] Add Sensor Location char (Read) if applicable
- [ ] Add SC Control Point char (Write+Indicate) + CCCD if Total Distance or Calibration supported
- [ ] Notify RSC Measurement ~1 Hz when CCCD enabled
- [ ] All multi-byte fields little-endian
- [ ] Discard measurement if notification fails (time-sensitive data)

## Advanced details

See [REFERENCE.md](REFERENCE.md) for SC Control Point procedures, error handling, and timing requirements.

---
name: ftms-protocol
description: "Bluetooth FTMS (Fitness Machine Service) protocol implementation guide. Covers GATT service structure, Treadmill/Indoor Bike/Rower/Cross Trainer data characteristic parsing, Fitness Machine Feature bitfields, Control Point opcodes, and advertising data. Use when implementing FTMS client or server, parsing FTMS notification data, working with 0x1826 service or 0x2ACD/0x2AD2/0x2AD1 characteristics, converting FTMS units, or building fitness machine BLE bridges."
---

# FTMS Protocol

## Quick Reference

| Item | UUID |
|------|------|
| Service | 0x1826 |
| Treadmill Data | 0x2ACD |
| Indoor Bike Data | 0x2AD2 |
| Rower Data | 0x2AD1 |
| Cross Trainer Data | 0x2ACE |
| Fitness Machine Feature | 0x2ACC |
| Control Point | 0x2AD9 |
| Machine Status | 0x2ADA |
| Supported Speed Range | 0x2AD4 |

All multi-byte fields: **little-endian**.

## Treadmill Data (0x2ACD) — Parsing

Flags field is uint16. **Bit 0 uses inverted logic**: 0 = speed present, 1 = speed NOT present.

| Bit | When 1 = field present | Size |
|-----|----------------------|------|
| 0 | (inverted) Speed NOT present | — |
| 1 | Average Speed | uint16 |
| 2 | Total Distance | uint24 (3 bytes) |
| 3 | Inclination + Ramp Angle | sint16 + sint16 |
| 4 | Elevation Gain (pos + neg) | uint16 + uint16 |
| 5 | Instantaneous Pace | uint8 |
| 6 | Average Pace | uint8 |
| 7 | Expended Energy | uint16 + uint16 + uint8 |
| 8 | Heart Rate | uint8 |
| 9 | Metabolic Equivalent | uint8 |
| 10 | Elapsed Time | uint16 |
| 11 | Remaining Time | uint16 |
| 12 | Force on Belt + Power Output | sint16 + sint16 |

Key fields: Speed = uint16 in 0.01 km/h. Total Distance = uint24 in metres.

## Parsing Example (Rust, no_std)

```rust
pub fn parse_treadmill(data: &[u8]) -> Option<(u16, Option<u32>)> {
    if data.len() < 4 { return None; }
    let flags = u16::from_le_bytes([data[0], data[1]]);
    let mut off = 2;
    // Bit 0 = 0 → speed present
    let speed = if flags & 0x01 == 0 {
        let s = u16::from_le_bytes([data[off], data[off+1]]);
        off += 2; s
    } else { return None; };
    if flags & 0x02 != 0 { off += 2; } // avg speed
    let dist = if flags & 0x04 != 0 {
        let d = data[off] as u32 | (data[off+1] as u32) << 8 | (data[off+2] as u32) << 16;
        Some(d)
    } else { None };
    Some((speed, dist))
}
```

## Unit Conversions

| From (FTMS) | To | Formula |
|-------------|-----|---------|
| Speed (0.01 km/h) | m/s | `value * 10 / 3600` |
| Speed (0.01 km/h) | RSC (1/256 m/s) | `value * 10000 / 921600` |
| Distance (metres) | RSC (1/10 m) | `value * 10` |

## Control Point (0x2AD9) — Key Op Codes

| Op | Name | Param |
|----|------|-------|
| 0x00 | Request Control | — |
| 0x01 | Reset | — |
| 0x02 | Set Target Speed | uint16 (0.01 km/h) |
| 0x07 | Start/Resume | — |
| 0x08 | Stop/Pause | uint8 (1=Stop, 2=Pause) |
| 0x80 | Response | opcode + result |

Results: 0x01=Success, 0x02=Unsupported, 0x03=Invalid, 0x04=Failed, 0x05=Not Permitted.

## Advertising — Machine Type Bits

Bit 0=Treadmill, 1=Cross Trainer, 2=Step Climber, 3=Stair Climber, 4=Rower, 5=Indoor Bike.

## Advanced

See [REFERENCE.md](REFERENCE.md) for Indoor Bike, Rower, Cross Trainer field layouts, full Feature bitfields, Status opcodes, Supported Range formats, and multi-notification rules.

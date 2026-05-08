# RSC Protocol Reference

> Detailed protocol behavior from Bluetooth RSCS V1.0 specification.

## Service characteristics table

| Characteristic | Requirement | Properties | Condition |
|---------------|-------------|------------|-----------|
| RSC Measurement | M | Notify | — |
| RSC Measurement CCCD | M | Read, Write | — |
| RSC Feature | M | Read | — |
| Sensor Location | C.1 | Read | Mandatory if Multiple Sensor Locations supported, else optional |
| SC Control Point | C.2 | Write, Indicate | Mandatory if Calibration/Total Distance/Multiple Locations supported |
| SC Control Point CCCD | C.2 | Read, Write | Same as SC Control Point |

## RSC Measurement field details

### Instantaneous Speed (mandatory)

- Type: UINT16
- Unit: meters per second with resolution 1/256
- Example: value 0x0500 = 1280 decimal = 1280/256 = 5.0 m/s = 18 km/h
- Range: 0 to 255.996 m/s (theoretical max)

### Instantaneous Cadence (mandatory)

- Type: UINT8
- Unit: 1/minute (steps per minute where each count is one foot-fall)
- Range: 0–255 steps/min
- Typical running: 150–190 spm

### Instantaneous Stride Length (optional, bit 0)

- Type: UINT16
- Unit: 1/100 meter (centimeters)
- Example: value 250 = 2.50 meters stride
- Only present if RSC Feature bit 0 is set AND Flags bit 0 is 1

### Total Distance (optional, bit 1)

- Type: UINT32
- Unit: 1/10 meter (decimeters)
- Example: value 50000 = 5000.0 meters = 5 km
- Max representable: 429,496.7296 km (will not roll over)
- Only present if RSC Feature bit 1 is set AND Flags bit 1 is 1
- Typically transmitted every 2–3 seconds (not every notification)

### Walking or Running Status (bit 2)

- Not a field — just a flag bit
- 0 = walking, 1 = running
- Only meaningful if RSC Feature bit 2 is set
- If feature not supported, this bit shall be 0

## Transmission interval

- RSC Measurement notified approximately once per second
- Total Distance field included approximately every 2–3 seconds
- Intervals determined by Server, not configurable by Client

## SC Control Point procedures

### Set Cumulative Value (Op Code 0x01)

Resets the Total Distance to a given UINT32 value.

**Request format:**
```
Byte 0: 0x01 (Op Code)
Bytes 1–4: UINT32 new total distance value (little-endian)
```

**Indication response:**
```
Byte 0: 0x10 (Response Code)
Byte 1: 0x01 (Request Op Code)
Byte 2: Response Value (0x01=Success, 0x04=Operation Failed)
```

Mandatory if Total Distance Measurement feature is supported.

### Start Sensor Calibration (Op Code 0x02)

Initiates device-specific calibration.

**Request format:**
```
Byte 0: 0x02 (Op Code)
```

**Indication response:**
```
Byte 0: 0x10 (Response Code)
Byte 1: 0x02 (Request Op Code)
Byte 2: Response Value (0x01=Success, 0x04=Operation Failed)
```

Mandatory if Sensor Calibration Procedure feature is supported.

### Update Sensor Location (Op Code 0x03)

**Request format:**
```
Byte 0: 0x03 (Op Code)
Byte 1: UINT8 sensor location value
```

**Indication response:**
```
Byte 0: 0x10 (Response Code)
Byte 1: 0x03 (Request Op Code)
Byte 2: Response Value (0x01=Success, 0x03=Invalid Parameter, 0x04=Operation Failed)
```

Mandatory if Multiple Sensor Locations feature is supported.

### Request Supported Sensor Locations (Op Code 0x04)

**Request format:**
```
Byte 0: 0x04 (Op Code)
```

**Indication response (success):**
```
Byte 0: 0x10 (Response Code)
Byte 1: 0x04 (Request Op Code)
Byte 2: 0x01 (Success)
Bytes 3–N: Byte array of supported sensor location values
```

Max 17 locations with default ATT MTU.

## Error handling

| Condition | Response |
|-----------|----------|
| Unsupported Op Code | Response Value = 0x02 (Op Code Not Supported) |
| Invalid Parameter | Response Value = 0x03 (Invalid Parameter) |
| Operation in progress | ATT error 0x80 (Procedure Already In Progress) |
| CCCD not configured for indications | ATT error 0x81 (CCC Improperly Configured) |

## Procedure timeout

- ATT transaction timeout: 30 seconds
- If no handle/value confirmation received within 30s, Server stops sending indications/notifications and may disconnect

## Time-sensitive data requirements

RSC Measurement data has no timestamp. If a notification cannot be delivered (link loss, connection not established), the measurement value shall be discarded — do not queue stale data.

## Byte order

All multi-byte fields are little-endian (least significant octet first).

## Rust implementation patterns

### Building an RSC Measurement notification payload

```rust
fn build_rsc_measurement(
    speed_256: u16,       // speed in 1/256 m/s
    cadence: u8,          // steps per minute
    stride_len: Option<u16>,  // centimeters (1/100 m)
    total_distance: Option<u32>, // decimeters (1/10 m)
    is_running: bool,
) -> heapless::Vec<u8, 10> {
    let mut buf = heapless::Vec::new();

    let mut flags: u8 = 0;
    if stride_len.is_some() { flags |= 0x01; }
    if total_distance.is_some() { flags |= 0x02; }
    if is_running { flags |= 0x04; }

    let _ = buf.push(flags);
    let _ = buf.extend_from_slice(&speed_256.to_le_bytes());
    let _ = buf.push(cadence);

    if let Some(sl) = stride_len {
        let _ = buf.extend_from_slice(&sl.to_le_bytes());
    }
    if let Some(td) = total_distance {
        let _ = buf.extend_from_slice(&td.to_le_bytes());
    }

    buf
}
```

### Parsing an RSC Measurement notification

```rust
struct RscMeasurement {
    speed_256: u16,
    cadence: u8,
    stride_length_cm: Option<u16>,
    total_distance_dm: Option<u32>,
    is_running: bool,
}

fn parse_rsc_measurement(data: &[u8]) -> Option<RscMeasurement> {
    if data.len() < 4 { return None; }

    let flags = data[0];
    let speed_256 = u16::from_le_bytes([data[1], data[2]]);
    let cadence = data[3];

    let mut offset = 4;

    let stride_length_cm = if flags & 0x01 != 0 {
        if data.len() < offset + 2 { return None; }
        let val = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        Some(val)
    } else {
        None
    };

    let total_distance_dm = if flags & 0x02 != 0 {
        if data.len() < offset + 4 { return None; }
        let val = u32::from_le_bytes([
            data[offset], data[offset + 1],
            data[offset + 2], data[offset + 3],
        ]);
        Some(val)
    } else {
        None
    };

    let is_running = flags & 0x04 != 0;

    Some(RscMeasurement {
        speed_256,
        cadence,
        stride_length_cm,
        total_distance_dm,
        is_running,
    })
}
```

### Converting km/h to RSC speed field

```rust
/// Convert speed in km/h to RSC UINT16 (1/256 m/s)
fn kmh_to_rsc_speed(kmh: f32) -> u16 {
    let mps = kmh / 3.6;
    (mps * 256.0) as u16
}

/// Convert RSC UINT16 speed to km/h
fn rsc_speed_to_kmh(raw: u16) -> f32 {
    (raw as f32 / 256.0) * 3.6
}
```

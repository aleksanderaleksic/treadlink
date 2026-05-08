# FTMS Protocol Reference

Source: Bluetooth SIG Fitness Machine Service v1.0 (2017-Feb-14)

## Service UUID

- Fitness Machine Service: **0x1826**

## Characteristic UUIDs

| UUID | Name | Properties | Requirement |
|------|------|-----------|-------------|
| 0x2ACC | Fitness Machine Feature | Read | Mandatory |
| 0x2ACD | Treadmill Data | Notify | Optional |
| 0x2ACE | Cross Trainer Data | Notify | Optional |
| 0x2ACF | Step Climber Data | Notify | Optional |
| 0x2AD0 | Stair Climber Data | Notify | Optional |
| 0x2AD1 | Rower Data | Notify | Optional |
| 0x2AD2 | Indoor Bike Data | Notify | Optional |
| 0x2AD3 | Training Status | Read, Notify | Optional |
| 0x2AD4 | Supported Speed Range | Read | Conditional |
| 0x2AD5 | Supported Inclination Range | Read | Conditional |
| 0x2AD6 | Supported Resistance Level Range | Read | Conditional |
| 0x2AD7 | Supported Power Range | Read | Conditional |
| 0x2AD8 | Supported Heart Rate Range | Read | Conditional |
| 0x2AD9 | Fitness Machine Control Point | Write, Indicate | Optional |
| 0x2ADA | Fitness Machine Status | Notify | Conditional |

## Byte Order

All multi-byte fields are little-endian (LSO first).

---

## Fitness Machine Feature (0x2ACC) — 8 bytes

### Bytes 0–3: Fitness Machine Features Field

| Bit | Feature |
|-----|---------|
| 0 | Average Speed Supported |
| 1 | Cadence Supported |
| 2 | Total Distance Supported |
| 3 | Inclination Supported |
| 4 | Elevation Gain Supported |
| 5 | Pace Supported |
| 6 | Step Count Supported |
| 7 | Resistance Level Supported |
| 8 | Stride Count Supported |
| 9 | Expended Energy Supported |
| 10 | Heart Rate Measurement Supported |
| 11 | Metabolic Equivalent Supported |
| 12 | Elapsed Time Supported |
| 13 | Remaining Time Supported |
| 14 | Power Measurement Supported |
| 15 | Force on Belt and Power Output Supported |
| 16 | User Data Retention Supported |
| 17–31 | Reserved |

### Bytes 4–7: Target Setting Features Field

| Bit | Feature |
|-----|---------|
| 0 | Speed Target Setting Supported |
| 1 | Inclination Target Setting Supported |
| 2 | Resistance Target Setting Supported |
| 3 | Power Target Setting Supported |
| 4 | Heart Rate Target Setting Supported |
| 5 | Targeted Expended Energy Configuration Supported |
| 6 | Targeted Step Number Configuration Supported |
| 7 | Targeted Stride Number Configuration Supported |
| 8 | Targeted Distance Configuration Supported |
| 9 | Targeted Training Time Configuration Supported |
| 10 | Targeted Time in Two HR Zones Supported |
| 11 | Targeted Time in Three HR Zones Supported |
| 12 | Targeted Time in Five HR Zones Supported |
| 13 | Indoor Bike Simulation Parameters Supported |
| 14 | Wheel Circumference Configuration Supported |
| 15 | Spin Down Control Supported |
| 16 | Targeted Cadence Configuration Supported |
| 17–31 | Reserved |

---

## Indoor Bike Data (0x2AD2)

### Flags (uint16)

| Bit | When 0 | When 1 |
|-----|--------|--------|
| 0 | Speed present | Speed NOT present (More Data) |
| 1 | — | Average Speed present |
| 2 | Cadence present | Cadence NOT present |
| 3 | — | Average Cadence present |
| 4 | — | Total Distance present (uint24) |
| 5 | — | Resistance Level present |
| 6 | — | Instantaneous Power present |
| 7 | — | Average Power present |
| 8 | — | Expended Energy present (3 fields) |
| 9 | — | Heart Rate present |
| 10 | — | Metabolic Equivalent present |
| 11 | — | Elapsed Time present |
| 12 | — | Remaining Time present |

**Note**: Bit 2 for Indoor Bike uses inverted logic (like bit 0): Cadence IS present when bit 2 = 0.

### Indoor Bike Field Sizes

| Field | Type | Unit | Resolution |
|-------|------|------|------------|
| Instantaneous Speed | uint16 | km/h | 0.01 |
| Average Speed | uint16 | km/h | 0.01 |
| Instantaneous Cadence | uint16 | 1/min | 0.5 |
| Average Cadence | uint16 | 1/min | 0.5 |
| Total Distance | uint24 | metres | 1 |
| Resistance Level | sint16 | unitless | 1 |
| Instantaneous Power | sint16 | watts | 1 |
| Average Power | sint16 | watts | 1 |
| Total Energy | uint16 | kcal | 1 |
| Energy per Hour | uint16 | kcal | 1 |
| Energy per Minute | uint8 | kcal | 1 |
| Heart Rate | uint8 | bpm | 1 |
| Metabolic Equivalent | uint8 | ME | 0.1 |
| Elapsed Time | uint16 | seconds | 1 |
| Remaining Time | uint16 | seconds | 1 |

---

## Rower Data (0x2AD1)

### Flags (uint16)

| Bit | When 0 | When 1 |
|-----|--------|--------|
| 0 | Stroke Rate + Count present | NOT present (More Data) |
| 1 | — | Average Stroke Rate present |
| 2 | — | Total Distance present (uint24) |
| 3 | — | Instantaneous Pace present |
| 4 | — | Average Pace present |
| 5 | — | Instantaneous Power present |
| 6 | — | Average Power present |
| 7 | — | Resistance Level present |
| 8 | — | Expended Energy present (3 fields) |
| 9 | — | Heart Rate present |
| 10 | — | Metabolic Equivalent present |
| 11 | — | Elapsed Time present |
| 12 | — | Remaining Time present |

### Rower Field Sizes

| Field | Type | Unit | Resolution |
|-------|------|------|------------|
| Stroke Rate | uint8 | 1/min | 0.5 |
| Stroke Count | uint16 | strokes | 1 |
| Average Stroke Rate | uint8 | 1/min | 0.5 |
| Total Distance | uint24 | metres | 1 |
| Instantaneous Pace | uint16 | sec/500m | 1 |
| Average Pace | uint16 | sec/500m | 1 |
| Instantaneous Power | sint16 | watts | 1 |
| Average Power | sint16 | watts | 1 |
| Resistance Level | sint16 | unitless | 1 |

---

## Cross Trainer Data (0x2ACE)

### Flags (uint24 — 3 bytes)

| Bit | When 0 | When 1 |
|-----|--------|--------|
| 0 | Speed present | Speed NOT present (More Data) |
| 1 | — | Average Speed present |
| 2 | — | Total Distance present |
| 3 | — | Step Count present (SPM + Avg) |
| 4 | — | Stride Count present |
| 5 | — | Elevation Gain present |
| 6 | — | Inclination + Ramp Angle present |
| 7 | — | Resistance Level present |
| 8 | — | Instantaneous Power present |
| 9 | — | Average Power present |
| 10 | — | Expended Energy present |
| 11 | — | Heart Rate present |
| 12 | — | Metabolic Equivalent present |
| 13 | — | Elapsed Time present |
| 14 | — | Remaining Time present |
| 15 | Forward | Backward (Movement Direction) |

---

## Fitness Machine Control Point (0x2AD9)

### Op Codes

| Value | Name | Parameter | Requirement |
|-------|------|-----------|-------------|
| 0x00 | Request Control | None | M |
| 0x01 | Reset | None | M |
| 0x02 | Set Target Speed | uint16 (0.01 km/h) | C |
| 0x03 | Set Target Inclination | sint16 (0.1%) | C |
| 0x04 | Set Target Resistance | uint8 (0.1 unitless) | C |
| 0x05 | Set Target Power | sint16 (1 W) | C |
| 0x06 | Set Target Heart Rate | uint8 (1 bpm) | C |
| 0x07 | Start or Resume | None | M |
| 0x08 | Stop or Pause | uint8 (0x01=Stop, 0x02=Pause) | M |
| 0x09 | Set Targeted Expended Energy | uint16 (1 kcal) | C |
| 0x0A | Set Targeted Steps | uint16 (1 step) | C |
| 0x0B | Set Targeted Strides | uint16 (1 stride) | C |
| 0x0C | Set Targeted Distance | uint24 (1 m) | C |
| 0x0D | Set Targeted Training Time | uint16 (1 s) | C |
| 0x0E | Set Time in 2 HR Zones | 2×uint16 (seconds) | C |
| 0x0F | Set Time in 3 HR Zones | 3×uint16 (seconds) | C |
| 0x10 | Set Time in 5 HR Zones | 5×uint16 (seconds) | C |
| 0x11 | Set Indoor Bike Sim Params | see below | C |
| 0x12 | Set Wheel Circumference | uint16 (0.1 mm) | O |
| 0x13 | Spin Down Control | uint8 (0x01=Start, 0x02=Ignore) | O |
| 0x14 | Set Targeted Cadence | uint16 (0.5 1/min) | C |
| 0x80 | Response Code | req_opcode + result + param | M |

### Indoor Bike Simulation Parameters (Op 0x11)

| Offset | Size | Field | Unit | Resolution |
|--------|------|-------|------|------------|
| 0 | 2 | Wind Speed | m/s | 0.001 |
| 2 | 2 | Grade | % | 0.01 |
| 4 | 1 | Crr | unitless | 0.0001 |
| 5 | 1 | Cw | kg/m | 0.01 |

### Response Indication Format

```
[0x80] [Request Op Code] [Result Code] [Response Parameter...]
```

Result Codes:
- 0x01 = Success
- 0x02 = Op Code Not Supported
- 0x03 = Invalid Parameter
- 0x04 = Operation Failed
- 0x05 = Control Not Permitted

### Control Flow

1. Client writes `0x00` (Request Control) → Server indicates `[0x80, 0x00, 0x01]` (Success)
2. Client writes target op codes → Server indicates response
3. Permission valid until disconnect, Reset, or Control Permission Lost status

---

## Fitness Machine Status (0x2ADA)

| Op Code | Definition | Parameter |
|---------|-----------|-----------|
| 0x01 | Reset | None |
| 0x02 | Stopped/Paused by User | uint8 (0x01=Stop, 0x02=Pause) |
| 0x03 | Stopped by Safety Key | None |
| 0x04 | Started/Resumed by User | None |
| 0x05 | Target Speed Changed | uint16 (0.01 km/h) |
| 0x06 | Target Incline Changed | sint16 (0.1%) |
| 0x07 | Target Resistance Changed | uint8 (0.1) |
| 0x08 | Target Power Changed | sint16 (1 W) |
| 0x09 | Target Heart Rate Changed | uint8 (1 bpm) |
| 0x12 | Indoor Bike Sim Params Changed | 6 bytes (see sim params) |
| 0x14 | Spin Down Status | uint8 |
| 0xFF | Control Permission Lost | None |

---

## Supported Range Characteristics

All range characteristics have the same 6-byte format:

```
[Minimum (uint16)] [Maximum (uint16)] [Increment (uint16)]
```

| Characteristic | UUID | Unit | Resolution |
|---------------|------|------|------------|
| Supported Speed Range | 0x2AD4 | km/h | 0.01 |
| Supported Inclination Range | 0x2AD5 | % | 0.1 |
| Supported Resistance Level Range | 0x2AD6 | unitless | 0.1 |
| Supported Power Range | 0x2AD7 | W | 1 |
| Supported Heart Rate Range | 0x2AD8 | bpm | 1 |

---

## Training Status (0x2AD3)

### Structure

```
[Flags (1 byte)] [Training Status (1 byte)] [String (variable, optional)]
```

### Training Status Values

| Value | Meaning |
|-------|---------|
| 0x00 | Other |
| 0x01 | Idle |
| 0x02 | Warming Up |
| 0x03 | Low Intensity Interval |
| 0x04 | High Intensity Interval |
| 0x05 | Recovery Interval |
| 0x06 | Isometric |
| 0x07 | Heart Rate Control |
| 0x08 | Fitness Test |
| 0x09 | Speed Outside Control Region - Low |
| 0x0A | Speed Outside Control Region - High |
| 0x0B | Cool Down |
| 0x0C | Watt Control |
| 0x0D | Manual Mode (Quick Start) |
| 0x0E | Pre-Workout |
| 0x0F | Post-Workout |

---

## Advertising Data Format

Service Data AD Type (total 6 bytes payload after AD header):

```
[FTMS UUID LE: 0x26, 0x18] [Flags: 1 byte] [Machine Type: 2 bytes LE]
```

### Flags byte
- Bit 0: Fitness Machine Available (1=True)
- Bits 1–7: Reserved

### Machine Type bits
- Bit 0: Treadmill
- Bit 1: Cross Trainer
- Bit 2: Step Climber
- Bit 3: Stair Climber
- Bit 4: Rower
- Bit 5: Indoor Bike
- Bits 6–15: Reserved

---

## Multi-Notification (More Data) Rules

When a Data Record exceeds ATT_MTU:
1. First notification: More Data bit = 1, mandatory fields NOT present
2. Middle notifications: More Data bit = 1
3. Last notification: More Data bit = 0, mandatory fields present

If link loss during multi-notification: discard entire Data Record.

---

## Special Values (Data Not Available)

| Type | Value | Meaning |
|------|-------|---------|
| SINT16 | 0x7FFF (32767) | Data Not Available |
| UINT16 | 0xFFFF (65535) | Data Not Available |
| UINT8 | 0xFF (255) | Data Not Available |

Used when a field must be present (per flags) but the server cannot compute it.

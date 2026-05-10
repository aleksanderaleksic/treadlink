# PRD: TreadLink — FTMS to Footpod BLE Bridge

## Problem Statement

Fitness watches and apps that support BLE footpods (RSC profile) cannot receive data directly from FTMS-enabled treadmills. Runners using these devices on treadmills have no way to get live speed and distance data on their watch without a physical footpod sensor.

## Proposed Solution

TreadLink is a firmware for the nrf52840-dk that acts as a BLE bridge: it connects to a treadmill as a Central (FTMS client), converts the treadmill data, and exposes it as a Peripheral advertising a Running Speed and Cadence (RSC) footpod service.

## User Stories

**US-1**: As a runner, I want my watch to receive live speed from my treadmill so I can see my pace without wearing a footpod.

**US-2**: As a runner, I want total distance relayed to my watch so my workout log is accurate.

**US-3**: As a runner, I want TreadLink to work automatically when powered on so I don't have to configure anything each session.

**US-4**: As a runner, I want TreadLink to reconnect to my treadmill if the connection drops so my session isn't interrupted.

## Requirements

### Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | Scan for BLE devices advertising FTMS (0x1826) | Must |
| FR-2 | Select strongest RSSI device above minimum threshold (-70 dBm) | Must |
| FR-3 | Retry scanning indefinitely if no device found | Must |
| FR-4 | Connect to treadmill and subscribe to Treadmill Data (0x2ACD) notifications | Must |
| FR-5 | Convert FTMS instantaneous speed to RSC speed units | Must |
| FR-6 | Convert FTMS total distance to RSC total distance units | Must |
| FR-7 | Advertise RSC service (0x1814) after first FTMS notification received | Must |
| FR-8 | Send RSC Measurement notifications to connected consumer | Must |
| FR-9 | Mark cadence as unsupported in RSC Measurement | Must |
| FR-10 | On treadmill disconnect: stop advertising, drop consumer, rescan | Must |
| FR-11 | On consumer disconnect: maintain treadmill connection, keep advertising | Must |
| FR-12 | Support exactly 1 treadmill + 1 consumer connection | Must |
| FR-13 | LED status feedback for scanning/connected/bridging states | Nice-to-have |

### Non-Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| NFR-1 | Fully automatic — no buttons or configuration | Must |
| NFR-2 | RTT logging via defmt for debugging | Must |
| NFR-3 | Latest-value-only semantics (no buffering/queuing) | Must |
| NFR-4 | Operate on nrf52840-dk | Must |

## Technical Design

### Platform

- **MCU**: nrf52840-dk
- **Runtime**: Embassy (async)
- **BLE stack**: trouble (pure-Rust, Central + Peripheral)
- **Logging**: defmt + defmt-rtt

### Architecture


┌─────────────────────────────────────────────────┐
│                  nrf52840                         │
│                                                  │
│  ┌──────────────┐   Signal   ┌───────────────┐  │
│  │ Central Task │───────────▶│Peripheral Task│  │
│  │  (FTMS sub)  │            │  (RSC adv)    │  │
│  └──────┬───────┘            └───────┬───────┘  │
│         │                            │           │
└─────────┼────────────────────────────┼───────────┘
         │ BLE Central                │ BLE Peripheral
         ▼                            ▼
   ┌───────────┐               ┌───────────┐
   │ Treadmill │               │   Watch   │
   │  (FTMS)   │               │   (RSC)   │
   └───────────┘               └───────────┘

### State Machine

| State | Central | Peripheral | Transition |
|-------|---------|------------|------------|
| SCANNING | Scanning for FTMS | Not advertising | → CONNECTING on device found |
| CONNECTING | Connecting + discovering | Not advertising | → BRIDGING on subscribe success |
| BRIDGING | Receiving notifications | Advertising / connected | → SCANNING on treadmill disconnect |

### Data Conversion

| Field | FTMS Source | RSC Output | Formula |
|-------|-------------|------------|---------|
| Speed | uint16, 0.01 km/h | uint16, 1/256 m/s |  |
| Total Distance | uint24, metres | uint24, 1/10 m |  |
| Cadence | — | Not present | Flag bit = 0 |

### Inter-task Communication

 — single-value, overwrite semantics. Central writes latest converted measurement; Peripheral reads when notifying consumer.

### Project Structure


treadlink/
├── .cargo/config.toml
├── Cargo.toml
├── memory.x
├── src/
│   ├── main.rs
│   ├── central.rs
│   ├── peripheral.rs
│   ├── converter.rs
│   └── protocol.rs

## Configuration Constants

| Constant | Default | Description |
|----------|---------|-------------|
|  | -70 dBm | Minimum RSSI to consider a device |
|  | 100ms | Scan window/interval |
|  |  | BLE advertised name |

## Out of Scope

- Cadence estimation
- Multiple treadmills or consumers
- Bonding/pairing/encryption
- OTA firmware updates
- Persistent storage
- Button/configuration UI

## Success Criteria

1. TreadLink boots, finds a treadmill, and begins relaying within 10 seconds of treadmill being available
2. A Garmin/Apple Watch connects and displays live pace from the treadmill
3. Treadmill power-off triggers rescan; treadmill power-on resumes relay without manual intervention
4. No data loss visible to consumer (latest value always available)

## Open Questions

1. What RSSI threshold value works best in practice? (Starting with -70 dBm, may need tuning)
2. Should RSC distance overflow (uint24 max) trigger a reset or wrap silently?
3. Is there a need for a "stride length" characteristic in RSC Feature for watch compatibility?

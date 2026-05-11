# Implementation Plan: FTMS-RSC Bridge

## Overview

Implement the TreadLink BLE bridge firmware that connects to an FTMS treadmill (Central), converts speed/distance data, and re-exposes it as an RSC peripheral for a Garmin Fenix 6X. Implementation proceeds bottom-up: pure modules (protocol, converter) first with property tests, then async task integration (central, peripheral, LED), then final wiring in main.rs.

## Tasks

- [x] 1. Implement protocol module — data types and FTMS parsing/serialization
  - [x] 1.1 Define data types and error enum in `protocol.rs`
    - Define `TreadmillData` struct with `flags: u16`, `speed: Option<u16>`, `total_distance: Option<u32>`
    - Define `RscMeasurement` struct with `flags: u8`, `speed: u16`, `cadence: u8`, `stride_length: Option<u16>`, `total_distance: Option<u32>`
    - Define `ParseError` enum with `InsufficientData { expected: usize, actual: usize }` variant
    - Derive `Debug`, `PartialEq`, `Clone`, `Copy` and `defmt::Format` on all types
    - _Requirements: 3.1, 9.1, 3.4_

  - [x] 1.2 Implement `parse_treadmill_data` and `serialize_treadmill_data` in `protocol.rs`
    - Parse flags (2 bytes LE), extract speed when bit 0 is clear, skip average speed (2 bytes) when bit 1 is set, extract uint24 total_distance when bit 2 is set
    - Return `ParseError::InsufficientData` when byte slice is too short for declared flags
    - Serialize: encode flags + speed + optional 3-byte distance into `heapless::Vec<u8, 7>`
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 4.1, 4.2, 4.3, 4.4, 4.5_

  - [x] 1.3 Implement `parse_rsc_measurement` and `serialize_rsc_measurement` in `protocol.rs`
    - Parse flags byte, extract speed (2 bytes LE), cadence (1 byte), optional stride_length (2 bytes LE if bit 0), optional total_distance (4 bytes LE if bit 1)
    - Return `ParseError::InsufficientData` when byte slice is too short
    - Serialize: encode flags + speed + cadence + optional stride_length + optional total_distance into `heapless::Vec<u8, 10>`
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 9.1, 9.2, 9.3, 9.4, 9.5, 9.6, 9.7_

  - [x] 1.4 Write property test: FTMS Treadmill Data round-trip
    - **Property 1: FTMS Treadmill Data round-trip**
    - Generate random `TreadmillData` (speed: Option<0..=65535>, total_distance: Option<0..=16_777_215>)
    - Assert `parse_treadmill_data(serialize_treadmill_data(&data)) == Ok(data)`
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.5, 4.1, 4.2, 4.3, 4.4, 4.5**

  - [x] 1.5 Write property test: FTMS parse rejects truncated data
    - **Property 2: FTMS parse rejects truncated data**
    - Generate random `TreadmillData`, serialize, truncate to `0..serialized.len()-1` bytes
    - Assert parser returns `ParseError::InsufficientData`
    - **Validates: Requirements 3.4**

  - [x] 1.6 Write property test: RSC Measurement round-trip
    - **Property 3: RSC Measurement round-trip**
    - Generate random `RscMeasurement` (speed: 0..=65535, cadence: 0..=255, stride_length: Option<0..=65535>, total_distance: Option<0..=u32::MAX>)
    - Assert `parse_rsc_measurement(serialize_rsc_measurement(&data)) == Ok(data)`
    - **Validates: Requirements 8.1, 8.2, 8.3, 8.4, 8.5, 9.1, 9.2, 9.3, 9.4, 9.5, 9.6**

  - [x] 1.7 Write property test: RSC parse rejects truncated data
    - **Property 4: RSC parse rejects truncated data**
    - Generate random `RscMeasurement`, serialize, truncate to `0..serialized.len()-1` bytes
    - Assert parser returns `ParseError::InsufficientData`
    - **Validates: Requirements 9.7**

  - [x] 1.8 Write example-based unit tests for `protocol.rs`
    - Test parsing a known FTMS byte sequence from the BLE spec
    - Test parsing FTMS with average speed flag set (verify offset skipping per Req 3.7)
    - Test parsing FTMS with speed-not-present flag (bit 0 set, Req 3.6)
    - _Requirements: 3.1, 3.2, 3.3, 3.6, 3.7_

- [x] 2. Implement converter module — unit conversion and RSC assembly
  - [x] 2.1 Implement `convert_speed` and `convert_distance` in `converter.rs`
    - `convert_speed(ftms_speed: u16) -> u16`: compute `(ftms_speed as u32 * 32) / 45` with truncating division
    - `convert_distance(ftms_metres: u32) -> u32`: compute `ftms_metres * 10`
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 6.1, 6.2_

  - [x] 2.2 Implement `assemble_rsc` in `converter.rs`
    - Takes `&TreadmillData`, returns `RscMeasurement`
    - Set cadence = 0, stride_length = None, walking/running bit = 0
    - Convert speed via `convert_speed`, convert distance via `convert_distance` if present
    - Set flags byte: bit 1 = total_distance.is_some(), bits 0 and 2 always 0
    - _Requirements: 6.3, 7.1, 7.2, 7.3, 7.4, 7.5_

  - [x] 2.3 Write property test: Speed conversion correctness and range
    - **Property 5: Speed conversion correctness and range**
    - Generate random `u16` speed value
    - Assert `convert_speed(s) == (s as u32 * 32) / 45` and result ≤ 65535
    - **Validates: Requirements 5.1, 5.2, 5.3, 5.4**

  - [x] 2.4 Write property test: Distance conversion correctness
    - **Property 6: Distance conversion correctness**
    - Generate random `u32` in `0..=16_777_215`
    - Assert `convert_distance(d) == d * 10` and result fits u32
    - **Validates: Requirements 6.1, 6.2**

  - [x] 2.5 Write property test: RSC assembly invariants
    - **Property 7: RSC assembly invariants**
    - Generate random `TreadmillData`
    - Assert: cadence == 0, flags & 0x01 == 0, flags & 0x04 == 0
    - Assert: total_distance.is_some() iff input had total_distance.is_some()
    - Assert: flags bit 1 matches total_distance presence
    - **Validates: Requirements 6.3, 7.1, 7.2, 7.3, 7.4, 7.5**

  - [x] 2.6 Write example-based unit tests for `converter.rs`
    - Test speed=0 → RSC speed=0 (Req 5.4)
    - Test max speed 65535 → result fits u16 (Req 5.3)
    - Test max distance 16_777_215 → result fits u32 (Req 6.2)
    - _Requirements: 5.3, 5.4, 6.2_

- [x] 3. Checkpoint — Pure modules complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Implement central task — scan, connect, GATT discovery, FTMS subscription
  - [x] 4.1 Implement scan candidate collection logic in `central.rs`
    - Define `ScanCandidate { address: BdAddr, rssi: i8 }`
    - Implement `collect_candidates` pure function: accepts iterator of (address, rssi) events, returns `heapless::Vec<ScanCandidate, 4>` sorted by RSSI desc, deduplicated by address (keep max RSSI), discard RSSI < -70
    - _Requirements: 1.2, 1.3_

  - [x] 4.2 Write property test: Candidate collector invariants
    - **Property 8: Candidate collector invariants**
    - Generate random `Vec<(BdAddr, i8)>` advertisement events
    - Assert: length ≤ 4, sorted by RSSI desc, no duplicate addresses, all RSSI ≥ -70, max RSSI kept per address
    - **Validates: Requirements 1.2, 1.3**

  - [x] 4.3 Implement `central_task` async function in `central.rs`
    - Accept `Central<'static>`, `&'static Signal<..>`, `&'static Watch<..>`
    - Outer loop: publish Scanning state, scan 3s, collect candidates
    - If no candidates: restart scan (Req 1.5); on controller error: log + delay 1s (Req 1.6)
    - For each candidate (strongest first): connect (5s timeout), GATT discover (5s timeout), verify 0x2ACD, subscribe notifications
    - On validation success: publish Connecting → Bridging
    - Inner receive loop: parse FTMS notification → assemble_rsc → signal.signal(measurement)
    - On parse error: log, discard, continue (Req 17.1–17.4)
    - On disconnect: publish Scanning, restart outer loop
    - _Requirements: 1.1, 1.4, 1.5, 1.6, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 14.1, 14.2, 14.4, 14.6, 15.1, 15.3, 17.1, 17.2, 17.3, 17.4, 18.2_

- [x] 5. Implement peripheral task — RSC GATT server, advertising, notification relay
  - [x] 5.1 Define RSC GATT server struct in `peripheral.rs`
    - Use trouble-host `#[gatt_server]` and `#[gatt_service]` macros
    - RSC_Measurement (0x2A53): notify
    - RSC_Feature (0x2A54): read, value 0x0002 LE
    - Sensor_Location (0x2A5D): read, value 0x02
    - SC_Control_Point (0x2A55): write + indicate
    - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.5_

  - [x] 5.2 Implement SC Control Point handler as a pure function in `peripheral.rs`
    - `handle_sc_control_point(opcode: u8, cccd_configured: bool, procedure_in_progress: bool) -> ScCpResponse`
    - Define `ScCpResponse` enum: `AttError(u8)` for 0x80/0x81, `Indication([u8; 3])` for response
    - If CCCD not configured: return ATT error 0x81
    - If procedure in progress: return ATT error 0x80
    - If opcode == 0x01: return `[0x10, 0x01, 0x01]` (Success)
    - Otherwise: return `[0x10, op, 0x02]` (Op Code Not Supported)
    - _Requirements: 13.1, 13.2, 13.3, 13.4_

  - [x] 5.3 Write property test: SC Control Point rejects unsupported opcodes
    - **Property 9: SC Control Point rejects unsupported opcodes**
    - Generate random `u8` where `!= 0x01`, with cccd_configured=true, procedure_in_progress=false
    - Assert response is `[0x10, op, 0x02]`
    - **Validates: Requirements 13.2**

  - [x] 5.4 Write example-based unit tests for SC Control Point
    - Test opcode 0x01 → `[0x10, 0x01, 0x01]` (Success, Req 13.1)
    - Test CCCD not configured → ATT error 0x81 (Req 13.3)
    - Test procedure in progress → ATT error 0x80 (Req 13.4)
    - _Requirements: 13.1, 13.3, 13.4_

  - [x] 5.5 Implement `peripheral_task` async function in `peripheral.rs`
    - Accept `Peripheral<'static>`, `&'static RscServer`, `&'static Signal<..>`, `&'static Watch<..>`
    - Subscribe to Watch for BridgeState changes
    - When Bridging: advertise (100ms interval, name "TreadLink", appearance 0x0443)
    - On consumer connect: stop advertising, relay Signal values as RSC notifications via serialize_rsc_measurement
    - Handle SC Control Point writes using `handle_sc_control_point`
    - On consumer disconnect while Bridging: resume advertising
    - On state != Bridging: disconnect consumer, stop advertising
    - On notification delivery failure: log, discard, continue
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7, 12.1, 12.2, 12.3, 14.3, 14.5, 15.2, 15.4, 17.5, 18.3_

- [x] 6. Implement LED task — GPIO status feedback
  - [x] 6.1 Implement `led_task` async function in `central.rs` or a new `led.rs` module
    - Accept `&'static Watch<..>` and GPIO pins for LED1–LED4
    - Subscribe to Watch for BridgeState changes
    - Scanning: blink LED1 at 500ms period (250ms on/off)
    - Connecting: LED1 solid on
    - Bridging, no consumer: LED2 solid on
    - Bridging + consumer: LED2 + LED3 solid on
    - Error/retry: blink LED4 at 100ms period (50ms on/off)
    - On state change: turn off irrelevant LEDs within one blink cycle
    - _Requirements: 16.1, 16.2, 16.3, 16.4, 16.5, 16.6, 18.4_

- [x] 7. Checkpoint — All tasks implemented independently
  - Ensure all tests pass, ask the user if questions arise.

- [x] 8. Wire everything together in `main.rs`
  - [x] 8.1 Initialize BLE stack and shared state in `main.rs`
    - Init embassy-nrf peripherals
    - Init nrf-sdc controller with 2-connection config (central + peripheral)
    - Init trouble-host BLE stack with `HostResources`
    - Create `Signal<CriticalSectionRawMutex, RscMeasurement>` and `Watch<CriticalSectionRawMutex, BridgeState, 2>` as statics via `static_cell`
    - Define `BridgeState` enum in an appropriate location (e.g., `main.rs` or a shared module)
    - _Requirements: 14.1, 14.7, 15.1, 15.3, 18.1_

  - [x] 8.2 Spawn all three tasks from `main`
    - Spawn `central_task` with Central handle, Signal ref, Watch ref
    - Spawn `peripheral_task` with Peripheral handle, RscServer ref, Signal ref, Watch ref
    - Spawn `led_task` with Watch ref and GPIO pins
    - Add `heapless` dependency to Cargo.toml if not present
    - Add `proptest` as a dev-dependency with appropriate features for host testing
    - _Requirements: 18.1, 18.5_

- [x] 9. Final checkpoint — Full integration
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties from the design document (9 properties total)
- Unit tests validate specific examples and edge cases
- Pure modules (`protocol.rs`, `converter.rs`) are implemented first to enable early testing
- The `proptest` crate should be added as a dev-dependency; tests run on host (not target)
- All `#[cfg(test)]` modules compile only for host targets, avoiding `no_std` conflicts

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1", "2.1"] },
    { "id": 1, "tasks": ["1.2", "1.3", "2.2"] },
    { "id": 2, "tasks": ["1.4", "1.5", "1.6", "1.7", "1.8", "2.3", "2.4", "2.5", "2.6"] },
    { "id": 3, "tasks": ["4.1", "5.1", "5.2"] },
    { "id": 4, "tasks": ["4.2", "4.3", "5.3", "5.4", "5.5", "6.1"] },
    { "id": 5, "tasks": ["8.1"] },
    { "id": 6, "tasks": ["8.2"] }
  ]
}
```

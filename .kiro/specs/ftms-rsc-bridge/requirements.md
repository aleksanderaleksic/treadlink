# Requirements Document

## Introduction

TreadLink is a BLE bridge firmware for the nrf52840-dk that connects to an FTMS-enabled treadmill as a BLE Central, converts treadmill speed and distance data, and re-exposes it as a BLE Peripheral advertising the Running Speed and Cadence (RSC) service. The target consumer is a Garmin Fenix 6X watch. The firmware runs on Embassy (async Rust) using trouble-host for the BLE stack and nrf-sdc as the controller.

## Glossary

- **Bridge**: The TreadLink firmware running on the nrf52840-dk that relays data between treadmill and consumer
- **Central_Task**: The async task responsible for scanning, connecting to, and receiving data from the treadmill
- **Peripheral_Task**: The async task responsible for advertising the RSC service and notifying the consumer
- **LED_Task**: The async task responsible for driving status LEDs based on bridge state
- **Treadmill**: A BLE peripheral advertising the FTMS service (0x1826) with a Treadmill Data characteristic (0x2ACD)
- **Consumer**: A BLE central (fitness watch) that connects to the Bridge to receive RSC notifications
- **FTMS**: Fitness Machine Service (BLE service UUID 0x1826)
- **Treadmill_Data**: The FTMS characteristic (UUID 0x2ACD) that provides speed and distance notifications
- **RSC**: Running Speed and Cadence (BLE service UUID 0x1814)
- **RSC_Measurement**: The RSC characteristic (UUID 0x2A53) carrying speed and optional distance
- **RSC_Feature**: The RSC characteristic (UUID 0x2A54) declaring supported features
- **SC_Control_Point**: The RSC characteristic (UUID 0x2A55) for control operations
- **Sensor_Location**: The RSC characteristic (UUID 0x2A5D) reporting sensor placement
- **BridgeState**: An enum representing the current lifecycle phase: Scanning, Connecting, or Bridging
- **Signal**: An Embassy synchronization primitive providing latest-value-only semantics between tasks
- **Watch**: An Embassy synchronization primitive allowing multiple subscribers to observe state changes
- **RSSI**: Received Signal Strength Indicator, measured in dBm
- **Parser**: The module (`protocol.rs`) responsible for decoding raw BLE characteristic bytes into structured data
- **Converter**: The module (`converter.rs`) responsible for unit conversion and RSC_Measurement assembly
- **Pretty_Printer**: The component responsible for encoding structured RSC data back into raw bytes for BLE notifications

## Requirements

### Requirement 1: FTMS Scan and Discovery

**User Story:** As a runner, I want TreadLink to automatically find my treadmill when powered on, so that I do not need to configure anything each session.

#### Acceptance Criteria

1. WHEN the Bridge enters the Scanning state, THE Central_Task SHALL scan for BLE devices advertising the FTMS service UUID (0x1826) for a 3-second window
2. WHILE scanning, THE Central_Task SHALL collect up to 4 unique candidate devices (deduplicated by device address), sorted by descending RSSI, updating a candidate's RSSI if a stronger advertisement is received from the same device
3. WHILE scanning, THE Central_Task SHALL discard any candidate with an RSSI below -70 dBm
4. WHEN the scan window completes with at least one candidate, THE Central_Task SHALL select the candidate with the strongest RSSI and transition the Bridge to the Connecting state
5. WHEN the scan window completes with zero candidates, THE Central_Task SHALL immediately begin a new 3-second scan window
6. IF a scan cycle fails due to a controller error, THEN THE Central_Task SHALL log the error via defmt and begin a new scan cycle after a delay of 1 second

### Requirement 2: Treadmill Connection and Validation

**User Story:** As a runner, I want TreadLink to connect to my treadmill and verify it provides speed data, so that the bridge only operates with compatible equipment.

#### Acceptance Criteria

1. WHEN a candidate is selected, THE Central_Task SHALL initiate a BLE connection to that device with a connection timeout of 5 seconds
2. IF a connection attempt does not complete within 5 seconds, THEN THE Central_Task SHALL abort the attempt and proceed to the next candidate from the scan results
3. WHEN connected to a candidate, THE Central_Task SHALL perform GATT service discovery and verify the presence of the Treadmill_Data characteristic (0x2ACD) within 5 seconds of connection establishment
4. IF the connected device does not expose the Treadmill_Data characteristic or GATT discovery does not complete within 5 seconds, THEN THE Central_Task SHALL disconnect and attempt the next candidate from the scan results
5. IF all candidates fail validation, THEN THE Central_Task SHALL return to the Scanning state and begin a new scan cycle
6. WHEN the Treadmill_Data characteristic is confirmed, THE Central_Task SHALL subscribe to notifications on that characteristic by writing to its Client Characteristic Configuration Descriptor
7. IF notification subscription fails, THEN THE Central_Task SHALL disconnect and attempt the next candidate from the scan results
8. WHEN notification subscription succeeds, THE Central_Task SHALL transition the BridgeState from Connecting to Bridging

### Requirement 3: FTMS Treadmill Data Parsing

**User Story:** As a runner, I want TreadLink to correctly interpret my treadmill's speed and distance data, so that accurate values reach my watch.

#### Acceptance Criteria

1. WHEN a Treadmill_Data notification is received, THE Parser SHALL decode the first 2 bytes as a little-endian uint16 flags field and produce a structured result containing the flags and all present data fields
2. WHEN instantaneous speed is present in the notification (flags bit 0 clear), THE Parser SHALL extract it as a little-endian uint16 value at bytes 2–3 representing speed in units of 0.01 km/h (valid range 0 to 65535, i.e. 0.00 to 655.35 km/h)
3. WHEN total distance is present in the notification (flags bit 2 set), THE Parser SHALL extract it as a little-endian uint24 value in units of metres at the offset determined by the preceding present fields
4. IF a Treadmill_Data notification contains fewer bytes than required by the set flags (minimum 2 bytes for flags alone, plus 2 bytes when speed is present, plus 2 bytes when average speed is present, plus 3 bytes when total distance is present), THEN THE Parser SHALL return a parse error and not produce a partial result
5. THE Parser SHALL serialize a structured result back to bytes such that parsing the serialized output produces a structured result with identical field values (round-trip property)
6. IF flags bit 0 is set (instantaneous speed not present), THEN THE Parser SHALL produce a structured result with speed marked as absent
7. WHEN flag bits for average speed (bit 1) are set in the notification, THE Parser SHALL skip the corresponding 2-byte field to correctly locate subsequent fields

### Requirement 4: FTMS Treadmill Data Pretty Printing

**User Story:** As a developer, I want to serialize Treadmill Data structures back to bytes, so that round-trip correctness can be verified in tests.

#### Acceptance Criteria

1. THE Pretty_Printer SHALL encode a parsed Treadmill_Data structure into a byte sequence of length 2 + 2 + (3 if total_distance is present), where the first 2 bytes are the flags field, followed by the speed field, followed by the total_distance field if present
2. THE Pretty_Printer SHALL set bit 0 of the flags field to 0 when instantaneous speed is present, and set bit 2 of the flags field to 1 when total_distance is present and to 0 when total_distance is absent
3. THE Pretty_Printer SHALL encode speed as a little-endian uint16 in units of 0.01 km/h with a valid range of 0 to 65535
4. WHEN total distance is present, THE Pretty_Printer SHALL encode it as a little-endian uint24 in units of metres with a valid range of 0 to 16777215
5. WHEN a Treadmill_Data structure is encoded and the resulting bytes are parsed back, THE Pretty_Printer SHALL produce output that, when re-parsed by the parser, yields a structure equal to the original input

### Requirement 5: Speed Conversion

**User Story:** As a runner, I want my treadmill speed accurately converted to the format my watch expects, so that my pace display is correct.

#### Acceptance Criteria

1. WHEN a speed value in the range 0..=65535 is extracted from Treadmill_Data, THE Converter SHALL convert it from 0.01 km/h units to 1/256 m/s units using the formula `(ftms_speed as u32 * 32) / 45`
2. THE Converter SHALL use truncating integer division for the speed conversion
3. FOR ALL valid FTMS speed inputs (0..=65535), THE Converter SHALL produce a result that fits in a uint16
4. WHEN the FTMS speed value is 0, THE Converter SHALL produce an RSC speed value of 0

### Requirement 6: Distance Conversion

**User Story:** As a runner, I want my treadmill distance accurately relayed to my watch, so that my workout log reflects the correct total.

#### Acceptance Criteria

1. WHEN total distance is present in the parsed Treadmill_Data, THE Converter SHALL convert the uint24 metre value to RSC 1/10 metre units (UINT32) by multiplying by 10
2. THE Converter SHALL store the converted distance value in a UINT32 field without clamping or wrapping, since the maximum FTMS uint24 input (16,777,215 metres × 10 = 167,772,150) fits within UINT32 range
3. WHEN total distance is not present in the parsed Treadmill_Data, THE Converter SHALL omit the distance field from the RSC_Measurement and clear the Total Distance Present flag bit (bit 1) in the Flags byte

### Requirement 7: RSC Measurement Assembly

**User Story:** As a runner, I want the converted data packaged correctly for my watch, so that it displays speed and distance without errors.

#### Acceptance Criteria

1. THE Converter SHALL assemble an RSC_Measurement byte array in little-endian format containing: Flags (UINT8), Instantaneous Speed (UINT16, units of 1/256 m/s), Instantaneous Cadence (UINT8), and optionally Total Distance (UINT32, units of 1/10 m), in that field order
2. THE Converter SHALL set the Instantaneous Cadence field to zero in every RSC_Measurement
3. IF the most recent FTMS Treadmill Data notification included a total distance value, THEN THE Converter SHALL set the Total Distance Present flag (bit 1) to 1 and append the Total Distance field (4 bytes) after the cadence byte
4. IF the most recent FTMS Treadmill Data notification did not include a total distance value, THEN THE Converter SHALL set the Total Distance Present flag (bit 1) to 0 and omit the Total Distance field, producing a 4-byte payload
5. THE Converter SHALL always set the Walking or Running status bit (bit 2) and the Stride Length Present bit (bit 0) to 0 in the Flags byte

### Requirement 8: RSC Measurement Serialization

**User Story:** As a developer, I want RSC Measurement structures serialized to the correct byte format, so that BLE notifications are well-formed.

#### Acceptance Criteria

1. THE Pretty_Printer SHALL encode an RSC_Measurement structure into the byte format defined by the RSC specification
2. THE Pretty_Printer SHALL encode the flags byte first, followed by speed as little-endian uint16, then cadence as uint8
3. WHEN total distance is present, THE Pretty_Printer SHALL append it as a little-endian uint32 in 1/10 metre units after the cadence byte
4. WHEN total distance is not present, THE Pretty_Printer SHALL produce a 4-byte payload (flags + speed + cadence)
5. FOR ALL valid RSC_Measurement structures, parsing then printing then parsing SHALL produce an equivalent structured result (round-trip property)

### Requirement 9: RSC Measurement Parsing

**User Story:** As a developer, I want to parse RSC Measurement bytes back into structures, so that round-trip correctness can be verified in tests.

#### Acceptance Criteria

1. WHEN an RSC_Measurement byte sequence is received, THE Parser SHALL decode the flags byte (byte 0) to determine which optional fields are present: stride length (bit 0), total distance (bit 1), and walking-or-running status (bit 2)
2. WHEN an RSC_Measurement byte sequence is received, THE Parser SHALL extract instantaneous speed from bytes 1–2 as a little-endian uint16 in 1/256 m/s units
3. WHEN an RSC_Measurement byte sequence is received, THE Parser SHALL extract instantaneous cadence from byte 3 as a uint8 in steps per minute
4. IF the stride length flag (bit 0) is set, THEN THE Parser SHALL extract instantaneous stride length as a little-endian uint16 in 1/100 metre units from the 2 bytes immediately following cadence
5. IF the total distance flag (bit 1) is set, THEN THE Parser SHALL extract total distance as a little-endian uint32 in 1/10 metre units from the 4 bytes immediately following the last present prior field
6. WHEN an RSC_Measurement byte sequence is received, THE Parser SHALL extract the walking-or-running status from bit 2 of the flags byte where 0 indicates walking and 1 indicates running
7. IF the byte sequence length is less than 4 bytes or less than the length required by the flags (4 bytes base, plus 2 if stride length flag set, plus 4 if total distance flag set), THEN THE Parser SHALL return a parse error indicating insufficient data

### Requirement 10: RSC Peripheral Advertising

**User Story:** As a runner, I want my watch to discover TreadLink as a footpod, so that it can connect and receive data.

#### Acceptance Criteria

1. WHEN the BridgeState transitions to Bridging, THE Peripheral_Task SHALL begin BLE advertising with the RSC service UUID (0x1814)
2. THE Peripheral_Task SHALL advertise with a 100ms advertising interval
3. THE Peripheral_Task SHALL advertise the device name "TreadLink"
4. THE Peripheral_Task SHALL advertise with appearance value 0x0443 (Running Walking Sensor: In-Shoe)
5. WHEN the BridgeState transitions away from Bridging, THE Peripheral_Task SHALL stop advertising
6. WHEN a consumer connects, THE Peripheral_Task SHALL stop advertising and accept at most 1 concurrent consumer connection
7. WHEN the consumer disconnects while BridgeState is Bridging, THE Peripheral_Task SHALL resume advertising within 1 second

### Requirement 11: RSC GATT Server

**User Story:** As a runner, I want my watch to read the correct service characteristics, so that it recognizes TreadLink as a compatible footpod.

#### Acceptance Criteria

1. THE Peripheral_Task SHALL register the Running Speed and Cadence service (0x1814) as a Primary Service containing all RSC characteristics
2. THE Peripheral_Task SHALL expose the RSC_Measurement characteristic (0x2A53) with notify permission and a Client Characteristic Configuration Descriptor (CCCD)
3. THE Peripheral_Task SHALL expose the RSC_Feature characteristic (0x2A54) with read permission and value 0x0002 (Total Distance Measurement Supported), encoded as a little-endian UINT16
4. THE Peripheral_Task SHALL expose the Sensor_Location characteristic (0x2A5D) with read permission and value 0x02 (In Shoe), encoded as a UINT8
5. THE Peripheral_Task SHALL expose the SC_Control_Point characteristic (0x2A55) with write and indicate permissions and a Client Characteristic Configuration Descriptor (CCCD)

### Requirement 12: RSC Notification Delivery

**User Story:** As a runner, I want live speed updates on my watch as the treadmill reports them, so that my pace display stays current.

#### Acceptance Criteria

1. WHEN the Central_Task produces a new RSC_Measurement via the Signal, THE Peripheral_Task SHALL serialize the RSC_Measurement and send a BLE notification on the RSC_Measurement characteristic (0x2A53) to the connected Consumer
2. THE Peripheral_Task SHALL use latest-value-only semantics: if multiple measurements arrive before a notification is sent, only the most recent value is transmitted
3. WHILE no Consumer is connected, THE Peripheral_Task SHALL discard incoming measurements from the Signal

### Requirement 13: SC Control Point Handling

**User Story:** As a runner, I want my watch's control requests handled gracefully, so that the connection remains stable.

#### Acceptance Criteria

1. WHEN the Consumer writes a Set Cumulative Value opcode (0x01) with a UINT32 parameter to the SC_Control_Point, THE Peripheral_Task SHALL respond with an indication containing Response Code 0x10, Request Op Code 0x01, and Result 0x01 (Success) without applying the value
2. WHEN the Consumer writes any other opcode to the SC_Control_Point, THE Peripheral_Task SHALL respond with an indication containing Response Code 0x10, Request Op Code matching the received opcode, and Result 0x02 (Op Code Not Supported)
3. IF the Consumer writes to the SC_Control_Point while the SC_Control_Point CCCD is not configured for indications, THEN THE Peripheral_Task SHALL reject the write with an ATT error 0x81 (CCC Improperly Configured) without processing the opcode
4. IF the Consumer writes to the SC_Control_Point while a previously initiated SC Control Point procedure has not yet completed, THEN THE Peripheral_Task SHALL reject the write with an ATT error 0x80 (Procedure Already In Progress) without processing the new opcode

### Requirement 14: Bridge State Management

**User Story:** As a runner, I want TreadLink to handle connection changes gracefully, so that my session resumes automatically after interruptions.

#### Acceptance Criteria

1. THE Central_Task SHALL publish BridgeState changes via a Watch primitive with 2 subscribers (Peripheral_Task and LED_Task), where BridgeState is one of {Scanning, Connecting, Bridging}
2. WHEN the treadmill BLE connection-lost event is received, THE Central_Task SHALL transition BridgeState to Scanning and begin scanning for FTMS devices
3. WHEN BridgeState transitions to Scanning, THE Peripheral_Task SHALL disconnect any connected Consumer and stop advertising within 1 second of the state change
4. WHEN a treadmill advertising FTMS is discovered during a rescan, THE Central_Task SHALL reconnect and transition BridgeState to Connecting, retrying the scan indefinitely until a device is found
5. WHEN the Consumer disconnects, THE Peripheral_Task SHALL resume advertising within 1 second while maintaining the treadmill connection and preserving the current BridgeState
6. IF a reconnection attempt to the treadmill fails, THEN THE Central_Task SHALL remain in Scanning state and continue scanning indefinitely without manual intervention
7. THE Bridge SHALL support exactly 1 treadmill connection and 1 consumer connection simultaneously (2-connection cap via nrf-sdc)

### Requirement 15: Inter-Task Data Flow

**User Story:** As a developer, I want a clear data path between tasks, so that the system is predictable and free of race conditions.

#### Acceptance Criteria

1. WHEN Central_Task receives a converted RSC_Measurement from the FTMS data stream, THE Central_Task SHALL publish the value to a Signal with CriticalSectionRawMutex
2. WHILE a consumer is subscribed to RSC Measurement notifications, THE Peripheral_Task SHALL await new values from the Signal before sending each notification
3. THE Signal SHALL use overwrite semantics: a new publish replaces any unread previous value, and the Signal SHALL hold at most 1 element
4. IF the Signal contains no value since system boot, THEN THE Peripheral_Task SHALL remain awaiting until the first value is published without sending any notification

### Requirement 16: LED Status Feedback

**User Story:** As a runner, I want to see at a glance whether TreadLink is searching, connected, or bridging, so that I can troubleshoot without a computer.

#### Acceptance Criteria

1. WHILE BridgeState is Scanning, THE LED_Task SHALL blink LED1 at a 500ms period (250ms on, 250ms off)
2. WHILE BridgeState is Connecting, THE LED_Task SHALL hold LED1 solid on
3. WHILE BridgeState is Bridging and no Consumer is connected, THE LED_Task SHALL hold LED2 solid on
4. WHILE BridgeState is Bridging and a Consumer is connected, THE LED_Task SHALL hold LED2 and LED3 solid on
5. WHILE an error or retry condition is active, THE LED_Task SHALL blink LED4 at a 100ms period (50ms on, 50ms off)
6. WHEN BridgeState changes, THE LED_Task SHALL turn off all LEDs not relevant to the new state within one blink cycle

### Requirement 17: Error Handling and Recovery

**User Story:** As a runner, I want TreadLink to recover from transient errors without manual intervention, so that my session is not interrupted.

#### Acceptance Criteria

1. IF a connection attempt to a candidate fails, THEN THE Central_Task SHALL log the error via defmt and attempt the next candidate in the scan results list
2. IF all candidates in the scan results list have been attempted without success, THEN THE Central_Task SHALL return to the Scanning state and restart discovery within 1 second
3. IF GATT discovery fails on a connected device, THEN THE Central_Task SHALL disconnect from that device and attempt the next candidate in the scan results list
4. IF notification subscription fails, THEN THE Central_Task SHALL disconnect from the device and return to the Scanning state
5. IF a notification delivery to the Consumer fails, THEN THE Peripheral_Task SHALL log the error via defmt, discard the measurement, and continue advertising and relaying subsequent notifications from the Central_Task
6. THE Bridge SHALL use defmt logging via RTT for all connection failures, GATT discovery failures, subscription failures, and notification delivery failures

### Requirement 18: Task Architecture

**User Story:** As a developer, I want a clean separation of concerns across async tasks, so that the firmware is maintainable and testable.

#### Acceptance Criteria

1. THE Bridge SHALL consist of exactly 3 Embassy async tasks: Central_Task, Peripheral_Task, and LED_Task
2. THE Central_Task SHALL own all BLE Central operations (scanning, connecting, GATT discovery, notification reception) and SHALL NOT perform advertising, GATT server hosting, or GPIO operations
3. THE Peripheral_Task SHALL own all BLE Peripheral operations (advertising, GATT server, notification sending) and SHALL NOT perform scanning, Central connections, or GPIO operations
4. THE LED_Task SHALL own all GPIO operations for the 4 status LEDs and SHALL NOT perform any BLE operations
5. WHEN Central_Task produces a converted measurement, THE Central_Task SHALL communicate it to Peripheral_Task via a single shared Signal with overwrite semantics (latest-value-only, no queuing)
6. THE `protocol` module SHALL contain all byte-level parsing of FTMS Treadmill Data notifications and all serialization of RSC Measurement payloads, and SHALL be callable from any task without owning hardware resources
7. THE `converter` module SHALL contain all unit conversion math (FTMS speed/distance to RSC units) and RSC_Measurement assembly logic, and SHALL be callable from any task without owning hardware resources

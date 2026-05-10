// Feature: ftms-rsc-bridge, Task 1.8: Example-based unit tests for protocol.rs
//
// Host-side example-based tests for FTMS Treadmill Data parsing.
// Validates specific byte sequences from the BLE spec and edge cases.

use treadlink_pbt::protocol::{parse_treadmill_data, TreadmillData};

/// Test parsing a known FTMS byte sequence: speed present, no distance
/// Input: flags=0x0000 (speed present), speed=1000 (10.00 km/h)
///
/// **Validates: Requirements 3.1, 3.2**
#[test]
fn parse_ftms_speed_only() {
    let bytes = [0x00, 0x00, 0xE8, 0x03];
    let result = parse_treadmill_data(&bytes).unwrap();
    assert_eq!(
        result,
        TreadmillData {
            flags: 0,
            speed: Some(1000),
            total_distance: None
        }
    );
}

/// Test parsing FTMS with average speed flag set (Req 3.7)
/// The parser must skip the 2-byte average speed field to correctly locate subsequent fields.
/// Input: flags=0x0002 (avg speed present), speed=1000, avg_speed=500 (skipped)
///
/// **Validates: Requirements 3.7**
#[test]
fn parse_ftms_with_average_speed_flag() {
    let bytes = [0x02, 0x00, 0xE8, 0x03, 0xF4, 0x01];
    let result = parse_treadmill_data(&bytes).unwrap();
    assert_eq!(
        result,
        TreadmillData {
            flags: 2,
            speed: Some(1000),
            total_distance: None
        }
    );
}

/// Test parsing FTMS with speed-not-present flag (Req 3.6)
/// Input: flags=0x0001 (bit 0 set = speed NOT present)
///
/// **Validates: Requirements 3.6**
#[test]
fn parse_ftms_speed_not_present() {
    let bytes = [0x01, 0x00];
    let result = parse_treadmill_data(&bytes).unwrap();
    assert_eq!(
        result,
        TreadmillData {
            flags: 1,
            speed: None,
            total_distance: None
        }
    );
}

/// Test parsing FTMS with speed + total distance
/// Input: flags=0x0004 (distance present), speed=1000, distance=10000m (uint24 LE)
///
/// **Validates: Requirements 3.1, 3.2, 3.3**
#[test]
fn parse_ftms_speed_and_distance() {
    let bytes = [0x04, 0x00, 0xE8, 0x03, 0x10, 0x27, 0x00];
    let result = parse_treadmill_data(&bytes).unwrap();
    assert_eq!(
        result,
        TreadmillData {
            flags: 4,
            speed: Some(1000),
            total_distance: Some(10000)
        }
    );
}

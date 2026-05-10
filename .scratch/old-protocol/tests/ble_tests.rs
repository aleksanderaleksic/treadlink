use protocol::{adv_contains_ftms_uuid, select_best_rssi, convert_ftms_to_rsc};

// ---------------------------------------------------------------------------
// Unit tests for adv_contains_ftms_uuid
// Feature: refactoring — Validates: Requirements 5.1
// ---------------------------------------------------------------------------

#[test]
fn adv_ftms_uuid_present() {
    let data: &[u8] = &[
        0x02, 0x01, 0x06,
        0x03, 0x03, 0x26, 0x18,
    ];
    assert!(adv_contains_ftms_uuid(data));
}

#[test]
fn adv_ftms_uuid_present_incomplete_list() {
    let data: &[u8] = &[
        0x05, 0x02, 0x0D, 0x18, 0x26, 0x18,
    ];
    assert!(adv_contains_ftms_uuid(data));
}

#[test]
fn adv_ftms_uuid_absent() {
    let data: &[u8] = &[
        0x02, 0x01, 0x06,
        0x03, 0x03, 0x0D, 0x18,
    ];
    assert!(!adv_contains_ftms_uuid(data));
}

#[test]
fn adv_empty_payload() {
    assert!(!adv_contains_ftms_uuid(&[]));
}

#[test]
fn adv_malformed_length() {
    let data: &[u8] = &[0x0A, 0x03, 0x26];
    assert!(!adv_contains_ftms_uuid(data));
}

#[test]
fn adv_zero_length_ad_structure() {
    let data: &[u8] = &[0x00];
    assert!(!adv_contains_ftms_uuid(data));
}

// ---------------------------------------------------------------------------
// Unit tests for select_best_rssi
// Feature: refactoring — Validates: Requirements 5.2
// ---------------------------------------------------------------------------

#[test]
fn best_rssi_empty() {
    assert_eq!(select_best_rssi(&[]), None);
}

#[test]
fn best_rssi_single() {
    let addr = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
    assert_eq!(select_best_rssi(&[(addr, -50)]), Some(addr));
}

#[test]
fn best_rssi_picks_strongest() {
    let a1 = [0x01; 6];
    let a2 = [0x02; 6];
    let a3 = [0x03; 6];
    assert_eq!(select_best_rssi(&[(a1, -80), (a2, -40), (a3, -60)]), Some(a2));
}

#[test]
fn best_rssi_tie_returns_first() {
    let a1 = [0x01; 6];
    let a2 = [0x02; 6];
    assert_eq!(select_best_rssi(&[(a1, -50), (a2, -50)]), Some(a1));
}

// ---------------------------------------------------------------------------
// Unit tests for convert_ftms_to_rsc
// Feature: refactoring — Validates: Requirements 7.1
// ---------------------------------------------------------------------------

#[test]
fn convert_speed_only() {
    let ftms: &[u8] = &[0x00, 0x00, 0xE8, 0x03];
    let mut buf = [0u8; 8];
    let len = convert_ftms_to_rsc(ftms, &mut buf).unwrap();

    assert_eq!(len, 4);
    let rsc_speed = u16::from_le_bytes([buf[1], buf[2]]);
    assert_eq!(rsc_speed, protocol::ftms_speed_to_rsc(1000));
    assert_eq!(buf[0] & 0x02, 0); // no distance
    assert_ne!(buf[0] & 0x04, 0); // running
}

#[test]
fn convert_with_distance() {
    let ftms: &[u8] = &[0x04, 0x00, 0x20, 0x03, 0x88, 0x13, 0x00];
    let mut buf = [0u8; 8];
    let len = convert_ftms_to_rsc(ftms, &mut buf).unwrap();

    assert_eq!(len, 8);
    assert_ne!(buf[0] & 0x02, 0); // distance present
    let rsc_dist = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    assert_eq!(rsc_dist, protocol::ftms_distance_to_rsc(5000));
}

#[test]
fn convert_parse_error() {
    let ftms: &[u8] = &[0x00, 0x00, 0xE8];
    let mut buf = [0u8; 8];
    assert!(convert_ftms_to_rsc(ftms, &mut buf).is_err());
}

#[test]
fn convert_buffer_too_small() {
    let ftms: &[u8] = &[0x04, 0x00, 0x20, 0x03, 0x88, 0x13, 0x00];
    let mut buf = [0u8; 4];
    assert!(convert_ftms_to_rsc(ftms, &mut buf).is_err());
}

// Host-side example-based unit tests for the converter module.
// These validate specific edge cases and boundary conditions.

use treadlink_pbt::converter::*;

/// **Validates: Requirement 5.4**
/// Speed input of 0 must produce RSC speed of 0.
#[test]
fn speed_zero_returns_zero() {
    assert_eq!(convert_speed(0), 0);
}

/// **Validates: Requirement 5.3**
/// Maximum FTMS speed (65535) must produce a result that fits in u16.
/// Expected: (65535 * 32) / 45 = 46_602 (truncating division).
#[test]
fn speed_max_fits_u16() {
    let result = convert_speed(65535);
    assert_eq!(result, 46_602);
    assert!(result <= u16::MAX);
}

/// Known-value sanity check: 10.00 km/h = 1000 in FTMS units.
/// Expected: (1000 * 32) / 45 = 711 (truncated).
#[test]
fn speed_known_value() {
    assert_eq!(convert_speed(1000), 711);
}

/// **Validates: Requirement 6.2**
/// Maximum uint24 distance (16_777_215 metres) must produce a result that fits in u32.
/// Expected: 16_777_215 * 10 = 167_772_150.
#[test]
fn distance_max_fits_u32() {
    let result = convert_distance(16_777_215);
    assert_eq!(result, 167_772_150);
    assert!(result <= u32::MAX);
}

/// Distance input of 0 must produce RSC distance of 0.
#[test]
fn distance_zero_returns_zero() {
    assert_eq!(convert_distance(0), 0);
}

// Host-side tests for the converter module.
// These are compiled only for the host target, bypassing no_std constraints.
//
// NOTE: Property-based tests for the converter module live in tests/pbt/tests/converter_pbt.rs
// because the main crate's embedded toolchain prevents proptest from compiling here.

#[path = "../src/protocol.rs"]
mod protocol;

#[path = "../src/converter.rs"]
mod converter;

use converter::*;

#[test]
fn speed_zero_returns_zero() {
    assert_eq!(convert_speed(0), 0);
}

#[test]
fn speed_max_fits_u16() {
    let result = convert_speed(65535);
    // (65535 * 32) / 45 = 46_602 (truncating division)
    assert_eq!(result, 46_602);
    assert!(result <= u16::MAX);
}

#[test]
fn speed_known_value() {
    // 10.00 km/h = 1000 in FTMS units
    // Expected: (1000 * 32) / 45 = 32000 / 45 = 711 (truncated)
    assert_eq!(convert_speed(1000), 711);
}

#[test]
fn distance_max_fits_u32() {
    let result = convert_distance(16_777_215);
    assert_eq!(result, 167_772_150);
}

#[test]
fn distance_zero_returns_zero() {
    assert_eq!(convert_distance(0), 0);
}

// Feature: ftms-rsc-bridge, Property 5: Speed conversion correctness and range
// Validates: Requirements 5.1, 5.2, 5.3, 5.4
mod prop_speed_conversion {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn speed_conversion_correctness(s in 0u16..=65535) {
            let result = convert_speed(s);
            let expected = ((s as u32 * 32) / 45) as u16;
            prop_assert_eq!(result, expected);
            prop_assert!(result <= u16::MAX);
        }
    }
}

// Feature: ftms-rsc-bridge, Property 1: FTMS Treadmill Data round-trip
//
// Host-side property-based test for the FTMS Treadmill Data round-trip property.
// Validates that serializing and parsing TreadmillData is lossless.

use proptest::prelude::*;
use treadlink_pbt::protocol::{
    parse_rsc_measurement, parse_treadmill_data, serialize_rsc_measurement,
    serialize_treadmill_data, ParseError, RscMeasurement, TreadmillData,
};

prop_compose! {
    fn arb_treadmill_data()(
        speed in proptest::option::of(0u16..=65535u16),
        total_distance in proptest::option::of(0u32..=16_777_215u32),
    ) -> TreadmillData {
        // Compute flags as serialize would:
        // Bit 0: 0 = speed present, 1 = speed NOT present
        // Bit 2: 1 = total distance present
        let mut flags: u16 = 0;
        if speed.is_none() { flags |= 0x0001; }
        if total_distance.is_some() { flags |= 0x0004; }
        TreadmillData { flags, speed, total_distance }
    }
}

// Feature: ftms-rsc-bridge, Property 3: RSC Measurement round-trip
prop_compose! {
    fn arb_rsc_measurement()(
        speed in 0u16..=65535,
        cadence in 0u8..=255,
        stride_length in proptest::option::of(0u16..=65535),
        total_distance in proptest::option::of(proptest::num::u32::ANY),
        walking_running in 0u8..=1,
    ) -> RscMeasurement {
        let mut flags: u8 = 0;
        if stride_length.is_some() { flags |= 0x01; }
        if total_distance.is_some() { flags |= 0x02; }
        flags |= walking_running << 2;
        RscMeasurement { flags, speed, cadence, stride_length, total_distance }
    }
}

proptest! {
    /// **Validates: Requirements 3.1, 3.2, 3.3, 3.5, 4.1, 4.2, 4.3, 4.4, 4.5**
    ///
    /// For any valid TreadmillData (speed in 0..=65535, total_distance in 0..=16_777_215),
    /// serializing to bytes and parsing back produces the original value.
    #[test]
    fn ftms_treadmill_data_round_trip(data in arb_treadmill_data()) {
        let serialized = serialize_treadmill_data(&data);
        let parsed = parse_treadmill_data(&serialized);
        prop_assert_eq!(parsed, Ok(data));
    }

    // Feature: ftms-rsc-bridge, Property 3: RSC Measurement round-trip
    /// **Validates: Requirements 8.1, 8.2, 8.3, 8.4, 8.5, 9.1, 9.2, 9.3, 9.4, 9.5, 9.6**
    ///
    /// For any valid RscMeasurement (speed in 0..=65535, cadence in 0..=255,
    /// optional stride_length in 0..=65535, optional total_distance in 0..=u32::MAX,
    /// walking_or_running in {0,1}), serializing to bytes and parsing back produces
    /// the original value.
    #[test]
    fn rsc_measurement_round_trip(data in arb_rsc_measurement()) {
        let serialized = serialize_rsc_measurement(&data);
        let parsed = parse_rsc_measurement(&serialized);
        prop_assert_eq!(parsed, Ok(data));
    }
}

// Feature: ftms-rsc-bridge, Property 2: FTMS parse rejects truncated data
proptest! {
    /// **Validates: Requirements 3.4**
    #[test]
    fn ftms_parse_rejects_truncated(data in arb_treadmill_data()) {
        let serialized = serialize_treadmill_data(&data);
        for len in 0..serialized.len() {
            let result = parse_treadmill_data(&serialized[..len]);
            prop_assert!(matches!(result, Err(ParseError::InsufficientData { .. })),
                "Expected InsufficientData for truncation to {} bytes, got {:?}", len, result);
        }
    }
}

// Feature: ftms-rsc-bridge, Property 4: RSC parse rejects truncated data
proptest! {
    /// **Validates: Requirements 9.7**
    #[test]
    fn rsc_parse_rejects_truncated(data in arb_rsc_measurement()) {
        let serialized = serialize_rsc_measurement(&data);
        for len in 0..serialized.len() {
            let result = parse_rsc_measurement(&serialized[..len]);
            prop_assert!(matches!(result, Err(ParseError::InsufficientData { .. })),
                "Expected InsufficientData for truncation to {} bytes, got {:?}", len, result);
        }
    }
}

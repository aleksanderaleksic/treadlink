// Feature: ftms-rsc-bridge, Property 7: RSC assembly invariants
//
// Host-side property-based test for the RSC assembly invariants.
// Validates that assemble_rsc produces correct flags, cadence, and distance presence.

use proptest::prelude::*;
use treadlink_pbt::converter::assemble_rsc;
use treadlink_pbt::protocol::TreadmillData;

prop_compose! {
    fn arb_treadmill_data()(
        speed in proptest::option::of(0u16..=65535u16),
        total_distance in proptest::option::of(0u32..=16_777_215u32),
    ) -> TreadmillData {
        let mut flags: u16 = 0;
        if speed.is_none() { flags |= 0x0001; }
        if total_distance.is_some() { flags |= 0x0004; }
        TreadmillData { flags, speed, total_distance }
    }
}

proptest! {
    /// **Validates: Requirements 6.3, 7.1, 7.2, 7.3, 7.4, 7.5**
    ///
    /// For any valid TreadmillData input, the RscMeasurement produced by assemble_rsc
    /// satisfies all assembly invariants:
    /// - cadence == 0
    /// - flags & 0x01 == 0 (stride length never present)
    /// - flags & 0x04 == 0 (walking/running status always 0)
    /// - total_distance.is_some() iff input had total_distance.is_some()
    /// - flags bit 1 matches total_distance presence
    #[test]
    fn rsc_assembly_invariants(input in arb_treadmill_data()) {
        let result = assemble_rsc(&input);

        // Cadence always 0 (Req 7.2)
        prop_assert_eq!(result.cadence, 0);
        // Stride length never present (Req 7.3)
        prop_assert_eq!(result.flags & 0x01, 0);
        // Walking/running always 0 (Req 7.4)
        prop_assert_eq!(result.flags & 0x04, 0);
        // Distance presence matches input (Req 6.3, 7.5)
        prop_assert_eq!(result.total_distance.is_some(), input.total_distance.is_some());
        // Flags bit 1 matches distance presence (Req 7.1)
        if result.total_distance.is_some() {
            prop_assert_eq!(result.flags & 0x02, 0x02);
        } else {
            prop_assert_eq!(result.flags & 0x02, 0);
        }
    }
}

// Feature: ftms-rsc-bridge, Property 5: Speed conversion correctness and range
mod prop_speed_conversion {
    use proptest::prelude::*;
    use treadlink_pbt::converter::convert_speed;

    proptest! {
        /// **Validates: Requirements 5.1, 5.2, 5.3, 5.4**
        ///
        /// For any FTMS speed value s in 0..=65535, convert_speed(s) equals
        /// (s as u32 * 32) / 45 (truncating integer division) and the result fits in u16.
        #[test]
        fn speed_conversion_correctness(s in 0u16..=65535) {
            let result = convert_speed(s);
            let expected = ((s as u32 * 32) / 45) as u16;
            prop_assert_eq!(result, expected);
            prop_assert!(result <= u16::MAX);
        }
    }
}

// Feature: ftms-rsc-bridge, Property 6: Distance conversion correctness
mod prop_distance_conversion {
    use proptest::prelude::*;
    use treadlink_pbt::converter::convert_distance;

    proptest! {
        /// **Validates: Requirements 6.1, 6.2**
        ///
        /// For any FTMS distance value d in 0..=16_777_215 (uint24 range),
        /// convert_distance(d) equals d * 10 and the result fits in u32.
        #[test]
        fn distance_conversion_correctness(d in 0u32..=16_777_215) {
            let result = convert_distance(d);
            prop_assert_eq!(result, d * 10);
            // Max: 16_777_215 * 10 = 167_772_150, fits u32
        }
    }
}

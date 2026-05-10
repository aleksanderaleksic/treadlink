// Feature: ftms-rsc-bridge, Property 9: SC Control Point rejects unsupported opcodes
//
// Host-side property-based test for the SC Control Point handler.
// Validates that any opcode != 0x01 produces a "not supported" response.

use proptest::prelude::*;
use treadlink_pbt::peripheral::{handle_sc_control_point, ScCpResponse};

proptest! {
    /// **Validates: Requirements 13.2**
    ///
    /// For any opcode byte `op` where `op != 0x01`, the SC Control Point handler
    /// with cccd_configured=true and procedure_in_progress=false SHALL produce
    /// a response of [0x10, op, 0x02] (Response Code, Request Op Code, Op Code Not Supported).
    #[test]
    fn sc_cp_rejects_unsupported_opcodes(op in 0u8..=255u8) {
        prop_assume!(op != 0x01);
        let result = handle_sc_control_point(op, true, false);
        prop_assert_eq!(result, ScCpResponse::Indication([0x10, op, 0x02]));
    }
}

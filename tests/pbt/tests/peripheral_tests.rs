use treadlink_pbt::peripheral::{handle_sc_control_point, ScCpResponse};

/// Validates: Requirements 13.1
/// WHEN the Consumer writes Set Cumulative Value (0x01) with CCCD configured and no procedure
/// in progress, the handler SHALL respond with [0x10, 0x01, 0x01] (Success).
#[test]
fn sc_cp_set_cumulative_value_success() {
    let result = handle_sc_control_point(0x01, true, false);
    assert_eq!(result, ScCpResponse::Indication([0x10, 0x01, 0x01]));
}

/// Validates: Requirements 13.3
/// IF the Consumer writes to the SC_Control_Point while the CCCD is not configured for
/// indications, the handler SHALL reject with ATT error 0x81 (CCC Improperly Configured).
#[test]
fn sc_cp_cccd_not_configured() {
    let result = handle_sc_control_point(0x01, false, false);
    assert_eq!(result, ScCpResponse::AttError(0x81));
}

/// Validates: Requirements 13.4
/// IF the Consumer writes to the SC_Control_Point while a previously initiated procedure has
/// not yet completed, the handler SHALL reject with ATT error 0x80 (Procedure Already In Progress).
#[test]
fn sc_cp_procedure_in_progress() {
    let result = handle_sc_control_point(0x01, true, true);
    assert_eq!(result, ScCpResponse::AttError(0x80));
}

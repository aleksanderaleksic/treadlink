// Host-side mirror of the SC Control Point handler for property-based testing.
// This replicates the pure function logic from src/peripheral.rs without GATT macros.

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ScCpResponse {
    AttError(u8),
    Indication([u8; 3]),
}

pub fn handle_sc_control_point(opcode: u8, cccd_configured: bool, procedure_in_progress: bool) -> ScCpResponse {
    if !cccd_configured {
        return ScCpResponse::AttError(0x81);
    }
    if procedure_in_progress {
        return ScCpResponse::AttError(0x80);
    }
    if opcode == 0x01 {
        ScCpResponse::Indication([0x10, 0x01, 0x01])
    } else {
        ScCpResponse::Indication([0x10, opcode, 0x02])
    }
}

// Mirror of src/converter.rs for host-side testing (without defmt/no_std dependencies).

use crate::protocol::{RscMeasurement, TreadmillData};

/// Convert FTMS speed (0.01 km/h units) to RSC speed (1/256 m/s units).
///
/// Uses u32 intermediate to avoid overflow on the multiplication,
/// then truncating integer division.
///
/// The maximum result is (65535 * 32) / 45 = 46_602, which fits in u16.
pub fn convert_speed(ftms_speed: u16) -> u16 {
    ((ftms_speed as u32 * 32) / 45) as u16
}

/// Convert FTMS distance (metres, uint24 range) to RSC distance (1/10 metres).
///
/// Maximum input 16_777_215 × 10 = 167_772_150, which fits in u32.
pub fn convert_distance(ftms_metres: u32) -> u32 {
    ftms_metres * 10
}

/// Assemble an RscMeasurement from parsed FTMS treadmill data.
///
/// - Speed is converted via `convert_speed`; if absent in input, treated as 0.
/// - Distance is converted via `convert_distance` if present.
/// - Cadence is always 0, stride_length is always None.
/// - Flags: bit 0 = 0 (no stride length), bit 1 = distance present, bit 2 = 0 (walking/running).
pub fn assemble_rsc(data: &TreadmillData) -> RscMeasurement {
    let speed = convert_speed(data.speed.unwrap_or(0));
    let total_distance = data.total_distance.map(convert_distance);

    let flags: u8 = (total_distance.is_some() as u8) << 1;

    RscMeasurement {
        flags,
        speed,
        cadence: 0,
        stride_length: None,
        total_distance,
    }
}

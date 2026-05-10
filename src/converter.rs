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

    let flags: u8 = 0x03; // stride length + total distance present (matching working ESP32 reference)

    RscMeasurement {
        flags,
        speed,
        cadence: 0,
        stride_length: Some(100),
        total_distance: data.total_distance.map(convert_distance),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn distance_max_fits_u32() {
        let result = convert_distance(16_777_215);
        assert_eq!(result, 167_772_150);
        assert!(result <= u32::MAX);
    }

    #[test]
    fn distance_zero_returns_zero() {
        assert_eq!(convert_distance(0), 0);
    }

    #[test]
    fn speed_known_value() {
        // 10.00 km/h = 1000 in FTMS units
        // Expected: (1000 * 32) / 45 = 32000 / 45 = 711 (truncated)
        assert_eq!(convert_speed(1000), 711);
    }
}

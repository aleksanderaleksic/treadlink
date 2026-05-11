use libm::round;

/// Convert FTMS speed (0.01 km/h) to RSC speed (1/256 m/s).
///
/// Saturates at `u16::MAX` to guard against malformed data.
pub fn ftms_speed_to_rsc(ftms_speed_raw: u16) -> u16 {
    let speed_ms: f64 = ftms_speed_raw as f64 * 0.01 / 3.6;
    let rsc = round(speed_ms * 256.0);
    if rsc > u16::MAX as f64 {
        u16::MAX
    } else if rsc < 0.0 {
        0
    } else {
        rsc as u16
    }
}

/// Convert FTMS total distance (1 m) to RSC total distance (1/10 m).
pub fn ftms_distance_to_rsc(ftms_distance_m: u32) -> u32 {
    ftms_distance_m * 10
}

/// Estimate cadence (steps/min) from speed (m/s) and stride length (m).
///
/// Returns 0 if `stride_length` is zero or negative.
pub fn estimate_cadence(speed_ms: f64, stride_length: f64) -> u8 {
    if stride_length <= 0.0 {
        return 0;
    }
    let cadence = libm::fmin(round(speed_ms / stride_length * 60.0), 255.0);
    if cadence < 0.0 {
        0
    } else {
        cadence as u8
    }
}

/// Helper: convert FTMS speed raw (0.01 km/h) to m/s as f64.
pub fn ftms_speed_to_ms(ftms_speed_raw: u16) -> f64 {
    ftms_speed_raw as f64 * 0.01 / 3.6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_conversion_10kmh() {
        // 10.00 km/h = 1000 raw → 2.778 m/s → 711.11 → 711
        let rsc = ftms_speed_to_rsc(1000);
        assert_eq!(rsc, 711);
    }

    #[test]
    fn speed_conversion_zero() {
        assert_eq!(ftms_speed_to_rsc(0), 0);
    }

    #[test]
    fn speed_conversion_high_value() {
        // 40.00 km/h = 4000 raw → 11.111 m/s → 2844.44 → 2844
        let rsc = ftms_speed_to_rsc(4000);
        assert_eq!(rsc, 2844);
    }

    #[test]
    fn speed_saturates_at_max() {
        // u16::MAX as FTMS speed = 655.35 km/h → absurd but should not wrap
        let rsc = ftms_speed_to_rsc(u16::MAX);
        // 655.35 km/h = 182.04 m/s → 46602 — fits u16, but verify no overflow
        assert!(rsc > 0);
        assert!(rsc <= u16::MAX);
    }

    #[test]
    fn distance_conversion() {
        assert_eq!(ftms_distance_to_rsc(5000), 50000);
        assert_eq!(ftms_distance_to_rsc(0), 0);
        assert_eq!(ftms_distance_to_rsc(1), 10);
    }

    #[test]
    fn cadence_estimation() {
        // 2.778 m/s with 0.8m stride → 208.3 → 208 steps/min
        let c = estimate_cadence(2.778, 0.8);
        assert_eq!(c, 208);
    }

    #[test]
    fn cadence_zero_stride() {
        assert_eq!(estimate_cadence(2.778, 0.0), 0);
    }

    #[test]
    fn cadence_negative_stride() {
        assert_eq!(estimate_cadence(2.778, -1.0), 0);
    }

    #[test]
    fn cadence_saturates_at_255() {
        // Very high speed, short stride
        let c = estimate_cadence(10.0, 0.1);
        assert_eq!(c, 255);
    }

    #[test]
    fn ftms_speed_to_ms_conversion() {
        let ms = ftms_speed_to_ms(1000);
        assert!((ms - 2.7778).abs() < 0.001);
    }
}

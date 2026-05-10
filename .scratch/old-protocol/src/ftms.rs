use crate::error::ParseError;

/// Parsed FTMS Treadmill Data (characteristic 0x2ACD).
///
/// Only the fields needed for Phase 1 are extracted. The parser correctly
/// skips all optional fields defined by the flags bitfield.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtmsTreadmillData {
    /// Instantaneous speed in units of 0.01 km/h.
    pub speed_raw: u16,
    /// Total distance in meters (from uint24 field). `None` if not present.
    pub total_distance_m: Option<u32>,
    /// Inclination in units of 0.1%. `None` if not present.
    pub inclination_raw: Option<i16>,
}

/// Minimum valid payload: 2 bytes flags + 2 bytes speed.
const MIN_LEN: usize = 4;

impl FtmsTreadmillData {
    /// Parse an FTMS Treadmill Data notification payload.
    ///
    /// The parser reads the flags bitfield, extracts the always-present speed,
    /// then walks through bits 1–12 to skip or extract optional fields.
    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() < MIN_LEN {
            return Err(ParseError::TooShort {
                expected: MIN_LEN,
                actual: data.len(),
            });
        }

        let flags = u16::from_le_bytes([data[0], data[1]]);
        let speed_raw = u16::from_le_bytes([data[2], data[3]]);

        let mut offset: usize = 4;
        let mut total_distance_m: Option<u32> = None;
        let mut inclination_raw: Option<i16> = None;

        // Bit 1: Average Speed (uint16)
        if flags & (1 << 1) != 0 {
            offset = advance(data, offset, 2)?;
        }

        // Bit 2: Total Distance (uint24)
        if flags & (1 << 2) != 0 {
            check_remaining(data, offset, 3)?;
            total_distance_m = Some(read_u24_le(data, offset));
            offset += 3;
        }

        // Bit 3: Inclination (sint16) + Ramp Angle (sint16)
        if flags & (1 << 3) != 0 {
            check_remaining(data, offset, 4)?;
            inclination_raw = Some(i16::from_le_bytes([data[offset], data[offset + 1]]));
            offset += 4; // skip both inclination and ramp angle
        }

        // Bit 4: Positive Elevation Gain (uint16) + Negative Elevation Gain (uint16)
        if flags & (1 << 4) != 0 {
            offset = advance(data, offset, 4)?;
        }

        // Bit 5: Instantaneous Pace (uint8)
        if flags & (1 << 5) != 0 {
            offset = advance(data, offset, 1)?;
        }

        // Bit 6: Average Pace (uint8)
        if flags & (1 << 6) != 0 {
            offset = advance(data, offset, 1)?;
        }

        // Bit 7: Expended Energy: Total (uint16) + Per Hour (uint16) + Per Minute (uint8)
        if flags & (1 << 7) != 0 {
            offset = advance(data, offset, 5)?;
        }

        // Bit 8: Heart Rate (uint8)
        if flags & (1 << 8) != 0 {
            offset = advance(data, offset, 1)?;
        }

        // Bit 9: Metabolic Equivalent (uint8)
        if flags & (1 << 9) != 0 {
            offset = advance(data, offset, 1)?;
        }

        // Bit 10: Elapsed Time (uint16)
        if flags & (1 << 10) != 0 {
            offset = advance(data, offset, 2)?;
        }

        // Bit 11: Remaining Time (uint16)
        if flags & (1 << 11) != 0 {
            offset = advance(data, offset, 2)?;
        }

        // Bit 12: Force on Belt (sint16) + Power Output (sint16)
        if flags & (1 << 12) != 0 {
            offset = advance(data, offset, 4)?;
        }

        let _ = offset; // suppress unused warning

        Ok(Self {
            speed_raw,
            total_distance_m,
            inclination_raw,
        })
    }
}

/// Read a uint24 little-endian value at the given offset.
fn read_u24_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], 0])
}

/// Check that `needed` bytes are available at `offset`.
fn check_remaining(data: &[u8], offset: usize, needed: usize) -> Result<(), ParseError> {
    if offset + needed > data.len() {
        return Err(ParseError::UnexpectedEnd {
            offset,
            needed,
            remaining: data.len().saturating_sub(offset),
        });
    }
    Ok(())
}

/// Skip `size` bytes at `offset`, returning the new offset.
fn advance(data: &[u8], offset: usize, size: usize) -> Result<usize, ParseError> {
    check_remaining(data, offset, size)?;
    Ok(offset + size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_speed_only() {
        // Flags: 0x0000 (no optional fields), Speed: 1000 = 10.00 km/h
        let data = [0x00, 0x00, 0xE8, 0x03];
        let result = FtmsTreadmillData::parse(&data).unwrap();
        assert_eq!(result.speed_raw, 1000);
        assert_eq!(result.total_distance_m, None);
        assert_eq!(result.inclination_raw, None);
    }

    #[test]
    fn parse_with_distance() {
        // Flags: bit 2 set (Total Distance present)
        // Speed: 800 = 8.00 km/h, Distance: 5000 m (uint24 LE)
        let data = [0x04, 0x00, 0x20, 0x03, 0x88, 0x13, 0x00];
        let result = FtmsTreadmillData::parse(&data).unwrap();
        assert_eq!(result.speed_raw, 800);
        assert_eq!(result.total_distance_m, Some(5000));
        assert_eq!(result.inclination_raw, None);
    }

    #[test]
    fn parse_with_distance_and_inclination() {
        // Flags: bits 2 and 3 set (distance + inclination)
        // Speed: 1200, Distance: 3000 m, Inclination: 50 (5.0%), Ramp Angle: 30
        let data = [
            0x0C, 0x00, // flags
            0xB0, 0x04, // speed = 1200
            0xB8, 0x0B, 0x00, // distance = 3000
            0x32, 0x00, // inclination = 50
            0x1E, 0x00, // ramp angle = 30 (skipped)
        ];
        let result = FtmsTreadmillData::parse(&data).unwrap();
        assert_eq!(result.speed_raw, 1200);
        assert_eq!(result.total_distance_m, Some(3000));
        assert_eq!(result.inclination_raw, Some(50));
    }

    #[test]
    fn parse_skips_average_speed_before_distance() {
        // Flags: bits 1 and 2 set (average speed + distance)
        // Speed: 1000, Avg Speed: 950 (skipped), Distance: 2000
        let data = [
            0x06, 0x00, // flags: bits 1+2
            0xE8, 0x03, // speed = 1000
            0xB6, 0x03, // avg speed = 950 (skipped)
            0xD0, 0x07, 0x00, // distance = 2000
        ];
        let result = FtmsTreadmillData::parse(&data).unwrap();
        assert_eq!(result.speed_raw, 1000);
        assert_eq!(result.total_distance_m, Some(2000));
    }

    #[test]
    fn parse_negative_inclination() {
        // Flags: bit 3 set (inclination)
        // Speed: 600, Inclination: -10 (-1.0%), Ramp Angle: 0
        let data = [
            0x08, 0x00, // flags: bit 3
            0x58, 0x02, // speed = 600
            0xF6, 0xFF, // inclination = -10 (signed)
            0x00, 0x00, // ramp angle = 0
        ];
        let result = FtmsTreadmillData::parse(&data).unwrap();
        assert_eq!(result.speed_raw, 600);
        assert_eq!(result.inclination_raw, Some(-10));
    }

    #[test]
    fn parse_too_short() {
        let data = [0x00, 0x00, 0xE8];
        let result = FtmsTreadmillData::parse(&data);
        assert_eq!(
            result,
            Err(ParseError::TooShort {
                expected: 4,
                actual: 3
            })
        );
    }

    #[test]
    fn parse_flags_exceed_payload() {
        // Flags claim distance is present, but payload is only 4 bytes (speed only)
        let data = [0x04, 0x00, 0xE8, 0x03];
        let result = FtmsTreadmillData::parse(&data);
        assert!(matches!(result, Err(ParseError::UnexpectedEnd { .. })));
    }

    #[test]
    fn parse_all_fields_before_distance() {
        // Flags: bits 1, 2 set. Verify correct offset calculation
        // when skipping average speed to reach distance.
        let data = [
            0x06, 0x00, // flags: bits 1+2
            0xE8, 0x03, // speed = 1000
            0x00, 0x00, // avg speed (skipped)
            0x01, 0x00, 0x00, // distance = 1
        ];
        let result = FtmsTreadmillData::parse(&data).unwrap();
        assert_eq!(result.total_distance_m, Some(1));
    }
}

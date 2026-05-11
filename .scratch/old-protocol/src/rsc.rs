use crate::error::ParseError;

/// RSC Measurement data to encode for characteristic 0x2A53.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RscMeasurement {
    /// Instantaneous speed in units of 1/256 m/s.
    pub speed_raw: u16,
    /// Instantaneous cadence in steps/min. 0 if not estimated.
    pub cadence: u8,
    /// Total distance in units of 1/10 m. `None` to omit.
    pub total_distance: Option<u32>,
}

impl RscMeasurement {
    /// Encode into the RSC Measurement wire format.
    ///
    /// Returns the number of bytes written, or an error if the buffer is too small.
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, ParseError> {
        let has_distance = self.total_distance.is_some();
        let is_running = self.speed_raw > 0;

        let needed = if has_distance { 8 } else { 4 };
        if buf.len() < needed {
            return Err(ParseError::TooShort {
                expected: needed,
                actual: buf.len(),
            });
        }

        // Flags byte
        let mut flags: u8 = 0;
        if has_distance {
            flags |= 0x02; // bit 1: Total Distance Present
        }
        if is_running {
            flags |= 0x04; // bit 2: Running
        }
        buf[0] = flags;

        // Speed (uint16 LE)
        let speed_bytes = self.speed_raw.to_le_bytes();
        buf[1] = speed_bytes[0];
        buf[2] = speed_bytes[1];

        // Cadence (uint8)
        buf[3] = self.cadence;

        // Total Distance (uint32 LE) — at offset 4 when stride length is absent
        if let Some(dist) = self.total_distance {
            let dist_bytes = dist.to_le_bytes();
            buf[4] = dist_bytes[0];
            buf[5] = dist_bytes[1];
            buf[6] = dist_bytes[2];
            buf[7] = dist_bytes[3];
        }

        Ok(needed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_speed_only() {
        let rsc = RscMeasurement {
            speed_raw: 710, // ~2.77 m/s
            cadence: 0,
            total_distance: None,
        };
        let mut buf = [0u8; 16];
        let len = rsc.encode(&mut buf).unwrap();
        assert_eq!(len, 4);
        assert_eq!(buf[0], 0x04); // running, no distance
        assert_eq!(u16::from_le_bytes([buf[1], buf[2]]), 710);
        assert_eq!(buf[3], 0); // cadence
    }

    #[test]
    fn encode_with_distance() {
        let rsc = RscMeasurement {
            speed_raw: 512, // 2.0 m/s
            cadence: 170,
            total_distance: Some(50000), // 5000.0 m
        };
        let mut buf = [0u8; 16];
        let len = rsc.encode(&mut buf).unwrap();
        assert_eq!(len, 8);
        assert_eq!(buf[0], 0x06); // running + distance
        assert_eq!(u16::from_le_bytes([buf[1], buf[2]]), 512);
        assert_eq!(buf[3], 170);
        assert_eq!(u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]), 50000);
    }

    #[test]
    fn encode_zero_speed_not_running() {
        let rsc = RscMeasurement {
            speed_raw: 0,
            cadence: 0,
            total_distance: None,
        };
        let mut buf = [0u8; 16];
        let len = rsc.encode(&mut buf).unwrap();
        assert_eq!(len, 4);
        assert_eq!(buf[0], 0x00); // not running, no distance
    }

    #[test]
    fn encode_buffer_too_small() {
        let rsc = RscMeasurement {
            speed_raw: 512,
            cadence: 0,
            total_distance: Some(1000),
        };
        let mut buf = [0u8; 6]; // needs 8
        let result = rsc.encode(&mut buf);
        assert!(matches!(result, Err(ParseError::TooShort { .. })));
    }
}

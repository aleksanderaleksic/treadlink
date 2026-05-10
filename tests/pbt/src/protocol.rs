// Mirror of src/protocol.rs for host-side testing (without defmt dependency).

use heapless::Vec;

/// Parsed FTMS Treadmill Data characteristic (0x2ACD)
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct TreadmillData {
    pub flags: u16,
    /// Instantaneous speed in 0.01 km/h units. Present when flags bit 0 is clear.
    pub speed: Option<u16>,
    /// Total distance in metres (uint24 stored in u32). Present when flags bit 2 is set.
    pub total_distance: Option<u32>,
}

/// Parsed RSC Measurement characteristic (0x2A53)
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct RscMeasurement {
    pub flags: u8,
    /// Instantaneous speed in 1/256 m/s units
    pub speed: u16,
    /// Instantaneous cadence in steps/min (always 0 for bridge)
    pub cadence: u8,
    /// Stride length in 1/100 m units. Present when flags bit 0 is set.
    pub stride_length: Option<u16>,
    /// Total distance in 1/10 m units. Present when flags bit 1 is set.
    pub total_distance: Option<u32>,
}

/// Errors returned when parsing BLE characteristic byte slices
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ParseError {
    /// The byte slice was shorter than required by the declared flags
    InsufficientData { expected: usize, actual: usize },
}

/// Parse an FTMS Treadmill Data characteristic notification (0x2ACD).
///
/// Wire format:
/// - Bytes 0-1: flags (u16 LE)
/// - Bit 0: 0 = instantaneous speed present, 1 = speed NOT present
/// - Bit 1: average speed present (2 bytes to skip)
/// - Bit 2: total distance present (3 bytes, uint24 LE)
pub fn parse_treadmill_data(bytes: &[u8]) -> Result<TreadmillData, ParseError> {
    if bytes.len() < 2 {
        return Err(ParseError::InsufficientData {
            expected: 2,
            actual: bytes.len(),
        });
    }

    let flags = u16::from_le_bytes([bytes[0], bytes[1]]);
    let mut offset: usize = 2;

    let speed_present = flags & 0x0001 == 0;
    let avg_speed_present = flags & 0x0002 != 0;
    let distance_present = flags & 0x0004 != 0;

    let mut expected = 2usize;
    if speed_present {
        expected += 2;
    }
    if avg_speed_present {
        expected += 2;
    }
    if distance_present {
        expected += 3;
    }

    if bytes.len() < expected {
        return Err(ParseError::InsufficientData {
            expected,
            actual: bytes.len(),
        });
    }

    let speed = if speed_present {
        let val = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;
        Some(val)
    } else {
        None
    };

    if avg_speed_present {
        offset += 2;
    }

    let total_distance = if distance_present {
        let val = bytes[offset] as u32
            | (bytes[offset + 1] as u32) << 8
            | (bytes[offset + 2] as u32) << 16;
        Some(val)
    } else {
        None
    };

    Ok(TreadmillData {
        flags,
        speed,
        total_distance,
    })
}

/// Serialize a `TreadmillData` structure back to bytes.
///
/// The serializer sets flags based on the struct fields:
/// - Bit 0 = 0 when speed is Some, bit 0 = 1 when speed is None
/// - Bit 2 = 1 when total_distance is Some
/// - Bit 1 is never set (we don't serialize average speed)
pub fn serialize_treadmill_data(data: &TreadmillData) -> Vec<u8, 7> {
    let mut buf: Vec<u8, 7> = Vec::new();

    let mut flags: u16 = 0;

    if data.speed.is_none() {
        flags |= 0x0001;
    }

    if data.total_distance.is_some() {
        flags |= 0x0004;
    }

    let flag_bytes = flags.to_le_bytes();
    let _ = buf.push(flag_bytes[0]);
    let _ = buf.push(flag_bytes[1]);

    if let Some(speed) = data.speed {
        let speed_bytes = speed.to_le_bytes();
        let _ = buf.push(speed_bytes[0]);
        let _ = buf.push(speed_bytes[1]);
    }

    if let Some(dist) = data.total_distance {
        let _ = buf.push((dist & 0xFF) as u8);
        let _ = buf.push(((dist >> 8) & 0xFF) as u8);
        let _ = buf.push(((dist >> 16) & 0xFF) as u8);
    }

    buf
}

/// Parse an RSC Measurement characteristic (0x2A53) byte slice.
pub fn parse_rsc_measurement(bytes: &[u8]) -> Result<RscMeasurement, ParseError> {
    const BASE_LEN: usize = 4;

    if bytes.len() < BASE_LEN {
        return Err(ParseError::InsufficientData {
            expected: BASE_LEN,
            actual: bytes.len(),
        });
    }

    let flags = bytes[0];
    let speed = u16::from_le_bytes([bytes[1], bytes[2]]);
    let cadence = bytes[3];

    let stride_length_present = flags & 0x01 != 0;
    let total_distance_present = flags & 0x02 != 0;

    let mut expected_len = BASE_LEN;
    if stride_length_present {
        expected_len += 2;
    }
    if total_distance_present {
        expected_len += 4;
    }

    if bytes.len() < expected_len {
        return Err(ParseError::InsufficientData {
            expected: expected_len,
            actual: bytes.len(),
        });
    }

    let mut offset = BASE_LEN;

    let stride_length = if stride_length_present {
        let val = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;
        Some(val)
    } else {
        None
    };

    let total_distance = if total_distance_present {
        let val = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        Some(val)
    } else {
        None
    };

    Ok(RscMeasurement {
        flags,
        speed,
        cadence,
        stride_length,
        total_distance,
    })
}

/// Serialize an `RscMeasurement` into the RSC Measurement characteristic wire format.
pub fn serialize_rsc_measurement(data: &RscMeasurement) -> heapless::Vec<u8, 10> {
    let mut buf = heapless::Vec::<u8, 10>::new();

    let mut flags: u8 = data.flags & 0x04;
    if data.stride_length.is_some() {
        flags |= 0x01;
    }
    if data.total_distance.is_some() {
        flags |= 0x02;
    }

    let _ = buf.push(flags);

    let speed_bytes = data.speed.to_le_bytes();
    let _ = buf.push(speed_bytes[0]);
    let _ = buf.push(speed_bytes[1]);

    let _ = buf.push(data.cadence);

    if let Some(stride) = data.stride_length {
        let stride_bytes = stride.to_le_bytes();
        let _ = buf.push(stride_bytes[0]);
        let _ = buf.push(stride_bytes[1]);
    }

    if let Some(dist) = data.total_distance {
        let dist_bytes = dist.to_le_bytes();
        let _ = buf.push(dist_bytes[0]);
        let _ = buf.push(dist_bytes[1]);
        let _ = buf.push(dist_bytes[2]);
        let _ = buf.push(dist_bytes[3]);
    }

    buf
}

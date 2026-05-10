use heapless::Vec;

/// Parsed FTMS Treadmill Data characteristic (0x2ACD)
#[derive(Debug, PartialEq, Clone, Copy, defmt::Format)]
pub struct TreadmillData {
    pub flags: u16,
    /// Instantaneous speed in 0.01 km/h units. Present when flags bit 0 is clear.
    pub speed: Option<u16>,
    /// Total distance in metres (uint24 stored in u32). Present when flags bit 2 is set.
    pub total_distance: Option<u32>,
}

/// Parsed RSC Measurement characteristic (0x2A53)
#[derive(Debug, PartialEq, Clone, Copy, defmt::Format)]
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
#[derive(Debug, PartialEq, Clone, Copy, defmt::Format)]
pub enum ParseError {
    /// The byte slice was shorter than required by the declared flags
    InsufficientData { expected: usize, actual: usize },
}

/// Parse an RSC Measurement characteristic (0x2A53) byte slice into an `RscMeasurement`.
///
/// Wire format:
/// - Byte 0: flags (u8)
/// - Bytes 1-2: instantaneous speed (u16 LE, 1/256 m/s)
/// - Byte 3: instantaneous cadence (u8, steps/min)
/// - If flags bit 0 set: next 2 bytes = stride length (u16 LE, 1/100 m)
/// - If flags bit 1 set: next 4 bytes = total distance (u32 LE, 1/10 m)
/// - Bit 2: walking/running status (informational, no extra bytes)
pub fn parse_rsc_measurement(bytes: &[u8]) -> Result<RscMeasurement, ParseError> {
    // Minimum payload is 4 bytes: flags + speed(2) + cadence(1)
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

    // Calculate required length based on flags
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
///
/// Output: [flags, speed_lo, speed_hi, cadence, stride_lo?, stride_hi?, dist_0?, dist_1?, dist_2?, dist_3?]
/// Maximum 10 bytes.
pub fn serialize_rsc_measurement(data: &RscMeasurement) -> heapless::Vec<u8, 10> {
    let mut buf = heapless::Vec::<u8, 10>::new();

    // Build flags byte: set bit 0 if stride_length present, bit 1 if total_distance present,
    // preserve bit 2 (walking/running) from the struct's flags field.
    let mut flags: u8 = data.flags & 0x04; // preserve walking/running status bit
    if data.stride_length.is_some() {
        flags |= 0x01;
    }
    if data.total_distance.is_some() {
        flags |= 0x02;
    }

    // Flags byte
    let _ = buf.push(flags);

    // Speed (u16 LE)
    let speed_bytes = data.speed.to_le_bytes();
    let _ = buf.push(speed_bytes[0]);
    let _ = buf.push(speed_bytes[1]);

    // Cadence (u8)
    let _ = buf.push(data.cadence);

    // Optional stride length (u16 LE)
    if let Some(stride) = data.stride_length {
        let stride_bytes = stride.to_le_bytes();
        let _ = buf.push(stride_bytes[0]);
        let _ = buf.push(stride_bytes[1]);
    }

    // Optional total distance (u32 LE)
    if let Some(dist) = data.total_distance {
        let dist_bytes = dist.to_le_bytes();
        let _ = buf.push(dist_bytes[0]);
        let _ = buf.push(dist_bytes[1]);
        let _ = buf.push(dist_bytes[2]);
        let _ = buf.push(dist_bytes[3]);
    }

    buf
}

/// Parse an FTMS Treadmill Data characteristic notification (0x2ACD).
///
/// Wire format:
/// - Bytes 0-1: flags (u16 LE)
/// - Bit 0: 0 = instantaneous speed present, 1 = speed NOT present
/// - Bit 1: average speed present (2 bytes to skip)
/// - Bit 2: total distance present (3 bytes, uint24 LE)
pub fn parse_treadmill_data(bytes: &[u8]) -> Result<TreadmillData, ParseError> {
    // Need at least 2 bytes for flags
    if bytes.len() < 2 {
        return Err(ParseError::InsufficientData {
            expected: 2,
            actual: bytes.len(),
        });
    }

    let flags = u16::from_le_bytes([bytes[0], bytes[1]]);
    let mut offset: usize = 2;

    // Calculate required length based on flags
    let speed_present = flags & 0x0001 == 0; // bit 0 clear = speed present
    let avg_speed_present = flags & 0x0002 != 0; // bit 1 set = average speed present
    let distance_present = flags & 0x0004 != 0; // bit 2 set = total distance present

    let mut expected = 2usize; // flags
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

    // Extract speed if present
    let speed = if speed_present {
        let val = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;
        Some(val)
    } else {
        None
    };

    // Skip average speed if present
    if avg_speed_present {
        offset += 2;
    }

    // Extract total distance (uint24 LE) if present
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
/// Output format: [flags_lo, flags_hi, speed_lo, speed_hi (if present), dist_0, dist_1, dist_2 (if present)]
///
/// The serializer sets flags based on the struct fields:
/// - Bit 0 = 0 when speed is Some, bit 0 = 1 when speed is None
/// - Bit 2 = 1 when total_distance is Some
/// - Bit 1 is never set (we don't serialize average speed)
pub fn serialize_treadmill_data(data: &TreadmillData) -> Vec<u8, 7> {
    let mut buf: Vec<u8, 7> = Vec::new();

    let mut flags: u16 = 0;

    // Bit 0: 0 = speed present, 1 = speed not present
    if data.speed.is_none() {
        flags |= 0x0001;
    }

    // Bit 2: 1 = total distance present
    if data.total_distance.is_some() {
        flags |= 0x0004;
    }

    // Encode flags (2 bytes LE)
    let flag_bytes = flags.to_le_bytes();
    let _ = buf.push(flag_bytes[0]);
    let _ = buf.push(flag_bytes[1]);

    // Encode speed if present
    if let Some(speed) = data.speed {
        let speed_bytes = speed.to_le_bytes();
        let _ = buf.push(speed_bytes[0]);
        let _ = buf.push(speed_bytes[1]);
    }

    // Encode total distance as uint24 LE if present
    if let Some(dist) = data.total_distance {
        let _ = buf.push((dist & 0xFF) as u8);
        let _ = buf.push(((dist >> 8) & 0xFF) as u8);
        let _ = buf.push(((dist >> 16) & 0xFF) as u8);
    }

    buf
}

/// BLE advertisement parsing and RSSI candidate selection.
///
/// These are pure functions used by the firmware for scanning and bridge
/// operations. They live in the `protocol` crate so they can be tested
/// on the host with proptest.

/// FTMS service UUID (0x1826) bytes in little-endian.
const FTMS_UUID_LE: [u8; 2] = [0x26, 0x18];

/// BLE AD type: Incomplete List of 16-bit Service Class UUIDs.
const AD_TYPE_INCOMPLETE_UUID16: u8 = 0x02;

/// BLE AD type: Complete List of 16-bit Service Class UUIDs.
const AD_TYPE_COMPLETE_UUID16: u8 = 0x03;

/// Default stride length in meters for cadence estimation.
const STRIDE_LENGTH: f64 = 0.80;

/// Check if advertisement data contains the FTMS service UUID 0x1826.
///
/// Iterates through AD structures in the payload. Each AD structure is:
///   `[length, type, data...]`
///
/// Looks for type 0x02 (Incomplete List of 16-bit Service UUIDs) or
/// type 0x03 (Complete List of 16-bit Service UUIDs) containing 0x1826
/// in little-endian format: `[0x26, 0x18]`.
pub fn adv_contains_ftms_uuid(data: &[u8]) -> bool {
    let mut i = 0;
    while i < data.len() {
        let len = data[i] as usize;
        if len == 0 {
            break;
        }
        if i + len >= data.len() {
            break;
        }
        let ad_type = data[i + 1];
        if ad_type == AD_TYPE_INCOMPLETE_UUID16 || ad_type == AD_TYPE_COMPLETE_UUID16 {
            let uuid_data = &data[i + 2..i + 1 + len];
            let mut j = 0;
            while j + 1 < uuid_data.len() {
                if uuid_data[j] == FTMS_UUID_LE[0] && uuid_data[j + 1] == FTMS_UUID_LE[1] {
                    return true;
                }
                j += 2;
            }
        }
        i += 1 + len;
    }
    false
}

/// Select the candidate with the strongest (least negative) RSSI.
/// Returns `None` if the slice is empty, or the first address on tie.
///
/// Uses `[u8; 6]` for the address type so this function works in both
/// `no_std` firmware (where the caller maps to/from `trouble_host::Address`)
/// and host tests.
pub fn select_best_rssi(candidates: &[([u8; 6], i8)]) -> Option<[u8; 6]> {
    let mut best: Option<([u8; 6], i8)> = None;
    for &(addr, rssi) in candidates {
        match best {
            Some((_, best_rssi)) if rssi <= best_rssi => {}
            _ => best = Some((addr, rssi)),
        }
    }
    best.map(|(addr, _)| addr)
}

/// Errors from the FTMS→RSC conversion pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    /// FTMS payload could not be parsed.
    Parse(crate::ParseError),
    /// RSC encode buffer too small.
    Encode(crate::ParseError),
}

impl core::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "FTMS parse error: {e}"),
            Self::Encode(e) => write!(f, "RSC encode error: {e}"),
        }
    }
}

/// Convert an FTMS Treadmill Data payload to RSC Measurement bytes.
///
/// Uses a fixed stride length of 0.80 m for cadence estimation.
/// Returns the number of bytes written to `buf`, or an error.
pub fn convert_ftms_to_rsc(ftms_payload: &[u8], buf: &mut [u8]) -> Result<usize, ConvertError> {
    let data = crate::FtmsTreadmillData::parse(ftms_payload).map_err(ConvertError::Parse)?;

    let rsc_speed = crate::ftms_speed_to_rsc(data.speed_raw);
    let rsc_distance = data.total_distance_m.map(crate::ftms_distance_to_rsc);
    let speed_ms = crate::ftms_speed_to_ms(data.speed_raw);
    let cadence = crate::estimate_cadence(speed_ms, STRIDE_LENGTH);

    let measurement = crate::RscMeasurement {
        speed_raw: rsc_speed,
        cadence,
        total_distance: rsc_distance,
    };
    measurement.encode(buf).map_err(ConvertError::Encode)
}

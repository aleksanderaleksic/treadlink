#![no_std]
#![warn(clippy::all)]

mod error;
mod ftms;
mod rsc;
mod convert;
pub mod ble;

pub use error::ParseError;
pub use ftms::FtmsTreadmillData;
pub use rsc::RscMeasurement;
pub use convert::{ftms_speed_to_rsc, ftms_distance_to_rsc, ftms_speed_to_ms, estimate_cadence};
pub use ble::{adv_contains_ftms_uuid, select_best_rssi, convert_ftms_to_rsc, ConvertError};

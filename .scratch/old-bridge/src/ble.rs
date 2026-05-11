use trouble_host::prelude::*;

/// RSC GATT Server — hosts the Running Speed and Cadence service (0x1814).
#[gatt_server]
pub(crate) struct Server {
    pub rsc: RscService,
}

/// Running Speed and Cadence service (UUID 0x1814).
#[gatt_service(uuid = "1814")]
pub(crate) struct RscService {
    /// RSC Measurement (UUID 0x2A53): notify + read.
    /// Max 8 bytes: 1 flags + 2 speed + 1 cadence + 4 distance.
    #[characteristic(uuid = "2A53", read, notify)]
    pub measurement: [u8; 8],

    /// RSC Feature (UUID 0x2A54): read-only.
    /// Value 0x0002 = Total Distance Supported.
    #[characteristic(uuid = "2A54", read, value = [0x02, 0x00])]
    pub feature: [u8; 2],
}

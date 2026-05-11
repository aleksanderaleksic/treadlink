use proptest::prelude::*;
use protocol::{adv_contains_ftms_uuid, select_best_rssi, convert_ftms_to_rsc};

// ---------------------------------------------------------------------------
// Generators for BLE advertisement data
// ---------------------------------------------------------------------------

const AD_TYPE_INCOMPLETE_16: u8 = 0x02;
const AD_TYPE_COMPLETE_16: u8 = 0x03;
const FTMS_LE: [u8; 2] = [0x26, 0x18];

fn ad_structure(ad_type: u8, payload: Vec<u8>) -> Vec<u8> {
    let len = 1 + payload.len();
    assert!(len <= 255);
    let mut out = Vec::with_capacity(1 + len);
    out.push(len as u8);
    out.push(ad_type);
    out.extend_from_slice(&payload);
    out
}

fn non_ftms_uuid_list() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        (0u8..=0xFF, 0u8..=0xFF).prop_filter("not FTMS UUID", |&(lo, hi)| {
            !(lo == FTMS_LE[0] && hi == FTMS_LE[1])
        }),
        0..=4,
    )
    .prop_map(|pairs| pairs.into_iter().flat_map(|(lo, hi)| [lo, hi]).collect())
}

fn ftms_uuid_list() -> impl Strategy<Value = Vec<u8>> {
    (
        prop::collection::vec(
            (0u8..=0xFF, 0u8..=0xFF).prop_filter("not FTMS", |&(lo, hi)| {
                !(lo == FTMS_LE[0] && hi == FTMS_LE[1])
            }),
            0..=3,
        ),
        prop::collection::vec(
            (0u8..=0xFF, 0u8..=0xFF).prop_filter("not FTMS", |&(lo, hi)| {
                !(lo == FTMS_LE[0] && hi == FTMS_LE[1])
            }),
            0..=3,
        ),
    )
        .prop_map(|(before, after)| {
            let mut payload = Vec::new();
            for (lo, hi) in &before { payload.push(*lo); payload.push(*hi); }
            payload.push(FTMS_LE[0]);
            payload.push(FTMS_LE[1]);
            for (lo, hi) in &after { payload.push(*lo); payload.push(*hi); }
            payload
        })
}

fn non_uuid_ad_structure() -> impl Strategy<Value = Vec<u8>> {
    let ad_type = prop_oneof![Just(0x01u8), Just(0x08), Just(0x09), Just(0xFF)];
    (ad_type, prop::collection::vec(any::<u8>(), 1..=10))
        .prop_map(|(t, payload)| ad_structure(t, payload))
}

fn adv_payload_with_ftms() -> impl Strategy<Value = Vec<u8>> {
    let uuid_type = prop_oneof![Just(AD_TYPE_INCOMPLETE_16), Just(AD_TYPE_COMPLETE_16)];
    (
        prop::collection::vec(non_uuid_ad_structure(), 0..=2),
        uuid_type,
        ftms_uuid_list(),
        prop::collection::vec(non_uuid_ad_structure(), 0..=2),
    )
        .prop_map(|(before, utype, uuid_payload, after)| {
            let mut data = Vec::new();
            for s in &before { data.extend_from_slice(s); }
            data.extend_from_slice(&ad_structure(utype, uuid_payload));
            for s in &after { data.extend_from_slice(s); }
            data
        })
}

fn adv_payload_without_ftms() -> impl Strategy<Value = Vec<u8>> {
    let uuid_struct = (
        prop_oneof![Just(AD_TYPE_INCOMPLETE_16), Just(AD_TYPE_COMPLETE_16)],
        non_ftms_uuid_list(),
    )
        .prop_map(|(t, payload)| ad_structure(t, payload));
    let any_struct = prop_oneof![non_uuid_ad_structure(), uuid_struct];
    prop::collection::vec(any_struct, 0..=4).prop_map(|structs| {
        structs.into_iter().flatten().collect()
    })
}

// ---------------------------------------------------------------------------
// Feature: refactoring, Property 1: FTMS UUID detection in advertisement data
// **Validates: Requirements 5.1**
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_ftms_uuid_detected_when_present(data in adv_payload_with_ftms()) {
        prop_assert!(adv_contains_ftms_uuid(&data),
            "Expected true for payload containing FTMS UUID: {:?}", data);
    }

    #[test]
    fn prop_ftms_uuid_not_detected_when_absent(data in adv_payload_without_ftms()) {
        prop_assert!(!adv_contains_ftms_uuid(&data),
            "Expected false for payload without FTMS UUID: {:?}", data);
    }
}

// ---------------------------------------------------------------------------
// Feature: refactoring, Property 2: Best RSSI selection returns the maximum
// **Validates: Requirements 5.2**
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_best_rssi_returns_maximum(
        candidates in prop::collection::vec(
            (prop::array::uniform6(any::<u8>()), any::<i8>()), 1..=16
        )
    ) {
        let result = select_best_rssi(&candidates);
        prop_assert!(result.is_some());
        let best_addr = result.unwrap();
        let max_rssi = candidates.iter().map(|(_, r)| *r).max().unwrap();
        let returned_rssi = candidates.iter()
            .find(|(a, _)| *a == best_addr).map(|(_, r)| *r).unwrap();
        prop_assert_eq!(returned_rssi, max_rssi);
    }
}

// ---------------------------------------------------------------------------
// Generators for FTMS payloads
// ---------------------------------------------------------------------------

fn ftms_payload_strategy() -> impl Strategy<Value = (Vec<u8>, u16, Option<u32>, bool)> {
    (
        any::<u16>(),           // speed_raw
        any::<bool>(),          // has_avg_speed (bit 1)
        any::<bool>(),          // has_distance (bit 2)
        any::<bool>(),          // has_inclination (bit 3)
        0u32..=16_777_215u32,   // distance value
        any::<u16>(),           // avg_speed filler
        any::<i16>(),           // inclination
        any::<i16>(),           // ramp_angle
    )
        .prop_map(|(speed, has_avg, has_dist, has_incl, distance, avg_speed, incl, ramp)| {
            let mut flags: u16 = 0;
            let mut payload = Vec::new();
            if has_avg { flags |= 1 << 1; }
            if has_dist { flags |= 1 << 2; }
            if has_incl { flags |= 1 << 3; }
            payload.extend_from_slice(&flags.to_le_bytes());
            payload.extend_from_slice(&speed.to_le_bytes());
            if has_avg { payload.extend_from_slice(&avg_speed.to_le_bytes()); }
            if has_dist {
                let d = distance.to_le_bytes();
                payload.extend_from_slice(&d[..3]);
            }
            if has_incl {
                payload.extend_from_slice(&incl.to_le_bytes());
                payload.extend_from_slice(&ramp.to_le_bytes());
            }
            let opt_dist = if has_dist { Some(distance) } else { None };
            (payload, speed, opt_dist, has_dist)
        })
}

// ---------------------------------------------------------------------------
// Feature: refactoring, Property 3: FTMS-to-RSC conversion preserves data
// **Validates: Requirements 7.1**
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_ftms_to_rsc_preserves_speed(
        (payload, speed_raw, _opt_dist, has_dist) in ftms_payload_strategy()
    ) {
        let mut buf = [0u8; 8];
        let len = convert_ftms_to_rsc(&payload, &mut buf).unwrap();
        let rsc_speed = u16::from_le_bytes([buf[1], buf[2]]);
        prop_assert_eq!(rsc_speed, protocol::ftms_speed_to_rsc(speed_raw));
        if has_dist { prop_assert_eq!(len, 8); } else { prop_assert_eq!(len, 4); }
    }

    #[test]
    fn prop_ftms_to_rsc_preserves_cadence(
        (payload, speed_raw, _opt_dist, _has_dist) in ftms_payload_strategy()
    ) {
        let mut buf = [0u8; 8];
        convert_ftms_to_rsc(&payload, &mut buf).unwrap();
        let expected = protocol::estimate_cadence(protocol::ftms_speed_to_ms(speed_raw), 0.80);
        prop_assert_eq!(buf[3], expected);
    }

    #[test]
    fn prop_ftms_to_rsc_preserves_distance(
        (payload, _speed_raw, opt_dist, has_dist) in ftms_payload_strategy()
    ) {
        let mut buf = [0u8; 8];
        let len = convert_ftms_to_rsc(&payload, &mut buf).unwrap();
        if has_dist {
            prop_assert_eq!(len, 8);
            prop_assert!(buf[0] & 0x02 != 0);
            let rsc_dist = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
            prop_assert_eq!(rsc_dist, protocol::ftms_distance_to_rsc(opt_dist.unwrap()));
        } else {
            prop_assert_eq!(len, 4);
            prop_assert!(buf[0] & 0x02 == 0);
        }
    }
}

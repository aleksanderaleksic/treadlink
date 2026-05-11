// Feature: ftms-rsc-bridge, Property 8: Candidate collector invariants
//
// Host-side property-based test for the candidate collector.
// Validates that collect_candidates produces a correctly bounded, sorted,
// deduplicated list with RSSI threshold enforcement and max-RSSI-per-address semantics.

use proptest::prelude::*;
use std::collections::HashMap;
use treadlink_pbt::central::{collect_candidates, BdAddr};

fn arb_bd_addr() -> impl Strategy<Value = BdAddr> {
    prop::array::uniform6(0u8..=255).prop_map(BdAddr::new)
}

fn arb_events() -> impl Strategy<Value = Vec<(BdAddr, i8)>> {
    prop::collection::vec((arb_bd_addr(), -128i8..=127i8), 0..50)
}

proptest! {
    /// **Validates: Requirements 1.2, 1.3**
    ///
    /// For any sequence of advertisement events (address + RSSI pairs), the resulting
    /// candidate list satisfies all of:
    /// - Length ≤ 4
    /// - Sorted by RSSI in descending order
    /// - No duplicate addresses
    /// - All candidates have RSSI ≥ -70
    /// - For each address in the result, the stored RSSI is the maximum observed for that address
    #[test]
    fn candidate_collector_invariants(events in arb_events()) {
        let result = collect_candidates(events.iter().cloned());

        // Length ≤ 4
        prop_assert!(result.len() <= 4);

        // Sorted by RSSI descending
        for w in result.windows(2) {
            prop_assert!(w[0].rssi >= w[1].rssi);
        }

        // No duplicate addresses
        for i in 0..result.len() {
            for j in (i + 1)..result.len() {
                prop_assert_ne!(result[i].address, result[j].address);
            }
        }

        // All RSSI ≥ -70
        for c in result.iter() {
            prop_assert!(c.rssi >= -70);
        }

        // Max RSSI kept per address: for each address in the result,
        // the stored RSSI must equal the maximum RSSI seen for that address
        // among events that passed the threshold.
        let mut max_rssi: HashMap<[u8; 6], i8> = HashMap::new();
        for (addr, rssi) in &events {
            if *rssi >= -70 {
                let entry = max_rssi.entry(*addr.raw()).or_insert(*rssi);
                if *rssi > *entry {
                    *entry = *rssi;
                }
            }
        }
        for c in result.iter() {
            if let Some(&expected_rssi) = max_rssi.get(c.address.raw()) {
                prop_assert_eq!(c.rssi, expected_rssi);
            }
        }
    }
}

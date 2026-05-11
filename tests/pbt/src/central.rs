// Host-side mirror of the candidate collector logic from src/central.rs.
// Uses a local BdAddr type to avoid pulling in trouble-host (which requires no_std/embedded).

use heapless::Vec;

/// A simple BLE device address wrapper for host-side testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BdAddr([u8; 6]);

impl BdAddr {
    pub fn new(raw: [u8; 6]) -> Self {
        Self(raw)
    }

    pub fn raw(&self) -> &[u8; 6] {
        &self.0
    }
}

/// A BLE scan candidate with its device address and signal strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanCandidate {
    pub address: BdAddr,
    pub rssi: i8,
}

/// Minimum acceptable RSSI for a scan candidate (dBm).
const RSSI_THRESHOLD: i8 = -70;

/// Collect up to 4 scan candidates from an iterator of advertisement events.
///
/// Algorithm:
/// 1. Discard events with RSSI < -70 dBm
/// 2. Deduplicate by address, keeping the maximum observed RSSI per address
/// 3. Keep only the top 4 strongest candidates (replace weakest if full and new is stronger)
/// 4. Sort by RSSI descending
pub fn collect_candidates(
    events: impl Iterator<Item = (BdAddr, i8)>,
) -> Vec<ScanCandidate, 4> {
    let mut candidates: Vec<ScanCandidate, 4> = Vec::new();

    for (address, rssi) in events {
        // Discard weak signals
        if rssi < RSSI_THRESHOLD {
            continue;
        }

        // Check if this address is already in the list
        if let Some(existing) = candidates.iter_mut().find(|c| c.address == address) {
            // Keep the maximum RSSI for this address
            if rssi > existing.rssi {
                existing.rssi = rssi;
            }
        } else if candidates.len() < 4 {
            // List not full, just add
            let _ = candidates.push(ScanCandidate { address, rssi });
        } else {
            // List is full — find the weakest candidate
            let weakest_idx = candidates
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| c.rssi)
                .map(|(i, _)| i)
                .unwrap(); // safe: candidates is non-empty (len == 4)

            if rssi > candidates[weakest_idx].rssi {
                candidates[weakest_idx] = ScanCandidate { address, rssi };
            }
        }
    }

    // Sort by RSSI descending
    candidates.sort_unstable_by(|a, b| b.rssi.cmp(&a.rssi));

    candidates
}

use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;
use embassy_time::{Duration, Timer, with_timeout};
use heapless::Vec;
use trouble_host::prelude::*;

use crate::converter::assemble_rsc;
use crate::protocol::{parse_treadmill_data, RscMeasurement};

// Re-import bt-hci types needed for Scanner::scan where bounds
use bt_hci::cmd::le::{
    LeAddDeviceToFilterAcceptList, LeClearFilterAcceptList, LeCreateConn,
    LeSetScanEnable, LeSetScanParams,
};
use bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync};

/// Bridge lifecycle state, published via Watch for peripheral and LED tasks.
#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum BridgeState {
    /// Scanning for FTMS treadmills
    Scanning,
    /// Connecting to a candidate device
    Connecting,
    /// Actively bridging FTMS → RSC data
    Bridging,
}

/// A BLE scan candidate with its device address and signal strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanCandidate {
    pub address: BdAddr,
    pub addr_kind: AddrKind,
    pub rssi: i8,
}

impl defmt::Format for ScanCandidate {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "ScanCandidate {{ address: {:?}, rssi: {} }}", self.address.raw(), self.rssi);
    }
}

/// Minimum acceptable RSSI for a scan candidate (dBm).
const RSSI_THRESHOLD: i8 = -100;

/// Helper to format AddrKind as a human-readable string for logging.
fn addr_kind_str(kind: AddrKind) -> &'static str {
    if kind == AddrKind::PUBLIC {
        "public"
    } else if kind == AddrKind::RANDOM {
        "random"
    } else {
        "unknown"
    }
}

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
            let _ = candidates.push(ScanCandidate { address, addr_kind: AddrKind::PUBLIC, rssi });
        } else {
            // List is full — find the weakest candidate
            let weakest_idx = candidates
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| c.rssi)
                .map(|(i, _)| i)
                .unwrap(); // safe: candidates is non-empty (len == 4)

            if rssi > candidates[weakest_idx].rssi {
                candidates[weakest_idx] = ScanCandidate { address, addr_kind: AddrKind::PUBLIC, rssi };
            }
        }
    }

    // Sort by RSSI descending
    candidates.sort_unstable_by(|a, b| b.rssi.cmp(&a.rssi));

    candidates
}

/// FTMS advertisement scanner — implements EventHandler to collect scan reports.
///
/// This struct is shared between the runner task (which calls `on_adv_reports`)
/// and the central task (which reads collected candidates after a scan window).
/// It filters for devices advertising the FTMS service UUID (0x1826).
pub struct FtmsScanner {
    /// Collected (address, addr_kind, rssi) tuples from advertisements containing FTMS UUID.
    events: RefCell<Vec<(BdAddr, AddrKind, i8), 32>>,
}

impl FtmsScanner {
    pub const fn new() -> Self {
        Self {
            events: RefCell::new(Vec::new()),
        }
    }

    /// Drain collected events and run them through candidate collection logic.
    pub fn take_candidates(&self) -> Vec<ScanCandidate, 4> {
        let mut events = self.events.borrow_mut();
        let mut candidates: Vec<ScanCandidate, 4> = Vec::new();

        for &(address, addr_kind, rssi) in events.iter() {
            if rssi < RSSI_THRESHOLD {
                continue;
            }

            if let Some(existing) = candidates.iter_mut().find(|c| c.address == address) {
                if rssi > existing.rssi {
                    existing.rssi = rssi;
                }
            } else if candidates.len() < 4 {
                let _ = candidates.push(ScanCandidate { address, addr_kind, rssi });
            } else {
                let weakest_idx = candidates
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, c)| c.rssi)
                    .map(|(i, _)| i)
                    .unwrap();

                if rssi > candidates[weakest_idx].rssi {
                    candidates[weakest_idx] = ScanCandidate { address, addr_kind, rssi };
                }
            }
        }

        candidates.sort_unstable_by(|a, b| b.rssi.cmp(&a.rssi));
        events.clear();
        candidates
    }

    /// Clear any previously collected events (call before starting a new scan).
    pub fn clear(&self) {
        self.events.borrow_mut().clear();
    }
}

impl EventHandler for FtmsScanner {
    fn on_adv_reports(&self, mut it: LeAdvReportsIter<'_>) {
        let mut events = self.events.borrow_mut();
        while let Some(Ok(report)) = it.next() {
            if ad_contains_ftms(report.data) {
                let _ = events.push((report.addr, report.addr_kind, report.rssi));
                defmt::debug!(
                    "[scan] FTMS device: addr={:?} kind={} rssi={}",
                    report.addr.raw(),
                    addr_kind_str(report.addr_kind),
                    report.rssi
                );
            }
        }
    }

    fn on_ext_adv_reports(&self, mut it: bt_hci::param::LeExtAdvReportsIter<'_>) {
        let mut events = self.events.borrow_mut();
        while let Some(Ok(report)) = it.next() {
            if ad_contains_ftms(report.data) {
                let _ = events.push((report.addr, report.addr_kind, report.rssi));
                defmt::debug!(
                    "[scan] FTMS device (ext): addr={:?} kind={} rssi={}",
                    report.addr.raw(),
                    addr_kind_str(report.addr_kind),
                    report.rssi
                );
            }
        }
    }
}

/// Returns true if the advertisement data contains the FTMS service UUID (0x1826).
fn ad_contains_ftms(data: &[u8]) -> bool {
    let mut ad_iter = AdStructure::decode(data);
    while let Some(Ok(ad)) = ad_iter.next() {
        match ad {
            AdStructure::IncompleteServiceUuids16(uuids) | AdStructure::CompleteServiceUuids16(uuids) => {
                for uuid_bytes in uuids {
                    if u16::from_le_bytes(*uuid_bytes) == 0x1826 {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// FTMS Service UUID (0x1826)
const FTMS_SERVICE_UUID: Uuid = Uuid::new_short(0x1826);

/// Treadmill Data Characteristic UUID (0x2ACD)
const TREADMILL_DATA_UUID: Uuid = Uuid::new_short(0x2ACD);

/// Scan duration
const SCAN_DURATION: Duration = Duration::from_secs(3);

/// Connection/discovery timeout
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Error recovery delay
const ERROR_DELAY: Duration = Duration::from_secs(1);

/// Central task: scans for FTMS treadmills, connects, subscribes to Treadmill Data
/// notifications, parses FTMS data, converts to RSC, and publishes measurements.
///
/// State machine:
///   Scanning → Connecting → Bridging → (on disconnect) → Scanning
///
/// # Type Parameters
/// The concrete controller type (`C`) is determined by the BLE stack initialization
/// in main.rs. It must implement `trouble_host::Controller` (which requires bt-hci 0.8
/// Controller trait + all required HCI command traits).
///
/// # Arguments
/// * `stack` - Reference to the BLE stack
/// * `central` - The BLE Central role handle
/// * `signal` - Signal for publishing RSC measurements to the peripheral task
/// * `watch` - Watch for publishing BridgeState changes to peripheral and LED tasks
///
/// NOTE: This function uses a generic controller type `C`. Since `#[embassy_executor::task]`
/// does not support generics, the actual task spawning in main.rs will need to use a
/// concrete monomorphized wrapper or the task macro on a concrete type alias.
/// The logic below is the complete central task implementation.
pub async fn central_task_impl<C>(
    stack: &'static Stack<'static, C, DefaultPacketPool>,
    mut central: Central<'static, C, DefaultPacketPool>,
    signal: &'static Signal<CriticalSectionRawMutex, RscMeasurement>,
    watch: &'static Watch<CriticalSectionRawMutex, BridgeState, 2>,
    scanner_handler: &'static FtmsScanner,
) where
    C: Controller
        + ControllerCmdSync<LeSetScanParams>
        + ControllerCmdSync<LeSetScanEnable>
        + ControllerCmdSync<LeClearFilterAcceptList>
        + ControllerCmdSync<LeAddDeviceToFilterAcceptList>
        + ControllerCmdAsync<LeCreateConn>,
{
    // TEMPORARY: Use exact same code as working connect_test binary
    defmt::info!("[central] Starting scan...");
    {
        let mut scanner = Scanner::new(central);
        let scan_config = ScanConfig {
            active: false,
            filter_accept_list: &[],
            timeout: Duration::from_secs(3),
            ..Default::default()
        };

        match scanner.scan(&scan_config).await {
            Ok(session) => {
                defmt::info!("[central] Scan running, waiting 3s...");
                Timer::after(Duration::from_secs(3)).await;
                drop(session);
                defmt::info!("[central] Scan session dropped");
            }
            Err(_) => {
                defmt::error!("[central] Scan failed");
            }
        }

        central = scanner.into_inner();
    }

    defmt::info!("[central] Waiting 500ms for scan to settle...");
    Timer::after(Duration::from_millis(500)).await;

    let target_addr = BdAddr::new([124, 49, 0, 8, 192, 246]);

    defmt::info!("[central] Connecting...");
    let config = ConnectConfig {
        connect_params: Default::default(),
        scan_config: ScanConfig {
            filter_accept_list: &[Address { kind: AddrKind::RANDOM, addr: target_addr }],
            ..Default::default()
        },
    };

    match with_timeout(CONNECT_TIMEOUT, central.connect(&config)).await {
        Ok(Ok(conn)) => {
            defmt::info!("[central] CONNECTED!");
            watch.sender().send(BridgeState::Bridging);
            let _ = discover_subscribe_and_bridge(stack, &conn, signal, watch).await;
            defmt::info!("[central] Bridge ended");
        }
        Ok(Err(e)) => {
            match &e {
                BleHostError::BleHost(err) => defmt::error!("[central] Connect failed: {:?}", err),
                BleHostError::Controller(_) => defmt::error!("[central] Connect failed (controller)"),
            }
        }
        Err(_) => {
            defmt::error!("[central] Connect timed out");
        }
    }

    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}

/// Perform GATT service discovery, verify Treadmill Data characteristic (0x2ACD),
/// subscribe to notifications, and run the bridging receive loop.
///
/// This function owns the GattClient for its entire lifetime, which allows the
/// NotificationListener (which borrows the client) to live long enough for the
/// receive loop.
///
/// The 5s timeout (Req 2.3, 2.4) is applied to the discovery and subscription phase.
/// The bridging receive loop runs indefinitely until the treadmill disconnects.
///
/// Returns `Ok(())` when the treadmill disconnects (bridging ended normally).
/// Returns `Err(...)` if discovery or subscription fails within the timeout.
async fn discover_subscribe_and_bridge<'conn, C: Controller>(
    stack: &'conn Stack<'conn, C, DefaultPacketPool>,
    conn: &Connection<'conn, DefaultPacketPool>,
    signal: &Signal<CriticalSectionRawMutex, RscMeasurement>,
    watch: &Watch<CriticalSectionRawMutex, BridgeState, 2>,
) -> Result<(), BleHostError<C::Error>> {
    // Create GATT client with 5s timeout (performs MTU exchange) (Req 2.3)
    let client: GattClient<'_, C, DefaultPacketPool, 4> = match with_timeout(
        CONNECT_TIMEOUT,
        GattClient::new(stack, conn),
    )
    .await
    {
        Ok(result) => {
            defmt::debug!("[central] GATT client created (MTU exchanged)");
            result?
        }
        Err(_) => {
            defmt::warn!("[central] GATT client creation timed out");
            return Err(BleHostError::BleHost(Error::Timeout));
        }
    };

    // Discover FTMS service (0x1826) with timeout (Req 2.3)
    let services = match with_timeout(
        CONNECT_TIMEOUT,
        client.services_by_uuid(&FTMS_SERVICE_UUID),
    )
    .await
    {
        Ok(result) => {
            defmt::debug!("[central] FTMS service discovered");
            result?
        }
        Err(_) => {
            defmt::warn!("[central] FTMS service discovery timed out");
            return Err(BleHostError::BleHost(Error::Timeout));
        }
    };
    let service = services.first().ok_or(BleHostError::BleHost(Error::NotFound))?;

    // Find Treadmill Data characteristic (0x2ACD) with timeout (Req 2.3)
    let characteristic: Characteristic<[u8]> = match with_timeout(
        CONNECT_TIMEOUT,
        client.characteristic_by_uuid(service, &TREADMILL_DATA_UUID),
    )
    .await
    {
        Ok(result) => {
            defmt::debug!("[central] Treadmill Data characteristic found (0x2ACD)");
            result?
        }
        Err(_) => {
            defmt::warn!("[central] Treadmill Data characteristic discovery timed out");
            return Err(BleHostError::BleHost(Error::Timeout));
        }
    };

    // Subscribe to notifications with timeout (Req 2.6)
    let mut listener = match with_timeout(
        CONNECT_TIMEOUT,
        client.subscribe(&characteristic, false),
    )
    .await
    {
        Ok(result) => {
            defmt::debug!("[central] Subscribed to Treadmill Data notifications");
            result?
        }
        Err(_) => {
            defmt::warn!("[central] Notification subscription timed out");
            return Err(BleHostError::BleHost(Error::Timeout));
        }
    };

    // Validation success: transition to Bridging (Req 2.8)
    watch.sender().send(BridgeState::Bridging);
    defmt::info!("[central] FTMS validated, bridging active");

    // --- Bridging phase: receive notifications ---
    // Run the GATT client task concurrently with notification processing.
    // client.task() handles incoming PDUs and dispatches to the listener.
    let _bridge_result = embassy_futures::select::select(
        client.task(),
        receive_notifications(&mut listener, signal),
    )
    .await;

    // Either the client task ended (disconnect) or the notification loop ended.
    // In both cases, the treadmill connection is lost.
    Ok(())
}

/// Inner receive loop: parse FTMS notifications, convert to RSC, publish via Signal.
///
/// Returns when the connection is lost (notification stream ends/cancelled by select).
async fn receive_notifications(
    listener: &mut NotificationListener<'_, 512>,
    signal: &Signal<CriticalSectionRawMutex, RscMeasurement>,
) {
    loop {
        let notification = listener.next().await;
        let data = notification.as_ref();

        // Parse FTMS Treadmill Data notification
        match parse_treadmill_data(data) {
            Ok(treadmill_data) => {
                // Convert to RSC measurement and publish (Req 15.1)
                let measurement = assemble_rsc(&treadmill_data);
                signal.signal(measurement);
            }
            Err(e) => {
                // Parse error: log, discard, continue (Req 17.1–17.4)
                defmt::warn!("[central] FTMS parse error: {:?}", e);
            }
        }
    }
}

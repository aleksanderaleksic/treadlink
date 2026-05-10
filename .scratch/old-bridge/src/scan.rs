use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use trouble_host::prelude::*;

/// Minimum RSSI to consider a candidate (ignore very weak signals).
const MIN_RSSI: i8 = -90;

/// Maximum FTMS candidates tracked per scan window.
const MAX_SCAN_CANDIDATES: usize = 16;

/// A discovered FTMS candidate: address bytes + RSSI.
#[derive(Clone, Copy)]
pub(crate) struct FtmsCandidate {
    pub addr: [u8; 6],
    pub addr_kind: bt_hci::param::AddrKind,
    pub rssi: i8,
}

/// Channel for passing FTMS candidates from the EventHandler to the scan loop.
/// Capacity of 8 is enough — we process candidates faster than they arrive.
pub(crate) static SCAN_CHANNEL: Channel<CriticalSectionRawMutex, FtmsCandidate, 8> = Channel::new();

/// EventHandler implementation that filters for FTMS advertisements and
/// sends candidates through the SCAN_CHANNEL.
pub(crate) struct FtmsScanHandler;

impl EventHandler for FtmsScanHandler {
    fn on_adv_reports(&self, reports: bt_hci::param::LeAdvReportsIter) {
        for result in reports {
            if let Ok(report) = result {
                if protocol::adv_contains_ftms_uuid(report.data) && report.rssi >= MIN_RSSI {
                    let candidate = FtmsCandidate {
                        addr: report.addr.0,
                        addr_kind: report.addr_kind,
                        rssi: report.rssi,
                    };
                    // try_send: drop if channel is full (non-blocking in sync context)
                    let _ = SCAN_CHANNEL.try_send(candidate);
                }
            }
        }
    }
}

/// Scan for BLE devices advertising FTMS (UUID 0x1826) and return the
/// address of the device with the strongest RSSI.
///
/// Rescans indefinitely until at least one candidate is found.
pub(crate) async fn find_treadmill<C: Controller>(
    scanner: &mut Scanner<'_, C, DefaultPacketPool>,
) -> Address
where
    C: bt_hci::controller::ControllerCmdSync<bt_hci::cmd::le::LeSetScanParams>
        + bt_hci::controller::ControllerCmdSync<bt_hci::cmd::le::LeSetScanEnable>
        + bt_hci::controller::ControllerCmdSync<bt_hci::cmd::le::LeClearFilterAcceptList>
        + bt_hci::controller::ControllerCmdSync<bt_hci::cmd::le::LeAddDeviceToFilterAcceptList>,
{
    let scan_config = ScanConfig {
        active: true,
        filter_accept_list: &[],
        timeout: embassy_time::Duration::from_secs(3),
        ..Default::default()
    };

    loop {
        defmt::info!("FTMS scan started");

        // Drain any stale candidates from previous scans
        while SCAN_CHANNEL.try_receive().is_ok() {}

        let mut candidates = [([0u8; 6], 0i8); MAX_SCAN_CANDIDATES];
        let mut addr_kinds = [bt_hci::param::AddrKind::PUBLIC; MAX_SCAN_CANDIDATES];
        let mut count: usize = 0;

        // Start scanning — reports arrive via FtmsScanHandler::on_adv_reports
        let _session = match scanner.scan(&scan_config).await {
            Ok(s) => s,
            Err(e) => {
                defmt::warn!("Scan start failed: {:?}", defmt::Debug2Format(&e));
                embassy_time::Timer::after_secs(1).await;
                continue;
            }
        };

        // Collect candidates until the scan timeout expires.
        // The scan session runs for `timeout` duration. We poll the channel
        // with a timeout slightly longer than the scan window.
        let deadline = embassy_time::Instant::now() + embassy_time::Duration::from_secs(4);
        loop {
            let remaining = deadline.saturating_duration_since(embassy_time::Instant::now());
            if remaining.as_ticks() == 0 {
                break;
            }
            match embassy_time::with_timeout(remaining, SCAN_CHANNEL.receive()).await {
                Ok(candidate) => {
                    // Deduplicate: update RSSI if we've seen this address before
                    let mut found = false;
                    for i in 0..count {
                        if candidates[i].0 == candidate.addr {
                            // Keep the strongest RSSI for this device
                            if candidate.rssi > candidates[i].1 {
                                candidates[i].1 = candidate.rssi;
                                addr_kinds[i] = candidate.addr_kind;
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        if count < MAX_SCAN_CANDIDATES {
                            defmt::info!(
                                "FTMS device: addr={=[u8]:x}, kind={}, rssi={}",
                                candidate.addr,
                                defmt::Debug2Format(&candidate.addr_kind),
                                candidate.rssi
                            );
                            candidates[count] = (candidate.addr, candidate.rssi);
                            addr_kinds[count] = candidate.addr_kind;
                            count += 1;
                        }
                    }
                }
                Err(_timeout) => break,
            }
        }

        // Explicitly drop the session to stop the scan before doing anything else.
        drop(_session);

        defmt::info!("FTMS scan stopped, {} candidates", count);

        if let Some(raw) = protocol::select_best_rssi(&candidates[..count]) {
            // Find the addr_kind for the selected address
            let kind = candidates[..count]
                .iter()
                .zip(addr_kinds[..count].iter())
                .find(|((a, _), _)| *a == raw)
                .map(|(_, k)| *k)
                .unwrap_or(bt_hci::param::AddrKind::PUBLIC);

            defmt::info!(
                "Selected treadmill: addr={=[u8]:x}, kind={}",
                raw,
                defmt::Debug2Format(&kind)
            );

            // Wait for the SDC to finish processing the scan disable.
            // The runner needs time to handle the LeSetScanEnable(false) HCI command
            // before we can issue LeCreateConn.
            embassy_time::Timer::after_millis(500).await;

            return Address {
                kind,
                addr: bt_hci::param::BdAddr(raw),
            };
        }

        defmt::info!("No FTMS devices found, rescanning...");
    }
}

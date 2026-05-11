use embassy_futures::select::{select, select3, Either, Either3};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;
use embassy_time::Duration;
use trouble_host::prelude::*;

use crate::central::BridgeState;
use crate::protocol::RscMeasurement;

/// RSC GATT Server — matches working treadlink-old implementation exactly.
#[gatt_server]
pub struct Server {
    pub rsc: RscService,
}

/// Running Speed and Cadence service (UUID 0x1814).
#[gatt_service(uuid = "1814")]
pub struct RscService {
    /// RSC Measurement (UUID 0x2A53): read + notify.
    /// Max 8 bytes: 1 flags + 2 speed + 1 cadence + 4 distance.
    #[characteristic(uuid = "2A53", read, notify)]
    pub measurement: [u8; 8],

    /// RSC Feature (UUID 0x2A54): read-only.
    /// Value 0x0002 = Total Distance Supported.
    #[characteristic(uuid = "2A54", read, value = [0x02, 0x00])]
    pub feature: [u8; 2],
}

/// Peripheral task: advertises RSC service, relays measurements.
#[embassy_executor::task]
pub async fn peripheral_task(
    mut peripheral: Peripheral<'static, nrf_sdc::SoftdeviceController<'static>, DefaultPacketPool>,
    server: &'static Server<'static>,
    signal: &'static Signal<CriticalSectionRawMutex, RscMeasurement>,
    watch: &'static Watch<CriticalSectionRawMutex, BridgeState, 2>,
) {
    let mut state_rx = watch.receiver().unwrap();

    loop {
        state_rx.changed_and(|s| *s == BridgeState::Bridging).await;
        defmt::info!("[peripheral] BridgeState::Bridging — starting advertising");

        'advertise: loop {
            let mut adv_buf = [0u8; 31];
            let adv_len = AdStructure::encode_slice(
                &[
                    AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                    AdStructure::IncompleteServiceUuids16(&[[0x14, 0x18]]),
                    AdStructure::ShortenedLocalName(b"TreadLink"),
                ],
                &mut adv_buf,
            )
            .unwrap_or(0);

            let mut scan_buf = [0u8; 31];
            let scan_len = AdStructure::encode_slice(
                &[
                    AdStructure::CompleteLocalName(b"TreadLink"),
                ],
                &mut scan_buf,
            )
            .unwrap_or(0);

            let params = AdvertisementParameters {
                interval_min: Duration::from_millis(100),
                interval_max: Duration::from_millis(100),
                ..Default::default()
            };

            let advertiser = match peripheral
                .advertise(
                    &params,
                    Advertisement::ConnectableScannableUndirected {
                        adv_data: &adv_buf[..adv_len],
                        scan_data: &scan_buf[..scan_len],
                    },
                )
                .await
            {
                Ok(a) => a,
                Err(e) => {
                    defmt::warn!("[peripheral] advertising failed: {}", defmt::Debug2Format(&e));
                    break 'advertise;
                }
            };

            let conn = match select(advertiser.accept(), state_rx.changed()).await {
                Either::First(Ok(conn)) => conn,
                Either::First(Err(_e)) => {
                    defmt::warn!("[peripheral] advertiser accept failed");
                    break 'advertise;
                }
                Either::Second(new_state) => {
                    defmt::info!("[peripheral] state changed to {:?} while advertising", new_state);
                    break 'advertise;
                }
            };

            defmt::info!("[peripheral] consumer connected");

            let gatt_conn = match conn.with_attribute_server(&server.server) {
                Ok(gc) => gc,
                Err(e) => {
                    defmt::warn!("[peripheral] GATT connection failed: {}", defmt::Debug2Format(&e));
                    continue 'advertise;
                }
            };

            let reason = serve_connection(&gatt_conn, server, signal, &mut state_rx).await;

            match reason {
                DisconnectReason::Consumer => {
                    defmt::info!("[peripheral] consumer disconnected, resuming advertising");
                    if let Some(state) = state_rx.try_get() {
                        if state != BridgeState::Bridging {
                            break 'advertise;
                        }
                    }
                    continue 'advertise;
                }
                DisconnectReason::StateChanged => {
                    defmt::info!("[peripheral] state no longer Bridging");
                    break 'advertise;
                }
            }
        }
    }
}

enum DisconnectReason {
    Consumer,
    StateChanged,
}

async fn serve_connection<'s, 'v>(
    gatt_conn: &GattConnection<'s, 'v, DefaultPacketPool>,
    server: &Server<'v>,
    signal: &Signal<CriticalSectionRawMutex, RscMeasurement>,
    state_rx: &mut embassy_sync::watch::Receiver<'_, CriticalSectionRawMutex, BridgeState, 2>,
) -> DisconnectReason {
    defmt::info!("[peripheral] serving connection — processing discovery first");
    let mut event_count: u16 = 0;
    let mut notify_count: u16 = 0;

    // Phase 1: Handle GATT discovery — don't send notifications yet
    let subscribed = loop {
        match select(gatt_conn.next(), state_rx.changed()).await {
            Either::First(event) => {
                event_count += 1;
                match event {
                    GattConnectionEvent::Disconnected { reason } => {
                        defmt::info!("[peripheral] disconnected during discovery: {:?}", reason);
                        return DisconnectReason::Consumer;
                    }
                    GattConnectionEvent::Gatt { event } => {
                        let is_write = matches!(&event, GattEvent::Write(_));
                        defmt::info!("[peripheral] GATT event #{} (write={})", event_count, is_write);
                        match event.accept() {
                            Ok(reply) => {
                                defmt::debug!("[peripheral] sending reply to write");
                                reply.send().await;
                                defmt::debug!("[peripheral] sent reply to write");
                            },
                            Err(e) => {
                                defmt::warn!("[peripheral] accept err: {:?}", e);
                            }
                        }
                        if is_write {
                            defmt::info!("[peripheral] CCCD subscription received!");
                            break true;
                        }
                    }
                    _ => {}
                }
            }
            Either::Second(new_state) => {
                if new_state != BridgeState::Bridging {
                    return DisconnectReason::StateChanged;
                }
            }
        }
    };

    if !subscribed {
        return DisconnectReason::Consumer;
    }

    defmt::info!("[peripheral] subscribed, starting notifications");

    // Phase 2: Send notifications and handle ongoing events

    loop {
        match select3(signal.wait(), gatt_conn.next(), state_rx.changed()).await {
            Either3::First(measurement) => {
                let speed_bytes = measurement.speed.to_le_bytes();
                let dist = measurement.total_distance.unwrap_or(0);
                let dist_bytes = dist.to_le_bytes();
                let flags: u8 = if measurement.total_distance.is_some() { 0x02 } else { 0x00 };

                let payload: [u8; 8] = [
                    flags,
                    speed_bytes[0], speed_bytes[1],
                    measurement.cadence,
                    dist_bytes[0], dist_bytes[1], dist_bytes[2], dist_bytes[3],
                ];

                match server.rsc.measurement.notify(gatt_conn, &payload).await {
                    Ok(_) => {
                        notify_count += 1;
                        if notify_count <= 3 || notify_count % 10 == 0 {
                            defmt::info!("[peripheral] notify #{} sent", notify_count);
                        }
                    }
                    Err(e) => {
                        defmt::warn!("[peripheral] notify #{} failed: {}", notify_count, defmt::Debug2Format(&e));
                    }
                }
            }
            Either3::Second(event) => {
                event_count += 1;
                match event {
                    GattConnectionEvent::Disconnected { reason } => {
                        defmt::info!("[peripheral] disconnected after {} events, {} notifies: {:?}", event_count, notify_count, reason);
                        return DisconnectReason::Consumer;
                    }
                    GattConnectionEvent::Gatt { event } => {
                        defmt::info!("[peripheral] GATT event #{}", event_count);
                        match event.accept() {
                            Ok(reply) => reply.send().await,
                            Err(e) => defmt::warn!("[peripheral] accept err: {}", defmt::Debug2Format(&e)),
                        }
                    }
                    _ => {
                        defmt::info!("[peripheral] conn event #{}", event_count);
                    }
                }
            }
            Either3::Third(new_state) => {
                if new_state != BridgeState::Bridging {
                    return DisconnectReason::StateChanged;
                }
            }
        }
    }
}

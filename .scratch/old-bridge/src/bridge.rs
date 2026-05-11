use trouble_host::prelude::*;

use crate::ble;

const HEARTBEAT_INTERVAL_SECS: u64 = 30;
const TREADMILL_DATA_UUID: u16 = 0x2ACD;
const DEVICE_NAME: &[u8] = b"TreadLink";
const FTMS_SERVICE_UUID_LE: [u8; 2] = [0x26, 0x18];
const RSC_SERVICE_UUID_LE: [u8; 2] = [0x14, 0x18];
const MAX_ADV_DATA_LEN: usize = 31;
const RSC_MEASUREMENT_BUF_SIZE: usize = 8;
const GATT_CLIENT_MAX_ATTRIBUTES: usize = 16;

#[derive(defmt::Format)]
#[allow(dead_code)]
pub(crate) enum BridgeError {
    Advertise,
    Discovery,
    Subscribe,
    Notify,
}

#[derive(defmt::Format)]
pub(crate) enum ConvertError {
    Parse,
    Encode,
}

pub(crate) fn convert_ftms_to_rsc(ftms_payload: &[u8], buf: &mut [u8]) -> Result<usize, ConvertError> {
    protocol::convert_ftms_to_rsc(ftms_payload, buf).map_err(|e| match e {
        protocol::ConvertError::Parse(_) => ConvertError::Parse,
        protocol::ConvertError::Encode(_) => ConvertError::Encode,
    })
}

pub(crate) async fn run<'a, C: Controller>(
    stack: &'a Stack<'a, C, DefaultPacketPool>,
    treadmill_conn: Connection<'a, DefaultPacketPool>,
    server: &'a ble::Server<'a>,
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
) -> Result<(), BridgeError>
where
    C: bt_hci::controller::ControllerCmdAsync<bt_hci::cmd::le::LeConnUpdate>
        + bt_hci::controller::ControllerCmdSync<bt_hci::cmd::le::LeReadLocalSupportedFeatures>,
{
    // Treadmill event consumer — runs from the very start to drain ATT PDUs
    // and handle conn param requests. Without this, the treadmill's packets
    // pile up in the pool and cause InsufficientSpace for the watch.
    let disconnect_watcher = async {
        loop {
            match treadmill_conn.next().await {
                trouble_host::connection::ConnectionEvent::Disconnected { reason } => {
                    defmt::info!("Treadmill disconnected: {:?}", reason);
                    break;
                }
                trouble_host::connection::ConnectionEvent::RequestConnectionParams(req) => {
                    defmt::info!("Treadmill conn param update");
                    let params = req.params().clone();
                    if let Err(e) = treadmill_conn.update_connection_params(stack, &params).await {
                        defmt::warn!("Conn param update failed: {:?}", defmt::Debug2Format(&e));
                    } else {
                        defmt::info!("Conn params updated");
                    }
                }
                _ => {}
            }
        }
    };

    let bridge_logic = async {
        // Phase 1: Advertise RSC and wait for watch
        let watch_gatt = match advertise_rsc(peripheral, server).await {
            Ok(gatt) => {
                defmt::info!("Watch connected");
                gatt
            }
            Err(e) => {
                defmt::warn!("RSC advertise error: {:?}", e);
                return Err(BridgeError::Advertise);
            }
        };

        // Phase 2: Watch GATT discovery — no FTMS client yet
        defmt::info!("Processing watch GATT discovery");
        let discovery_deadline = embassy_time::Instant::now() + embassy_time::Duration::from_secs(5);
        loop {
            let remaining = discovery_deadline.saturating_duration_since(embassy_time::Instant::now());
            if remaining.as_ticks() == 0 { break; }
            match embassy_time::with_timeout(remaining, watch_gatt.next()).await {
                Ok(GattConnectionEvent::Gatt { event }) => {
                    let is_write = matches!(&event, GattEvent::Write(_));
                    match event.accept() {
                        Ok(reply) => reply.send().await,
                        Err(e) => defmt::warn!("GATT accept error: {:?}", defmt::Debug2Format(&e)),
                    }
                    if is_write {
                        defmt::info!("Watch CCCD subscription received");
                        break;
                    }
                }
                Ok(GattConnectionEvent::Disconnected { reason }) => {
                    defmt::info!("Watch disconnected during discovery: {:?}", reason);
                    return Ok(());
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        // Phase 3: Create FTMS client, discover, subscribe
        defmt::info!("Creating FTMS GATT client");
        let ftms_client: GattClient<'_, C, DefaultPacketPool, GATT_CLIENT_MAX_ATTRIBUTES> =
            match embassy_time::with_timeout(
                embassy_time::Duration::from_secs(10),
                GattClient::new(stack, &treadmill_conn),
            ).await {
                Ok(Ok(c)) => c,
                Ok(Err(e)) => {
                    defmt::warn!("GATT client failed: {:?}", defmt::Debug2Format(&e));
                    return Err(BridgeError::Discovery);
                }
                Err(_) => {
                    defmt::warn!("GATT client timed out");
                    return Err(BridgeError::Discovery);
                }
            };

        // ftms_client.task() must run during discovery and relay
        let ftms_work = async {
            let ftms_uuid = Uuid::Uuid16(FTMS_SERVICE_UUID_LE);
            let services = ftms_client.services_by_uuid(&ftms_uuid).await.map_err(|e| {
                defmt::warn!("FTMS discovery failed: {:?}", defmt::Debug2Format(&e));
                BridgeError::Discovery
            })?;
            let ftms_service = services.first().ok_or_else(|| {
                defmt::warn!("No FTMS service");
                BridgeError::Discovery
            })?;
            defmt::info!("FTMS service discovered");

            let treadmill_data_uuid = Uuid::Uuid16(TREADMILL_DATA_UUID.to_le_bytes());
            let treadmill_char: Characteristic<[u8]> = ftms_client
                .characteristic_by_uuid(ftms_service, &treadmill_data_uuid)
                .await.map_err(|e| {
                    defmt::warn!("Treadmill Data not found: {:?}", defmt::Debug2Format(&e));
                    BridgeError::Discovery
                })?;
            defmt::info!("Treadmill Data found");

            let mut listener = ftms_client.subscribe(&treadmill_char, false).await.map_err(|e| {
                defmt::warn!("Subscribe failed: {:?}", defmt::Debug2Format(&e));
                BridgeError::Subscribe
            })?;
            defmt::info!("FTMS notifications enabled, relaying");

            // Phase 4: Relay
            let mut notify_count: u32 = 0;
            let mut last_heartbeat = embassy_time::Instant::now();

            loop {
                match embassy_futures::select::select(watch_gatt.next(), listener.next()).await {
                    embassy_futures::select::Either::First(ev) => match ev {
                        GattConnectionEvent::Disconnected { reason } => {
                            defmt::info!("Watch disconnected: {:?}", reason);
                            break;
                        }
                        GattConnectionEvent::Gatt { event } => {
                            if let Ok(reply) = event.accept() { reply.send().await; }
                        }
                        _ => {}
                    },
                    embassy_futures::select::Either::Second(notification) => {
                        let mut rsc_buf = [0u8; RSC_MEASUREMENT_BUF_SIZE];
                        let result = convert_ftms_to_rsc(notification.as_ref(), &mut rsc_buf);
                        drop(notification);
                        match result {
                            Ok(len) => {
                                if notify_count == 0 {
                                    defmt::info!("First RSC: {} bytes, data={=[u8]:x}", len, &rsc_buf[..len]);
                                }
                                if server.rsc.measurement.notify(&watch_gatt, &rsc_buf).await.is_err() {
                                    if notify_count > 0 { break; }
                                } else {
                                    notify_count += 1;
                                }
                            }
                            Err(e) => defmt::warn!("Convert error: {:?}", e),
                        }
                        let now = embassy_time::Instant::now();
                        if now.duration_since(last_heartbeat).as_secs() >= HEARTBEAT_INTERVAL_SECS {
                            defmt::info!("Heartbeat: {} notifications", notify_count);
                            last_heartbeat = now;
                        }
                    }
                }
            }
            Ok(())
        };

        match embassy_futures::select::select(ftms_client.task(), ftms_work).await {
            embassy_futures::select::Either::First(_) => {
                defmt::info!("FTMS client task ended");
                Ok(())
            }
            embassy_futures::select::Either::Second(result) => result,
        }
    };

    match embassy_futures::select::select(disconnect_watcher, bridge_logic).await {
        embassy_futures::select::Either::First(_) => {
            defmt::info!("Treadmill disconnected");
            Ok(())
        }
        embassy_futures::select::Either::Second(result) => result,
    }
}

async fn advertise_rsc<'values, 'server, C: Controller>(
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server ble::Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BridgeError> {
    let mut adv_buf = [0u8; MAX_ADV_DATA_LEN];
    let adv_len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[RSC_SERVICE_UUID_LE]),
        ],
        &mut adv_buf,
    ).map_err(|_| BridgeError::Advertise)?;

    let mut scan_buf = [0u8; MAX_ADV_DATA_LEN];
    let scan_len = AdStructure::encode_slice(
        &[AdStructure::CompleteLocalName(DEVICE_NAME)],
        &mut scan_buf,
    ).map_err(|_| BridgeError::Advertise)?;

    defmt::info!("RSC advertising started");
    let advertiser = peripheral.advertise(
        &Default::default(),
        Advertisement::ConnectableScannableUndirected {
            adv_data: &adv_buf[..adv_len],
            scan_data: &scan_buf[..scan_len],
        },
    ).await.map_err(|e| {
        defmt::warn!("Advertise failed: {:?}", defmt::Debug2Format(&e));
        BridgeError::Advertise
    })?;

    let conn = advertiser.accept().await
        .map_err(|e| { defmt::warn!("Accept failed: {:?}", defmt::Debug2Format(&e)); BridgeError::Advertise })?
        .with_attribute_server(server)
        .map_err(|e| { defmt::warn!("GATT attach failed: {:?}", defmt::Debug2Format(&e)); BridgeError::Advertise })?;

    defmt::info!("Watch connected via RSC");
    Ok(conn)
}

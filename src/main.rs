#![no_std]
#![no_main]

//! TreadLink — FTMS treadmill → RSC bridge for nRF52840.

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_nrf::{bind_interrupts, rng};
use embassy_time::{Duration, Timer, with_timeout};
use nrf_sdc::mpsl::MultiprotocolServiceLayer;
use nrf_sdc::{self as sdc, mpsl};
use panic_probe as _;
use static_cell::StaticCell;
use trouble_host::prelude::*;
use trouble_host::connection::RequestedConnParams;

use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;
use heapless::Vec as HVec;

mod central;
mod converter;
mod led;
mod peripheral;
mod protocol;

use central::BridgeState;
use converter::assemble_rsc;
use protocol::{parse_treadmill_data, RscMeasurement};

bind_interrupts!(struct Irqs {
    RNG => rng::InterruptHandler<embassy_nrf::peripherals::RNG>;
    EGU0_SWI0 => mpsl::LowPrioInterruptHandler;
    CLOCK_POWER => mpsl::ClockInterruptHandler;
    RADIO => mpsl::HighPrioInterruptHandler;
    TIMER0 => mpsl::HighPrioInterruptHandler;
    RTC0 => mpsl::HighPrioInterruptHandler;
});

#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}

const L2CAP_TXQ: u8 = 6;
const L2CAP_RXQ: u8 = 6;
const SCAN_SECS: u64 = 3;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const SETTLE_MS: u64 = 500;

fn build_sdc<'d, const N: usize>(
    p: sdc::Peripherals<'d>,
    rng: &'d mut rng::Rng<'static, embassy_nrf::mode::Async>,
    mpsl: &'d MultiprotocolServiceLayer,
    mem: &'d mut sdc::Mem<N>,
) -> Result<sdc::SoftdeviceController<'d>, sdc::Error> {
    sdc::Builder::new()?
        .support_scan()
        .support_central()
        .support_peripheral()
        .support_adv()
        .central_count(1)?
        .peripheral_count(1)?
        .buffer_cfg(
            DefaultPacketPool::MTU as u16,
            DefaultPacketPool::MTU as u16,
            L2CAP_TXQ,
            L2CAP_RXQ,
        )?
        .build(p, rng, mpsl, mem)
}

// ---------------------------------------------------------------------------
// FTMS advertisement handler
// ---------------------------------------------------------------------------

/// Stored scan candidate
#[derive(Clone, Copy)]
struct Candidate {
    addr: BdAddr,
    kind: AddrKind,
    rssi: i8,
}

struct FtmsHandler {
    candidates: RefCell<HVec<Candidate, 8>>,
}

impl FtmsHandler {
    const fn new() -> Self {
        Self { candidates: RefCell::new(HVec::new()) }
    }

    fn clear(&self) {
        self.candidates.borrow_mut().clear();
    }

    fn best(&self) -> Option<Candidate> {
        self.candidates.borrow().iter().max_by_key(|c| c.rssi).copied()
    }
}

/// Check if raw adv data contains FTMS UUID 0x1826
fn has_ftms_uuid(data: &[u8]) -> bool {
    let mut i = 0;
    while i < data.len() {
        let len = data[i] as usize;
        if len == 0 || i + len >= data.len() {
            break;
        }
        let ad_type = data[i + 1];
        // 0x02 = Incomplete 16-bit UUIDs, 0x03 = Complete 16-bit UUIDs
        if ad_type == 0x02 || ad_type == 0x03 {
            let payload = &data[i + 2..i + 1 + len];
            let mut j = 0;
            while j + 1 < payload.len() {
                if u16::from_le_bytes([payload[j], payload[j + 1]]) == 0x1826 {
                    return true;
                }
                j += 2;
            }
        }
        i += 1 + len;
    }
    false
}

impl EventHandler for FtmsHandler {
    fn on_adv_reports(&self, mut it: bt_hci::param::LeAdvReportsIter<'_>) {
        let mut candidates = self.candidates.borrow_mut();
        while let Some(Ok(report)) = it.next() {
            if has_ftms_uuid(report.data) {
                // Update existing or insert new
                if let Some(c) = candidates.iter_mut().find(|c| c.addr == report.addr) {
                    if report.rssi > c.rssi {
                        c.rssi = report.rssi;
                    }
                } else {
                    let _ = candidates.push(Candidate {
                        addr: report.addr,
                        kind: report.addr_kind,
                        rssi: report.rssi,
                    });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());
    info!("TreadLink booting...");

    let mpsl_p = mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31);
    let lfclk_cfg = mpsl::raw::mpsl_clock_lfclk_cfg_t {
        source: mpsl::raw::MPSL_CLOCK_LF_SRC_RC as u8,
        rc_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_CTIV as u8,
        rc_temp_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_TEMP_CTIV as u8,
        accuracy_ppm: mpsl::raw::MPSL_DEFAULT_CLOCK_ACCURACY_PPM as u16,
        skip_wait_lfclk_started: mpsl::raw::MPSL_DEFAULT_SKIP_WAIT_LFCLK_STARTED != 0,
    };

    static MPSL: StaticCell<MultiprotocolServiceLayer> = StaticCell::new();
    let mpsl = MPSL.init(mpsl::MultiprotocolServiceLayer::new(mpsl_p, Irqs, lfclk_cfg).unwrap());
    spawner.spawn(mpsl_task(mpsl).expect("spawn mpsl"));

    let sdc_p = sdc::Peripherals::new(
        p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23,
        p.PPI_CH24, p.PPI_CH25, p.PPI_CH26, p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
    );

    static RNG: StaticCell<rng::Rng<'static, embassy_nrf::mode::Async>> = StaticCell::new();
    let rng_inst = RNG.init(rng::Rng::new(p.RNG, Irqs));

    static MEM: StaticCell<sdc::Mem<9072>> = StaticCell::new();
    let mem = MEM.init(sdc::Mem::new());

    let sdc = build_sdc(sdc_p, rng_inst, mpsl, mem).unwrap();
    info!("SDC initialized");

    static RESOURCES: StaticCell<HostResources<sdc::SoftdeviceController<'static>, DefaultPacketPool, 2, 6>> = StaticCell::new();
    let resources = RESOURCES.init(HostResources::new());

    let address = {
        let ficr = embassy_nrf::pac::FICR;
        let addr_lo = ficr.deviceaddr(0).read();
        let addr_hi = ficr.deviceaddr(1).read() as u16;
        Address::random([
            addr_lo as u8,
            (addr_lo >> 8) as u8,
            (addr_lo >> 16) as u8,
            (addr_lo >> 24) as u8,
            addr_hi as u8,
            (addr_hi >> 8) as u8 | 0xC0,
        ])
    };

    static STACK: StaticCell<Stack<'static, sdc::SoftdeviceController<'static>, DefaultPacketPool>> = StaticCell::new();
    let stack: &'static _ = STACK.init(trouble_host::new(sdc, resources).set_random_address(address).build());

    let mut central = stack.central();
    let mut peripheral = stack.peripheral();
    let mut runner = stack.runner();

    use embassy_nrf::gpio::{Level, Output, OutputDrive};

    static HANDLER: StaticCell<FtmsHandler> = StaticCell::new();
    let handler: &'static FtmsHandler = HANDLER.init(FtmsHandler::new());

    let led1 = Output::new(p.P0_13, Level::High, OutputDrive::Standard);
    let led2 = Output::new(p.P0_14, Level::High, OutputDrive::Standard);
    let led3 = Output::new(p.P0_15, Level::High, OutputDrive::Standard);
    let led4 = Output::new(p.P0_16, Level::High, OutputDrive::Standard);

    // GATT server — use new_with_config matching working treadlink-old
    static SERVER: StaticCell<peripheral::Server> = StaticCell::new();
    let server: &'static _ = SERVER.init(peripheral::Server::new_with_config(
        GapConfig::Peripheral(PeripheralConfig {
            name: "TreadLink",
            appearance: &appearance::UNKNOWN,
        })
    ).unwrap());

    static SIGNAL: StaticCell<Signal<CriticalSectionRawMutex, RscMeasurement>> = StaticCell::new();
    let signal: &'static _ = SIGNAL.init(Signal::new());

    static WATCH: StaticCell<Watch<CriticalSectionRawMutex, BridgeState, 2>> = StaticCell::new();
    let watch: &'static _ = WATCH.init(Watch::new());
    watch.sender().send(BridgeState::Scanning);

    spawner.spawn(runner_task(runner, handler).expect("spawn runner"));
    spawner.spawn(central_task(central, stack, signal, watch, handler).expect("spawn central"));
    spawner.spawn(led::led_task(watch, led1, led2, led3, led4).expect("spawn led"));
    spawner.spawn(peripheral::peripheral_task(peripheral, server, signal, watch).expect("spawn peripheral"));

    info!("All tasks spawned");
}

// ---------------------------------------------------------------------------
// Central task — concrete, no generics
// ---------------------------------------------------------------------------

#[embassy_executor::task]
async fn central_task(
    mut central: Central<'static, sdc::SoftdeviceController<'static>, DefaultPacketPool>,
    stack: &'static Stack<'static, sdc::SoftdeviceController<'static>, DefaultPacketPool>,
    signal: &'static Signal<CriticalSectionRawMutex, RscMeasurement>,
    watch: &'static Watch<CriticalSectionRawMutex, BridgeState, 2>,
    handler: &'static FtmsHandler,
) {
    loop {
        // --- SCAN ---
        watch.sender().send(BridgeState::Scanning);
        handler.clear();
        info!("[central] Scanning for FTMS devices...");

        {
            let mut scanner = Scanner::new(central);
            let scan_config = ScanConfig {
                active: false,
                filter_accept_list: &[],
                timeout: Duration::from_secs(SCAN_SECS),
                ..Default::default()
            };

            let scan_ok = match scanner.scan(&scan_config).await {
                Ok(session) => {
                    Timer::after(Duration::from_secs(SCAN_SECS)).await;
                    drop(session);
                    true
                }
                Err(e) => {
                    error!("[central] Scan failed: {:?}", e);
                    false
                }
            };
            central = scanner.into_inner();
            if !scan_ok {
                Timer::after(Duration::from_secs(2)).await;
                continue;
            }
        }

        // Pick best candidate
        let candidate = match handler.best() {
            Some(c) => c,
            None => {
                info!("[central] No FTMS devices found");
                continue;
            }
        };
        info!("[central] Found treadmill rssi={}", candidate.rssi);

        // --- SETTLE ---
        Timer::after(Duration::from_millis(SETTLE_MS)).await;

        // --- CONNECT ---
        watch.sender().send(BridgeState::Connecting);
        info!("[central] Connecting...");

        let conn = match central.connect(&ConnectConfig {
            scan_config: ScanConfig {
                filter_accept_list: &[Address { kind: candidate.kind, addr: candidate.addr }],
                timeout: Duration::from_secs(10),
                ..Default::default()
            },
            connect_params: RequestedConnParams {
                min_connection_interval: Duration::from_millis(20),
                max_connection_interval: Duration::from_millis(20),
                max_latency: 0,
                supervision_timeout: Duration::from_millis(2560),
                min_event_length: Duration::from_micros(0),
                max_event_length: Duration::from_micros(0),
            },
        }).await {
            Ok(conn) => conn,
            Err(e) => {
                warn!("[central] Connect failed: {:?}", e);
                Timer::after(Duration::from_secs(1)).await;
                continue;
            }
        };

        info!("[central] Connected!");

        // --- GATT DISCOVERY + BRIDGE ---
        let bridge_result = bridge(&conn, stack, signal, watch).await;
        if let Err(()) = bridge_result {
            warn!("[central] Bridge failed");
        }

        info!("[central] Disconnected, will rescan...");
        Timer::after(Duration::from_secs(1)).await;
    }
}

/// GATT discovery, subscribe, and bridge loop. Returns Ok(()) on clean disconnect,
/// Err(()) on discovery/subscribe failure.
async fn bridge<'a>(
    conn: &Connection<'a, DefaultPacketPool>,
    stack: &'a Stack<'a, sdc::SoftdeviceController<'a>, DefaultPacketPool>,
    signal: &Signal<CriticalSectionRawMutex, RscMeasurement>,
    watch: &Watch<CriticalSectionRawMutex, BridgeState, 2>,
) -> Result<(), ()> {
    // Create GATT client (performs MTU exchange)
    let client: GattClient<'_, sdc::SoftdeviceController<'_>, DefaultPacketPool, 4> =
        match with_timeout(CONNECT_TIMEOUT, GattClient::new(stack, conn)).await {
            Ok(Ok(c)) => c,
            _ => {
                error!("[central] GATT client creation failed");
                return Err(());
            }
        };

    // Discover FTMS service (0x1826)
    let ftms_uuid = Uuid::new_short(0x1826);

    // client.task() must run concurrently for GATT operations to work
    let discovery_and_bridge = async {
        let services = match client.services_by_uuid(&ftms_uuid).await {
            Ok(s) => s,
            Err(e) => {
                error!("[central] Service discovery error: {:?}", e);
                return;
            }
        };
        let service = match services.first() {
            Some(s) => s,
            None => {
                error!("[central] FTMS service not found");
                return;
            }
        };
        info!("[central] Found FTMS service");

        // Discover Treadmill Data characteristic (0x2ACD)
        let td_uuid = Uuid::new_short(0x2ACD);
        let characteristic: Characteristic<[u8]> = match client.characteristic_by_uuid(service, &td_uuid).await {
            Ok(c) => c,
            Err(e) => {
                error!("[central] Treadmill Data char not found: {:?}", e);
                return;
            }
        };
        info!("[central] Found Treadmill Data characteristic");

        // Subscribe to notifications
        let mut listener = match client.subscribe(&characteristic, false).await {
            Ok(l) => l,
            Err(e) => {
                error!("[central] Subscribe failed: {:?}", e);
                return;
            }
        };
        info!("[central] Subscribed — bridge active");
        watch.sender().send(BridgeState::Bridging);

        // Bridge loop: forward FTMS notifications as RSC measurements
        loop {
            let data = listener.next().await;
            match parse_treadmill_data(data.as_ref()) {
                Ok(td) => {
                    let rsc = assemble_rsc(&td);
                    defmt::info!("[bridge] speed={} cadence={}", rsc.speed, rsc.cadence);
                    signal.signal(rsc);
                }
                Err(e) => {
                    warn!("[central] Parse error: {:?}", e);
                }
            }
        }
    };

    // Run client task alongside discovery+bridge. Either ending means disconnect.
    let _result = select(client.task(), discovery_and_bridge).await;

    Ok(())
}

// ---------------------------------------------------------------------------
// Runner task
// ---------------------------------------------------------------------------

#[embassy_executor::task]
async fn runner_task(
    mut runner: Runner<'static, sdc::SoftdeviceController<'static>, DefaultPacketPool>,
    handler: &'static FtmsHandler,
) {
    runner.run_with_handler(handler).await.unwrap();
}

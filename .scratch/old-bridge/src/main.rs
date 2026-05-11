#![no_std]
#![no_main]
#![deny(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

mod ble;
mod bridge;
mod scan;

use embassy_executor::Spawner;
use embassy_nrf::bind_interrupts;
use embassy_time::Instant;
use nrf_sdc::mpsl;
use static_cell::StaticCell;
use trouble_host::prelude::*;
use {defmt_rtt as _, panic_probe as _};

/// Number of central connections supported.
const CENTRAL_COUNT: u8 = 1;

/// Number of peripheral connections supported.
const PERIPHERAL_COUNT: u8 = 1;

/// Firmware version string for boot log.
const FIRMWARE_VERSION: &str = "0.1.0";

/// Max simultaneous BLE connections (1 central + 1 peripheral).
const CONNECTIONS_MAX: usize = 2;

/// L2CAP channel count for trouble-host resources.
const L2CAP_CHANNELS_MAX: usize = 6;

/// How many outgoing L2CAP buffers per link.
const L2CAP_TXQ: u8 = 6;

/// How many incoming L2CAP buffers per link.
const L2CAP_RXQ: u8 = 6;

/// Static memory pool size for SDC.
const SDC_MEM_SIZE: usize = 9072;

/// Bitmask to set the two most-significant bits of a BLE random static address.
const RANDOM_STATIC_ADDR_MASK: u8 = 0xC0;

/// Low-frequency clock accuracy in parts per million (external crystal).
const LFCLK_ACCURACY_PPM: u16 = 20;

/// BLE device name used in GAP configuration.
const DEVICE_NAME: &str = "TreadLink";

defmt::timestamp!("{=u64:us}", Instant::now().as_micros());

bind_interrupts!(struct Irqs {
    RNG => embassy_nrf::rng::InterruptHandler<embassy_nrf::peripherals::RNG>;
    EGU0_SWI0 => mpsl::LowPrioInterruptHandler;
    CLOCK_POWER => mpsl::ClockInterruptHandler;
    RADIO => mpsl::HighPrioInterruptHandler;
    TIMER0 => mpsl::HighPrioInterruptHandler;
    RTC0 => mpsl::HighPrioInterruptHandler;
});

#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static mpsl::MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = embassy_nrf::interrupt::Priority::P2;
    config.time_interrupt_priority = embassy_nrf::interrupt::Priority::P2;
    let p = embassy_nrf::init(config);

    defmt::info!("TreadLink firmware v{}", FIRMWARE_VERSION);

    // --- MPSL initialization ---
    let mpsl_p = mpsl::Peripherals::new(
        p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31,
    );
    let lfclk_cfg = mpsl::raw::mpsl_clock_lfclk_cfg_t {
        source: mpsl::raw::MPSL_CLOCK_LF_SRC_XTAL as u8,
        rc_ctiv: 0,
        rc_temp_ctiv: 0,
        accuracy_ppm: LFCLK_ACCURACY_PPM,
        skip_wait_lfclk_started: false,
    };

    static MPSL: StaticCell<mpsl::MultiprotocolServiceLayer> = StaticCell::new();
    let mpsl = MPSL.init(match mpsl::MultiprotocolServiceLayer::new(mpsl_p, Irqs, lfclk_cfg) {
        Ok(m) => m,
        Err(e) => {
            defmt::error!("MPSL init failed: {:?}", e);
            loop {}
        }
    });
    defmt::info!("MPSL initialized");

    match mpsl_task(mpsl) {
        Ok(token) => spawner.spawn(token),
        Err(_) => {
            defmt::error!("Failed to spawn mpsl_task");
            loop {}
        }
    }

    // --- SDC initialization ---
    let sdc_p = nrf_sdc::Peripherals::new(
        p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22,
        p.PPI_CH23, p.PPI_CH24, p.PPI_CH25, p.PPI_CH26, p.PPI_CH27,
        p.PPI_CH28, p.PPI_CH29,
    );

    static RNG: StaticCell<embassy_nrf::rng::Rng<'static, embassy_nrf::mode::Async>> = StaticCell::new();
    let rng = RNG.init(embassy_nrf::rng::Rng::new(p.RNG, Irqs));

    // Read the factory-programmed device address from FICR registers.
    // This gives a stable address so the watch reconnects to the same sensor.
    let ficr = embassy_nrf::pac::FICR;
    let addr_lo = ficr.deviceaddr(0).read();
    let addr_hi = ficr.deviceaddr(1).read() as u16;
    let addr_bytes = [
        addr_lo as u8,
        (addr_lo >> 8) as u8,
        (addr_lo >> 16) as u8,
        (addr_lo >> 24) as u8,
        addr_hi as u8,
        (addr_hi >> 8) as u8 | RANDOM_STATIC_ADDR_MASK, // MSBs = 11 for random static
    ];
    let address = Address::random(addr_bytes);
    defmt::info!("BLE address: {:?}", address);

    static SDC_MEM: StaticCell<nrf_sdc::Mem<SDC_MEM_SIZE>> = StaticCell::new();
    let sdc_mem = SDC_MEM.init(nrf_sdc::Mem::new());

    let sdc = match build_sdc(sdc_p, rng, mpsl, sdc_mem) {
        Ok(sdc) => sdc,
        Err(e) => {
            defmt::error!("SDC init failed: {:?}", e);
            loop {}
        }
    };
    defmt::info!("SDC initialized");

    // --- trouble-host Stack construction ---
    static HOST_RESOURCES: StaticCell<HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>> =
        StaticCell::new();
    let resources = HOST_RESOURCES.init(HostResources::new());

    static STACK: StaticCell<Stack<'static, nrf_sdc::SoftdeviceController<'static>, DefaultPacketPool>> =
        StaticCell::new();
    let stack: &'static _ = STACK.init(trouble_host::new(sdc, resources).set_random_address(address));

    // --- GATT server (created before build() so it shares the same lifetime scope) ---
    static SERVER: StaticCell<ble::Server<'static>> = StaticCell::new();
    let server: &'static _ = SERVER.init(match ble::Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: DEVICE_NAME,
        appearance: &appearance::UNKNOWN,
    })) {
        Ok(s) => s,
        Err(e) => {
            defmt::error!("GATT server registration failed: {:?}", defmt::Debug2Format(&e));
            loop {}
        }
    });

    let Host {
        mut peripheral,
        mut runner,
        central,
        ..
    } = stack.build();

    defmt::info!("BLE stack initialized");
    defmt::info!(
        "Config: connections={}, l2cap_channels={}, sdc_mem={}",
        CONNECTIONS_MAX,
        L2CAP_CHANNELS_MAX,
        SDC_MEM_SIZE,
    );

    // Run the BLE runner concurrently with the state machine.
    // The runner uses our FtmsScanHandler to receive advertisement reports.
    let scan_handler = scan::FtmsScanHandler;

    let state_machine = async {
        // Wrap central in a Scanner for the scanning phase.
        // Scanner takes ownership of Central, but we can get it back via into_inner().
        let mut scanner = Scanner::new(central);

        loop {
            // Phase 1: Scan for FTMS treadmill
            let addr = scan::find_treadmill(&mut scanner).await;

            // Phase 2: Connect to treadmill — need Central back from Scanner
            let mut central = scanner.into_inner();
            // Include both PUBLIC and RANDOM addr kinds in the filter accept list,
            // since some treadmills report one kind in advertisements but actually
            // use the other. The controller will match whichever is correct.
            let connect_config = ConnectConfig {
                scan_config: ScanConfig {
                    filter_accept_list: &[
                        (bt_hci::param::AddrKind::PUBLIC, &addr.addr),
                        (bt_hci::param::AddrKind::RANDOM, &addr.addr),
                    ],
                    ..Default::default()
                },
                connect_params: trouble_host::connection::RequestedConnParams {
                    min_connection_interval: embassy_time::Duration::from_millis(15),
                    max_connection_interval: embassy_time::Duration::from_millis(30),
                    max_latency: 4,
                    supervision_timeout: embassy_time::Duration::from_secs(5),
                    ..Default::default()
                },
            };
            let conn = match central.connect(&connect_config).await {
                Ok(c) => {
                    defmt::info!("Connected to treadmill");
                    c
                }
                Err(e) => {
                    defmt::warn!("Connect failed: {:?}", defmt::Debug2Format(&e));
                    scanner = Scanner::new(central);
                    continue;
                }
            };

            // Phase 3: Bridge — relay FTMS→RSC
            match bridge::run(&stack, conn, &server, &mut peripheral).await {
                Ok(()) => defmt::info!("Bridge ended normally, rescanning"),
                Err(e) => defmt::warn!("Bridge error: {:?}, rescanning", e),
            }

            // Wrap central back into Scanner for next scan cycle
            scanner = Scanner::new(central);
        }
    };

    let ble_runner = async {
        loop {
            if let Err(e) = runner.run_with_handler(&scan_handler).await {
                defmt::warn!("BLE runner error: {:?}", defmt::Debug2Format(&e));
            }
        }
    };

    embassy_futures::join::join(ble_runner, state_machine).await;
}

/// Build the SDC with dual-role support (scan + adv + central + peripheral).
fn build_sdc<'d>(
    sdc_p: nrf_sdc::Peripherals<'d>,
    rng: &'d mut embassy_nrf::rng::Rng<'d, embassy_nrf::mode::Async>,
    mpsl: &'d mpsl::MultiprotocolServiceLayer<'d>,
    mem: &'d mut nrf_sdc::Mem<SDC_MEM_SIZE>,
) -> Result<nrf_sdc::SoftdeviceController<'d>, nrf_sdc::Error> {
    nrf_sdc::Builder::new()?
        .support_scan()
        .support_adv()
        .support_central()
        .support_peripheral()
        .central_count(CENTRAL_COUNT)?
        .peripheral_count(PERIPHERAL_COUNT)?
        .buffer_cfg(
            DefaultPacketPool::MTU as u16,
            DefaultPacketPool::MTU as u16,
            L2CAP_TXQ,
            L2CAP_RXQ,
        )?
        .build(sdc_p, rng, mpsl, mem)
}

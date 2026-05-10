#![no_std]
#![no_main]

//! Scan-then-connect test — reproduces the scan→connect transition
//! in a minimal binary to isolate the SDC panic.

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{bind_interrupts, rng};
use embassy_time::{Duration, Timer};
use nrf_sdc::mpsl::MultiprotocolServiceLayer;
use nrf_sdc::{self as sdc, mpsl};
use panic_probe as _;
use static_cell::StaticCell;
use trouble_host::prelude::*;
use bt_hci::param::LeAdvReportsIter;

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

const L2CAP_TXQ: u8 = 3;
const L2CAP_RXQ: u8 = 3;

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

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());
    info!("scan_connect_test: booting");

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

    static MEM: StaticCell<sdc::Mem<8192>> = StaticCell::new();
    let mem = MEM.init(sdc::Mem::new());

    let sdc = build_sdc(sdc_p, rng_inst, mpsl, mem).unwrap();
    info!("scan_connect_test: SDC initialized");

    static RESOURCES: StaticCell<HostResources<DefaultPacketPool, 2, 1, 1>> = StaticCell::new();
    let resources = RESOURCES.init(HostResources::new());

    let address = Address::random([0xff, 0x8f, 0x1b, 0x05, 0xe4, 0xff]);

    static STACK: StaticCell<Stack<'static, sdc::SoftdeviceController<'static>, DefaultPacketPool>> = StaticCell::new();
    let stack = STACK.init(trouble_host::new(sdc, resources).set_random_address(address));

    let host = stack.build();
    let Host { central, runner, .. } = host;

    static SCANNER: StaticCell<FtmsHandler> = StaticCell::new();
    let handler = SCANNER.init(FtmsHandler::new());
    spawner.spawn(runner_task(runner, handler).expect("spawn runner"));

    // Run scan-then-connect in a separate spawned task (like main binary does)
    spawner.spawn(central_task(central).expect("spawn central"));

    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}

#[embassy_executor::task]
async fn central_task(
    central: Central<'static, sdc::SoftdeviceController<'static>, DefaultPacketPool>,
) {
    scan_then_connect(central).await;
}

async fn scan_then_connect(
    mut central: Central<'static, sdc::SoftdeviceController<'static>, DefaultPacketPool>,
) {
    // Phase 1: Scan for 3 seconds
    info!("scan_connect_test: starting scan...");
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
                info!("scan_connect_test: scan running, waiting 3s...");
                Timer::after(Duration::from_secs(3)).await;
                drop(session);
                info!("scan_connect_test: scan session dropped");
            }
            Err(_) => {
                error!("scan_connect_test: scan failed");
            }
        }

        central = scanner.into_inner();
    }

    // Phase 2: Wait for scan to settle
    info!("scan_connect_test: waiting 500ms for scan to settle...");
    Timer::after(Duration::from_millis(500)).await;

    // Phase 3: Connect to the treadmill
    let target_addr = BdAddr::new([124, 49, 0, 8, 192, 246]);
    let target = (AddrKind::RANDOM, &target_addr);

    info!("scan_connect_test: connecting...");
    let config = ConnectConfig {
        connect_params: Default::default(),
        scan_config: ScanConfig {
            filter_accept_list: &[target],
            ..Default::default()
        },
    };

    match central.connect(&config).await {
        Ok(conn) => {
            info!("scan_connect_test: CONNECTED! handle={}", conn.handle());
            Timer::after(Duration::from_secs(5)).await;
        }
        Err(e) => {
            error!("scan_connect_test: connect failed: {:?}", e);
        }
    }
}

use core::cell::RefCell;
use heapless::Vec as HVec;

struct FtmsHandler {
    events: RefCell<HVec<i8, 32>>,
}

impl FtmsHandler {
    const fn new() -> Self {
        Self { events: RefCell::new(HVec::new()) }
    }
}

impl EventHandler for FtmsHandler {
    fn on_adv_reports(&self, mut it: LeAdvReportsIter<'_>) {
        let mut events = self.events.borrow_mut();
        while let Some(Ok(report)) = it.next() {
            let _ = events.push(report.rssi);
        }
    }
}

#[embassy_executor::task]
async fn runner_task(
    mut runner: Runner<'static, sdc::SoftdeviceController<'static>, DefaultPacketPool>,
    handler: &'static FtmsHandler,
) {
    runner.run_with_handler(handler).await.unwrap();
}

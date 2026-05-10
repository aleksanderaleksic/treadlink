#![no_std]
#![no_main]

//! Minimal RSC peripheral test using trouble-host from main branch.

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::select::select;
use embassy_nrf::{bind_interrupts, rng};
use embassy_time::{Duration, Timer};
use nrf_sdc::mpsl::MultiprotocolServiceLayer;
use nrf_sdc::{self as sdc, mpsl};
use panic_probe as _;
use static_cell::StaticCell;
use trouble_host::prelude::*;

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
const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 2;

#[gatt_server]
struct Server {
    rsc: RscService,
}

#[gatt_service(uuid = "1814")]
struct RscService {
    #[characteristic(uuid = "2A53", read, notify)]
    measurement: [u8; 8],

    #[characteristic(uuid = "2A54", read, value = [0x02, 0x00])]
    feature: [u8; 2],
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());
    info!("rsc_test: booting");

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
    let sdc_mem = MEM.init(sdc::Mem::new());

    let sdc = sdc::Builder::new().unwrap()
        .support_adv()
        .support_peripheral()
        .peripheral_count(1).unwrap()
        .buffer_cfg(DefaultPacketPool::MTU as u16, DefaultPacketPool::MTU as u16, L2CAP_TXQ, L2CAP_RXQ).unwrap()
        .build(sdc_p, rng_inst, mpsl, sdc_mem).unwrap();

    info!("rsc_test: SDC initialized");

    // Use FICR address
    let ficr = embassy_nrf::pac::FICR;
    let addr_lo = ficr.deviceaddr(0).read();
    let addr_hi = ficr.deviceaddr(1).read() as u16;
    let address: Address = Address::random([
        addr_lo as u8, (addr_lo >> 8) as u8, (addr_lo >> 16) as u8, (addr_lo >> 24) as u8,
        addr_hi as u8, (addr_hi >> 8) as u8 | 0xC0,
    ]);
    info!("rsc_test: address = {:?}", address);

    // New trouble-host API from main branch
    let mut resources: HostResources<_, DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> = HostResources::new();
    let stack = trouble_host::new(sdc, &mut resources)
        .set_random_address(address)
        .build();

    let mut peripheral = stack.peripheral();
    let mut runner = stack.runner();

    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "TreadLink",
        appearance: &appearance::UNKNOWN,
    })).unwrap();

    info!("rsc_test: starting");

    let _ = join(
        async {
            loop {
                if let Err(e) = runner.run().await {
                    error!("runner error: {:?}", defmt::Debug2Format(&e));
                }
            }
        },
        async {
            loop {
                // Advertise
                let mut adv_buf = [0u8; 31];
                let adv_len = AdStructure::encode_slice(
                    &[
                        AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                        AdStructure::IncompleteServiceUuids16(&[[0x14, 0x18]]),
                        AdStructure::CompleteLocalName(b"TreadLink"),
                    ],
                    &mut adv_buf,
                ).unwrap_or(0);

                let advertiser = match peripheral.advertise(
                    &Default::default(),
                    Advertisement::ConnectableScannableUndirected {
                        adv_data: &adv_buf[..adv_len],
                        scan_data: &[],
                    },
                ).await {
                    Ok(a) => a,
                    Err(e) => {
                        error!("advertise failed: {:?}", defmt::Debug2Format(&e));
                        Timer::after(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                info!("rsc_test: advertising...");

                let conn = match advertiser.accept().await {
                    Ok(c) => match c.with_attribute_server(&server) {
                        Ok(gc) => gc,
                        Err(e) => { error!("gatt attach: {:?}", defmt::Debug2Format(&e)); continue; }
                    },
                    Err(e) => { error!("accept: {:?}", defmt::Debug2Format(&e)); continue; }
                };

                info!("rsc_test: connected!");

                // Handle GATT events + send notifications
                let gatt_events = async {
                    let level = server.rsc.measurement;
                    loop {
                        match conn.next().await {
                            GattConnectionEvent::Disconnected { reason } => {
                                info!("rsc_test: disconnected: {:?}", reason);
                                break;
                            }
                            GattConnectionEvent::Gatt { event } => {
                                let is_write = matches!(&event, GattEvent::Write(_));
                                info!("rsc_test: GATT event (write={})", is_write);
                                match event.accept() {
                                    Ok(reply) => reply.send().await,
                                    Err(e) => warn!("accept err: {:?}", defmt::Debug2Format(&e)),
                                }
                            }
                            _ => {}
                        }
                    }
                };

                let notifications = async {
                    Timer::after(Duration::from_secs(2)).await;
                    let mut tick: u32 = 0;
                    loop {
                        tick += 1;
                        let speed: u16 = 711;
                        let distance: u32 = tick * 28;
                        let payload: [u8; 8] = [
                            0x02,
                            (speed & 0xFF) as u8, (speed >> 8) as u8,
                            180,
                            (distance & 0xFF) as u8, ((distance >> 8) & 0xFF) as u8,
                            ((distance >> 16) & 0xFF) as u8, ((distance >> 24) & 0xFF) as u8,
                        ];
                        if server.rsc.measurement.notify(&conn, &payload).await.is_err() {
                            info!("rsc_test: notify failed");
                            break;
                        }
                        if tick <= 3 || tick % 10 == 0 {
                            info!("rsc_test: notify #{}", tick);
                        }
                        Timer::after(Duration::from_secs(1)).await;
                    }
                };

                select(gatt_events, notifications).await;
            }
        },
    ).await;
}

# trouble-host + nrf-sdc Reference

## Cargo.toml Configuration

```toml
[dependencies]
trouble-host = { version = "0.6", features = ["central", "peripheral", "gatt", "scan", "default-packet-pool"] }
bt-hci = { version = "0.8", features = ["defmt"] }
nrf-sdc = { version = "0.4", default-features = false, features = ["nrf52840", "peripheral", "central", "defmt"] }
nrf-mpsl = { version = "0.3", default-features = false, features = ["defmt", "critical-section-impl"] }
embassy-executor = { version = "0.10", features = ["defmt", "platform-cortex-m", "executor-thread"] }
embassy-nrf = { version = "0.10", features = ["nrf52840", "time-driver-rtc1"] }
embassy-time = "0.5"
embassy-sync = "0.7"
embassy-futures = "0.1"
static_cell = "2"
```

### Feature Flags (trouble-host)

| Feature | Purpose |
|---------|---------|
| `central` | Enable Central role (scan + connect) |
| `peripheral` | Enable Peripheral role (advertise + accept) |
| `gatt` | GATT server/client support |
| `scan` | Scanner type for active/passive scanning |
| `default-packet-pool` | Provides `DefaultPacketPool` type alias |
| `derive` | Proc-macro support for `#[gatt_server]` / `#[gatt_service]` |
| `defmt` | defmt logging integration |

### Feature Flags (nrf-sdc)

| Feature | Purpose |
|---------|---------|
| `nrf52840` | Target chip |
| `nrf52832` | Target chip |
| `nrf5340` | Target chip (app core) |
| `peripheral` | BLE peripheral role support |
| `central` | BLE central role support |
| `defmt` | defmt logging |

## MPSL Initialization

MPSL (Multiprotocol Service Layer) is required by the SoftDevice Controller.

```rust
use nrf_sdc::mpsl::{MultiprotocolServiceLayer, Peripherals as MpslPeripherals};

// Required peripherals: RTC0, TIMER0, TEMP, 3 PPI channels
let mpsl_p = MpslPeripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31);

let lfclk_cfg = nrf_sdc::mpsl::raw::mpsl_clock_lfclk_cfg_t {
    source: nrf_sdc::mpsl::raw::MPSL_CLOCK_LF_SRC_RC as u8,
    rc_ctiv: nrf_sdc::mpsl::raw::MPSL_RECOMMENDED_RC_CTIV as u8,
    rc_temp_ctiv: nrf_sdc::mpsl::raw::MPSL_RECOMMENDED_RC_TEMP_CTIV as u8,
    accuracy_ppm: nrf_sdc::mpsl::raw::MPSL_WORST_CLOCK_ACCURACY_PPM as u16,
    skip_wait_lfclk_started: false,
};

let mpsl = MPSL.init(MultiprotocolServiceLayer::new(mpsl_p, Irqs, lfclk_cfg).unwrap());

// MPSL must run continuously
#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}
```

### Clock Source Options

| Source | Constant | Use Case |
|--------|----------|----------|
| RC oscillator | `MPSL_CLOCK_LF_SRC_RC` | No external crystal (DK boards) |
| External XTAL | `MPSL_CLOCK_LF_SRC_XTAL` | Production with 32.768 kHz crystal |
| Synthesized | `MPSL_CLOCK_LF_SRC_SYNTH` | From HFCLK (higher power) |

## SDC Builder Configuration

```rust
let sdc_p = nrf_sdc::Peripherals::new(
    p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21,
    p.PPI_CH22, p.PPI_CH23, p.PPI_CH24, p.PPI_CH25,
    p.PPI_CH26, p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
);

let sdc = nrf_sdc::Builder::new()?
    .support_central()
    .support_peripheral()
    .support_adv()
    .support_scan()
    .central_count(1)?
    .peripheral_count(1)?
    .build(sdc_p, rng, mpsl, sdc_mem)?;
```

### Memory Sizing

```rust
const SDC_MEM_SIZE: usize = 8192; // Generous for 1 central + 1 peripheral
static SDC_MEM: StaticCell<nrf_sdc::Mem<SDC_MEM_SIZE>> = StaticCell::new();
```

Rule of thumb: ~3–4 KB per connection + scan/adv buffers. 8 KB covers 2 connections comfortably.

## Interrupt Bindings (nRF52840)

```rust
use embassy_nrf::bind_interrupts;

bind_interrupts!(struct Irqs {
    EGU0_SWI0 => nrf_sdc::mpsl::LowPrioInterruptHandler;
    CLOCK_POWER => nrf_sdc::mpsl::ClockInterruptHandler;
    RADIO => nrf_sdc::mpsl::HighPrioInterruptHandler;
    TIMER0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
    RTC0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
    RNG => embassy_nrf::rng::InterruptHandler<embassy_nrf::peripherals::RNG>;
});
```

## HostResources Type Parameters

```rust
HostResources<PacketPool, CONNS, L2CAP_CHANNELS, ADV_SETS>
```

| Param | Meaning | Typical |
|-------|---------|---------|
| `CONNS` | Max simultaneous connections | 2 (1 central + 1 peripheral) |
| `L2CAP_CHANNELS` | L2CAP CoC channels | 1 (or 0 if unused) |
| `ADV_SETS` | Advertising sets | 1 |

## GATT Server Macros

### `#[gatt_server]`

Generates an `AttributeServer` with GAP service. Creates `new_default(name)` constructor.

```rust
#[gatt_server]
struct MyServer {
    svc: MyService,
}
// Generated: MyServer { server: AttributeServer, svc: MyServiceHandle }
// Constructor: MyServer::new_default("DeviceName") -> Result<Self, Error>
```

### `#[gatt_service(uuid = "...")]`

Defines a GATT service with characteristics.

```rust
#[gatt_service(uuid = "1814")]
struct RscService {
    #[characteristic(uuid = "2A53", notify)]
    measurement: [u8; 8],

    #[characteristic(uuid = "2A54", read, value = [0x02, 0x00])]
    feature: [u8; 2],

    #[characteristic(uuid = "2A5D", read, value = [0x02])]
    sensor_location: [u8; 1],

    #[characteristic(uuid = "2A55", write, indicate)]
    sc_control_point: [u8; 5],
}
```

### Characteristic Attributes

| Attribute | Effect |
|-----------|--------|
| `read` | Readable by client |
| `write` | Writable by client |
| `notify` | Server can send notifications |
| `indicate` | Server can send indications |
| `value = [...]` | Static initial value |

### Generated Handles

Each characteristic generates a handle struct with:
- `.handle` — the characteristic value handle
- `.cccd_handle` — Option<handle> for CCCD (if notify/indicate)
- `.notify(gatt_conn, &data)` — send notification
- `.indicate(gatt_conn, &data)` — send indication
- `.set(&server, &data)` — set value (triggers indication if CCCD configured)

## GattClient Usage

```rust
// Create (performs MTU exchange)
let client: GattClient<'_, C, DefaultPacketPool, 4> = GattClient::new(stack, &conn).await?;

// Discover services
let services = client.services_by_uuid(&Uuid::new_short(0x1826)).await?;

// Discover characteristics within a service
let char: Characteristic<[u8]> = client.characteristic_by_uuid(&service, &uuid).await?;

// Subscribe to notifications
let mut listener = client.subscribe(&char, false).await?; // false = notifications, true = indications

// IMPORTANT: Run client.task() concurrently — it dispatches incoming PDUs
embassy_futures::select::select(
    client.task(),
    async {
        loop {
            let notif = listener.next().await;
            let data: &[u8] = notif.as_ref();
            // process...
        }
    },
).await;
```

The `4` in `GattClient<..., 4>` is the notification listener capacity.

## GattConnection (Server Side)

```rust
let gatt_conn = conn.with_attribute_server(&server.server)?;

loop {
    match gatt_conn.next().await {
        GattConnectionEvent::Disconnected { reason } => break,
        GattConnectionEvent::Gatt { event } => {
            match event {
                GattEvent::Write(write_event) => {
                    let handle = write_event.handle();
                    let data = write_event.data();
                    write_event.accept(); // or .reject(AttErrorCode::new(code))
                }
                _ => { event.accept(); }
            }
        }
        _ => {} // Phy updates, etc.
    }
}
```

## Advertising

### Advertisement Data Encoding

```rust
let mut adv_buf = [0u8; 31];
let len = AdStructure::encode_slice(
    &[
        AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
        AdStructure::ServiceUuids16(&[0x1814_u16.to_le_bytes()]),
        AdStructure::Unknown { ty: 0x19, data: &0x0443_u16.to_le_bytes() }, // Appearance
    ],
    &mut adv_buf,
).unwrap_or(0);
```

### AdStructure Variants

| Variant | Use |
|---------|-----|
| `Flags(u8)` | Discovery mode flags |
| `ServiceUuids16(&[[u8; 2]])` | 16-bit service UUIDs |
| `ServiceUuids128(&[[u8; 16]])` | 128-bit service UUIDs |
| `CompleteLocalName(&[u8])` | Device name |
| `ShortenedLocalName(&[u8])` | Shortened name |
| `Unknown { ty, data }` | Raw AD type + data |

### AdvertisementParameters

```rust
AdvertisementParameters {
    interval_min: Duration::from_millis(100),
    interval_max: Duration::from_millis(100),
    ..Default::default()
}
```

### Advertisement Types

| Variant | Connectable | Scannable |
|---------|-------------|-----------|
| `ConnectableScannableUndirected` | Yes | Yes |
| `ConnectableNonscannableDirected` | Yes | No |
| `NonconnectableScannableUndirected` | No | Yes |
| `NonconnectableNonscannableUndirected` | No | No |

## Connection Management

### ScanConfig

```rust
ScanConfig {
    active: false,                    // passive scan
    filter_accept_list: &[],          // or &[(AddrKind, &BdAddr)]
    timeout: Duration::from_secs(5),
    ..Default::default()
}
```

### ConnectConfig

```rust
ConnectConfig {
    scan_config: ScanConfig { ... },
    connect_params: Default::default(), // uses BLE defaults
}
```

### Connection Lifetime

- `Connection` is a handle — dropping it does NOT disconnect
- Call `conn.disconnect()` to explicitly disconnect
- Connection is invalid after remote disconnect (operations return errors)

## Runner Patterns

### Basic (no scan reports needed)

```rust
#[embassy_executor::task]
async fn ble_runner(mut runner: Runner<'static, Controller, DefaultPacketPool>) {
    runner.run().await.expect("BLE runner failed");
}
```

### With EventHandler (for scan reports)

```rust
#[embassy_executor::task]
async fn ble_runner(
    mut runner: Runner<'static, Controller, DefaultPacketPool>,
    handler: &'static MyScanner,
) {
    runner.run_with_handler(handler).await.expect("BLE runner failed");
}
```

## Address Types

```rust
// Random static address (bits 47:46 = 0b11)
let addr = Address::random([0xf0, 0xde, 0xf1, 0xca, 0xfe, 0xc0]);

// From raw bytes
let bd_addr = BdAddr::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
```

## Error Handling

```rust
enum BleHostError<E> {
    BleHost(Error),    // trouble-host errors
    Controller(E),     // controller-specific errors
}

enum Error {
    Timeout,
    NotFound,
    InvalidValue,
    // ... other variants
}
```

## Task Architecture Pattern (nRF52840)

```
main() → spawns:
├── mpsl_task()          — MPSL event loop (never returns)
├── ble_runner_task()    — trouble-host HCI event loop
├── central_task()       — scan/connect/GATT client logic
├── peripheral_task()    — advertise/serve/GATT server logic
└── led_task()           — status feedback (optional)
```

All tasks share state via:
- `Signal<CriticalSectionRawMutex, T>` — latest-value channel (overwrite)
- `Watch<CriticalSectionRawMutex, T, N>` — broadcast state with N subscribers

## Version Compatibility Matrix

| nrf-sdc | bt-hci | trouble-host | embassy-nrf |
|---------|--------|--------------|-------------|
| 0.4 | 0.8 | 0.6 | 0.10 |
| 0.3 | 0.4 | 0.4–0.5 | 0.7 |

Ensure all crates agree on the same `bt-hci` major version. The `Controller` trait is defined in `bt-hci` and must match between nrf-sdc (implements it) and trouble-host (requires it).

## Useful Links

- [trouble-host docs](https://docs.rs/trouble-host/0.6.0/trouble_host/)
- [trouble examples (nrf52)](https://github.com/embassy-rs/trouble/tree/main/examples/nrf52)
- [nrf-sdc repo](https://github.com/alexmoon/nrf-sdc)
- [bt-hci docs](https://docs.rs/bt-hci/0.8)
- [Embassy docs](https://embassy.dev)

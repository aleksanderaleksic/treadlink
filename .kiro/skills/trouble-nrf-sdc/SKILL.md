---
name: trouble-nrf-sdc
description: "Guide for building BLE applications with trouble-host 0.6 and nrf-sdc 0.4 on nRF52/54 using Embassy. Covers stack initialization, Central/Peripheral roles, GATT server macros, GATT client discovery and notifications, scanning with EventHandler, advertising, connection management, and nrf-sdc controller setup with MPSL. Use when working with trouble-host, nrf-sdc, BLE stack init, GATT server/client, scanning, advertising, Central, Peripheral, Runner, SoftdeviceController, or MPSL on nRF52840/nRF5340."
---

# trouble-host + nrf-sdc

## Architecture

```
┌──────────────────────────────────────────────┐
│  Application (Central / Peripheral / GATT)   │
├──────────────────────────────────────────────┤
│  trouble-host 0.6  (async BLE Host, no_std)  │
├──────────────────────────────────────────────┤
│  bt-hci 0.8  (HCI trait interface)           │
├──────────────────────────────────────────────┤
│  nrf-sdc 0.4  (SoftdeviceController)         │
│  nrf-mpsl 0.3 (MultiprotocolServiceLayer)    │
└──────────────────────────────────────────────┘
```

## Quick Start — Stack Init

```rust
use trouble_host::prelude::*;
use static_cell::StaticCell;

static HOST_RESOURCES: StaticCell<HostResources<DefaultPacketPool, 2, 1, 1>> = StaticCell::new();
static STACK: StaticCell<Stack<'static, nrf_sdc::SoftdeviceController<'static>, DefaultPacketPool>> = StaticCell::new();

let resources = HOST_RESOURCES.init(HostResources::new());
let address = Address::random([0xf0, 0xde, 0xf1, 0xca, 0xfe, 0xc0]);
let stack = STACK.init(trouble_host::new(controller, resources).set_random_address(address));
let Host { central, peripheral, runner, .. } = stack.build();
// Spawn runner — required, drives HCI event loop
```

## Key Types

| Type | Role |
|------|------|
| `Stack<'_, C, P>` | Owns BLE host state |
| `Runner` | HCI event loop — must run continuously |
| `Central<'_, C, P>` | Scan + connect |
| `Peripheral<'_, C, P>` | Advertise + accept |
| `GattClient<'_, C, P, N>` | Discovery, read, write, subscribe |
| `GattConnection<'s, 'v, P>` | Server-side connection with attribute table |
| `NotificationListener<'_, N>` | Receives GATT notifications |
| `EventHandler` (trait) | Receives scan reports via `on_adv_reports` |

## Common Pitfalls

1. **Runner must run** — without `runner.run()` or `run_with_handler()`, no HCI events process
2. **Scanner borrows Central** — `Scanner::new(central)` / `scanner.into_inner()` pattern
3. **GattClient needs concurrent task** — always `select(client.task(), your_logic)`
4. **`#[embassy_executor::task]` no generics** — monomorphize with concrete controller type
5. **Static lifetimes** — Stack, Server, Signal, Watch all need `StaticCell`
6. **bt-hci version alignment** — nrf-sdc and trouble-host must use same bt-hci major

## Task Architecture

```
main() → spawns:
├── mpsl_task()        — MPSL event loop (never returns)
├── ble_runner_task()  — trouble-host HCI event loop
├── central_task()     — scan/connect/GATT client
├── peripheral_task()  — advertise/serve/GATT server
└── app tasks          — LED, sensors, etc.
```

Shared state: `Signal` (latest-value), `Watch` (broadcast with N subscribers).

## Lessons Learned

1. **trouble-host 0.6 has a critical packet pool exhaustion bug** — During GATT service discovery on the peripheral side, the 8th ATT response fails with `InsufficientSpace` because the controller only has 1 TX buffer slot and responses queue up. Fix: upgrade to trouble-host from git main branch (post-0.6).

2. **Generic async functions cause stack overflow** — A generic `central_task_impl<C>` function compiled as an async task produces a massive future that corrupts SDC memory. Fix: use concrete (non-generic) `#[embassy_executor::task]` functions with the specific controller type.

3. **Scanner::scan() corrupts SDC state for subsequent LeCreateConn** — After dropping a scan session, the SDC needs time to process the scan disable. Fix: add a 500ms settle delay between scan drop and connect, OR use trouble-host main which handles this better.

4. **SDC buffer_cfg is required** — Without `.buffer_cfg(MTU, MTU, TXQ, RXQ)` the controller defaults to 27-byte buffers which causes assertions. Always configure it matching the trouble-host example.

5. **reply.send().await is required for GATT server events** — Just calling `event.accept()` is not enough. You must call `event.accept()?.send().await` to actually transmit the ATT response PDU back to the client.

6. **HostResources API changed in main** — In 0.6: `HostResources<Pool, CONNS, CHANNELS>`. In main: `HostResources<Controller, Pool, CONNS, CHANNELS>`.

7. **AdStructure enum renamed** — `ServiceUuids16` → `IncompleteServiceUuids16` / `CompleteServiceUuids16` in main branch.

8. **filter_accept_list changed** — In 0.6: `&[(AddrKind, &BdAddr)]`. In main: `&[Address]`.

## Advanced

See [REFERENCE.md](REFERENCE.md) for: Cargo.toml config, MPSL init, SDC Builder, interrupt bindings, memory sizing, GATT macros, GattClient usage, advertising details, connection management, and version compatibility matrix.

---
name: domain-ble
description: "Stack-agnostic Bluetooth Low Energy expert knowledge for embedded developers. Covers BLE architecture (LL/HCI/Host/GATT), GAP roles (Peripheral, Central, Broadcaster, Observer), advertising and scanning parameters, connection parameter math (interval/latency/timeout), multi-role concurrency on a single radio, GATT services/characteristics/notify/indicate, MTU/DLE/PHY throughput, and pairing/bonding vocabulary. Use when designing BLE peripherals, centrals, or multi-role devices, choosing advertising or connection parameters, debugging connection drops, sizing packet pools, reasoning about throughput, or asking conceptual BLE questions not tied to a specific stack. Keywords: BLE, Bluetooth, GAP, GATT, advertising, scanning, Central, Peripheral, Broadcaster, Observer, multi-role, connection interval, slave latency, supervision timeout, MTU, DLE, ATT, EATT, PHY, 2M, Coded, CCCD, notify, indicate, pairing, bonding, LESC, RPA, privacy, HCI."
---

# Domain: BLE

Stack-neutral BLE knowledge. For concrete code on nRF, see `trouble-nrf-sdc`. For profiles, see `ftms-protocol`, `rsc-protocol`.

## Stack architecture

```
Application     ┐
GATT / ATT      │  Host (usually in firmware as a library)
GAP / SMP / L2CAP│
────────────────┼──── HCI (command/event interface)
Link Layer (LL) │  Controller (radio + timing)
PHY (1M/2M/Coded)┘
```

Host and Controller can be on the same MCU (softdevice, nrf-sdc) or split across a UART/SPI HCI link.

## GAP roles

| Role | Radio activity | Connection state |
|------|----------------|------------------|
| Broadcaster | Non-connectable adv | None |
| Observer | Passive/active scan | None |
| Peripheral | Connectable adv → Slave | Has one or more connections as Slave |
| Central | Scan → Initiate → Master | Has one or more connections as Master |

Multi-role = any combination simultaneously. Controller capability limits how many connections + concurrent adv/scan.

## GATT essentials

- **Service** = group of related **Characteristics**
- Characteristic = value + properties (Read/Write/Notify/Indicate) + optional **Descriptors**
- **CCCD** (0x2902): client writes to enable notify (bit 0) or indicate (bit 1)
- **Notify**: no ack, fire-and-forget, 1 per conn event (legacy ATT)
- **Indicate**: requires ack, slower, guaranteed delivery
- **MTU**: default 23 (ATT payload 20), negotiate up to 247+ for throughput

## Connection parameter math

```
effective_interval   = connInterval  × 1.25 ms   (range 6–3200  → 7.5ms–4s)
supervision_timeout  = connTimeout    × 10   ms   (range 10–3200 → 100ms–32s)
slave_latency        = number of intervals Peripheral may skip (0–499)

Constraint: timeout > (1 + latency) × interval × 2
```

Typical profiles:

| Use case | Interval | Latency | Timeout |
|----------|---------:|--------:|--------:|
| Low-latency control (HID, RC) | 7.5–15 ms | 0 | 2–4 s |
| Sensor streaming (HRM, FTMS) | 15–30 ms | 0 | 2–4 s |
| Battery-saving periodic | 200–500 ms | 4–10 | 6–20 s |

Peripheral *requests*, Central *decides*. Always validate the timeout inequality or iOS/Android will reject.

## Throughput rules of thumb (1M PHY)

| Config | Upper bound |
|--------|------------:|
| Default MTU 23, 7.5 ms interval | ~6 kB/s |
| MTU 247 + DLE, 7.5 ms interval | ~100 kB/s |
| MTU 247 + DLE + 2M PHY | ~180 kB/s |

Real throughput also depends on controller: packets/event, concurrent roles, scheduling.

## Common pitfalls

1. **Timeout violates inequality** → connection rejected or drops after first param update.
2. **Advertising too slow while scanning** → peer never finds device; scan interval should be < adv interval of peer.
3. **CCCD not bonded** → client re-subscribes every reconnect; enable bonding or accept re-subscribe.
4. **MTU not exchanged** → stuck at 20-byte ATT payloads even after DLE.
5. **Notify faster than conn interval** → host queue fills, packets coalesced or dropped.
6. **Multi-role without budgeting** → scan window starves adv or vice versa; respect controller's role limits.
7. **Random static address changed across reboots** → bonded peers see a new device.

## Decision quick refs

- "Should I use notify or indicate?" → Notify unless loss is unacceptable (config writes, critical events).
- "What PHY?" → 1M by default. 2M for throughput when both ends support it. Coded only for long range, halves throughput.
- "Active or passive scan?" → Passive unless you need Scan Response data (richer adv payload).
- "Legacy or Extended advertising?" → Legacy (31 bytes) unless you need >31 bytes, periodic, or multiple adv sets.

## Lessons Learned

1. **Garmin watches require trouble-host main (not 0.6)** — The packet pool bug in 0.6 prevents proper GATT service discovery completion, causing Garmin to reject the sensor.

2. **Garmin BLE foot pod validation** — Garmin watches DO support BLE RSC foot pods, but they require complete GATT service discovery to succeed (all ATT responses must be delivered). The `InsufficientSpace` error in trouble-host 0.6 was the root cause of Garmin "could not connect" errors.

3. **Connection parameters for treadmills** — FTMS treadmills often request specific connection parameters (e.g., 20ms interval). Use `RequestedConnParams` matching the treadmill's preference to avoid disconnection due to "Unacceptable Connection Parameters".

## Related skills

| Topic | Skill |
|-------|-------|
| Concrete stack on nRF | `trouble-nrf-sdc` |
| FTMS profile | `ftms-protocol` |
| RSC profile | `rsc-protocol` |
| no_std / MCU constraints | `domain-embedded` |

## Advanced

See [REFERENCE.md](REFERENCE.md) for: advertising AD structures + timing, scanning policies, connection establishment sequence, multi-role scheduling, MTU/DLE/EATT detail, PHY trade-offs, security/pairing vocabulary, and embedded sizing (packet pools, HCI queue depth).

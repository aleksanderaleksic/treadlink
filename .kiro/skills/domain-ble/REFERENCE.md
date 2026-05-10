# Domain BLE — Reference

Deeper material for when the `SKILL.md` summary isn't enough. All timing is Bluetooth Core 5.x.

## 1. Advertising

### 1.1 Advertising types (legacy PDU)

| Type | PDU | Connectable | Scannable | Directed | Use |
|------|-----|:-:|:-:|:-:|-----|
| ADV_IND | 0x0 | ✓ | ✓ | ✗ | Standard connectable |
| ADV_DIRECT_IND (high duty) | 0x1 | ✓ | ✗ | ✓ | Fast reconnect (<1.28 s) |
| ADV_NONCONN_IND | 0x2 | ✗ | ✗ | ✗ | Pure broadcast (iBeacon) |
| ADV_SCAN_IND | 0x6 | ✗ | ✓ | ✗ | Broadcast + scan response |
| ADV_DIRECT_IND (low duty) | 0x4 | ✓ | ✗ | ✓ | Directed reconnect |

### 1.2 Extended advertising (5.0+)

- Primary channel announces, secondary channel carries payload
- Up to 254 bytes per PDU, chain up to 1650 bytes
- Required for: periodic adv, multiple simultaneous adv sets, long-range Coded PHY adv

### 1.3 AD structure

Each element in the adv payload is `[len][type][data...]`. Common types:

| Type | Name | Notes |
|-----:|------|------|
| 0x01 | Flags | LE General/Limited Discoverable, BR/EDR Not Supported |
| 0x02 / 0x03 | Incomplete / Complete List of 16-bit UUIDs | |
| 0x06 / 0x07 | Incomplete / Complete List of 128-bit UUIDs | |
| 0x08 / 0x09 | Shortened / Complete Local Name | |
| 0x0A | TX Power Level | sint8 dBm |
| 0x16 | Service Data (16-bit UUID) | Profile-specific, e.g. FTMS |
| 0xFF | Manufacturer Specific Data | First 2 bytes = Company ID |

### 1.4 Timing

```
advInterval  = N × 0.625 ms   (20 ms – 10.24 s for connectable/scannable)
                              (100 ms – 10.24 s for non-connectable, spec min; modern stacks allow 20 ms)
advDelay     = random 0–10 ms added to each interval (anti-collision)
```

High-duty directed adv: interval ≤ 3.75 ms, bounded to 1.28 s max duration.

Power vs discoverability trade-off: halving the interval roughly doubles radio-on time.

## 2. Scanning

### 2.1 Parameters

```
scanInterval = N × 0.625 ms   (2.5 ms – 10.24 s)
scanWindow   = M × 0.625 ms   (2.5 ms – 10.24 s, M ≤ N)
duty_cycle   = scanWindow / scanInterval
```

- `window == interval` → continuous scan, highest discovery probability, highest current.
- `window ≪ interval` → low power, may miss peers whose adv interval is longer than the gap.

### 2.2 Passive vs Active

- **Passive**: listens only, gets AdvData (≤31 bytes legacy).
- **Active**: sends SCAN_REQ, gets AdvData + ScanRspData (≤62 bytes total legacy). More power, more air time.

### 2.3 Filter policy

| Value | Accepts |
|------:|---------|
| 0 | All advs (default) |
| 1 | Only whitelisted advs |
| 2 | All advs + directed advs to random unresolvable address (privacy) |
| 3 | Whitelisted advs + directed advs |

Whitelist is a controller-side address filter. Scales better than host-side filtering when many devices nearby.

## 3. Connection establishment

```
Central                            Peripheral
  │                                    │
  │  scan_req/scan_rsp (if active)     │
  │ ◄────────────────────────────────  │
  │            ADV_IND                 │
  │ ◄────────────────────────────────  │
  │            CONNECT_IND             │
  │ ──────────────────────────────────►│
  │                                    │
  │         Connection event (LL)      │
  │ ◄──────────────────────────────────►
  │                                    │
  │  Feature exchange, Version, MTU,   │
  │  DLE, PHY update, Connection Param │
  │  Update (host-driven order varies) │
```

CONNECT_IND fields include connInterval, connLatency, connTimeout, connWinOffset, connWinSize, channelMap, hopIncrement, SCA.

## 4. Connection parameter update

Peripheral procedure: `L2CAP Connection Parameter Update Request` (legacy) or `LL Connection Parameter Request` (5.0+). Central replies with accept/reject and issues LL_CONNECTION_UPDATE_IND.

Validation rules the Central MUST apply:

```
7.5 ms  ≤ interval_min ≤ interval_max ≤ 4 s
0       ≤ latency ≤ 499
(1 + latency) × interval_max × 2 ≤ timeout ≤ 32 s
```

Apple/Android have stricter rules (e.g. iOS: interval ≥ 15 ms, timeout ≥ 2 s, interval_max − interval_min ≥ 20 ms, latency ≤ 30). Validate against target platforms.

## 5. Multi-role

### 5.1 Typical controller capabilities

| Capability | Softdevice S140 / nrf-sdc | Typical other |
|------------|---------------------------|----------------|
| Concurrent Peripheral links | up to 20 (config) | 1–8 |
| Concurrent Central links | up to 20 (config) | 1–8 |
| Simultaneous adv sets (ext) | up to ~4 | 1 |
| Adv + scan + connections | yes | depends |

Each link/adv set costs RAM (packet buffers, TX/RX queues, link context).

### 5.2 Scheduling constraints

Radio is single-channel. Controller time-slices: connection events, adv events, scan windows. Violating time budget = dropped adv, missed scan windows, slipped connection events.

Design rules:

- Keep scan window < min connection interval to leave room for conn events.
- Avoid adv interval colliding with conn events of existing links.
- If adv set never gets air time, reduce interval of other activities or reduce number of concurrent roles.

### 5.3 Typical bridge topology (e.g. treadlink)

```
 Treadmill (FTMS Peripheral) ── connected ── Central role ──┐
                                                             │ same MCU
                                                             ▼
                                                    Peripheral role ── advertises RSC ── Watch/App
```

Both roles share one radio. Plan:

- Central: fixed connection interval to treadmill (e.g. 30 ms, latency 0).
- Peripheral: adv interval >30 ms so it fits between conn events, then switches to its own conn interval once a client connects.

## 6. GATT in detail

### 6.1 Attribute table

Every service/characteristic/descriptor is a flat attribute with: handle (u16), type (UUID), permissions, value.

Discovery flow from Central:

```
Primary Service Discovery      → list of (start, end, UUID)
Characteristic Discovery       → list of (decl handle, properties, value handle, UUID)
Descriptor Discovery           → list of (handle, UUID), incl. CCCD (0x2902)
```

### 6.2 Properties

Broadcast, Read, WriteWithoutResponse, Write, Notify, Indicate, AuthSignedWrite, ExtendedProperties.

### 6.3 MTU

- ATT_MTU default 23 → ATT payload = MTU − 3 = 20
- Exchange MTU at start of connection; both sides pick min of the two requests.
- Notification/Indication payload limited by negotiated MTU, not DLE.

### 6.4 DLE (Data Length Extension, 4.2+)

- LL-layer: raises max PDU from 27 to 251 bytes.
- Independent of MTU negotiation; do both for max throughput.

### 6.5 EATT (5.2+)

- Multiple ATT bearers over L2CAP credit-based channels.
- Parallel requests on different characteristics.
- Mostly improves apps with many clients/features; still niche in embedded.

### 6.6 Subscriptions after reconnect

CCCD is per-bond. Without bonding, client must re-write CCCD on every reconnect. With bonding + "restore state" supported, stack restores subscriptions automatically.

## 7. PHY

| PHY | Symbol rate | Range (relative) | Max adv/data payload throughput |
|-----|------------:|:----------------:|:-------------------------------:|
| LE 1M | 1 Msym/s | 1× | baseline |
| LE 2M | 2 Msym/s | ~0.8× | ~2× |
| LE Coded S=2 | 500 ksym/s | ~2× | ~0.5× |
| LE Coded S=8 | 125 ksym/s | ~4× | ~0.125× |

PHY update procedure is host-agnostic LL procedure; both sides announce supported PHYs, negotiate preferred.

## 8. Addressing and privacy

| Address type | Persistence | Privacy |
|--------------|-------------|---------|
| Public | Hardware-assigned (IEEE OUI) | None |
| Random Static | Set once per power cycle (or persisted in flash) | Low |
| Resolvable Private Address (RPA) | Rotates (default 15 min) using IRK | High, resolvable by bonded peer |
| Non-Resolvable Private Address | Rotates, not resolvable | Highest, but breaks bonding |

Use RPA for privacy; share IRK at pairing with LESC bonding so peer can resolve.

## 9. Security — vocabulary

- **Pairing**: generate keys (STK/LTK). Methods:
  - Just Works (no MITM protection)
  - Passkey Entry (6-digit)
  - Numeric Comparison (LESC only)
  - Out-of-Band (OOB)
- **LESC (LE Secure Connections, 4.2+)**: ECDH-based pairing, MITM-resistant even with Just Works if OOB/NC used correctly.
- **Bonding**: persist keys (LTK, IRK, CSRK) to flash for fast secure reconnect.
- **Encryption**: AES-CCM, enabled after pairing/bonding LL_ENC_REQ.
- Default recommendation for new designs: **LESC + bonding**, method chosen by IO capabilities.

## 10. Embedded sizing

### 10.1 RAM per link (order-of-magnitude)

| Item | Cost |
|------|-----:|
| Per-connection context | ~0.5–2 KB |
| Per adv set | ~0.2–1 KB |
| ATT/GATT table | proportional to attribute count × (UUID width + value) |
| Packet pool | `pool_size × MTU_rounded_up` |

### 10.2 Packet pools

- Sized from Host side (trouble-host: `HostResources<DefaultPacketPool, N_CONN, N_CHAN, N_ADV>`).
- Under-sized pool → TX queue full → notifications dropped or backpressured.

### 10.3 HCI queue depth

If Host and Controller are split (UART HCI), the UART and HCI event queue become bottlenecks. Scan reports burst fast in dense RF environments → increase event queue or filter in controller (whitelist, dedup).

### 10.4 Power

Rule of thumb: radio is the dominant consumer.

- Adv at 20 ms ≈ ~1 mA avg on nRF52.
- Adv at 1 s ≈ ~50 µA avg.
- Connection at 30 ms interval, 0 latency ≈ ~0.5–1 mA.
- Connection at 1 s interval, 10 latency ≈ ~10–30 µA.

Latency lets Peripheral skip empty events (save power) while keeping supervision alive.

## 11. Cross-platform gotchas

- **iOS**: requires conn interval ≥15 ms, max latency 30, timeout ≥ 2 s, GAP Device Name char, Appearance char.
- **Android**: widely varying behavior; always handle connection rejection + retry with relaxed params.
- **macOS**: caches bonds aggressively, "forget device" in Bluetooth settings during dev.
- **Linux (BlueZ)**: tends to impose conservative intervals; inspect `btmon` for actual values.

## 12. Debugging checklist

- [ ] Sniff with Nordic nRF Sniffer or Ellisys to see real LL traffic, not just host logs.
- [ ] Confirm MTU was exchanged (sniffer shows EXCHANGE_MTU_REQ/RSP).
- [ ] Confirm DLE happened (LENGTH_REQ/RSP, PDU size change).
- [ ] Confirm PHY updated if expected.
- [ ] Log controller-side supervision timeout reason (0x08 on disconnect).
- [ ] Check that CCCD was actually written (attribute write event).
- [ ] Validate address type consistency across reboots (bonding breaks if Peripheral's resolvable / static address changes unexpectedly).

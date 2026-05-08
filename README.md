# Treadlink

Treadlink is an embedded Rust firmware that bridges BLE-enabled treadmills to Garmin devices. It connects to treadmills exposing the FTMS (Fitness Machine Service) BLE profile, reads speed and incline data, and re-broadcasts it as a footpod (Running Speed and Cadence) BLE service that Garmin watches can consume for accurate indoor running metrics.

## Why

Garmin devices don't natively support FTMS treadmill data. Treadlink sits in between — acting as a BLE footpod that your watch already knows how to use — so you get real-time pace and cadence without calibration drift.

## Hardware

Supported targets:

| Board | Chip | Notes |
|-------|------|-------|
| nRF52840-DK (PCA10056) | Nordic nRF52840 (Cortex-M4F, BLE 5.0) | Primary dev board |
| ESP32-based boards | Espressif ESP32 | WiFi + BLE combo |

## Tech Stack

- **Language:** Rust (no_std, embedded)
- **Async runtime:** [Embassy](https://embassy.dev/)
- **BLE stack:** [Trouble](https://github.com/embassy-rs/trouble) (Embassy's host-side BLE stack)
- **Build targets:** `thumbv7em-none-eabihf` (nRF), `xtensa-esp32-none-elf` / `riscv32imc-unknown-none-elf` (ESP32 variants)

## Getting Started

### Prerequisites

- Rust toolchain via [rustup](https://rustup.rs/)
- Target support:
  ```sh
  # nRF52840
  rustup target add thumbv7em-none-eabihf

  # ESP32 (install espup for Xtensa targets)
  cargo install espup
  espup install
  ```
- [probe-rs](https://probe.rs/) for flashing nRF boards
- [espflash](https://github.com/esp-rs/espflash) for flashing ESP32 boards

### Build

```sh
cargo build --release
```

### Flash

```sh
# nRF52840-DK (via probe-rs)
cargo run --release

# ESP32 (via espflash)
espflash flash target/release/treadlink
```

### Usage

1. Power on your treadmill (must expose BLE FTMS service).
2. Flash and power the device.
3. Treadlink scans for the treadmill, connects, and begins reading speed data.
4. On your Garmin watch, add a footpod sensor — Treadlink will appear as a Running Speed & Cadence device.
5. Start your run. Pace and cadence are now sourced from the treadmill.

## Project Structure

```
src/
├── main.rs          # Entry point, task spawning
├── ble/             # BLE central (FTMS client) and peripheral (footpod server)
├── conversion/      # FTMS → RSC data conversion logic
└── board/           # Board-specific pin/peripheral config
```

## License

TBD

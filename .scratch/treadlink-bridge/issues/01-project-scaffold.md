Status: ready-for-agent

# Project scaffold & build infrastructure

## Parent

.scratch/treadlink-bridge/PRD.md

## What to build

Set up the complete build infrastructure for an Embassy-based nrf52840-dk firmware. The project should compile, flash, and print a defmt log message on boot.

This includes:
- `Cargo.toml` with Embassy (executor, nrf HAL, time), trouble (BLE), defmt + defmt-rtt, and panic-probe dependencies
- `.cargo/config.toml` targeting `thumbv7em-none-eabihf` with probe-rs runner
- `memory.x` linker script for nrf52840 (1MB flash, 256KB RAM)
- `src/main.rs` with Embassy entry point that initializes the HAL and logs a startup message via defmt

## Acceptance criteria

- [ ] `cargo build --release` compiles without errors targeting `thumbv7em-none-eabihf`
- [ ] `cargo run --release` flashes the nrf52840-dk and prints a defmt log line visible via probe-rs
- [ ] Project structure matches the layout in the PRD (main.rs, placeholders for central.rs, peripheral.rs, converter.rs, protocol.rs)
- [ ] defmt logging is functional (at least one `info!` message on boot)

## Blocked by

None - can start immediately

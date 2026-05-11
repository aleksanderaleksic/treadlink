Status: ready-for-agent

# LED status feedback

## Parent

.scratch/treadlink-bridge/PRD.md

## What to build

Drive the nrf52840-dk onboard LEDs to indicate the current system state:

- **Scanning:** LED1 blinking
- **Connected to treadmill (no consumer):** LED2 solid
- **Bridging (treadmill + consumer connected):** LED3 solid

This is a nice-to-have (FR-13) that provides visual feedback during development and use.

## Acceptance criteria

- [ ] LED1 blinks during scanning state
- [ ] LED2 lights solid when connected to treadmill but no consumer is connected
- [ ] LED3 lights solid when actively bridging (both connections established)
- [ ] LEDs transition correctly as the system moves between states
- [ ] LED control does not interfere with BLE timing or task scheduling

## Blocked by

- .scratch/treadlink-bridge/issues/05-end-to-end-bridge.md

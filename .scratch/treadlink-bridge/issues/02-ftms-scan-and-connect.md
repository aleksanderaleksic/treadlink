Status: ready-for-agent

# FTMS scan & connect (Central task)

## Parent

.scratch/treadlink-bridge/PRD.md

## What to build

Implement the BLE Central task that scans for treadmills advertising the FTMS service (UUID 0x1826), connects to the strongest signal above the RSSI threshold (-70 dBm), discovers the Treadmill Data characteristic (UUID 0x2ACD), and subscribes to notifications.

The task should retry scanning indefinitely if no device is found. On successful subscription, log raw notification bytes via defmt.

This covers FR-1, FR-2, FR-3, FR-4, and FR-12 (single treadmill connection).

## Acceptance criteria

- [ ] Central task scans for devices advertising FTMS UUID (0x1826)
- [ ] Only connects to devices with RSSI above -70 dBm threshold
- [ ] Retries scanning indefinitely when no suitable device is found
- [ ] Connects and discovers FTMS service and Treadmill Data characteristic (0x2ACD)
- [ ] Subscribes to Treadmill Data notifications
- [ ] Logs raw notification payloads via defmt
- [ ] Supports exactly one treadmill connection at a time (FR-12)

## Blocked by

- .scratch/treadlink-bridge/issues/01-project-scaffold.md

Status: ready-for-agent

# RSC peripheral advertising & notifications

## Parent

.scratch/treadlink-bridge/PRD.md

## What to build

Implement the BLE Peripheral task that advertises the Running Speed and Cadence service (UUID 0x1814) and sends RSC Measurement notifications to a connected consumer (watch).

For this slice, use a hardcoded test measurement value to prove the peripheral works independently. The task should:
- Advertise with the configured device name and RSC service UUID
- Accept exactly one consumer connection (FR-12)
- Send RSC Measurement notifications at a regular interval
- Mark cadence as unsupported in the RSC Feature characteristic

Covers FR-7, FR-8, FR-9, FR-12.

## Acceptance criteria

- [ ] Peripheral advertises RSC service UUID (0x1814) with configured device name
- [ ] A BLE scanner (phone app or watch) can discover the device as an RSC sensor
- [ ] One consumer can connect and receive RSC Measurement notifications
- [ ] RSC Measurement format is correct (speed present, cadence not present, distance present)
- [ ] Only one consumer connection is supported at a time
- [ ] Advertising starts only after being triggered (will be wired to first FTMS notification in slice 5; for now, start immediately for testing)

## Blocked by

- .scratch/treadlink-bridge/issues/01-project-scaffold.md

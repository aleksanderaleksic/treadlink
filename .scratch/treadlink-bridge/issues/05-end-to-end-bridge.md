Status: ready-for-agent

# End-to-end bridge: Central → Signal → Peripheral

## Parent

.scratch/treadlink-bridge/PRD.md

## What to build

Wire the Central task, conversion logic, and Peripheral task together into a working bridge. The Central task writes converted RSC measurements to a shared signal (latest-value-only, overwrite semantics — e.g. `embassy_sync::watch::Watch` or `Signal`). The Peripheral task reads from this signal when notifying the consumer.

Advertising should begin only after the first FTMS notification is received (FR-7). The system uses latest-value-only semantics with no buffering (NFR-3).

This slice delivers the core value: live treadmill speed appearing on a connected watch.

Covers FR-7, FR-8, US-1, US-2, US-3, NFR-3.

## Acceptance criteria

- [ ] Central task writes converted measurements to a shared signal after each FTMS notification
- [ ] Peripheral task reads the latest value from the signal when sending RSC notifications
- [ ] Advertising begins only after the first FTMS notification is received
- [ ] A connected watch displays live pace sourced from the treadmill
- [ ] No buffering or queuing — always latest value (NFR-3)
- [ ] System operates fully automatically with no buttons or configuration (NFR-1)

## Blocked by

- .scratch/treadlink-bridge/issues/02-ftms-scan-and-connect.md
- .scratch/treadlink-bridge/issues/03-ftms-data-parsing-and-conversion.md
- .scratch/treadlink-bridge/issues/04-rsc-peripheral-advertising.md

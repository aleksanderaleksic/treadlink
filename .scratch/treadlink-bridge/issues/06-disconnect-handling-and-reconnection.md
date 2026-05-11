Status: ready-for-agent

# Disconnect handling & automatic reconnection

## Parent

.scratch/treadlink-bridge/PRD.md

## What to build

Implement the disconnect and reconnection logic for both the treadmill (Central) and consumer (Peripheral) sides:

- **Treadmill disconnect (FR-10):** Stop advertising, drop any consumer connection, and restart scanning. When the treadmill reappears, reconnect and resume bridging automatically.
- **Consumer disconnect (FR-11):** Maintain the treadmill connection, continue receiving FTMS data, and re-advertise so a new consumer can connect.
- **Scan retry:** If the treadmill is not found during rescan, retry indefinitely (FR-3).

The state machine transitions: BRIDGING → SCANNING on treadmill disconnect; consumer disconnect stays in BRIDGING (just re-advertises).

Covers FR-3, FR-10, FR-11, US-3, US-4.

## Acceptance criteria

- [ ] Treadmill power-off triggers: stop advertising, disconnect consumer, return to scanning state
- [ ] Treadmill power-on after disconnect triggers automatic reconnection and bridging resumes
- [ ] Consumer disconnect does not affect treadmill connection
- [ ] After consumer disconnect, peripheral re-advertises and a new consumer can connect
- [ ] Scanning retries indefinitely until a treadmill is found
- [ ] No manual intervention required for any reconnection scenario (NFR-1)

## Blocked by

- .scratch/treadlink-bridge/issues/05-end-to-end-bridge.md

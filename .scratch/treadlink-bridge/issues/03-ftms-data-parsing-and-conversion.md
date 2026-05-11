Status: ready-for-agent

# FTMS data parsing & conversion

## Parent

.scratch/treadlink-bridge/PRD.md

## What to build

Implement the FTMS Treadmill Data characteristic parser (`protocol.rs`) and the unit conversion logic (`converter.rs`) that transforms FTMS values into RSC Measurement format.

Parsing: decode the Treadmill Data characteristic (flags field + instantaneous speed as uint16 in 0.01 km/h + total distance as uint24 in metres).

Conversion:
- Speed: FTMS uint16 (0.01 km/h) → RSC uint16 (1/256 m/s)
- Distance: FTMS uint24 (metres) → RSC uint24 (1/10 m)
- Cadence: not present (flag bit = 0)

This is pure `no_std` logic with no hardware dependencies, suitable for host-target unit testing.

Covers FR-5, FR-6, FR-9.

## Acceptance criteria

- [ ] `protocol.rs` parses FTMS Treadmill Data bytes into a structured type (speed, distance, flags)
- [ ] `converter.rs` converts FTMS speed to RSC speed units (formula: `value * 10000 / (36 * 256)` or equivalent)
- [ ] `converter.rs` converts FTMS total distance to RSC distance units (metres → 1/10 m: multiply by 10)
- [ ] RSC output struct marks cadence as not present
- [ ] Unit tests pass on host (`cargo test` with appropriate cfg)
- [ ] Handles edge cases: zero speed, maximum uint16/uint24 values

## Blocked by

- .scratch/treadlink-bridge/issues/01-project-scaffold.md

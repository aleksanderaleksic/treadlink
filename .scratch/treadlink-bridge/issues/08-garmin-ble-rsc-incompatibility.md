# Garmin BLE RSC Foot Pod Incompatibility

## Status: blocked

## Summary

The FTMS→RSC bridge works correctly end-to-end (confirmed via nRF Connect). However, the Garmin Fenix 6X watch cannot use the BLE RSC peripheral as a foot pod for pace/distance.

## Evidence

- nRF Connect confirms correct RSC data (speed, cadence, distance)
- Garmin finds the sensor, connects at BLE level, does GATT service discovery
- Garmin never writes the CCCD (subscribes to notifications)
- Garmin shows "could not connect to foot sensor" after ~30-60 seconds
- Same behavior with multiple GATT table configurations (with/without Feature, SC Control Point, Sensor Location, Battery Service, Device Information Service)
- Same behavior with different packet pool sizes, MTU values, connection parameters
- The old treadlink-old project also fails with the same `InsufficientSpace` error and Garmin incompatibility
- The qdomyos-zwift project (GitHub issue #449) reports identical behavior with Garmin Vivoactive 3
- The Espruino discussion (GitHub #3602) reports identical behavior with Garmin FR935
- Stryd (commercial foot pod) recommends ANT+ only for Garmin watches

## Root Cause

Garmin watches have a proprietary validation step for BLE foot pods that rejects standard BLE RSC implementations. This is not a spec compliance issue — our implementation is correct per the Bluetooth RSC Service Specification V1.0.

## Options

1. **ANT+ via nrf-softdevice (S340)** — Switch to the full SoftDevice which supports both BLE+ANT. Requires rewriting the BLE stack code but guarantees Garmin compatibility.

2. **Connect IQ data field** — Write a Garmin Connect IQ app that reads BLE RSC directly, bypassing the native sensor management.

3. **Accept BLE-only** — The bridge works for any BLE RSC consumer (Zwift, phone apps, other watches). Accept that Garmin requires ANT+.

## Technical Notes

- trouble-host 0.6 has a packet pool exhaustion issue: the 8th ATT response during GATT service discovery fails with `InsufficientSpace` because the controller only has 1 TX buffer slot and 7 responses queue up waiting
- This is NOT the cause of the Garmin issue (the old project has the same error and was previously working)
- The `reply.send().await` pattern is required for GATT responses (not just `event.accept()`)

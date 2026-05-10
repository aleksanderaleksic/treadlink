use embassy_futures::select::{select, Either};
use embassy_nrf::gpio::Output;
use embassy_sync::watch::Watch;
use embassy_time::{Duration, Timer};

use crate::central::BridgeState;

/// Raw mutex type used for the Watch primitive.
type RawMutex = embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

/// LED task: drives LED1–LED4 based on BridgeState changes.
///
/// LED mapping:
/// - Scanning: blink LED1 at 500ms period (250ms on/off)
/// - Connecting: LED1 solid on
/// - Bridging (no consumer): LED2 solid on
/// - Bridging + consumer: LED2 + LED3 solid on (consumer detection deferred to task 8)
/// - Error/retry: blink LED4 at 100ms period (50ms on/off)
///
/// On state change, all irrelevant LEDs are turned off within one blink cycle.
///
/// Note: nrf52840-dk LEDs are active LOW (set_low = on, set_high = off).
#[embassy_executor::task]
pub async fn led_task(
    watch: &'static Watch<RawMutex, BridgeState, 2>,
    mut led1: Output<'static>,
    mut led2: Output<'static>,
    mut led3: Output<'static>,
    mut led4: Output<'static>,
) {
    // Start with all LEDs off (high = off on active-low)
    led1.set_high();
    led2.set_high();
    led3.set_high();
    led4.set_high();

    let mut receiver = watch.receiver().unwrap();

    loop {
        // Wait for the first state (or next state change)
        let state = receiver.changed().await;

        // Turn off all LEDs on state change
        led1.set_high();
        led2.set_high();
        led3.set_high();
        led4.set_high();

        match state {
            BridgeState::Scanning => {
                // Blink LED1 at 500ms period (250ms on, 250ms off)
                loop {
                    led1.set_low(); // on
                    match select(
                        Timer::after(Duration::from_millis(250)),
                        receiver.changed_and(|s| *s != BridgeState::Scanning),
                    )
                    .await
                    {
                        Either::First(()) => {}
                        Either::Second(_) => break,
                    }

                    led1.set_high(); // off
                    match select(
                        Timer::after(Duration::from_millis(250)),
                        receiver.changed_and(|s| *s != BridgeState::Scanning),
                    )
                    .await
                    {
                        Either::First(()) => {}
                        Either::Second(_) => break,
                    }
                }
            }
            BridgeState::Connecting => {
                // LED1 solid on
                led1.set_low();

                // Wait until state changes away from Connecting
                receiver
                    .changed_and(|s| *s != BridgeState::Connecting)
                    .await;
            }
            BridgeState::Bridging => {
                // LED2 solid on (consumer detection deferred — MVP treats Bridging as LED2 only)
                led2.set_low();

                // Wait until state changes away from Bridging
                receiver
                    .changed_and(|s| *s != BridgeState::Bridging)
                    .await;
            }
        }
    }
}

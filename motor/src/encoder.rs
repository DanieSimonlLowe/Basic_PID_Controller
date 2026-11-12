//! Quadrature encoder reader (noise-hardened).
//!
//! The FIT0450's encoder has two Hall channels (A/B), 90 degrees out of
//! phase. We decode both edges of both channels in a GPIO interrupt
//! (`IO_IRQ_BANK0`), giving 4x the encoder's native pulse count per
//! revolution (a standard "x4 decoding" quadrature scheme). DFRobot quotes
//! 1920 pulses/output-revolution for single-edge counting on one channel -
//! with this x4 decoder, expect on the order of 4x that per revolution.
//! Calibrate empirically (spin the shaft exactly one turn by hand and see
//! what `count()` reads).
//!
//! # Noise handling
//!
//! * Schmitt-trigger input hysteresis is enabled on both pins.
//! * Invalid transitions (both channels changing at once) are ignored and
//!   counted in [`Encoder::error_count`]; the decoder resynchronises on the
//!   next valid edge instead of getting stuck.
//! * Glitches that bounce back (A: 0->1->0) produce +1 then -1 and cancel.
//! * An interrupt-storm guard: if the pins fire far faster than the motor can
//!   physically turn (e.g. a floating/shorted wire), the pin interrupts are
//!   disabled for a short cool-down so the main loop is never starved. They
//!   are re-armed automatically from [`Encoder::count`] / [`Encoder::service`].
//!   Call one of those regularly (any normal control loop does).
//! * The counter uses wrapping arithmetic, so it can never panic in the ISR.
//!
//! Hardware still matters most: use short/twisted/shielded encoder wires,
//! a 1-10 nF capacitor from each channel to GND close to the Pico, and
//! consider a ~1 kOhm series resistor.
//!
//! # This crate owns `IO_IRQ_BANK0`
//!
//! [`Encoder::new`] installs an `IO_IRQ_BANK0` interrupt handler. Don't
//! define your own `IO_IRQ_BANK0` handler elsewhere in the same firmware.
//!
//! Only one [`Encoder`] can usefully exist at a time in this version.

use core::cell::RefCell;

use critical_section::Mutex;
use embedded_hal::digital::InputPin;
use rp2040_hal::gpio::{
    DynPinId, FunctionSioInput,
    Interrupt::{EdgeHigh, EdgeLow},
    Pin, PullUp,
};
use rp2040_hal::pac::interrupt;

/// A GPIO pin, erased down to just its pin number, configured as a
/// pulled-up digital input.
///
/// ```ignore
/// let enc_a = pins.gpio16
///     .reconfigure::<rp2040_hal::gpio::FunctionSioInput, rp2040_hal::gpio::PullUp>()
///     .into_dyn_pin();
/// ```
pub type EncoderInputPin = Pin<DynPinId, FunctionSioInput, PullUp>;

/// Storm guard: more than this many interrupts inside one window is treated
/// as noise. The motor tops out around ~20k edges/s (~20 per ms), so 100 per
/// ms leaves a wide safety margin. Adjust if you use a faster motor.
const STORM_WINDOW_US: u32 = 1_000;
const STORM_MAX_IRQS: u32 = 100;
/// How long the pin interrupts stay disabled after a storm.
const STORM_COOLDOWN_US: u32 = 5_000;

struct State {
    count: i32,
    /// Invalid (double-step) transitions seen.
    errors: u32,
    /// Number of times the storm guard tripped.
    storms: u32,
    storm_active: bool,
    storm_started_us: u32,
    window_start_us: u32,
    window_irqs: u32,
}

static STATE: Mutex<RefCell<State>> = Mutex::new(RefCell::new(State {
    count: 0,
    errors: 0,
    storms: 0,
    storm_active: false,
    storm_started_us: 0,
    window_start_us: 0,
    window_irqs: 0,
}));

// last_state is packed alongside the pins: bit1 = channel A, bit0 = channel B.
static ENCODER_PINS: Mutex<RefCell<Option<(EncoderInputPin, EncoderInputPin, u8)>>> =
    Mutex::new(RefCell::new(None));

/// Quadrature transition table. Index = `(previous_state << 2) | new_state`,
/// where `state = (a << 1) | b`. Value = signed tick delta for that
/// transition (0 for no-change or invalid transitions).
#[rustfmt::skip]
const QUAD_TABLE: [i8; 16] = [
     0, -1,  1,  0,
     1,  0,  0, -1,
    -1,  0,  0,  1,
     0,  1, -1,  0,
];

/// Free-running 1 MHz microsecond counter (lower 32 bits). Read-only access
/// to the TIMER peripheral, so it's safe to use from the ISR.
#[inline]
fn now_us() -> u32 {
    // Safety: read-only access to a free-running counter register.
    unsafe { (*rp2040_hal::pac::TIMER::ptr()).timerawl().read().bits() }
}

fn clear_all(a: &mut EncoderInputPin, b: &mut EncoderInputPin) {
    a.clear_interrupt(EdgeHigh);
    a.clear_interrupt(EdgeLow);
    b.clear_interrupt(EdgeHigh);
    b.clear_interrupt(EdgeLow);
}

fn set_irqs(a: &mut EncoderInputPin, b: &mut EncoderInputPin, enabled: bool) {
    a.set_interrupt_enabled(EdgeHigh, enabled);
    a.set_interrupt_enabled(EdgeLow, enabled);
    b.set_interrupt_enabled(EdgeHigh, enabled);
    b.set_interrupt_enabled(EdgeLow, enabled);
}

fn read_state(a: &mut EncoderInputPin, b: &mut EncoderInputPin) -> u8 {
    let a = a.is_high().unwrap_or(false);
    let b = b.is_high().unwrap_or(false);
    ((a as u8) << 1) | (b as u8)
}

/// Reads ticks from a quadrature encoder via GPIO interrupt.
pub struct Encoder {
    _private: (),
}

impl Encoder {
    /// Take ownership of the two encoder channel pins and start counting
    /// ticks in the background.
    pub fn new(mut pin_a: EncoderInputPin, mut pin_b: EncoderInputPin) -> Self {
        // Input hysteresis rejects slow/shallow noise on the line.
        pin_a.set_schmitt_enabled(true);
        pin_b.set_schmitt_enabled(true);

        clear_all(&mut pin_a, &mut pin_b);
        set_irqs(&mut pin_a, &mut pin_b, true);

        let initial_state = read_state(&mut pin_a, &mut pin_b);

        critical_section::with(|cs| {
            ENCODER_PINS
                .borrow(cs)
                .replace(Some((pin_a, pin_b, initial_state)));
            *STATE.borrow(cs).borrow_mut() = State {
                count: 0,
                errors: 0,
                storms: 0,
                storm_active: false,
                storm_started_us: 0,
                window_start_us: now_us(),
                window_irqs: 0,
            };
        });

        // Safety: we're only unmasking an interrupt whose handler is defined
        // in this module, after the shared state it relies on is populated.
        unsafe {
            rp2040_hal::pac::NVIC::unmask(rp2040_hal::pac::Interrupt::IO_IRQ_BANK0);
        }

        Encoder { _private: () }
    }

    /// Re-arm the pin interrupts if the storm guard tripped and the
    /// cool-down has passed. Called automatically by [`count`](Self::count);
    /// call it yourself if you don't poll `count()` regularly.
    pub fn service(&self) {
        critical_section::with(|cs| {
            let mut st = STATE.borrow(cs).borrow_mut();
            if !st.storm_active {
                return;
            }
            if now_us().wrapping_sub(st.storm_started_us) < STORM_COOLDOWN_US {
                return;
            }
            let mut pins = ENCODER_PINS.borrow(cs).borrow_mut();
            if let Some((pin_a, pin_b, last_state)) = pins.as_mut() {
                // Resync to whatever the lines look like now, then re-arm.
                clear_all(pin_a, pin_b);
                *last_state = read_state(pin_a, pin_b);
                st.window_start_us = now_us();
                st.window_irqs = 0;
                st.storm_active = false;
                set_irqs(pin_a, pin_b, true);
            }
        });
    }

    /// Current signed tick count. Which direction is "positive" depends on
    /// which wire you called `pin_a` vs `pin_b` - swap them to flip the sign.
    pub fn count(&self) -> i32 {
        self.service();
        critical_section::with(|cs| STATE.borrow(cs).borrow().count)
    }

    /// Reset the tick count to zero.
    pub fn reset(&self) {
        critical_section::with(|cs| STATE.borrow(cs).borrow_mut().count = 0);
    }

    /// Number of invalid transitions (skipped/noisy edges) seen so far.
    /// A steadily climbing value means the wiring is noisy or the motor is
    /// spinning faster than the ISR can keep up with.
    pub fn error_count(&self) -> u32 {
        critical_section::with(|cs| STATE.borrow(cs).borrow().errors)
    }

    /// Number of times the interrupt-storm guard has tripped.
    pub fn storm_count(&self) -> u32 {
        critical_section::with(|cs| STATE.borrow(cs).borrow().storms)
    }
}

#[interrupt]
fn IO_IRQ_BANK0() {
    let now = now_us();
    critical_section::with(|cs| {
        let mut pins = ENCODER_PINS.borrow(cs).borrow_mut();
        let mut st = STATE.borrow(cs).borrow_mut();
        let Some((pin_a, pin_b, last_state)) = pins.as_mut() else {
            return;
        };

        // Clear first, then sample: an edge arriving after the sample just
        // raises a fresh interrupt.
        clear_all(pin_a, pin_b);

        // Storm guard: too many interrupts in a short window means the line
        // is floating/shorted/very noisy. Back off so we never starve the
        // main loop; `service()` re-arms us after the cool-down.
        if now.wrapping_sub(st.window_start_us) > STORM_WINDOW_US {
            st.window_start_us = now;
            st.window_irqs = 0;
        }
        st.window_irqs += 1;
        if st.window_irqs > STORM_MAX_IRQS {
            set_irqs(pin_a, pin_b, false);
            st.storm_active = true;
            st.storm_started_us = now;
            st.storms = st.storms.wrapping_add(1);
            return;
        }

        let new_state = read_state(pin_a, pin_b);
        if new_state == *last_state {
            return; // glitch that already returned to the previous level
        }

        let index = ((*last_state << 2) | new_state) as usize;
        let delta = QUAD_TABLE[index];
        if delta == 0 {
            // Both channels changed at once: an edge was missed or noise hit.
            // Don't guess a direction; just record it and resync below.
            st.errors = st.errors.wrapping_add(1);
        } else {
            st.count = st.count.wrapping_add(delta as i32);
        }
        *last_state = new_state;
    });
}

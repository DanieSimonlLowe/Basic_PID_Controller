//! Quadrature encoder reader.
//!
//! The FIT0450's encoder has two Hall channels (A/B), 90 degrees out of
//! phase. We decode both edges of both channels in a GPIO interrupt
//! (`IO_IRQ_BANK0`), giving 4x the encoder's native pulse count per
//! revolution (a standard "x4 decoding" quadrature scheme). DFRobot quotes
//! 1920 pulses/output-revolution for single-edge counting on one channel -
//! with this x4 decoder, expect on the order of 4x that per revolution.
//! Treat that as a starting point and calibrate empirically (spin the shaft
//! exactly one turn by hand and see what `count()` reads) rather than
//! trusting the datasheet number exactly, since gearbox backlash and
//! datasheet rounding both nudge the real figure.
//!
//! # This crate owns `IO_IRQ_BANK0`
//!
//! [`Encoder::new`] installs an `IO_IRQ_BANK0` interrupt handler. Don't
//! define your own `IO_IRQ_BANK0` handler elsewhere in the same firmware, or
//! you'll get a duplicate-symbol link error. If you need other GPIO
//! interrupts too, you'll need to extend `encoder.rs`'s handler rather than
//! adding a second one.
//!
//! Only one [`Encoder`] can usefully exist at a time in this version - if you
//! have two motors, extend `ENCODER_PINS`/`ENCODER_COUNT` into small arrays
//! indexed by motor, or run a second copy of this module under a different
//! name.

use core::cell::RefCell;

use critical_section::Mutex;
use embedded_hal::digital::InputPin;
use rp2040_hal::gpio::{
    DynPinId,
    FunctionSioInput,
    Interrupt::{EdgeHigh, EdgeLow},
    Pin, PullUp,
};
use rp2040_hal::pac::interrupt;

/// A GPIO pin, erased down to just its pin number, configured as a
/// pulled-up digital input - the shape [`Encoder::new`] expects for both
/// encoder channels.
///
/// Get one of these from a `Pins` struct with, e.g.:
/// ```ignore
/// let enc_a = pins.gpio16
///     .reconfigure::<rp2040_hal::gpio::FunctionSioInput, rp2040_hal::gpio::PullUp>()
///     .into_dyn_pin();
/// ```
pub type EncoderInputPin = Pin<DynPinId, FunctionSioInput, PullUp>;

// Cortex-M0+ (the RP2040's core) has no atomic read-modify-write
// instructions, so `core::sync::atomic::AtomicI32::fetch_add` isn't
// available on this target - `Mutex<RefCell<i32>>` guarded by
// `critical_section` (a brief IRQ-disable) does the same job instead.
static ENCODER_COUNT: Mutex<RefCell<i32>> = Mutex::new(RefCell::new(0));

// last_state is packed alongside the pins: bit1 = channel A, bit0 = channel B.
static ENCODER_PINS: Mutex<RefCell<Option<(EncoderInputPin, EncoderInputPin, u8)>>> =
    Mutex::new(RefCell::new(None));

/// Quadrature transition table. Index = `(previous_state << 2) | new_state`,
/// where `state = (a << 1) | b`. Value = signed tick delta for that
/// transition (0 for invalid/bounce transitions, which are ignored).
#[rustfmt::skip]
const QUAD_TABLE: [i8; 16] = [
     0, -1,  1,  0,
     1,  0,  0, -1,
    -1,  0,  0,  1,
     0,  1, -1,  0,
];

/// Reads ticks from a quadrature encoder via GPIO interrupt.
pub struct Encoder {
    _private: (),
}

impl Encoder {
    /// Take ownership of the two encoder channel pins (order matters: `pin_a`
    /// then `pin_b`, matching however you decide to label the encoder's two
    /// signal wires) and start counting ticks in the background.
    pub fn new(mut pin_a: EncoderInputPin, mut pin_b: EncoderInputPin) -> Self {
        // Clear anything stale, then enable both edges on both channels.
        pin_a.clear_interrupt(EdgeHigh);
        pin_a.clear_interrupt(EdgeLow);
        pin_b.clear_interrupt(EdgeHigh);
        pin_b.clear_interrupt(EdgeLow);
        pin_a.set_interrupt_enabled(EdgeHigh, true);
        pin_a.set_interrupt_enabled(EdgeLow, true);
        pin_b.set_interrupt_enabled(EdgeHigh, true);
        pin_b.set_interrupt_enabled(EdgeLow, true);

        let a = pin_a.is_high().unwrap_or(false);
        let b = pin_b.is_high().unwrap_or(false);
        let initial_state = ((a as u8) << 1) | (b as u8);

        critical_section::with(|cs| {
            ENCODER_PINS
                .borrow(cs)
                .replace(Some((pin_a, pin_b, initial_state)));
            *ENCODER_COUNT.borrow(cs).borrow_mut() = 0;
        });

        // Safety: we're only unmasking an interrupt whose handler is defined
        // in this module, after the shared state it relies on is populated.
        unsafe {
            rp2040_hal::pac::NVIC::unmask(rp2040_hal::pac::Interrupt::IO_IRQ_BANK0);
        }

        Encoder { _private: () }
    }

    /// Current signed tick count. Increases when the shaft turns one way,
    /// decreases the other way. Which is "positive" depends on which wire
    /// you called `pin_a` vs `pin_b` - swap them if you want the sign
    /// flipped, rather than fighting it in software.
    pub fn count(&self) -> i32 {
        critical_section::with(|cs| *ENCODER_COUNT.borrow(cs).borrow())
    }

    /// Reset the tick count to zero.
    pub fn reset(&self) {
        critical_section::with(|cs| *ENCODER_COUNT.borrow(cs).borrow_mut() = 0);
    }
}

#[interrupt]
fn IO_IRQ_BANK0() {
    critical_section::with(|cs| {
        let mut binding = ENCODER_PINS.borrow(cs).borrow_mut();
        if let Some((pin_a, pin_b, last_state)) = binding.as_mut() {
            pin_a.clear_interrupt(EdgeHigh);
            pin_a.clear_interrupt(EdgeLow);
            pin_b.clear_interrupt(EdgeHigh);
            pin_b.clear_interrupt(EdgeLow);

            let a = pin_a.is_high().unwrap_or(false);
            let b = pin_b.is_high().unwrap_or(false);
            let new_state = ((a as u8) << 1) | (b as u8);

            let index = ((*last_state << 2) | new_state) as usize;
            let delta = QUAD_TABLE[index];
            if delta != 0 {
                *ENCODER_COUNT.borrow(cs).borrow_mut() += delta as i32;
            }
            *last_state = new_state;
        }
    });
}

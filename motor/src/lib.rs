//! Minimal Rust driver for a DFRobot **FIT0450** geared DC motor (6 V, 120:1,
//! quadrature encoder) wired to a Cytron **MAKER-PI-RP2040**.
//!
//! This crate does exactly three things:
//! 1. [`Motor`] - drive one of the board's two DC motor channels via PWM.
//! 2. [`Encoder`] - count quadrature ticks from the motor's encoder using a
//!    GPIO interrupt.
//! 3. [`UsbSerial`] - send/receive plain text strings over the board's
//!    micro-USB port (shows up as a normal COM port / `/dev/ttyACM*`).
//!
//! It does not attempt PID/closed-loop control, velocity estimation, or
//! anything else - that's on you, built on top of `count()` and
//! `set_speed()`.
//!
//! # Board pinout (MAKER-PI-RP2040 onboard motor driver)
//!
//! | Motor | Forward pin (`MxA`) | Backward pin (`MxB`) | PWM slice |
//! |-------|----------------------|------------------------|-----------|
//! | M1    | GP8                  | GP9                     | 4         |
//! | M2    | GP10                 | GP11                    | 5         |
//!
//! The encoder is *not* on a dedicated header - wire it to any free Grove
//! port. See `examples/basic_usage.rs` for a full wiring + code walkthrough
//! using Grove port 4 (GP16 / GP17).
//!
//! # Important hardware note
//!
//! RP2040 GPIOs are only 3.3 V tolerant. The FIT0450's encoder is spec'd to
//! run from 4.5-7.5 V. **Power the encoder's VCC pin from the Maker Pi's 3V3
//! rail, not 5V/VIN**, so its Hall outputs swing 0-3.3V. If you need to run
//! the encoder electronics at 5V for some reason, put a level shifter (or at
//! minimum a resistor divider) on the two signal lines before they reach the
//! RP2040 - do not feed 5V logic directly into a GPIO.
#![no_std]

pub mod encoder;
pub mod motor;
pub mod usb_serial;

pub use encoder::{Encoder, EncoderInputPin};
pub use motor::Motor;
pub use usb_serial::UsbSerial;

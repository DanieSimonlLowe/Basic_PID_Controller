//! PWM driver for one DC motor channel on the MAKER-PI-RP2040.
//!
//! The onboard driver takes two pins per motor (`MxA` / `MxB`). This matches
//! the board's own "Quick Test" buttons: pressing `MxA` spins the motor
//! forward at full speed, `MxB` spins it backward. We reproduce that with
//! PWM instead of a fixed full-speed drive:
//!
//! - PWM on `A`, `B` held low  -> forward, proportional to duty cycle
//! - PWM on `B`, `A` held low  -> backward, proportional to duty cycle
//! - both low                  -> coast (motor free-wheels)
//! - both high                 -> active brake
//!
//! If your motor spins the "wrong" way, it's not a bug - just swap the two
//! motor terminal wires on the screw connector (the board's own docs call
//! this out too).

use embedded_hal::pwm::SetDutyCycle;
use rp2040_hal::pwm::{AnySlice, Channel, A, B};

/// One DC motor channel, driven by a PWM pin pair (`MxA` / `MxB`).
///
/// `S` is the PWM slice the two channels belong to (e.g. the type you get
/// back from `pwm_slices.pwm4` for motor M1, or `pwm_slices.pwm5` for M2 on
/// the MAKER-PI-RP2040).
pub struct Motor<S: AnySlice> {
    forward: Channel<S, A>,
    backward: Channel<S, B>,
}

impl<S: AnySlice> Motor<S> {
    /// Build a motor driver from the two PWM channels already wired to a
    /// motor's `MxA` and `MxB` pins (via `channel.output_to(pin)`).
    ///
    /// ```ignore
    /// let mut pwm4 = pwm_slices.pwm4;
    /// pwm4.set_top(u16::MAX - 1); // pick your PWM resolution/frequency
    /// pwm4.enable();
    ///
    /// let mut ch_a = pwm4.channel_a;
    /// ch_a.output_to(pins.gpio8); // M1A
    /// let mut ch_b = pwm4.channel_b;
    /// ch_b.output_to(pins.gpio9); // M1B
    ///
    /// let mut motor1 = fit0450_rp2040::Motor::new(ch_a, ch_b);
    /// motor1.set_speed(0.5); // half speed, forward
    /// ```
    pub fn new(mut forward: Channel<S, A>, mut backward: Channel<S, B>) -> Self {
        forward.set_enabled(true);
        backward.set_enabled(true);
        let mut motor = Self { forward, backward };
        motor.stop();
        motor
    }

    /// Drive the motor. `speed` is clamped to `-1.0..=1.0`:
    /// positive = forward, negative = backward, `0.0` = coast.
    pub fn set_speed(&mut self, speed: f32) {
        let speed = speed.clamp(-1.0, 1.0);
        let max_fwd = self.forward.max_duty_cycle() as f32;
        let max_bwd = self.backward.max_duty_cycle() as f32;

        if speed >= 0.0 {
            let _ = self.forward.set_duty_cycle((speed * max_fwd) as u16);
            let _ = self.backward.set_duty_cycle(0);
        } else {
            let _ = self.forward.set_duty_cycle(0);
            let _ = self.backward.set_duty_cycle((-speed * max_bwd) as u16);
        }
    }

    /// Let the motor free-wheel to a stop (both outputs low).
    pub fn stop(&mut self) {
        let _ = self.forward.set_duty_cycle(0);
        let _ = self.backward.set_duty_cycle(0);
    }

    /// Actively brake the motor (both outputs driven high / shorted).
    pub fn brake(&mut self) {
        let max_fwd = self.forward.max_duty_cycle();
        let max_bwd = self.backward.max_duty_cycle();
        let _ = self.forward.set_duty_cycle(max_fwd);
        let _ = self.backward.set_duty_cycle(max_bwd);
    }
}

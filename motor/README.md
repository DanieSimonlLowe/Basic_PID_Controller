# fit0450-rp2040

A tiny Rust `no_std` library: talk to a **DFRobot FIT0450** geared DC motor
(6 V, 120:1 gearbox, quadrature encoder) through a **Cytron MAKER-PI-RP2040**.
That's it - no PID, no velocity filtering, just:

- `Motor` - PWM speed/direction control on one of the board's two motor channels.
- `Encoder` - background tick counting from the encoder, via GPIO interrupt.
- `UsbSerial` - send/receive plain text strings over the board's micro-USB port.

Built against `rp2040-hal` 0.10.x (embedded-hal 1.0 traits).

## Wiring

**Motor:** connect the FIT0450's two motor leads to the `M1` (or `M2`) screw
terminal on the board. Polarity decides which way "forward" spins - if it's
backwards, swap the two wires rather than fighting it in software.

**Encoder:** the encoder is a separate 3-4 wire connection (VCC, GND, channel
A, channel B) - it isn't part of the screw terminal. Wire it to any free
Grove port; the example uses Grove port 4:

| Encoder wire | Connect to |
|---|---|
| VCC | Board **3V3**, not 5V/VIN |
| GND | GND |
| Channel A | GP16 |
| Channel B | GP17 |

> **Why 3V3 and not 5V:** RP2040 GPIOs are only 3.3 V tolerant. The FIT0450's
> encoder is spec'd for 4.5-7.5 V and its Hall outputs swing to whatever
> voltage powers it. Powering it from 3V3 keeps its output logic at a safe
> 0-3.3 V. If you have a reason to run it at 5V, put a level shifter (or at
> least a resistor divider) on the two signal lines before they reach the
> RP2040 - don't feed 5V straight into a GPIO.

Onboard pin mapping used by this driver (fixed by the board, not configurable):

| Motor | Forward (`MxA`) | Backward (`MxB`) | PWM slice |
|---|---|---|---|
| M1 | GP8 | GP9 | 4 |
| M2 | GP10 | GP11 | 5 |

## Usage

```rust
use fit0450_rp2040::{Encoder, Motor};
use rp2040_hal::gpio::{FunctionSioInput, PullUp};

// --- Motor on M1 (GP8/GP9, PWM slice 4) ---
let mut pwm4 = pwm_slices.pwm4;
pwm4.set_top(u16::MAX - 1); // sets PWM resolution/frequency - tune to taste
pwm4.enable();

let mut ch_a = pwm4.channel_a;
ch_a.output_to(pins.gpio8);
let mut ch_b = pwm4.channel_b;
ch_b.output_to(pins.gpio9);

let mut motor1 = Motor::new(ch_a, ch_b);
motor1.set_speed(0.5);   // 50% forward
motor1.set_speed(-1.0);  // full backward
motor1.brake();          // active brake
motor1.stop();           // coast

// --- Encoder on Grove port 4 (GP16/GP17) ---
let enc_a = pins.gpio16
    .reconfigure::<FunctionSioInput, PullUp>()
    .into_dyn_pin();
let enc_b = pins.gpio17
    .reconfigure::<FunctionSioInput, PullUp>()
    .into_dyn_pin();

let encoder = Encoder::new(enc_a, enc_b);

loop {
    let ticks = encoder.count(); // signed i32, resets available via .reset()
}
```

### USB serial (send/receive strings over micro-USB)

```rust
use fit0450_rp2040::UsbSerial;

let mut usb_serial = UsbSerial::new(
    pac.USBCTRL_REGS,
    pac.USBCTRL_DPRAM,
    clocks.usb_clock,
    &mut pac.RESETS,
);

loop {
    usb_serial.poll(); // call every loop iteration - USB needs servicing every ~10ms

    if let Some(line) = usb_serial.read_line() {
        // `line` is a heapless::String<64> with the line ending stripped
        usb_serial.write_line("got it");
    }

    usb_serial.write_line("hello from the RP2040"); // don't call this every
                                                     // single iteration in a
                                                     // real project - see the
                                                     // example for a rate-
                                                     // limited version
}
```

Once flashed, the board shows up as a normal serial port on your computer
(`COMx` on Windows, `/dev/ttyACM*` on Linux/macOS) - open it in any terminal
program (PuTTY, `screen`, Arduino's Serial Monitor, Python's `pyserial`) to
send and receive lines of text. No drivers needed.

See `examples/basic_usage.rs` for a complete, buildable firmware `main.rs`
(clock init, watchdog, everything).

## Notes and limitations

- **One encoder at a time.** `Encoder::new` installs the `IO_IRQ_BANK0`
  interrupt handler itself. Don't also define your own `IO_IRQ_BANK0` handler
  elsewhere in your firmware (link error), and don't call `Encoder::new`
  twice if you're running two motors - extend `src/encoder.rs` into an array
  if you need both encoders read.
- **USB serial is ASCII-oriented and polled.** `UsbSerial` decodes incoming
  bytes one-to-one as ASCII characters (multi-byte UTF-8 input won't
  round-trip correctly), and needs `poll()` called at least every ~10ms - so
  avoid long blocking delays in a loop that also services USB. It also only
  supports one instance, matching the one physical USB peripheral.
- **Counts-per-revolution.** DFRobot quotes 1920 pulses per output-shaft
  revolution for single-edge counting on one channel. This driver does full
  x4 quadrature decoding (both edges, both channels), so expect roughly 4x
  that per revolution. Calibrate it yourself - turn the output shaft exactly
  one full revolution by hand and see what `count()` reads - rather than
  trusting the datasheet figure exactly, since gearbox backlash nudges it.
- **PWM frequency/resolution** is set via `slice.set_top(...)` before you
  build the `Motor` - lower `top` values give a higher PWM frequency (quieter
  to the ear, less torque resolution); higher `top` gives finer speed control
  at a lower frequency. `u16::MAX - 1` in the example lands around a few
  hundred Hz at the RP2040's default system clock - fine as a starting point,
  tune based on how the motor sounds/behaves.
- No stall/current protection is implemented - the FIT0450's stall current
  (2.8 A) exceeds the onboard driver's continuous rating (1 A/channel), so
  avoid stalling the motor for long under load.

## Integrating into your own firmware project

This is written as a library, not a standalone firmware image. Add it as a
path or git dependency in your own `no_std`/`no_main` binary crate's
`Cargo.toml`, alongside whatever HAL setup (clocks, watchdog, `Pins`) your
project already does - `examples/basic_usage.rs` shows the minimum full setup
if you're starting from scratch.

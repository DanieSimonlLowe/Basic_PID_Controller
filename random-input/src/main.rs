//! Full example firmware: drive a FIT0450 on motor channel M1, read its
//! encoder, and talk to it over USB serial - on a Cytron MAKER-PI-RP2040.
//!
//! Wiring:
//! - Motor terminals -> M1 screw terminal on the board (polarity picks
//!   direction; swap the two wires if "forward" spins the wrong way).
//! - Encoder VCC -> the board's 3V3 pin (NOT 5V/VIN - see src/lib.rs docs).
//! - Encoder GND -> GND.
//! - Encoder channel A -> Grove port 4, GP16.
//! - Encoder channel B -> Grove port 4, GP17.
//! - The board's micro-USB port doubles as both power and the virtual
//!   serial port - no extra wiring needed for that part.
//!
//! Once flashed, open a serial terminal (PuTTY, `screen /dev/ttyACM0
//! 115200`, Arduino's Serial Monitor, etc. - baud rate doesn't actually
//! matter over USB CDC, but most terminal programs want you to pick one) on
//! the port the board enumerates as. Type a number between -1.0 and 1.0 and
//! press enter to set the motor's speed; the board replies `ok` or an error,
//! and separately prints an encoder tick count every 100ms.
//!
//! This file is written the same way rp2040-hal's own `examples/` are, so it
//! builds standalone if you drop this whole `fit0450-rp2040` folder in as a
//! project by itself (`cargo run --example basic_usage`, with `elf2uf2-rs`
//! or `probe-rs` set as your runner). If you're instead pulling this crate
//! in as a dependency of your own firmware, just copy the body of `main()`
//! into your own `src/main.rs`.

#![no_std]
#![no_main]

mod rand;

use core::fmt::Write as _;

use panic_halt as _;

use rp2040_hal as hal;
use rp2040_hal::gpio::{FunctionSioInput, PullUp};
use rp2040_hal::pac;
use rp2040_hal::Timer;

use fit0450_rp2040::{Encoder, Motor, UsbSerial};

use rand::Rng;

#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

const XTAL_FREQ_HZ: u32 = 12_000_000u32;
const REPORT_INTERVAL_US: u64 = 1000; // 10ms

#[rp2040_hal::entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    let clocks = hal::clocks::init_clocks_and_plls(
        XTAL_FREQ_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    let sio = hal::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // --- Motor M1: GP8 (M1A) / GP9 (M1B), PWM slice 4 ---
    let pwm_slices = hal::pwm::Slices::new(pac.PWM, &mut pac.RESETS);
    let mut pwm4 = pwm_slices.pwm4;
    pwm4.set_top(u16::MAX - 1);
    pwm4.enable();

    let mut ch_a = pwm4.channel_a;
    ch_a.output_to(pins.gpio8);
    let mut ch_b = pwm4.channel_b;
    ch_b.output_to(pins.gpio9);

    let mut motor1 = Motor::new(ch_a, ch_b);

    // --- Encoder: Grove port 4, GP16 (A) / GP17 (B) ---
    let enc_a = pins
        .gpio16
        .reconfigure::<FunctionSioInput, PullUp>()
        .into_dyn_pin();
    let enc_b = pins
        .gpio17
        .reconfigure::<FunctionSioInput, PullUp>()
        .into_dyn_pin();
    let encoder = Encoder::new(enc_a, enc_b);

    // --- USB serial, over the board's micro-USB port ---
    let mut usb_serial = UsbSerial::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        &mut pac.RESETS,
    );

    let mut last_report_us = timer.get_counter().ticks();

    let mut rng = Rng::new(12345);
    let mut speed = rng.range_f32(-1.0, 1.0);
    motor1.set_speed(speed);
    let mut gap_us = rng.range_u64(500_000, 4_000_000);
    let mut last_speed_change = last_report_us;

    loop {
        // Service USB on every iteration - this loop has no blocking
        // delays, so it runs far more often than the ~10ms USB needs.
        usb_serial.poll();

        let now_us = timer.get_counter().ticks();
        if now_us.wrapping_sub(last_report_us) >= REPORT_INTERVAL_US {
            last_report_us = now_us;
            let mut msg: heapless::String<32> = heapless::String::new();
            let _ = write!(msg, "{},{},{}", now_us, encoder.count(), speed);
            usb_serial.write_line(&msg);

            if now_us.wrapping_sub(last_speed_change) >= gap_us {
                last_speed_change = now_us;
                gap_us = rng.range_u64(500_000, 4_000_000);
                speed = rng.range_f32(-1.0, 1.0);
                motor1.set_speed(speed);
            }
        }
    }
}

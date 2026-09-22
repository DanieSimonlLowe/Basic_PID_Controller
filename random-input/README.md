# fit0450-basic

The simplest possible program using the `fit0450-rp2040` library: spin the
motor on M1 forward at 40% speed for two seconds, then stop.

## Setup

This project depends on the `fit0450-rp2040` library via a **path
dependency** - it expects that folder to be right next to this one:

```
some-folder/
├── fit0450-rp2040/       <- the library
└── fit0450-basic/        <- this project
```

If you put it somewhere else, edit the path in `Cargo.toml`:

```toml
fit0450-rp2040 = { path = "../fit0450-rp2040" }
```

## Wiring

Just the motor - connect its two leads to the `M1` screw terminal on the
board. No encoder or USB wiring needed for this example.

## Build and flash

One-time setup (skip if you've already done this for another project):

```
rustup target add thumbv6m-none-eabi
cargo install elf2uf2-rs
```

Then, from this folder:

```
cargo build --release   # catch typos before touching hardware
```

Hold the board's BOOTSEL button, plug it in via USB, release the button,
then:

```
cargo run --release
```

This builds, converts the binary to a `.uf2` file, copies it to the board
(which shows up as a `RPI-RP2` USB drive while in bootloader mode), and the
board reboots into your program automatically.

## What's next

This program never talks back to you - it just moves once and stops. Once
this is working, the natural next steps (both already built out in the
`fit0450-rp2040` library, see its own README) are:

- Read the `Encoder` to see how far the motor actually turned.
- Add `UsbSerial` so you can send commands and see results from your
  computer, instead of hardcoding a speed and a delay.

The library's own `examples/basic_usage.rs` shows all three used together.

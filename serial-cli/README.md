# fit0450-serial-cli

A small command-line tool that runs on your **computer** (not the
microcontroller) to send and receive plain text over USB serial with the
Maker Pi RP2040 - or any USB CDC-ACM serial device, really.

Type a line, press Enter, it's sent to the board. Anything the board sends
back is printed as it arrives, prefixed with `<- `, without blocking your
typing.

This has been built and exercised end-to-end (real compile, plus a live test
against a virtual serial port pair simulating a device that echoes input and
sends unsolicited lines) - not just written from documentation.

## Build

```
cargo build --release
```

The binary ends up at `target/release/fit0450-serial-cli` (or
`fit0450-serial-cli.exe` on Windows).

## Run

```
fit0450-serial-cli <port> [baud]
```

Run with no arguments to get a list of currently connected serial ports:

```
fit0450-serial-cli
```

Examples:

```
fit0450-serial-cli COM5                    # Windows
fit0450-serial-cli /dev/ttyACM0            # Linux
fit0450-serial-cli /dev/cu.usbmodem14201   # macOS
```

The baud rate argument is accepted for compatibility (some tools/OSes expect
one) but doesn't actually mean anything to a USB CDC-ACM device - there's no
real UART clock involved on either end, so any value works. It defaults to
115200 if you leave it out.

Finding the port name on Windows: open Device Manager, look under "Ports
(COM & LPT)" while the board is plugged in.

## Using it with the `fit0450-rp2040` firmware

If you flashed the `basic_usage` example from the `fit0450-rp2040` crate,
connect and type a number between `-1.0` and `1.0` to set the motor speed:

```
Connected to COM5. Type a line and press Enter to send it (Ctrl+C to quit).
<- ticks: 0
0.5
<- ok
<- ticks: 142
<- ticks: 318
```

## How it works

Two threads share one serial port handle (via `try_clone`, the standard
pattern for this in the `serialport` crate): one blocks on your terminal
input and writes each line out, the other blocks reading bytes from the
board and prints each complete line as it arrives. That's what lets you type
a command and still see telemetry/replies show up in between keystrokes,
rather than everything happening in lockstep.

## Notes

- Asserts DTR when it opens the port. This tells the `fit0450-rp2040`
  firmware "a real application is listening" - the firmware uses that signal
  to decide whether to bother sending data back, so without it you can send
  to the board fine but never receive anything.
- Treats incoming bytes as ASCII, one-to-one - fine for simple text
  protocols, not for arbitrary UTF-8 from the device.
- On Linux, this needs `libudev` at build time for port enumeration
  (`sudo apt install libudev-dev` on Debian/Ubuntu, or the equivalent for
  your distro). Windows and macOS need nothing extra.
- Exits when you press Ctrl+C, or when stdin closes (e.g. if you pipe input
  from a file/script instead of typing interactively).

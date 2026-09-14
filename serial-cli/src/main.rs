//! A small PC-side tool to talk to the FIT0450 controller board (or any USB
//! CDC-ACM serial device) over the micro-USB connection.
//!
//! Runs on your computer, not the microcontroller: type a line and press
//! Enter to send it to the board; anything the board sends back is printed
//! as it arrives, prefixed with `<- `.
//!
//! Usage:
//!   fit0450-serial-cli <port> [baud]
//!
//! Examples:
//!   fit0450-serial-cli COM5            (Windows)
//!   fit0450-serial-cli /dev/ttyACM0    (Linux)
//!   fit0450-serial-cli /dev/cu.usbmodem14201   (macOS)
//!
//! Run with no arguments to see a list of currently connected serial ports.
//!
//! Note: the baud rate argument is accepted for compatibility but doesn't
//! actually mean anything to a USB CDC-ACM device like the RP2040's USB
//! serial port - there's no real UART clock involved, so any value works.

use std::env;
use std::io::{self, BufRead, Read, Write};
use std::thread;
use std::time::Duration;

fn print_available_ports() {
    match serialport::available_ports() {
        Ok(ports) if !ports.is_empty() => {
            eprintln!("Available ports:");
            for p in ports {
                eprintln!("  {}", p.port_name);
            }
        }
        Ok(_) => eprintln!("No serial ports detected. Is the board plugged in?"),
        Err(e) => eprintln!("Couldn't list serial ports: {e}"),
    }
}

fn main() {
    let mut args = env::args().skip(1);

    let port_name = match args.next() {
        Some(p) => p,
        None => {
            eprintln!("Usage: fit0450-serial-cli <port> [baud]\n");
            print_available_ports();
            eprintln!("\nExample: fit0450-serial-cli COM5");
            std::process::exit(1);
        }
    };

    let baud: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(115_200);

    let mut port = match serialport::new(&port_name, baud)
        .timeout(Duration::from_millis(50))
        .open()
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to open {port_name}: {e}\n");
            print_available_ports();
            std::process::exit(1);
        }
    };

    // Assert DTR ("a real application has this port open"). USB CDC-ACM
    // devices commonly use this signal to decide whether anyone's actually
    // listening before they bother transmitting - the fit0450-rp2040
    // firmware does exactly that, so without this line you can send to the
    // board fine but never receive anything back.
    if let Err(e) = port.write_data_terminal_ready(true) {
        eprintln!("Warning: couldn't assert DTR ({e}) - the device may not send data back.");
    }

    println!("Connected to {port_name}. Type a line and press Enter to send it (Ctrl+C to quit).");

    let mut reader = match port.try_clone() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to open a second handle to {port_name}: {e}");
            std::process::exit(1);
        }
    };
    let mut writer = port;

    // Reader thread: print every line the device sends, as it arrives, so
    // it doesn't block waiting for you to type something.
    let _reader_handle = thread::spawn(move || {
        let mut line_buf = String::new();
        let mut byte = [0u8; 1];
        loop {
            match reader.read(&mut byte) {
                Ok(0) => continue,
                Ok(_) => match byte[0] {
                    b'\n' => {
                        println!("<- {}", line_buf.trim_end_matches('\r'));
                        let _ = io::stdout().flush();
                        line_buf.clear();
                    }
                    // Drop bytes that aren't valid ASCII rather than
                    // corrupting the line - fine for simple text protocols;
                    // if you need full UTF-8 from the device this is the
                    // spot to change.
                    b if b.is_ascii() => line_buf.push(b as char),
                    _ => {}
                },
                Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
                Err(e) => {
                    eprintln!("\nSerial read error: {e} (device disconnected?)");
                    break;
                }
            }
        }
    });

    // Main thread: forward each line you type straight to the device.
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("stdin error: {e}");
                break;
            }
        };
        if writer.write_all(line.as_bytes()).is_err() || writer.write_all(b"\n").is_err() {
            eprintln!("Serial write error - device disconnected?");
            break;
        }
        let _ = writer.flush();
    }
    // Exiting here (stdin closed, or a write error) ends the process, which
    // tears down the reader thread too - no need to join it, and joining
    // would just hang forever waiting for a thread that only exits on error.
}

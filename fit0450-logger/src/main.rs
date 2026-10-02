//! Reads `{now_us},{encoder.count()},{speed}` lines from the FIT0450
//! controller board over its USB CDC-ACM serial connection and continuously
//! appends each parsed reading to a log file using the `structlog` crate.
//!
//! This tool is read-only: it never writes anything to the microcontroller.
//!
//! Usage:
//!   fit0450-logger <port> [output_file] [baud]
//!
//! Examples:
//!   fit0450-logger COM5                          (Windows, log -> readings.slog)
//!   fit0450-logger /dev/ttyACM0 run1.slog         (Linux, custom log file)
//!   fit0450-logger /dev/cu.usbmodem14201          (macOS)
//!
//! Run with no arguments to see a list of currently connected serial ports.
//!
//! Reading back the log later:
//!   for r in structlog::iter::<Reading, _>("readings.slog")? { println!("{:?}", r?); }
//!
//! Note: the baud rate argument is accepted for compatibility but doesn't
//! actually mean anything to a USB CDC-ACM device like the RP2040's USB
//! serial port - there's no real UART clock involved, so any value works.

use std::env;
use std::io::{self, Read};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// One line of telemetry from the board: `{now_us},{encoder.count()},{speed}`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
struct Reading {
    now_us: u64,
    count: i32,
    speed: f32,
}

/// Parses a line of the form "now_us,count,speed". Returns `None` (and lets
/// the caller warn) rather than panicking, so one bad or partial line -
/// noise on first connect, a boot banner, a torn read - doesn't kill a
/// long-running logging session.
fn parse_reading(line: &str) -> Option<Reading> {
    let mut parts = line.trim().splitn(3, ',');
    let now_us = parts.next()?.trim().parse().ok()?;
    let count = parts.next()?.trim().parse().ok()?;
    let speed = parts.next()?.trim().parse().ok()?;
    Some(Reading { now_us, count, speed })
}

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

// fsync the log every this many readings, rather than on every single one,
// so a crash loses at most this many already-flushed-to-OS records while
// keeping steady-state disk I/O low. Data is still pushed to the OS (flush)
// after every reading; this only controls how often it's forced to disk.
const SYNC_EVERY: usize = 20;

fn main() {
    let mut args = env::args().skip(1);

    let port_name = match args.next() {
        Some(p) => p,
        None => {
            eprintln!("Usage: fit0450-logger <port> [output_file] [baud]\n");
            print_available_ports();
            eprintln!("\nExample: fit0450-logger COM5 readings.slog");
            std::process::exit(1);
        }
    };

    let output_path = args.next().unwrap_or_else(|| "readings.slog".to_string());
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
    // firmware does exactly that, so without this line the board may never
    // send anything back.
    if let Err(e) = port.write_data_terminal_ready(true) {
        eprintln!("Warning: couldn't assert DTR ({e}) - the device may not send data back.");
    }

    let mut log = match structlog::LogWriter::<Reading>::open(&output_path) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to open log file {output_path}: {e}");
            std::process::exit(1);
        }
    };

    println!("Connected to {port_name}. Logging readings to {output_path} (Ctrl+C to quit).");

    let mut line_buf = String::new();
    let mut byte = [0u8; 1];
    let mut since_sync = 0usize;

    loop {
        match port.read(&mut byte) {
            Ok(0) => continue,
            Ok(_) => match byte[0] {
                b'\n' => {
                    let line = line_buf.trim_end_matches('\r').to_string();
                    line_buf.clear();
                    if line.is_empty() {
                        continue;
                    }

                    match parse_reading(&line) {
                        Some(reading) => {
                            println!(
                                "<- now_us={} count={} speed={}",
                                reading.now_us, reading.count, reading.speed
                            );
                            if let Err(e) = log.append(&reading) {
                                eprintln!("Failed to append reading to log: {e}");
                                continue;
                            }
                            if let Err(e) = log.flush() {
                                eprintln!("Failed to flush log: {e}");
                            }
                            since_sync += 1;
                            if since_sync >= SYNC_EVERY {
                                if let Err(e) = log.sync() {
                                    eprintln!("Failed to sync log to disk: {e}");
                                }
                                since_sync = 0;
                            }
                        }
                        None => {
                            eprintln!("Skipping unparseable line: {line:?}");
                        }
                    }
                }
                // Drop bytes that aren't valid ASCII rather than corrupting
                // the line - fine for this simple CSV protocol.
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

    // Final sync so a normal exit (device unplugged, read error) doesn't
    // leave recent readings sitting unflushed in the OS page cache.
    if let Err(e) = log.sync() {
        eprintln!("Failed to sync log on exit: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_line() {
        let r = parse_reading("123456789,-42,3.14").unwrap();
        assert_eq!(r.now_us, 123_456_789);
        assert_eq!(r.count, -42);
        assert!((r.speed - 3.14).abs() < 1e-6);
    }

    #[test]
    fn tolerates_stray_whitespace() {
        let r = parse_reading(" 1, 2, 3.0 \r").unwrap();
        assert_eq!(r.now_us, 1);
        assert_eq!(r.count, 2);
        assert_eq!(r.speed, 3.0);
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(parse_reading("").is_none());
        assert!(parse_reading("not,a,number").is_none());
        assert!(parse_reading("1,2").is_none()); // missing speed
        assert!(parse_reading("1,2,3,4").is_none()); // 4th field makes "3,4" an invalid speed
    }
}

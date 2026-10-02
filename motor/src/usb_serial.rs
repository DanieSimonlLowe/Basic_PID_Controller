//! USB CDC-ACM (virtual serial port) over the board's micro-USB connector.
//!
//! Once flashed, the board enumerates as a normal serial port - `COMx` on
//! Windows, `/dev/ttyACM0` (or similar) on Linux/macOS - that any terminal
//! program (PuTTY, `screen`, Arduino's Serial Monitor, Python's `pyserial`,
//! etc.) can open. No custom drivers needed.
//!
//! This is **polled**, not interrupt-driven: call [`UsbSerial::poll`] from
//! your main loop on basically every iteration. USB requires being serviced
//! at least once every ~10ms or the host will consider the device stalled,
//! so don't put long blocking delays between `poll()` calls.
//!
//! # Limitations
//! - ASCII-oriented: incoming bytes are decoded one-to-one as `char`s, so
//!   multi-byte UTF-8 input won't round-trip correctly. Fine for plain
//!   ASCII commands/telemetry, not for arbitrary Unicode text.
//! - Only one [`UsbSerial`] can exist - [`UsbSerial::new`] must only be
//!   called once, matching there being exactly one USB peripheral (same
//!   restriction as [`crate::Encoder::new`] and `IO_IRQ_BANK0`).
//! - [`UsbSerial::write_str`] gives up after a bounded number of retries if
//!   nothing is reading (e.g. no terminal has the port open) rather than
//!   hanging your control loop forever.

use heapless::{Deque, String};
use rp2040_hal as hal;
use usb_device::bus::UsbBusAllocator;
use usb_device::device::{StringDescriptors, UsbDevice, UsbDeviceBuilder, UsbVidPid};
use usbd_serial::SerialPort;

// Cortex-M0+ has no atomic compare-exchange instruction, which rules out
// crates like `static_cell` that rely on one (via `portable-atomic`) to
// guard a "create the 'static value exactly once" check. We get the same
// `'static` lifetime the USB stack needs the same way `encoder.rs` handles
// its own one-off static state: a plain `static mut`, guarded not by a
// runtime check but by the documented invariant that `UsbSerial::new` is
// only ever called once (there's only one physical USB peripheral anyway).
static mut USB_BUS: Option<UsbBusAllocator<hal::usb::UsbBus>> = None;

/// Max length of one received line. Bytes beyond this before a `\n` are
/// dropped and the (incomplete) line is discarded.
pub const LINE_CAPACITY: usize = 64;
/// Max number of complete lines buffered before `read_line()` catches up.
/// Once full, newly completed lines are dropped rather than overwriting
/// older unread ones.
const LINE_QUEUE_DEPTH: usize = 4;

/// A USB CDC-ACM virtual serial port for plain-text send/receive.
pub struct UsbSerial {
    device: UsbDevice<'static, hal::usb::UsbBus>,
    port: SerialPort<'static, hal::usb::UsbBus>,
    line_buf: String<LINE_CAPACITY>,
    completed_lines: Deque<String<LINE_CAPACITY>, LINE_QUEUE_DEPTH>,
}

impl UsbSerial {
    /// Set up the USB peripheral and enumerate as a serial port.
    ///
    /// Call **exactly once**, after clocks are initialised:
    /// ```ignore
    /// let usb_serial = UsbSerial::new(
    ///     pac.USBCTRL_REGS,
    ///     pac.USBCTRL_DPRAM,
    ///     clocks.usb_clock,
    ///     &mut pac.RESETS,
    /// );
    /// ```
    #[allow(static_mut_refs)]
    pub fn new(
        usbctrl_regs: hal::pac::USBCTRL_REGS,
        usbctrl_dpram: hal::pac::USBCTRL_DPRAM,
        usb_clock: hal::clocks::UsbClock,
        resets: &mut hal::pac::RESETS,
    ) -> Self {
        // `force_vbus_detect = true`: the Maker Pi RP2040 doesn't wire VBUS
        // sense to a dedicated pin the way some minimal boards require, so
        // we tell the peripheral to assume USB is always present rather
        // than waiting for a VBUS-detect signal that may never come.
        let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
            usbctrl_regs,
            usbctrl_dpram,
            usb_clock,
            true,
            resets,
        ));

        // Safety: `UsbSerial::new` is documented as call-once. Nothing else
        // in this module touches `USB_BUS`, and this write happens before
        // the reference below is ever handed out.
        unsafe {
            USB_BUS = Some(usb_bus);
        }
        let bus_ref: &'static UsbBusAllocator<hal::usb::UsbBus> =
            unsafe { USB_BUS.as_ref().unwrap() };

        let port = SerialPort::new(bus_ref);

        // 0x16c0 / 0x27dd is the open-source "test PID" range (pid.codes /
        // VOTI), fine for personal/hobby projects that aren't shipping a
        // commercial product under their own VID.
        let device = UsbDeviceBuilder::new(bus_ref, UsbVidPid(0x16c0, 0x27dd))
            .strings(&[StringDescriptors::default()
                .manufacturer("DIY Robotics")
                .product("FIT0450 motor controller")
                .serial_number("FIT0450-01")])
            .unwrap()
            .device_class(usbd_serial::USB_CLASS_CDC)
            .build();

        Self {
            device,
            port,
            line_buf: String::new(),
            completed_lines: Deque::new(),
        }
    }

    /// Service the USB stack and drain any newly-arrived bytes into the
    /// line buffer. Call this every main-loop iteration.
    pub fn poll(&mut self) {
        if self.device.poll(&mut [&mut self.port]) {
            self.drain_rx();
        }
    }

    fn drain_rx(&mut self) {
        let mut buf = [0u8; 64];
        loop {
            match self.port.read(&mut buf) {
                Ok(0) => break,
                Ok(count) => {
                    for &b in &buf[..count] {
                        match b {
                            b'\n' => {
                                let line = core::mem::replace(&mut self.line_buf, String::new());
                                // If the reader isn't keeping up, drop the
                                // newest line rather than panicking or
                                // overwriting an older unread one.
                                let _ = self.completed_lines.push_back(line);
                            }
                            b'\r' => { /* wait for the following \n */ }
                            other => {
                                if self.line_buf.push(other as char).is_err() {
                                    // Line exceeded LINE_CAPACITY - drop it
                                    // and start clean on the next byte
                                    // rather than silently truncating.
                                    self.line_buf.clear();
                                }
                            }
                        }
                    }
                }
                Err(_) => break, // WouldBlock (no more data) or a transient error
            }
        }
    }

    /// Pop the oldest complete line received (without its line ending), if
    /// any. Returns `None` if nothing new has arrived - keep calling
    /// `poll()` then `read_line()` from your main loop.
    pub fn read_line(&mut self) -> Option<String<LINE_CAPACITY>> {
        self.completed_lines.pop_front()
    }

    /// True once a host application has actually opened the port (DTR
    /// asserted) - useful to avoid writing telemetry into the void before
    /// anyone's listening.
    pub fn is_connected(&self) -> bool {
        self.port.dtr()
    }

    /// Write a string over USB serial, retrying (while servicing the USB
    /// stack) until every byte is queued, up to a bounded number of
    /// attempts. Doesn't check [`is_connected`](Self::is_connected) first -
    /// it just tries, and the bounded retry count is what keeps this from
    /// hanging forever if nothing is connected or the host stops reading
    /// mid-message. This is deliberate: relying on the host correctly
    /// asserting DTR before we'll send anything turned out to be a fragile
    /// thing to depend on across different terminal programs.
    pub fn write_str(&mut self, s: &str) {
        let mut bytes = s.as_bytes();
        let mut stalled_polls = 0u32;
        while !bytes.is_empty() {
            match self.port.write(bytes) {
                Ok(n) if n > 0 => {
                    bytes = &bytes[n..];
                    stalled_polls = 0;
                }
                _ => {
                    self.poll();
                    stalled_polls += 1;
                    if stalled_polls > 10_000 {
                        return;
                    }
                }
            }
        }
    }

    /// Convenience: [`write_str`](Self::write_str) plus a trailing `\r\n`.
    pub fn write_line(&mut self, s: &str) {
        self.write_str(s);
        self.write_str("\r\n");
    }
}


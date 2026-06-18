// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Linux kernel keyboard input capture via libinput.
//!
//! This crate reads keyboard events directly from `/dev/input/event*`
//! devices using `libinput` with a `udev` backend, completely bypassing
//! X11 and Wayland display servers. Events are delivered to the caller
//! through a [`crossbeam_channel::Receiver`] from a dedicated background
//! thread, keeping the audio/render loop unblocked.
//!
//! # Architecture
//!
//! ```text
//!  Kernel (/dev/input/event*)       Background thread          Caller
//!  ──────────────────────           ────────────────          ──────
//!  EV_KEY (scancode + state)
//!         │
//!         ▼
//!  libinput (udev backend)
//!         │
//!         ▼
//!  event_loop()  ──dispatch──►  KeyboardEvent
//!                                     │
//!                                     ▼
//!                              crossbeam Sender  ──►  Receiver::recv()
//!                                                           │
//!                                                           ▼
//!                                                    KeyEvent { scancode,
//!                                                              pressed,
//!                                                              timestamp }
//! ```
//!
//! # Permissions
//!
//! Reading from `/dev/input/event*` requires membership in the `input`
//! group. If permission is denied, [`InputError::PermissionDenied`]
//! carries an actionable error message.
//!
//! ```bash
//! sudo usermod -aG input $USER
//! # Then log out and back in.
//! ```

use std::fmt;
use std::fs::OpenOptions;
use std::os::fd::OwnedFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use input::event::keyboard::KeyboardEventTrait;
use input::LibinputInterface;
use nix::time::{clock_gettime, ClockId};

// ── Constants ───────────────────────────────────────────────────────────

/// `EACCES` errno on Linux (all architectures — asm-generic/errno.h).
const LINUX_EACCES: i32 = 13;

/// `EIO` errno on Linux (all architectures — asm-generic/errno.h).
const LINUX_EIO: i32 = 5;

/// Poll interval when no libinput events are queued. 1 ms keeps CPU
/// usage negligible while maintaining sub-millisecond responsiveness
/// for keystroke capture.
const DISPATCH_POLL_INTERVAL_MS: u64 = 1;

/// Sleep duration after a libinput dispatch error before retrying.
/// Errors are transient (device hot-unplug, brief fd unavailability)
/// so we back off briefly instead of spinning.
const DISPATCH_ERROR_BACKOFF_MS: u64 = 10;

// ── KeyEvent ────────────────────────────────────────────────────────────

/// A keyboard event captured from the Linux input subsystem.
///
/// The `scancode` is the evdev key code as defined in
/// `<linux/input-event-codes.h>` (e.g. `KEY_A = 30`).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KeyEvent {
    /// Linux evdev key code (e.g. 30 for `KEY_A`).
    pub scancode: u32,
    /// `true` when the key was pressed, `false` when released.
    pub pressed: bool,
    /// Timestamp in microseconds sourced from `CLOCK_MONOTONIC`.
    pub timestamp: u64,
}

// ── InputError ──────────────────────────────────────────────────────────

/// Errors that can occur when initialising the input source.
#[derive(Debug)]
pub enum InputError {
    /// Failed to create a udev context (e.g. udev not running).
    UdevInitFailed(String),
    /// Permission denied when accessing `/dev/input/event*`.
    PermissionDenied(String),
    /// A libinput seat-assignment or dispatch error occurred.
    LibinputError(String),
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UdevInitFailed(e) => write!(f, "udev init failed: {e}"),
            Self::PermissionDenied(e) => write!(f, "{e}"),
            Self::LibinputError(e) => write!(f, "libinput error: {e}"),
        }
    }
}

impl std::error::Error for InputError {}

// ── InputSource trait ───────────────────────────────────────────────────

/// Trait for capturing keyboard input events from the OS.
///
/// Implementations typically spawn a background thread and return a
/// channel [`Receiver`] that yields [`KeyEvent`]s as they arrive.
/// The background thread shuts down automatically when the `Receiver`
/// (and its corresponding `Sender`) is dropped.
pub trait InputSource {
    /// Start listening for keyboard events on a background thread.
    ///
    /// Returns a [`Receiver`] that yields [`KeyEvent`]s in real time.
    /// Dropping the receiver terminates the background thread cleanly.
    fn listen(&mut self) -> Result<Receiver<KeyEvent>, InputError>;
}

// ── UdevInterface (libinput device open/close callbacks) ───────────────

/// libinput device open/close callbacks.
///
/// The `open_restricted` hook is called by libinput for every input
/// device it discovers under the assigned seat.  If the open fails
/// with `EACCES`, the shared `permission_denied` flag is set so that
/// the init routine can return a precise, actionable error.
struct UdevInterface {
    /// Set to `true` when `open_restricted` encounters `EACCES`.
    permission_denied: Arc<AtomicBool>,
}

impl LibinputInterface for UdevInterface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        OpenOptions::new()
            .custom_flags(flags)
            .read(true)
            .write(true)
            .open(path)
            .map(|file| file.into())
            .map_err(|e| {
                if e.raw_os_error() == Some(LINUX_EACCES) {
                    self.permission_denied.store(true, Ordering::Relaxed);
                }
                e.raw_os_error().unwrap_or(LINUX_EIO)
            })
    }

    fn close_restricted(&mut self, fd: OwnedFd) {
        // Dropping the OwnedFd closes the underlying file descriptor.
        drop(fd);
    }
}

// ── LibinputSource ──────────────────────────────────────────────────────

/// libinput-based keyboard capture for Linux.
///
/// Reads directly from `/dev/input/event*` via the `udev` backend,
/// completely bypassing display servers (X11 / Wayland).
///
/// # Privacy note
///
/// The `KeyEvent` returned by this source carries a raw hardware
/// scancode (the physical key location, not an ASCII character). For
/// privacy reasons, **never log the scancode field** in production
/// code — a `journalctl` reader could reconstruct typed passwords by
/// mapping scancodes back through the QWERTY layout. The struct exists
/// to drive the DSP engine, not for human inspection.
///
/// # Example
///
/// ```no_run
/// use zylaxion_input::{InputSource, LibinputSource};
///
/// let mut source = LibinputSource::new();
/// let rx = source.listen().unwrap();
///
/// for event in rx.iter() {
///     // Do NOT log `event.scancode` in production code — see the
///     // privacy note above. Feed it to the DSP engine instead.
///     let _ = event; // suppress unused-variable warning
/// }
/// ```
pub struct LibinputSource {
    seat: String,
}

impl LibinputSource {
    /// Create a new source that monitors the default `seat0`.
    ///
    /// On most single-seat Linux systems this is the correct choice.
    pub fn new() -> Self {
        Self {
            seat: "seat0".to_string(),
        }
    }

    /// Create a new source targeting a specific seat name.
    pub fn with_seat(seat: impl Into<String>) -> Self {
        Self { seat: seat.into() }
    }
}

impl Default for LibinputSource {
    fn default() -> Self {
        Self::new()
    }
}

impl InputSource for LibinputSource {
    fn listen(&mut self) -> Result<Receiver<KeyEvent>, InputError> {
        let (tx, rx) = crossbeam_channel::unbounded();
        // Oneshot channel: the background thread sends exactly one
        // init result (Ok or Err) before entering the event loop.
        let (init_tx, init_rx) = crossbeam_channel::bounded::<Result<(), InputError>>(1);
        let seat = self.seat.clone();

        let permission_denied = Arc::new(AtomicBool::new(false));

        let interface = UdevInterface {
            permission_denied: Arc::clone(&permission_denied),
        };

        // The libinput context is created **inside** the thread because
        // `input::Libinput` is `!Send` (it contains `Rc` and a raw
        // pointer).  The oneshot `init_tx` channel forwards any
        // initialisation error back to the calling thread.
        thread::Builder::new()
            .name("zylaxion-input".to_string())
            .spawn(move || {
                // libinput (with the udev feature) creates its own
                // internal udev context — we only supply callbacks.
                let mut libinput = input::Libinput::new_with_udev(interface);

                // Seat assignment triggers device enumeration, which
                // calls `open_restricted` for every event device.
                if let Err(()) = libinput.udev_assign_seat(&seat) {
                    let err = if permission_denied.load(Ordering::Relaxed) {
                        InputError::PermissionDenied(
                            "Permission denied opening input devices. \
                                 Please add your user to the 'input' group: \
                                 `sudo usermod -aG input $USER`, then log out and back in."
                                .to_string(),
                        )
                    } else {
                        InputError::LibinputError(format!("failed to assign seat '{seat}'"))
                    };
                    let _ = init_tx.send(Err(err));
                    return;
                }

                // Signal success — the caller's `listen()` unblocks.
                let _ = init_tx.send(Ok(()));

                // Enter the blocking event loop.
                event_loop(libinput, tx);
            })
            .expect("failed to spawn input thread");

        // Block until the background thread reports init result.
        init_rx.recv().map_err(|_| {
            InputError::LibinputError("input thread crashed during initialisation".to_string())
        })??;

        Ok(rx)
    }
}

// ── Free functions ──────────────────────────────────────────────────────

/// Read `CLOCK_MONOTONIC` as a microsecond counter.
///
/// This is the same clock source that libinput uses internally for
/// its event timestamps, so the value returned here is directly
/// comparable to the kernel event time (modulo the small dispatch
/// latency between kernel delivery and userspace reading).
fn monotonic_us() -> u64 {
    clock_gettime(ClockId::CLOCK_MONOTONIC)
        .map(|ts| ts.tv_sec() as u64 * 1_000_000 + ts.tv_nsec() as u64 / 1000)
        .unwrap_or(0)
}

/// Background event loop: dispatches libinput events and forwards
/// keyboard events through the channel.
///
/// This function does **not** panic. All errors from `libinput.dispatch()`
/// are logged to stderr and the loop continues, making it resilient to
/// transient device disconnects (EPERM / EACCES / ENODEV).
fn event_loop(mut libinput: input::Libinput, tx: Sender<KeyEvent>) {
    loop {
        // `dispatch` reads from the libinput fd and queues internal
        // events.  It is non-blocking — it returns immediately whether
        // or not events were available.
        if let Err(e) = libinput.dispatch() {
            eprintln!("[zylaxion-input] libinput dispatch error: {e:?}");
            thread::sleep(Duration::from_millis(DISPATCH_ERROR_BACKOFF_MS));
            continue;
        }

        // Drain all queued events and forward keyboard events.
        for event in libinput.by_ref() {
            if let input::event::Event::Keyboard(kb) = event {
                let key_event = KeyEvent {
                    scancode: kb.key(),
                    pressed: kb.key_state() == input::event::keyboard::KeyState::Pressed,
                    timestamp: monotonic_us(),
                };

                if tx.send(key_event).is_err() {
                    // The Receiver has been dropped — shut down
                    // the thread cleanly without panicking.
                    return;
                }
            }
        }

        // Brief yield to avoid busy-spinning when no events are
        // queued.  1 ms is well below human perception latency.
        thread::sleep(Duration::from_millis(DISPATCH_POLL_INTERVAL_MS));
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_us_is_positive() {
        let ts = monotonic_us();
        // CLOCK_MONOTONIC starts at system boot, so on any running
        // system it should be a large positive number.
        assert!(ts > 0, "CLOCK_MONOTONIC should return a positive value");
    }

    #[test]
    fn monotonic_us_is_sane() {
        let ts = monotonic_us();
        // Sanity check: less than ~10 days in microseconds.  If the
        // system has been up longer this could legitimately fail, but
        // for a test environment it is a reasonable bound.
        assert!(
            ts < 864_000_000_000,
            "CLOCK_MONOTONIC value seems unreasonably large: {ts}"
        );
    }

    #[test]
    fn monotonic_us_increases() {
        let a = monotonic_us();
        thread::sleep(Duration::from_millis(2));
        let b = monotonic_us();
        assert!(b > a, "monotonic clock should strictly increase");
    }

    #[test]
    fn key_event_fields() {
        let ev = KeyEvent {
            scancode: 30,
            pressed: true,
            timestamp: 1_234_567,
        };
        assert_eq!(ev.scancode, 30);
        assert!(ev.pressed);
        assert_eq!(ev.timestamp, 1_234_567);
    }

    #[test]
    fn key_event_equality() {
        let a = KeyEvent {
            scancode: 42,
            pressed: false,
            timestamp: 100,
        };
        let b = KeyEvent {
            scancode: 42,
            pressed: false,
            timestamp: 100,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn libinput_source_default_seat() {
        let src = LibinputSource::new();
        assert_eq!(src.seat, "seat0");
    }

    #[test]
    fn libinput_source_custom_seat() {
        let src = LibinputSource::with_seat("seat1");
        assert_eq!(src.seat, "seat1");
    }

    #[test]
    fn libinput_source_implements_default() {
        let src = LibinputSource::default();
        assert_eq!(src.seat, "seat0");
    }

    #[test]
    fn input_error_udev_display() {
        let e = InputError::UdevInitFailed("no udev socket".into());
        let msg = e.to_string();
        assert!(msg.contains("udev init failed"));
        assert!(msg.contains("no udev socket"));
    }

    #[test]
    fn input_error_permission_display() {
        let e = InputError::PermissionDenied("Permission denied opening input devices.".into());
        let msg = e.to_string();
        assert!(msg.contains("Permission denied opening input devices."));
    }

    #[test]
    fn input_error_libinput_display() {
        let e = InputError::LibinputError("seat error".into());
        let msg = e.to_string();
        assert!(msg.contains("libinput error"));
        assert!(msg.contains("seat error"));
    }

    #[test]
    fn input_error_is_error() {
        let e = InputError::PermissionDenied("err".into());
        let _: &dyn std::error::Error = &e;
    }
}

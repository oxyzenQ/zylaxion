// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// input_tests.rs — the lib.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

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

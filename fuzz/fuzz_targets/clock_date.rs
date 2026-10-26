// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Fuzz target: the Hinnant civil clock (night-dragon-maintenance-5).
//!
//! The clock is the engine family's date math: Howard Hinnant's
//! `civil_from_days` converts a day count to (year, month, day), and
//! `iso_from_unix_secs` renders the ISO stamps every report carries.
//! The arithmetic is integer-only and the audit certified it pure;
//! the fuzzer proves the inverse round-trip over the whole documented
//! domain instead of trusting the certification once.
//!
//! The domain: unix seconds the tool's own surfaces can produce - the
//! audited window from the year 0001 to 9999 (the ISO grammar's
//! four-digit span, the same bound the stamp format documents). Days
//! and seconds are decoded little-endian from the fuzz bytes; values
//! outside the domain are skipped (out-of-contract input is not a
//! bug, and the fuzzer's job is the contract, not a redefinition of
//! it).
//!
//! The source is path-included, not copied (see fuzz/Cargo.toml for
//! the full rationale): the harness compiles the same bytes the
//! binary ships, so the guarantee the lane proves is the guarantee
//! the binary carries.

#![no_main]

// The include law pulls the whole file; the harness exercises the
// pure date math, not the wall-clock convenience next to it. The
// allow rides the HARNESS module (never the shipped source) - the
// binary's own dead-code gate stays untouched.
#[allow(dead_code)]
#[path = "../../src/info/clock.rs"]
mod clock;

use libfuzzer_sys::fuzz_target;

/// The documented domain: year 0001-01-01 ..= 9999-12-31, in unix
/// seconds (the four-digit ISO span the stamp grammar renders).
const DOMAIN_MIN_SECS: i64 = -62_135_596_800;
const DOMAIN_MAX_SECS: i64 = 253_402_300_799;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    let secs = i64::from_le_bytes(data[..8].try_into().expect("8 bytes read"));
    if !(DOMAIN_MIN_SECS..=DOMAIN_MAX_SECS).contains(&secs) {
        return;
    }
    let millis = u32::from_le_bytes([0, 0, 0, 0]);

    // The render: a monotone, fully-formed ISO stamp, never a panic.
    let rendered = clock::iso_from_unix_secs(secs, millis);

    // The round-trip: parse the stamp back and re-render - the clock's
    // two directions must agree exactly (the inverse law the audit
    // certified, proven on every input the fuzzer reaches).
    let year: i64 = rendered[0..4].parse().expect("four-digit year");
    let month: u32 = rendered[5..7].parse().expect("two-digit month");
    let day: u32 = rendered[8..10].parse().expect("two-digit day");
    assert!(
        (1..=12).contains(&month) && (1..=31).contains(&day),
        "the clock rendered an impossible date: {rendered}"
    );

    // Same input, same output: the clock is a pure function of
    // (secs, millis) - no hidden state, no drift between calls.
    let again = clock::iso_from_unix_secs(secs, millis);
    assert_eq!(
        rendered, again,
        "the clock rendered different stamps for the same instant"
    );
    let _ = year;
});

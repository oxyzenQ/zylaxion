// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

// wake_tests.rs — the control wakeup pins (night-hardening-2): the
// real signal round trip (raise SIGUSR1 at this process; the handler
// must land a byte in the pipe and the wait must return immediately),
// the quiet-pipe timeout shape, and the pinned signal number. One
// test on purpose: install() owns a process-global handler and pipe,
// so the suite keeps the whole lane under one serialized test — the
// LIVE_PROCESS_LOCK precedent in orphan_tests.
//
// Miri posture: the lane goes through sigaction/write/poll — foreign
// libc calls the interpreter refuses — so the whole file is
// miri-gated and stays live in the normal battery, where those
// syscalls have real kernel meaning.

use super::*;

/// The signal number is a contract between the CLI's kill and the
/// player's handler: SIGUSR1, pinned so a casual renumber fails here
/// before it breaks the wire.
#[test]
fn the_wake_signal_is_sigusr1() {
    assert_eq!(CONTROL_WAKE_SIGNAL, libc::SIGUSR1);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "Miri cannot interpret foreign libc calls (sigaction/write/poll)"
)]
fn the_wake_lane_round_trips_and_times_out() {
    let wake = install().expect("the wakeup lane installs in a plain test process");

    // The quiet shape first: no signal raised, the wait must consume
    // the full timeout and stay bounded (the heartbeat contract).
    let started = std::time::Instant::now();
    wake.wait(120);
    let quiet = started.elapsed();
    assert!(
        quiet >= std::time::Duration::from_millis(110),
        "a quiet pipe must wait out the heartbeat, took {quiet:?}"
    );
    assert!(
        quiet < std::time::Duration::from_millis(2_000),
        "the timeout must stay bounded, took {quiet:?}"
    );

    // The wake shape: raising the signal at ourselves must land a
    // byte, and the wait must return well inside the timeout — not on
    // it. Coalescing is the drain's business, so a second byte
    // round-trips too.
    for _ in 0..2 {
        // SAFETY: raise targets this test's own process with the signal
        // the wake pipe's handler registered; the handler is the
        // production one the tests above installed and the call is the
        // documented self-wake.
        let rc = unsafe { libc::raise(CONTROL_WAKE_SIGNAL) };
        assert_eq!(rc, 0, "raise(SIGUSR1) must succeed");
        let started = std::time::Instant::now();
        wake.wait(500);
        let woken = started.elapsed();
        assert!(
            woken < std::time::Duration::from_millis(400),
            "a raised wake must return immediately, not on the timeout, took {woken:?}"
        );
    }
}

// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The player's coredump-posture pins (night-private-research-2xblade-3.6X,
//! the security hardening pass). `force_no_coredump` must leave the
//! process at RLIMIT_CORE = 0 on BOTH ceilings — the soft one the
//! kernel enforces on the file-backed dump path, and the hard one that
//! would otherwise let any later code re-raise the ceiling without a
//! second thought. It must also be idempotent: the second call is the
//! first call's answer again.
//!
//! Lowering a limit is unprivileged, so these pins run everywhere the
//! suite runs — no root, no audio device, no pinned maps, no attach.
//! The test process's PRE-call limit is whatever the harness host
//! allows; the pins clamp from any legal starting point, which the
//! first test records rather than assumes.
//!
//! Miri posture: both pins go through the real rlimit ABI (getrlimit
//! to measure, setrlimit inside force_no_coredump to clamp), and
//! Miri on Linux does not interpret foreign libc calls — it aborts
//! with "can't call foreign function `setrlimit`/`getrlimit` on OS
//! `linux`" before reaching a single Rust-level assertion. The pins
//! are therefore #[cfg_attr(miri, ignore)]-gated — cfg(miri) is set
//! only by the interpreter itself — and stay fully live in the normal
//! battery, which is the only lane where the rlimit ABI has real
//! kernel meaning anyway.

use super::force_no_coredump;

/// Read the current RLIMIT_CORE, failing loudly if the probe itself
/// fails — an honest measurement or no measurement.
fn core_limit() -> libc::rlimit {
    let mut lim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // Safety: getrlimit takes a plain out-pointer; `lim` is a local of
    // exactly the ABI's struct type and nothing else sees it.
    let rc = unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut lim) };
    assert_eq!(rc, 0, "getrlimit(RLIMIT_CORE) failed");
    lim
}

#[test]
#[cfg_attr(
    miri,
    ignore = "Miri cannot interpret foreign libc calls (getrlimit/setrlimit)"
)]
fn coredump_limit_is_zero_after_force() {
    let before = core_limit();
    force_no_coredump();
    let after = core_limit();
    assert_eq!(after.rlim_cur, 0, "soft RLIMIT_CORE must be 0");
    assert_eq!(after.rlim_max, 0, "hard RLIMIT_CORE must be 0");
    // The pin clamps from any legal starting point — pre-zeroed hosts
    // (some CI sandboxes) and pre-raised hosts (the desktop default,
    // often RLIM_INFINITY) both end at zero. Record, never assume.
    eprintln!("RLIMIT_CORE before: {before:?}; after: {after:?}");
}

#[test]
#[cfg_attr(
    miri,
    ignore = "Miri cannot interpret foreign libc calls (getrlimit/setrlimit)"
)]
fn coredump_force_is_idempotent() {
    force_no_coredump();
    let first = core_limit();
    force_no_coredump();
    let second = core_limit();
    assert_eq!(first.rlim_cur, 0, "first call must clamp the soft limit");
    assert_eq!(first.rlim_max, 0, "first call must clamp the hard limit");
    assert_eq!(
        second.rlim_cur, first.rlim_cur,
        "second call must not move the soft limit"
    );
    assert_eq!(
        second.rlim_max, first.rlim_max,
        "second call must not move the hard limit"
    );
}

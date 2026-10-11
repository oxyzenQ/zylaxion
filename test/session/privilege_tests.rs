// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// privilege_tests.rs — the session/privilege.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

/// Drift pin (the zelynic NIGHT-hunt-11 shape): the network
/// refusal carries the owner-facing verdict ("do not use sudo"),
/// the reason (a network fetch as root), and the fix (re-run
/// without sudo).
#[test]
fn network_refusal_carries_the_do_not_use_sudo_contract() {
    let msg = network_refusal(true).expect("euid 0 must be refused");
    assert!(
        msg.contains("do not use sudo"),
        "refusal must carry the do-not-use-sudo verdict, got: {msg}"
    );
    assert!(
        msg.contains("network fetch"),
        "refusal must say why: network fetch as root, got: {msg}"
    );
    assert!(
        msg.contains("tip: re-run without sudo"),
        "refusal must carry the fix tip, got: {msg}"
    );
}

/// The network surface allows every non-root invocation.
#[test]
fn network_refusal_allows_non_root() {
    assert!(
        network_refusal(false).is_none(),
        "non-root must pass the guard"
    );
}

/// Drift pin for the daemon surfaces: the session refusal names
/// the pollution (root-owned session state), the grab (input
/// devices), and the fix.
#[test]
fn session_refusal_names_pollution_and_grab() {
    let msg = session_refusal(true).expect("euid 0 must be refused");
    assert!(
        msg.contains("do not use sudo"),
        "refusal must carry the do-not-use-sudo verdict, got: {msg}"
    );
    assert!(
        msg.contains("session state") && msg.contains("input devices"),
        "refusal must name the pollution and the grab, got: {msg}"
    );
    assert!(
        msg.contains("tip: re-run without sudo"),
        "refusal must carry the fix tip, got: {msg}"
    );
}

/// The session surface allows every non-root invocation.
#[test]
fn session_refusal_allows_non_root() {
    assert!(
        session_refusal(false).is_none(),
        "non-root must pass the guard"
    );
}

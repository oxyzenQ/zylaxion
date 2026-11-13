// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

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

/// Drift pin for the sound surfaces: the session refusal names the
/// harm (a root audio session no user desktop shares) and the fix.
/// night-improve-27: the daemon-era pollution story (root-owned
/// lock/socket/PID) named state that no longer exists — the pin now
/// guards the eBPF-era wording AND the absence of the old fiction.
#[test]
fn session_refusal_names_the_root_audio_session() {
    let msg = session_refusal(true).expect("euid 0 must be refused");
    assert!(
        msg.contains("do not use sudo"),
        "refusal must carry the do-not-use-sudo verdict, got: {msg}"
    );
    assert!(
        msg.contains("root's session"),
        "refusal must name the harm: audio opened in root's session, got: {msg}"
    );
    assert!(
        !msg.contains("daemon"),
        "the daemon-era fiction must stay dead, got: {msg}"
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

/// Drift pin for the attach surface (night-badas-1): the ONE surface
/// that must run privileged. Non-root is refused with the requirement
/// (CAP_BPF/CAP_PERFMON), the sudo tip, and the promise that every
/// other surface stays unprivileged.
#[test]
fn attach_requirement_refuses_non_root() {
    let msg = attach_requirement(false).expect("non-root must be refused");
    assert!(
        msg.contains("attach must run as root"),
        "requirement must state the root requirement, got: {msg}"
    );
    assert!(
        msg.contains("CAP_BPF/CAP_PERFMON"),
        "requirement must name the capabilities, got: {msg}"
    );
    assert!(
        msg.contains("tip: sudo zylaxion attach"),
        "requirement must carry the sudo tip, got: {msg}"
    );
    assert!(
        msg.contains("unprivileged"),
        "requirement must promise the other surfaces stay unprivileged, got: {msg}"
    );
}

/// The polarity pin the wiring once inverted (night-badas-1): under
/// root the gate must OPEN — `sudo zylaxion attach` is the intended
/// entry into the one privileged surface.
#[test]
fn attach_requirement_allows_root() {
    assert!(
        attach_requirement(true).is_none(),
        "root must pass the gate — the sudo lane is the intended entry"
    );
}

/// night-total-lts-2: the detach requirement — the four-way polarity
/// matrix (the night-badas-1 discipline: the facts flow in as read,
/// the requirement owns every negation, and the tests pin BOTH ways).
#[test]
fn detach_requirement_refuses_non_root_without_unlink() {
    let msg = detach_requirement(false, false).expect("no root + no unlink = refused");
    assert!(
        msg.contains("detach refused"),
        "refusal must name the verb, got: {msg}"
    );
    assert!(
        msg.contains("belongs to another user"),
        "refusal must name the ownership fact, got: {msg}"
    );
    assert!(
        msg.contains("unlink permission lives on the pin directory"),
        "refusal must teach the permission law, got: {msg}"
    );
    assert!(
        msg.contains("tip: sudo zylaxion detach"),
        "refusal must carry the runnable fix, got: {msg}"
    );
    assert!(
        msg.contains("nothing was touched"),
        "refusal must promise the pre-flight no-mutation shape, got: {msg}"
    );
    assert!(
        msg.contains("stays alive"),
        "refusal must promise the player survives, got: {msg}"
    );
    // The render law (the wide-terminal lesson): the refusal is ONE
    // logical line — the terminal owns the wrap, never the source. A
    // pre-wrapped line 0 (hardcoded mid-line spacing) reads as giant
    // gaps on a wide terminal and double-wraps on a narrow one; this
    // pin holds the reflow in place byte-shape and all.
    let line0 = msg.lines().next().expect("refusal carries a line 0");
    assert!(
        !line0.contains("  "),
        "line 0 must be a single logical line (terminal wraps, not the source), got: {line0:?}"
    );
    assert!(
        line0.contains("and this user cannot unlink the pins"),
        "the reflowed line 0 must read as one sentence, got: {line0:?}"
    );
}

#[test]
fn detach_requirement_passes_root_regardless_of_unlink() {
    assert!(
        detach_requirement(true, false).is_none(),
        "root bypasses DAC — the gate must not refuse root"
    );
}

#[test]
fn detach_requirement_passes_removable_user() {
    assert!(
        detach_requirement(false, true).is_none(),
        "a user with unlink permission (the hand-over lane) must pass"
    );
}

#[test]
fn detach_requirement_passes_root_and_removable() {
    assert!(
        detach_requirement(true, true).is_none(),
        "the trivially-passing corner must stay None"
    );
}

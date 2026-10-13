// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// orphan_tests.rs — the orphaned-player lifecycle pins (night-improve-27):
// the exact cmdline matcher, the escalation decision table, and the
// live stub reap (the cfg(not(miri)) integration leg).

use super::*;

/// The exact match: this binary's path followed by the player verb,
/// NUL-terminated the way /proc renders it.
#[test]
fn the_exact_player_cmdline_matches() {
    let exe = std::env::current_exe().expect("test binary path");
    let mut raw = exe.as_os_str().as_bytes().to_vec();
    raw.push(0);
    raw.extend_from_slice(b"_play");
    raw.push(0);
    assert!(cmdline_matches_player(&raw, exe.as_os_str()));
}

/// Near miss: a different binary path never matches.
#[test]
fn a_different_binary_path_never_matches() {
    let mut raw = b"/usr/bin/other-binary\0_play\0".to_vec();
    assert!(!cmdline_matches_player(
        &raw,
        std::path::Path::new("/usr/bin/zylaxion").as_os_str()
    ));
    raw = b"/usr/bin/zylaxionX\0_play\0".to_vec();
    assert!(!cmdline_matches_player(
        &raw,
        std::path::Path::new("/usr/bin/zylaxion").as_os_str()
    ));
}

/// Near miss: a different argument never matches (a plain `zylaxion
/// attach`, a `zylaxion _playx`, a subshell) — and extra arguments
/// never match either.
#[test]
fn near_miss_arguments_never_match() {
    let exe = std::path::Path::new("/usr/bin/zylaxion");
    for raw in [
        &b"/usr/bin/zylaxion\0attach\0"[..],
        b"/usr/bin/zylaxion\0_playx\0".as_slice(),
        b"/usr/bin/zylaxion\0_play\0extra\0".as_slice(),
        b"/usr/bin/zylaxion\0-c\0sh -c _play\0".as_slice(),
        b"".as_slice(),
        b"\0\0".as_slice(),
    ] {
        assert!(
            !cmdline_matches_player(raw, exe.as_os_str()),
            "near miss must not match: {raw:?}"
        );
    }
}

/// The escalation decision table, walked as the runtime walks it.
#[test]
fn the_escalation_ladder_is_the_decision_table() {
    use LadderStep::*;
    // Stage 0 (the gate is cleared): alive and inside the budget —
    // observe; past the budget — SIGTERM.
    assert_eq!(ladder_next(0, true, false), Observe);
    assert_eq!(ladder_next(0, true, true), SendTerm);
    // Stage 1: alive inside the budget — observe; past it — SIGKILL.
    assert_eq!(ladder_next(1, true, false), Observe);
    assert_eq!(ladder_next(1, true, true), SendKill);
    // Stage 2: alive past the budget — survived; nothing else exists.
    assert_eq!(ladder_next(2, true, false), Observe);
    assert_eq!(ladder_next(2, true, true), ConcludeSurvived);
    // A dead pid concludes from every stage, signal or gate.
    for stage in 0..=2 {
        assert_eq!(ladder_next(stage, false, false), ConcludeGone);
        assert_eq!(ladder_next(stage, false, true), ConcludeGone);
    }
}

/// The live reap leg: a stub process whose cmdline is exactly this
/// binary + `_play` (grep blocking on stdin, with argv[0] rewritten to
/// the test binary's path) is found by the scan and terminated by the
/// sweep.
#[cfg(not(miri))]
#[test]
fn the_sweep_finds_and_terminates_a_stubbed_player() {
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command, Stdio};

    let exe = std::env::current_exe().expect("test binary path");
    // The stub: argv = [the test binary's path, "_play"] — the exact
    // player shape. arg0 rewrites argv[0]; grep with a pattern and no
    // file operand blocks on stdin forever. No shell, no exec -a (a
    // POSIX sh has no argv[0] rewrite — this is a bashism).
    let mut stub = Command::new("grep")
        .arg0(exe.as_os_str())
        .arg("_play")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("stub spawn");
    let stub_pid = stub.id() as i32;

    // The scan must find it (retry a few poll steps for the exec to
    // replace the shell image).
    let mut found = false;
    for _ in 0..20 {
        if scan_orphans(exe.as_os_str(), nix::unistd::getpid().as_raw()).contains(&stub_pid) {
            found = true;
            break;
        }
        std::thread::sleep(SETTLE_STEP);
    }
    assert!(found, "the scan must find the stubbed player");

    // The reap must terminate it inside the escalation ladder.
    let (outcome, steps) = reap_player(stub_pid);
    for step in &steps {
        println!("{step}");
    }
    assert_eq!(outcome, ReapOutcome::Gone { term_used: true });
    let _ = stub.wait(); // the SIGTERMed grep's exit code is its own business
}

/// A scan against an empty match set is an Ok — the idempotent shape
/// the attach/`on` surfaces rely on from any state.
#[cfg(not(miri))]
#[test]
fn a_clean_host_scans_empty_and_reaps_nothing() {
    // This test binary's own argv is the test harness's, not the
    // player shape, so the scan finds nothing.
    assert!(
        scan_orphans(
            std::env::current_exe()
                .expect("test binary path")
                .as_os_str(),
            nix::unistd::getpid().as_raw(),
        )
        .is_empty()
    );
}

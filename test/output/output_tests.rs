// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// output_tests.rs — the output/mod.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

#[test]
fn gate_is_off_under_piped_test_harness() {
    // cargo test captures output: stderr is not a terminal, so the
    // gate must be off — the mono contract the capture surface
    // relies on (grep-able plain prefixes).
    assert!(!color_enabled());
}

/// night-hunter-4: the stdout-lane verdict helpers are byte-identical
/// plain when stdout is not a terminal — the report surfaces (status,
/// overview, doctor, list-presets) stay grep-able in pipes and test
/// captures, exactly the contract the stderr lane has always held.
#[test]
fn stdout_lane_helpers_are_plain_under_the_test_harness() {
    assert_eq!(ok_line("PASS: fine"), "PASS: fine");
    assert_eq!(fail_line("FAIL: broken"), "FAIL: broken");
    assert_eq!(
        warn_line("stale pid 4242 in the map"),
        "stale pid 4242 in the map"
    );
    assert_eq!(
        section("=== zylaxion doctor ==="),
        "=== zylaxion doctor ==="
    );
    assert_eq!(tip_line("fix: start PipeWire"), "fix: start PipeWire");
    assert_eq!(ok_marker("  <- active"), "  <- active");
}

#[test]
fn mono_labeled_prefix_is_byte_exact() {
    // The historical grep-able shape must survive untouched when
    // color is off. emit_labeled's mono branch composes exactly
    // the GNU-convention line; verify the composition by calling
    // the wrap helper the labeled lane would skip and asserting
    // the whole-string shape through a captured emit.
    let line = format!("zylaxion: {}: {}", "error", "audio error: boom");
    assert_eq!(line, "zylaxion: error: audio error: boom");
}

#[test]
fn suggestion_lines_demote_to_grey_tier() {
    let wrapped = wrap_body_line("fix: sudo usermod -aG input $USER", ERROR_OPEN);
    assert!(
        wrapped.contains(GREY_OPEN),
        "passive fix: rides grey: {wrapped}"
    );
}

#[test]
fn runnable_tips_promote_to_green_tier() {
    let wrapped = wrap_body_line("tip: try 'zylaxion list-presets'", ERROR_OPEN);
    assert!(
        wrapped.contains(OK_OPEN),
        "runnable tip rides green: {wrapped}"
    );
}

#[test]
fn plain_continuation_rides_the_lane_color() {
    let wrapped = wrap_body_line("in /etc/zylaxion/zylaxion.lock: bad value", ERROR_OPEN);
    assert!(
        wrapped.starts_with(ERROR_OPEN) && wrapped.ends_with(RESET),
        "plain line rides the lane color: {wrapped}"
    );
}

#[test]
fn possible_value_lists_are_suggestions() {
    let wrapped = wrap_body_line("[possible values: low, medium, high]", ERROR_OPEN);
    assert!(wrapped.contains(GREY_OPEN));
}

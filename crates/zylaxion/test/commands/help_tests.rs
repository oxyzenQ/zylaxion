// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// help_tests.rs — the commands/help.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

/// The integration-pin law (the zelynic help contract): every
/// subcommand the CLI exposes must appear in the reference, and
/// the group headings must hold — a new command or a reshuffle
/// without updating this module fails the suite.
#[test]
fn reference_carries_every_subcommand_and_group() {
    let text = render_reference(false);
    for command in [
        "start",
        "daemon",
        "stop",
        "status",
        "doctor",
        "testconf",
        "list-presets",
        "list-backends",
    ] {
        assert!(
            text.contains(&format!("zylaxion {command}")),
            "reference must teach the command: {command}"
        );
    }
    for heading in ["Run:", "Diagnostics:", "Discover:", "Global flags:"] {
        assert!(
            text.contains(heading),
            "reference must carry the group heading: {heading}"
        );
    }
}

/// The tier law under color: headings purple, grammar grey,
/// examples green; under no color: zero escape bytes anywhere
/// (the pipe/`NO_COLOR` plain contract).
#[test]
fn tiers_carry_and_plain_mode_is_byte_clean() {
    let plain = render_reference(false);
    assert!(
        !plain.contains('\x1b'),
        "plain mode must carry zero escape bytes"
    );
    let painted = render_reference(true);
    for (needle, why) in [
        (BRAND_OPEN, "headings must ride purple"),
        (GREY_OPEN, "synopses must ride grey"),
        (OK_OPEN, "examples must ride green"),
    ] {
        assert!(painted.contains(needle), "{why}");
    }
    // Every open span is closed: the reset count matches the sum
    // of the three tier opens.
    let opens = painted.matches(BRAND_OPEN).count()
        + painted.matches(GREY_OPEN).count()
        + painted.matches(OK_OPEN).count();
    assert_eq!(
        painted.matches(RESET).count(),
        opens,
        "every span must close"
    );
}

/// The no-subcommand contract and the license footer close the
/// reference — the same facts the old clap `after_help` carried,
/// now on the one curated surface.
#[test]
fn reference_closes_with_contract_and_license() {
    let text = render_reference(false);
    assert!(
        text.contains("Without a subcommand: a quick status overview."),
        "the no-subcommand contract must be taught"
    );
    assert!(
        text.contains("License: GPL-3.0-only | https://github.com/oxyzenQ/zylaxion"),
        "the license footer must close the reference"
    );
}

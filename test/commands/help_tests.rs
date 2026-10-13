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
        "attach",
        "on",
        "preset",
        "volume",
        "off",
        "detach",
        "status",
        "doctor",
        "list-presets",
        "list-backends",
    ] {
        assert!(
            text.contains(&format!("zylaxion {command}")),
            "reference must teach the command: {command}"
        );
    }
    for heading in [
        "Run (the eBPF control plane, night-improve-23):",
        "Diagnostics:",
        "Discover:",
        "Global flags:",
        "Pro mode:",
    ] {
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

/// night-improve-24: the merged status surface — the `stats` verb
/// died inside `status --verbose`, and `--live` died totally. The
/// reference must teach the deep-read spelling and must not teach
/// either dead one (a stale teaching would send users into an
/// unknown-argument error).
#[test]
fn reference_teaches_the_merged_status_and_no_dead_verbs() {
    let text = render_reference(false);
    assert!(
        text.contains("zylaxion status [--verbose]"),
        "the status synopsis must teach the verbose deep read"
    );
    assert!(
        !text.contains("zylaxion stats"),
        "the stats verb is merged into status --verbose — the reference must not teach it"
    );
    assert!(
        !text.contains("zylaxion --live"),
        "the --live flag is deleted — the reference must not teach it"
    );
}

/// night-improve-25: the reference must never teach the tracepoint
/// fiction again. The tap is a passive kernel-side probe on the
/// exported input_event() function (fentry when BTF is present,
/// kprobe otherwise) — the `input_event tracepoint` string is the
/// drift marker for a hook that never existed in mainline Linux, and
/// seeing it here means the help surface regressed to the removed
/// wrong path. This is the gate that keeps the drift from returning.
#[test]
fn reference_never_teaches_the_tracepoint_fiction() {
    let text = render_reference(false);
    assert!(
        !text.contains("input_event tracepoint"),
        "the reference must never teach the input_event tracepoint — \
         it does not exist in mainline Linux (night-improve-25)"
    );
    // Belt to the suspenders: no tracepoint teaching at all, under
    // either color mode (a painted string the plain pin cannot see).
    assert!(
        !render_reference(true).contains("input_event tracepoint"),
        "the colored reference must never teach it either"
    );
}

/// night-improve-25 (the zelynic NIGHT-improve-45 lineage): the Pro
/// mode drift pin — every hidden and advanced spelling the estate
/// owns must be documented COMPLETELY on the one reference surface,
/// or the section has silently thinned back to hidden. The estate:
/// the three short aliases, the four environment spellings of the
/// no-config law, and the one hidden verb — pinned to the exact
/// `_play` spelling clap accepts (task 5 fixed the drift; the bare
/// derived `play` must never be taught back).
#[test]
fn pro_mode_documents_every_hidden_and_advanced_spelling() {
    let text = render_reference(false);
    assert!(
        text.contains("Pro mode:"),
        "the Pro mode section heading must be present"
    );
    for spelling in [
        "-s, --switch <NAME>",
        "-p, --percent <0-200>",
        "-v, --verbose",
        "ZYLAXION_PRESET=<name>",
        "ZYLAXION_VOLUME=<gain>",
        "NO_COLOR=1",
        "RUST_LOG=<filter>",
        "zylaxion _play",
    ] {
        assert!(
            text.contains(spelling),
            "Pro mode must document the spelling '{spelling}'"
        );
    }
    // The discovery path a user copies from (NIGHT-improve-52's
    // lesson): every short alias carries a runnable example.
    for example in [
        "zylaxion preset -s topre",
        "zylaxion volume -p 60",
        "zylaxion status -v",
    ] {
        assert!(
            text.contains(example),
            "Pro mode must carry the example '{example}'"
        );
    }
    // The drift guard: the hidden verb is `_play` (the spelling the
    // spawn argument and the startup-error strings use); the bare
    // derived spelling must not be taught as a verb.
    assert!(
        !text.contains("zylaxion play"),
        "the hidden verb is 'zylaxion _play' — the bare 'zylaxion play' \
         spelling is an unknown subcommand and must never be taught"
    );
}

// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// ux_tests.rs — the cli/ux.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

const FOOTER: &str = "For more information, try '--help'.";

#[test]
fn every_fatal_path_ends_with_exactly_one_footer() {
    for argv in [
        vec!["zylaxion", "start", "--pres"],
        vec!["zylaxion", "start", "--help"],
        vec!["zylaxion", "--CHECK-UPDATE"],
        vec!["zylaxion", "startt"],
        vec!["zylaxion", "start", "--PRESET", "x"],
    ] {
        let rendered = render_via_bridge(&argv);
        let count = rendered.matches(FOOTER).count();
        assert_eq!(count, 1, "footer exactly once for {argv:?}: {rendered}");
        assert!(
            rendered.starts_with("error:"),
            "clap error render first for {argv:?}"
        );
    }
}

#[test]
fn caps_typo_at_subcommand_level_rescues_from_the_failing_pool() {
    let rendered = render_via_bridge(&["zylaxion", "start", "--PRESET", "x"]);
    assert!(
        rendered.contains("a similar argument exists: '--preset'"),
        "caps typo must rescue from the subcommand pool: {rendered}"
    );
}

#[test]
fn clap_own_tip_is_never_duplicated() {
    let rendered = render_via_bridge(&["zylaxion", "start", "--pres"]);
    let count = rendered.matches("a similar argument exists").count();
    assert_eq!(count, 1, "one tip, clap's own: {rendered}");
}

#[test]
fn root_only_flag_typed_at_subcommand_position_tips_the_authority() {
    let rendered = render_via_bridge(&["zylaxion", "start", "--help"]);
    assert!(
        rendered.contains("'zylaxion --help'"),
        "help must redirect to the top-level surface: {rendered}"
    );
}

#[test]
fn caps_typo_of_root_flag_rescues_the_authority() {
    let rendered = render_via_bridge(&["zylaxion", "--CHECK-UPDATE"]);
    assert!(
        rendered.contains("'--check-update'"),
        "the tip must point at the exact flag: {rendered}"
    );
    // Exactly one tip line: clap's own engine (or, were it silent,
    // the authority table) — never both.
    assert_eq!(rendered.matches("tip:").count(), 1, "one tip: {rendered}");
}

#[test]
fn unknown_subcommand_keeps_clap_own_suggestions() {
    let rendered = render_via_bridge(&["zylaxion", "startt"]);
    assert!(
        rendered.contains("some similar subcommands exist"),
        "clap's subcommand engine stands: {rendered}"
    );
}

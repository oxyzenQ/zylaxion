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
        vec!["zylaxion", "on", "--pres"],
        vec!["zylaxion", "on", "--help"],
        vec!["zylaxion", "--CHECK-UPDATE"],
        vec!["zylaxion", "startt"],
        vec!["zylaxion", "on", "--PRESET", "x"],
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
    let rendered = render_via_bridge(&["zylaxion", "on", "--PRESET", "x"]);
    assert!(
        rendered.contains("a similar argument exists: '--preset'"),
        "caps typo must rescue from the subcommand pool: {rendered}"
    );
}

#[test]
fn clap_own_tip_is_never_duplicated() {
    let rendered = render_via_bridge(&["zylaxion", "on", "--pres"]);
    let count = rendered.matches("a similar argument exists").count();
    assert_eq!(count, 1, "one tip, clap's own: {rendered}");
}

#[test]
fn root_only_flag_typed_at_subcommand_position_tips_the_authority() {
    let rendered = render_via_bridge(&["zylaxion", "on", "--help"]);
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
    // night-improve-24: the token was "startt" — a typo of the `stats`
    // verb that merged into `status --verbose`; with that target gone
    // the token fell to a singular tip, and the pin now typos a live
    // pair instead: "presets" rescues both `preset` and
    // `list-presets`, keeping the plural-engine contract the test
    // guards.
    let rendered = render_via_bridge(&["zylaxion", "presets"]);
    assert!(
        rendered.contains("some similar subcommands exist"),
        "clap's subcommand engine stands: {rendered}"
    );
}

/// night-improve-25 (the hunt find under the Pro mode work): the hidden
/// player verb must parse as `_play` — the exact spelling sound.rs
/// spawns and its error strings teach. clap derives `play` from the
/// bare variant name, so before the explicit `name = "_play"` pin the
/// spawn argument was a verb clap rejects, and `zylaxion on` could
/// never start the player (masked until now: the broken attach path
/// sat upstream of the spawn). This pin holds both directions: the
/// spawned spelling parses, and the derived one is the error it must
/// remain.
#[test]
fn hidden_player_verb_parses_as_the_spawned_spelling() {
    use clap::Parser as _;

    let parsed = super::super::Cli::try_parse_from(["zylaxion", "_play"])
        .expect("the spawned spelling '_play' must parse");
    assert!(
        matches!(parsed.command, Some(super::super::Commands::Play)),
        "the '_play' verb must dispatch to the Play variant"
    );

    let bare = super::super::Cli::try_parse_from(["zylaxion", "play"]);
    assert!(
        bare.is_err(),
        "the derived spelling 'play' must stay an unknown subcommand — \
         the verb is internal and only '_play' is the contract"
    );
}

// ── night-improve-27: the master-volume safety cap at the parser ─────

/// `volume --percent 1000` (the owner's own rehearsal input) must die
/// at the parser, exit 2, before any map write: the fatal-input lane.
#[test]
fn volume_percent_above_the_cap_is_refused_at_the_parser() {
    use clap::Parser as _;

    let err = super::super::Cli::try_parse_from(["zylaxion", "volume", "--percent", "1000"])
        .err()
        .expect("1000% must fail the night-improve-27 safety cap");
    let rendered = err.to_string();
    assert!(
        rendered.contains("0..=200"),
        "the parser refusal must name the 0..=200 domain, got: {rendered}"
    );
}

/// Exactly 201 is also above the cap — the boundary is inclusive at
/// 200, exclusive everywhere above it.
#[test]
fn volume_percent_201_is_one_past_the_cap() {
    use clap::Parser as _;

    assert!(
        super::super::Cli::try_parse_from(["zylaxion", "volume", "--percent", "201",]).is_err(),
        "201% is one past the ceiling and must be refused"
    );
}

/// The cap itself parses: 200 is the loudest value any surface takes.
#[test]
fn volume_percent_at_the_cap_parses() {
    use clap::Parser as _;

    let cli = super::super::Cli::try_parse_from(["zylaxion", "volume", "-p", "200"])
        .expect("200% is exactly the cap and must parse");
    match cli.command {
        Some(super::super::Commands::Volume { percent }) => {
            assert_eq!(percent, 200, "the parsed percent must survive intact")
        }
        Some(_) => panic!("expected the volume verb, got another variant"),
        None => panic!("expected a subcommand, got the no-args overview"),
    }
}

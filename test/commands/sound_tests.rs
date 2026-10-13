// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Pins for the `on` surface's player wire (night-improve-26).
//!
//! The wiring itself (the real spawn inside `cmd_on`) is unpinnable
//! under CI — it needs a kernel with the tap attached and an audio
//! server to fail into. The decision surface is what these pins guard:
//! the spawn-verb constant and the root refusal, both pure values of
//! their inputs (the night-badas-1 precedent — pin the decision, the
//! wiring mirrors a shape the pins already hold).

use super::PLAYER_SPAWN_VERB;
use super::on_root_refusal;

/// The spawn string is exactly the verb clap registers: the spawn's
/// `.arg(PLAYER_SPAWN_VERB)` rides the `name = "_play"` pin on the
/// hidden variant in src/cli/mod.rs. Before that registration pin
/// existed, the child died at clap parse time and `on` could never
/// start the player — this constant is the spawn half of the wire, and
/// if either half drifts this pin fails first.
#[test]
fn player_spawn_verb_is_the_underscore_spelling() {
    assert_eq!(
        PLAYER_SPAWN_VERB, "_play",
        "the spawn verb must stay the underscore spelling — clap registers \
         the hidden variant under this exact name"
    );
    assert!(
        PLAYER_SPAWN_VERB.starts_with('_'),
        "the underscore prefix is the internal-verb convention, not a typo"
    );
}

/// The `on` surface refuses root with the do-not-use-sudo verdict —
/// the same wording the player-side guard prints (session_refusal).
/// The parent owns the door: the refusal fires BEFORE the spawn, so a
/// root `on` sees the honest refusal, not a spawn error.
#[test]
fn on_surface_refuses_root_with_the_player_guard_wording() {
    let refusal = on_root_refusal(true).expect("euid 0 must be refused");
    assert!(
        refusal.contains("do not use sudo"),
        "the refusal must carry the owner-facing do-not-use-sudo contract"
    );
}

/// The negative polarity: a non-root `on` is never refused — the
/// decision keys on the privilege fact alone. Pin both polarities: the
/// negated call is exactly how the attach wiring broke once
/// (night-badas-1).
#[test]
fn on_surface_allows_the_non_root_caller() {
    assert!(
        on_root_refusal(false).is_none(),
        "a non-root on must pass the door — the guard keys on euid, nothing else"
    );
}

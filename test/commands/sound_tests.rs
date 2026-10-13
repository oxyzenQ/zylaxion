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
use super::volume_percent_from_gain;

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

// ── night-improve-27: the master-volume safety cap ────────────────────

/// The env lane maps the compiled default gain to exactly 100 percent —
/// the "100 = the compiled default" sentence every surface prints.
#[test]
fn default_gain_maps_to_100_percent() {
    assert_eq!(
        volume_percent_from_gain(crate::engine::VoicePool::DEFAULT_MASTER_VOLUME),
        Ok(100),
        "the compiled default gain must map to exactly 100%"
    );
}

/// The cap is inclusive: the gain that maps to exactly 200 percent is
/// the loudest value the env lane accepts.
#[test]
fn gain_at_the_cap_maps_to_200_percent() {
    let cap_gain = crate::engine::VoicePool::DEFAULT_MASTER_VOLUME * 2.0;
    assert_eq!(
        volume_percent_from_gain(cap_gain),
        Ok(200),
        "exactly 200% is the ceiling, accepted"
    );
}

/// Above the cap is a hard refusal naming the ceiling — never a clamp.
#[test]
fn gain_above_the_cap_is_refused_naming_the_ceiling() {
    let loud_gain = crate::engine::VoicePool::DEFAULT_MASTER_VOLUME * 3.0;
    let err = volume_percent_from_gain(loud_gain)
        .expect_err("a 300% mapping must be refused by the safety cap");
    assert!(
        err.contains("safety cap") && err.contains("200"),
        "the refusal must name the 200% ceiling, got: {err}"
    );
    assert!(
        !err.contains("10000"),
        "the old 0..=10000 domain wording must stay dead, got: {err}"
    );
}

/// A negative gain maps below silence — refused, with wording that
/// says "below silence", not the cap sentence (a negative value never
/// touched the ceiling).
#[test]
fn negative_gain_is_refused_as_below_silence() {
    let err = volume_percent_from_gain(-1.0)
        .expect_err("a negative gain maps below silence and must be refused");
    assert!(
        err.contains("below silence"),
        "the refusal must name the below-silence bound, got: {err}"
    );
}

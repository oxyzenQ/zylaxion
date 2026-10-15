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

// ── night-hunter-8: the player state vocabulary and its verdict law ─────

use super::status::state_verdict;
use super::{STATE_FAILED, STATE_IDLE, STATE_RUNNING, STATE_STARTING};

/// The wire contract: the four state values the player writes and the
/// status reader maps. Before night-hunter-8 the failed value existed
/// only as an anonymous `3` in two files and `starting` as an
/// anonymous `1` in a third — a silent renumber on either side would
/// have gone unnoticed. These values are the control map's language;
/// if any of them moves, every writer and every reader must move in
/// the same commit, and this pin fails first.
#[test]
fn player_state_vocabulary_values_are_the_wire_contract() {
    assert_eq!(STATE_IDLE, 0, "idle is the attach-time default state");
    assert_eq!(STATE_STARTING, 1, "starting precedes the running flip");
    assert_eq!(STATE_RUNNING, 2, "running is the healthy steady state");
    assert_eq!(STATE_FAILED, 3, "failed is the fatal-error verdict");
}

/// The verdict law: the state word IS the verdict — one word, its own
/// tier (green running, grey the passive waiting states, red failed,
/// yellow unknown). The harness runs plain (no TTY), so the tier
/// helpers return the bare words and the exact vocabulary is pinned
/// byte-for-byte; the color half rides the already-tested helpers
/// (ok_line/tip_line/fail_line/warn_line). The unknown fallback must
/// stay a caution word, not a failure word: an unnamed state from a
/// newer binary is a finding, not a crash.
#[test]
fn player_state_words_ride_the_verdict_tiers() {
    assert_eq!(state_verdict(STATE_RUNNING), "running");
    assert_eq!(state_verdict(STATE_STARTING), "starting");
    assert_eq!(state_verdict(STATE_IDLE), "idle");
    assert_eq!(state_verdict(STATE_FAILED), "failed");
    assert_eq!(state_verdict(7), "unknown");
    assert_eq!(state_verdict(9), "unknown");
}

// ── the errno law behind every "not attached" verdict ───────────────

/// night-total-lts-3: only NotFound earns the absent verdict, only
/// PermissionDenied earns the walk-block verdict, and every other
/// errno — None (a successful stat) included — defers to the syscall
/// that follows instead of folding into the idempotent shapes. The
/// night-dinner-3 fold sent the exotic errnos (EIO on a dying bpffs
/// mount, ELOOP on a tampered path) to Absent, which is the "sound
/// is off while the player plays" lie family for the rare case; this
/// pin keeps all three verdicts where they belong.
#[test]
fn pin_stat_classification_law() {
    use super::{ControlPinStat, classify_pin_stat};
    use std::io::ErrorKind;

    assert!(matches!(classify_pin_stat(None), ControlPinStat::Reachable));
    assert!(matches!(
        classify_pin_stat(Some(ErrorKind::NotFound)),
        ControlPinStat::Absent
    ));
    assert!(matches!(
        classify_pin_stat(Some(ErrorKind::PermissionDenied)),
        ControlPinStat::WalkBlocked
    ));
    // The unseen errnos defer — none of them may read as "not attached".
    for kind in [
        ErrorKind::Other,
        ErrorKind::InvalidData,
        ErrorKind::ConnectionRefused,
        ErrorKind::TimedOut,
    ] {
        assert!(
            matches!(classify_pin_stat(Some(kind)), ControlPinStat::Reachable),
            "{kind:?} must defer to the open that follows, never fold into absent"
        );
    }
}

// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// suggestion_tests.rs — the cli/suggestion.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

#[test]
fn closest_value_match_catches_preset_typos() {
    let candidates = ["gentle", "blue_switch", "thocky", "clacky"];
    assert_eq!(
        closest_value_match("thock", &candidates),
        Some("thocky".to_string())
    );
    assert_eq!(
        closest_value_match("THOCKY", &candidates),
        Some("thocky".to_string())
    );
}

#[test]
fn closest_value_match_rescues_mixed_case_vocabulary() {
    // The cherryMX case: a lowercase typo must not pay the case
    // substitutions against a mixed-case candidate, and the tip
    // quotes the ORIGINAL registry spelling.
    let candidates = ["cherryMX", "cherry_mx_black", "classic"];
    assert_eq!(
        closest_value_match("cherymx", &candidates),
        Some("cherryMX".to_string())
    );
}

#[test]
fn closest_value_match_ignores_distant_values() {
    let candidates = ["gentle", "thocky", "clacky"];
    assert_eq!(closest_value_match("fortnights", &candidates), None);
    assert_eq!(closest_value_match("", &candidates), None);
}

#[test]
fn jaro_ci_rescues_case_variants_case_sensitive_jaro_misses() {
    // Case-sensitive Jaro("PRESE", "preset") = 0.0 (no char
    // matches); the case-insensitive pass scores like the lowercase
    // twin.
    assert!(jaro_ci("PRESE", "preset") > 0.7);
    assert_eq!(jaro_ci("PRESE", "preset"), jaro_ci("prese", "preset"));
}

#[test]
fn jaro_ci_zero_for_unrelated_and_short() {
    assert_eq!(jaro_ci("zzzzqqqq", "preset"), 0.0);
    assert_eq!(jaro_ci("x", "preset"), 0.0);
    assert_eq!(jaro_ci("", "help"), 0.0);
    assert_eq!(jaro_ci("x", "x"), 1.0);
    assert_eq!(jaro_ci("x", "y"), 0.0);
}

#[test]
fn closest_long_flag_ci_suggests_preset_for_caps_typo() {
    let candidates = ["help", "check-update", "preset", "foreground", "live"];
    assert_eq!(
        closest_long_flag_ci("PRESET", &candidates),
        Some("preset".to_string())
    );
    assert_eq!(
        closest_long_flag_ci("prese", &candidates),
        Some("preset".to_string())
    );
}

#[test]
fn closest_long_flag_ci_stays_silent_for_distant_input() {
    let candidates = ["help", "check-update", "preset", "foreground", "live"];
    assert_eq!(closest_long_flag_ci("zzzzqqqq", &candidates), None);
    assert_eq!(closest_long_flag_ci("x", &candidates), None);
    assert_eq!(closest_long_flag_ci("", &candidates), None);
}

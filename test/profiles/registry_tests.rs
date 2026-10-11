// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// registry_tests.rs — the compiled preset registry test pins
// (night-improve-22, the no-config law). The registry replaces the
// retired config.toml; these tests replace the TOML parse/validate
// suite: every value the old file carried is now compile-time data,
// and the suite proves the data is complete, unique, in-range, and
// drift-free under validate_and_clamp.

use super::*;

#[test]
fn registry_ships_24_presets() {
    // The catalog count is the migration contract: the retired
    // config.toml defined exactly 24 [preset.*] tables, and the
    // registry must carry every one of them.
    assert_eq!(PRESETS.len(), 24);
    assert_eq!(all().len(), 24);
}

#[test]
fn preset_names_are_unique() {
    let mut names: Vec<&'static str> = PRESETS.iter().map(|p| p.name).collect();
    let count = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), count, "duplicate preset name in the registry");
}

#[test]
fn preset_names_are_nonempty_ascii() {
    for p in all() {
        assert!(!p.name.is_empty(), "empty preset name");
        assert!(
            p.name.bytes().all(|b| b.is_ascii_graphic()),
            "preset name '{}' contains non-ASCII or whitespace",
            p.name
        );
        assert!(
            !p.description.is_empty(),
            "empty description for {}",
            p.name
        );
    }
}

#[test]
fn lookup_finds_every_registered_name() {
    for p in all() {
        let found = lookup(p.name).expect("lookup must find every registered name");
        assert_eq!(found.name, p.name);
        // Same static entry, not a copy: pointer identity is the
        // cheapest proof the catalog is the single source.
        assert!(std::ptr::eq(found, p));
    }
}

#[test]
fn lookup_unknown_name_is_none() {
    assert!(lookup("nonexistent").is_none());
    assert!(lookup("").is_none());
    // Case-sensitivity contract: the old TOML lookup was exact-match,
    // the registry keeps it (the suggestion engine handles near-misses).
    assert!(lookup("cherrymx").is_none());
    assert!(lookup("CHERRYMX").is_none());
}

#[test]
fn default_preset_is_registered() {
    let p = lookup(DEFAULT_PRESET).expect("DEFAULT_PRESET must exist in the registry");
    assert_eq!(p.name, "technical");
}

#[test]
fn every_profile_survives_clamp_without_drift() {
    // The migration fidelity pin: every hand-tuned value must already
    // sit inside the safe ranges, so validate_and_clamp() is a no-op.
    // If a transcription error pushed any value out of range, the
    // clamp would silently rewrite it and this assert fires — the
    // same guarantee testconf used to give TOML, now enforced at
    // test time on compiled data.
    for p in all() {
        let mut clamped = p.profile;
        clamped.validate_and_clamp();
        assert_eq!(
            clamped.click.frequency, p.profile.click.frequency,
            "{}: click.frequency clamped",
            p.name
        );
        assert_eq!(
            clamped.click.resonance, p.profile.click.resonance,
            "{}: click.resonance clamped",
            p.name
        );
        assert_eq!(
            clamped.click.duration_ms, p.profile.click.duration_ms,
            "{}: click.duration_ms clamped",
            p.name
        );
        assert_eq!(
            clamped.click.amplitude, p.profile.click.amplitude,
            "{}: click.amplitude clamped",
            p.name
        );
        assert_eq!(
            clamped.spring.frequency, p.profile.spring.frequency,
            "{}: spring.frequency clamped",
            p.name
        );
        assert_eq!(
            clamped.spring.resonance, p.profile.spring.resonance,
            "{}: spring.resonance clamped",
            p.name
        );
        assert_eq!(
            clamped.spring.mix, p.profile.spring.mix,
            "{}: spring.mix clamped",
            p.name
        );
        assert_eq!(
            clamped.decay.coefficient, p.profile.decay.coefficient,
            "{}: decay.coefficient clamped",
            p.name
        );
        assert_eq!(
            clamped.decay.voice_off_threshold, p.profile.decay.voice_off_threshold,
            "{}: decay.voice_off_threshold clamped",
            p.name
        );
        assert_eq!(
            clamped.ambient.enabled, p.profile.ambient.enabled,
            "{}: ambient.enabled drifted",
            p.name
        );
        assert_eq!(
            clamped.ambient.noise_level, p.profile.ambient.noise_level,
            "{}: ambient.noise_level clamped",
            p.name
        );
        assert_eq!(
            clamped.ambient.noise_decay, p.profile.ambient.noise_decay,
            "{}: ambient.noise_decay clamped",
            p.name
        );
        assert_eq!(
            clamped.housing.frequency, p.profile.housing.frequency,
            "{}: housing.frequency clamped",
            p.name
        );
        assert_eq!(
            clamped.housing.resonance, p.profile.housing.resonance,
            "{}: housing.resonance clamped",
            p.name
        );
        assert_eq!(
            clamped.housing.mix, p.profile.housing.mix,
            "{}: housing.mix clamped",
            p.name
        );
    }
}

#[test]
fn decay_coefficients_are_strictly_below_one() {
    // The infinite-loop guard: coefficient >= 1.0 means a voice never
    // deactivates. The clamp enforces it at runtime; the registry must
    // never even approach it.
    for p in all() {
        assert!(
            p.profile.decay.coefficient < 1.0,
            "{}: decay.coefficient >= 1.0",
            p.name
        );
        assert!(
            p.profile.ambient.noise_decay < 1.0,
            "{}: ambient.noise_decay >= 1.0",
            p.name
        );
    }
}

#[test]
fn names_sorted_covers_the_catalog() {
    let names = names_sorted();
    assert_eq!(names.len(), 24);
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "names_sorted must return sorted names");
    // Spot pins from each series so a transcription slip cannot drop
    // a whole preset unnoticed.
    for expected in [
        "technical",
        "ibm_model_m",
        "topre",
        "buckling_spring",
        "alps_skcm",
        "rotary_encoder",
        "cyber_deck",
        "oceanic",
        "glass_tactile",
        "retro_typewriter",
        "neon_pulse",
    ] {
        assert!(names.contains(&expected), "missing preset: {expected}");
    }
}

#[test]
fn showcase_preset_values_match_the_retired_config() {
    // Byte-for-byte pins on a representative sample — the same table
    // the old config.toml shipped, now compiled in.
    let m = lookup("ibm_model_m").unwrap();
    assert_eq!(m.profile.click.frequency, 2200.0);
    assert_eq!(m.profile.click.resonance, 5.5);
    assert_eq!(m.profile.spring.frequency, 850.0);
    assert_eq!(m.profile.spring.resonance, 7.5);
    assert_eq!(m.profile.spring.mix, 0.85);
    assert_eq!(m.profile.decay.coefficient, 0.99975);
    assert_eq!(m.profile.decay.voice_off_threshold, 1e-6);
    assert!(m.profile.ambient.enabled);
    assert_eq!(m.profile.ambient.noise_level, 0.20);
    assert_eq!(m.profile.housing.frequency, 400.0);
    assert_eq!(m.profile.housing.mix, 0.4);

    let t = lookup("topre").unwrap();
    assert_eq!(t.profile.click.frequency, 650.0);
    assert_eq!(t.profile.click.duration_ms, 5.5);
    assert_eq!(t.profile.housing.frequency, 200.0);
    assert_eq!(t.profile.housing.mix, 0.8);

    let w = lookup("whisper").unwrap();
    assert_eq!(w.profile.click.amplitude, 0.15);
    assert!(!w.profile.ambient.enabled);
}

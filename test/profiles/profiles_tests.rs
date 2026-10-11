// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// profiles_tests.rs — the lib.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

/// Build a profile with all fields set to known-good values.
fn good_profile() -> KeyProfile {
    KeyProfile {
        click: ClickParams {
            frequency: 4500.0,
            resonance: 2.0,
            duration_ms: 1.5,
            amplitude: 0.8,
        },
        spring: SpringParams {
            frequency: 1800.0,
            resonance: 3.5,
            mix: 0.6,
        },
        decay: DecayParams {
            coefficient: 0.9994,
            voice_off_threshold: 1e-5,
            coefficient_fast: 0.0,
            fast_samples_ms: 0.0,
        },
        ambient: AmbientParams {
            enabled: false,
            noise_level: 0.1,
            noise_decay: 0.99,
        },
        housing: HousingParams {
            frequency: 400.0,
            resonance: 2.0,
            mix: 0.4,
        },
    }
}

#[test]
fn validate_and_clamp_is_noop_on_safe_profile() {
    let mut p = good_profile();
    let before = p;
    p.validate_and_clamp();
    // Bit-for-bit equality — no clamping should have occurred.
    assert_eq!(p.click.frequency, before.click.frequency);
    assert_eq!(p.click.resonance, before.click.resonance);
    assert_eq!(p.click.duration_ms, before.click.duration_ms);
    assert_eq!(p.click.amplitude, before.click.amplitude);
    assert_eq!(p.spring.frequency, before.spring.frequency);
    assert_eq!(p.spring.resonance, before.spring.resonance);
    assert_eq!(p.spring.mix, before.spring.mix);
    assert_eq!(p.decay.coefficient, before.decay.coefficient);
    assert_eq!(
        p.decay.voice_off_threshold,
        before.decay.voice_off_threshold
    );
}

#[test]
fn validate_and_clamp_clamps_out_of_bounds_decay() {
    let mut p = good_profile();
    // The killer case from the prompt: decay = 9999. Without
    // clamping this would cause an infinite render loop because
    // the envelope never decays below voice_off_threshold.
    p.decay.coefficient = 9999.0;
    p.validate_and_clamp();
    assert_eq!(p.decay.coefficient, ranges::DECAY_COEFF_MAX);
    assert!(p.decay.coefficient < 1.0, "must be strictly < 1.0");
}

#[test]
fn validate_and_clamp_clamps_infinite_decay() {
    let mut p = good_profile();
    p.decay.coefficient = f32::INFINITY;
    p.validate_and_clamp();
    assert_eq!(p.decay.coefficient, ranges::DECAY_COEFF_MAX);
}

#[test]
fn validate_and_clamp_clamps_nan_decay() {
    let mut p = good_profile();
    p.decay.coefficient = f32::NAN;
    p.validate_and_clamp();
    // NaN maps to MIN (the "safest" choice — instant decay).
    assert_eq!(p.decay.coefficient, ranges::DECAY_COEFF_MIN);
    assert!(!p.decay.coefficient.is_nan());
}

#[test]
fn validate_and_clamp_clamps_negative_decay() {
    let mut p = good_profile();
    p.decay.coefficient = -5.0;
    p.validate_and_clamp();
    assert_eq!(p.decay.coefficient, ranges::DECAY_COEFF_MIN);
}

#[test]
fn validate_and_clamp_clamps_excessive_resonance() {
    let mut p = good_profile();
    // resonance = 9999 would make k = 1/9999 ≈ 0, producing an
    // unstable TPT SVF (infinite Q).
    p.click.resonance = 9999.0;
    p.validate_and_clamp();
    assert_eq!(p.click.resonance, ranges::RESONANCE_MAX);
}

#[test]
fn validate_and_clamp_clamps_zero_resonance() {
    let mut p = good_profile();
    // resonance = 0 would divide by zero in `k = 1/resonance`.
    p.spring.resonance = 0.0;
    p.validate_and_clamp();
    assert_eq!(p.spring.resonance, ranges::RESONANCE_MIN);
}

#[test]
fn validate_and_clamp_clamps_frequency_to_audible_range() {
    let mut p = good_profile();
    p.click.frequency = 100_000.0; // way above Nyquist
    p.spring.frequency = 10.0; // below audible
    p.validate_and_clamp();
    assert_eq!(p.click.frequency, ranges::CLICK_FREQ_MAX);
    assert_eq!(p.spring.frequency, ranges::SPRING_FREQ_MIN);
}

#[test]
fn validate_and_clamp_clamps_nan_amplitude() {
    let mut p = good_profile();
    p.click.amplitude = f32::NAN;
    p.validate_and_clamp();
    assert_eq!(p.click.amplitude, ranges::AMPLITUDE_MIN);
    assert!(!p.click.amplitude.is_nan());
}

#[test]
fn profile_with_overrides_parses_default_only() {
    let toml = r#"
[default]
[default.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[default.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[default.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001
"#;
    let p = ProfileWithOverrides::parse(toml).expect("parse should succeed");
    assert!(p.overrides.is_empty());
    assert_eq!(p.default.click.frequency, 4500.0);
    assert_eq!(p.default.decay.coefficient, 0.9994);
}

#[test]
fn profile_with_overrides_parses_per_key_override() {
    let toml = r#"
[default]
[default.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[default.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[default.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001

[[keys]]
scancode = 28
[keys.click]
frequency = 3000.0
"#;
    let p = ProfileWithOverrides::parse(toml).expect("parse should succeed");
    assert_eq!(p.overrides.len(), 1);

    // Scancode 28 (Enter) should get the overridden click frequency.
    let enter_profile = p.for_scancode(28);
    assert_eq!(enter_profile.click.frequency, 3000.0);
    // But other parameters should fall back to default.
    assert_eq!(enter_profile.click.resonance, 2.0);
    assert_eq!(enter_profile.spring.frequency, 1800.0);
    assert_eq!(enter_profile.decay.coefficient, 0.9994);

    // Any other scancode should get the default profile entirely.
    let other = p.for_scancode(999);
    assert_eq!(other.click.frequency, 4500.0);
}

#[test]
fn profile_with_overrides_clamps_invalid_override_values() {
    let toml = r#"
[default]
[default.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[default.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[default.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001

[[keys]]
scancode = 28
[keys.decay]
coefficient = 9999.0
"#;
    let p = ProfileWithOverrides::parse(toml).expect("parse should succeed");
    let enter_profile = p.for_scancode(28);
    // The out-of-bounds decay coefficient should have been clamped.
    assert_eq!(enter_profile.decay.coefficient, ranges::DECAY_COEFF_MAX);
    assert!(enter_profile.decay.coefficient < 1.0);
}

#[test]
fn profile_with_overrides_rejects_missing_default() {
    let toml = r#"
[[keys]]
scancode = 28
[keys.click]
frequency = 3000.0
"#;
    let result = ProfileWithOverrides::parse(toml);
    assert!(result.is_err(), "missing [default] must error");
}

#[test]
fn profile_with_overrides_applies_ambient_override() {
    // Regression for B2: ProfileWithOverrides::parse previously
    // dropped `[[keys]].ambient.*` overrides because the merge
    // loop didn't handle the `ambient` field. The daemon path
    // (zylaxion::config::build_profile_from_entry) was correct,
    // but the public API of zactrix-profiles was inconsistent.
    // This test ensures both paths now agree.
    let toml = r#"
[default]
[default.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[default.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[default.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001
[default.ambient]
enabled = false
noise_level = 0.1
noise_decay = 0.99

[[keys]]
scancode = 28
[keys.ambient]
enabled = true
noise_level = 0.42
noise_decay = 0.97
"#;
    let p = ProfileWithOverrides::parse(toml).expect("parse should succeed");
    let enter = p.for_scancode(28);
    assert!(
        enter.ambient.enabled,
        "ambient override enabled was dropped"
    );
    assert_eq!(
        enter.ambient.noise_level, 0.42,
        "ambient override noise_level was dropped"
    );
    assert_eq!(
        enter.ambient.noise_decay, 0.97,
        "ambient override noise_decay was dropped"
    );

    // Sanity: scancodes without an override still get the default.
    let other = p.for_scancode(999);
    assert!(!other.ambient.enabled);
    assert_eq!(other.ambient.noise_level, 0.1);
}
